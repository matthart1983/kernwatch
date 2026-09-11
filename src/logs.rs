//! Nonblocking kernel log collection; polling never starts a subprocess.
use crate::domain::{Event, Quality};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    os::unix::fs::OpenOptionsExt,
};
pub struct KernelLog {
    file: Option<File>,
    pub quality: Quality,
    pub events: Vec<Event>,
}
impl Default for KernelLog {
    fn default() -> Self {
        match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/kmsg")
        {
            Ok(file) => Self {
                file: Some(file),
                quality: Quality::Available,
                events: Vec::new(),
            },
            Err(e) => Self {
                file: None,
                quality: match e.kind() {
                    io::ErrorKind::PermissionDenied => Quality::Denied(e.to_string()),
                    io::ErrorKind::NotFound => {
                        Quality::Unsupported("/dev/kmsg is not present".into())
                    }
                    _ => Quality::Error(e.to_string()),
                },
                events: Vec::new(),
            },
        }
    }
}
impl KernelLog {
    pub fn poll(&mut self) {
        let Some(file) = self.file.as_mut() else {
            return;
        };
        let mut buf = [0; 8192];
        for _ in 0..100 {
            match file.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Some(e) = parse(&String::from_utf8_lossy(&buf[..n])) {
                        self.events.push(e);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.raw_os_error() == Some(libc::EPIPE) => {
                    self.quality = Quality::Stale;
                }
                Err(e) => {
                    self.quality = Quality::Error(e.to_string());
                    break;
                }
            }
        }
        if self.events.len() > 300 {
            self.events.drain(..self.events.len() - 300);
        }
    }
}
pub fn parse(s: &str) -> Option<Event> {
    let (meta, message) = s.split_once(';')?;
    let v = meta.split(',').collect::<Vec<_>>();
    let priority = v.first()?.parse::<u32>().ok()? % 8;
    let seq = v.get(1)?;
    let at_ms = v.get(2)?.parse::<u64>().ok()? / 1000;
    Some(Event {
        id: format!("kmsg:{seq}"),
        at_ms,
        source: "kernel".into(),
        severity: if priority <= 3 {
            "error"
        } else if priority == 4 {
            "warn"
        } else {
            "info"
        }
        .into(),
        message: message.trim().into(),
        subject: "host".into(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinct_record() {
        let e = parse("4,10,1234567,-;hello\n continuation").unwrap();
        assert_eq!(e.at_ms, 1234);
        assert_eq!(e.severity, "warn");
        assert!(e.message.contains("continuation"));
    }
}

/// Journal fallback runs on its own bounded worker, never on the display thread.
#[derive(Default)]
pub struct Journal {
    cursor: Option<String>,
    events: Vec<Event>,
    last_ms: u64,
    quality: Quality,
}
impl Journal {
    pub fn sample(&mut self, t: &mut crate::domain::Telemetry) {
        if self.last_ms == 0 || t.at_ms.saturating_sub(self.last_ms) >= 5000 {
            self.last_ms = t.at_ms;
            let mut args = vec!["-b", "--no-pager", "-o", "json", "-n", "300"];
            if let Some(cursor) = &self.cursor {
                args.extend(["--after-cursor", cursor]);
            }
            args.extend(["_TRANSPORT=kernel", "+", "_TRANSPORT=audit"]);
            match crate::command::run(
                "journalctl",
                &args,
                std::time::Duration::from_millis(900),
                1024 * 1024,
            ) {
                Ok(out) if out.status == 0 => {
                    let raw = out.stdout;
                    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
                        .unwrap_or_default()
                        .replace('-', "");
                    let mut malformed = 0;
                    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
                        match journal_record(line, boot.trim()) {
                            Some((cursor, event)) => {
                                self.cursor = Some(cursor);
                                if !self.events.iter().any(|e| e.id == event.id) {
                                    self.events.push(event);
                                }
                            }
                            None => malformed += 1,
                        }
                    }
                    self.quality = if !out.stderr.is_empty() {
                        Quality::Error(format!("Journal coverage warning: {}", out.stderr))
                    } else if malformed == 0 {
                        Quality::Available
                    } else {
                        Quality::Error(format!(
                            "{malformed} invalid or foreign-boot journal records"
                        ))
                    };
                    if self.events.len() > 300 {
                        self.events.drain(..self.events.len() - 300);
                    }
                }
                Ok(out) => {
                    self.quality =
                        Quality::Error(format!("journalctl exit {}: {}", out.status, out.stderr));
                    self.cursor = None;
                }
                Err(e) => {
                    self.quality = Quality::Error(format!("journalctl: {e}"));
                    self.cursor = None;
                }
            }
        }
        t.events.extend(self.events.clone());
        t.capabilities
            .insert("journal".into(), self.quality.clone());
        t.details.insert("source:journal_access".into(),vec![("scope".into(),"Visible current-boot kernel and audit journal records; journal permissions may restrict coverage. Audit records require an existing audit source.".into())]);
    }
}
pub fn journal_record(line: &str, boot: &str) -> Option<(String, Event)> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v["_BOOT_ID"].as_str()? != boot {
        return None;
    }
    let cursor = v["__CURSOR"].as_str()?.to_string();
    let priority = v["PRIORITY"]
        .as_str()
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(6);
    let source = match v["_TRANSPORT"].as_str()? {
        "kernel" => "journal/kernel",
        "audit" => "journal/audit",
        _ => return None,
    };
    Some((
        cursor.clone(),
        Event {
            id: format!("journal:{cursor}"),
            at_ms: v["__MONOTONIC_TIMESTAMP"].as_str()?.parse::<u64>().ok()? / 1000,
            source: source.into(),
            severity: if priority <= 3 {
                "error"
            } else if priority == 4 {
                "warn"
            } else {
                "info"
            }
            .into(),
            message: v["MESSAGE"].as_str()?.to_string(),
            subject: v["_PID"]
                .as_str()
                .map(|p| format!("pid:{p}"))
                .unwrap_or("host".into()),
        },
    ))
}
#[cfg(test)]
mod journal_tests {
    use super::*;
    #[test]
    fn journal_boot_identity_and_audit() {
        let raw = r#"{"_BOOT_ID":"abc","__CURSOR":"c1","__MONOTONIC_TIMESTAMP":"1234567","_TRANSPORT":"audit","MESSAGE":"denied","PRIORITY":"3","_PID":"12"}"#;
        let (_, event) = journal_record(raw, "abc").unwrap();
        assert_eq!(event.at_ms, 1234);
        assert_eq!(event.source, "journal/audit");
        assert_eq!(event.severity, "error");
        assert!(journal_record(raw, "different").is_none());
        assert!(journal_record("broken", "abc").is_none());
    }
}
