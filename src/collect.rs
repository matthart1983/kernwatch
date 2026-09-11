use crate::model::{Snapshot, View};
use std::{
    collections::HashMap,
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Collector {
    enricher: crate::enrich::Enricher,
    previous: HashMap<String, Vec<u64>>,
}
impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}
fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}
fn numbers(s: &str) -> Vec<u64> {
    s.split_whitespace()
        .filter_map(|v| v.parse().ok())
        .collect()
}
fn unavailable(reason: String) -> View {
    View::new(
        &["Availability"],
        vec![vec![reason]],
        &["Unavailable values are not zero. No synthetic data is inserted in live mode."],
    )
}
pub fn cpu_usage(old: &[u64], new: &[u64]) -> f64 {
    if old.len() < 4 || new.len() < 4 {
        return 0.;
    }
    let total = new
        .iter()
        .take(8)
        .sum::<u64>()
        .saturating_sub(old.iter().take(8).sum());
    let idle =
        (new[3] + new.get(4).unwrap_or(&0)).saturating_sub(old[3] + old.get(4).unwrap_or(&0));
    if total == 0 {
        0.
    } else {
        100. * total.saturating_sub(idle) as f64 / total as f64
    }
}
pub fn parse_task(s: &str) -> Option<(String, String, usize, u64, u64)> {
    let start = s.find('(')?;
    let end = s.rfind(')')?;
    let v: Vec<_> = s.get(end + 2..)?.split_whitespace().collect();
    Some((
        s[start + 1..end].into(),
        v.first()?.to_string(),
        v.get(36)?.parse().ok()?,
        v.get(11)?
            .parse::<u64>()
            .ok()?
            .checked_add(v.get(12)?.parse().ok()?)?,
        v.get(21)?.parse().ok()?,
    ))
}
impl Collector {
    pub fn new() -> Self {
        Self {
            enricher: crate::enrich::Enricher::default(),
            previous: HashMap::new(),
        }
    }
    pub fn sample(&mut self) -> Snapshot {
        let mut s = Snapshot {
            captured: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            views: vec![View::default(); 13],
            ..Default::default()
        };
        match read("/proc/stat") {
            Ok(stat) => {
                for line in stat
                    .lines()
                    .filter(|l| l.starts_with("cpu") && !l.starts_with("cpu "))
                {
                    let key = line.split_whitespace().next().unwrap().to_string();
                    let current = numbers(line);
                    s.cpu.push(
                        self.previous
                            .get(&key)
                            .map(|p| cpu_usage(p, &current))
                            .unwrap_or(0.),
                    );
                    self.previous.insert(key, current);
                }
            }
            Err(e) => s.findings.push(e),
        }
        s.load = read("/proc/loadavg")
            .unwrap_or_else(|e| e)
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ");
        if let Ok(mem) = read("/proc/meminfo") {
            let vals: HashMap<_, _> = mem
                .lines()
                .filter_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    Some((k, numbers(v).first().copied().unwrap_or(0)))
                })
                .collect();
            let total = *vals.get("MemTotal").unwrap_or(&0);
            let avail = *vals.get("MemAvailable").unwrap_or(&0);
            s.mem_total = total / 1024;
            s.mem_used = total.saturating_sub(avail) / 1024;
            s.views[3]=View::new(&["Metric","MiB"],["MemTotal","MemAvailable","Cached","Slab","SReclaimable","SUnreclaim","Dirty","SwapTotal","SwapFree"].iter().map(|k|vec![k.to_string(),format!("{:.1}",vals.get(k).unwrap_or(&0).to_owned() as f64/1024.)]).collect(), &["Source: /proc/meminfo. Categories overlap; available includes reclaimable memory."]);
            s.views[3]
                .notes
                .push(read("/proc/pressure/memory").unwrap_or_else(|e| e));
        } else {
            s.views[3] = unavailable("Cannot read /proc/meminfo".into());
        }
        s.views[2] = View::new(
            &["CPU", "Busy %"],
            s.cpu
                .iter()
                .enumerate()
                .map(|(i, c)| vec![i.to_string(), format!("{c:.1}")])
                .collect(),
            &[
                "Source: deltas from /proc/stat, excluding guest double-counting.",
                "Scheduler latency and run-queue distributions require trace collection.",
            ],
        );
        s.views[2]
            .notes
            .push(read("/proc/pressure/cpu").unwrap_or_else(|e| e));
        s.views[5] = unavailable("Syscall tracing is not attached".into());
        s.views[6] = unavailable("IRQ counter worker warming".into());
        for (i, c) in s.cpu.iter().enumerate() {
            if *c >= 90. {
                s.findings.push(format!(
                    "Observed: CPU{i} utilization {c:.1}% in the last sample; cause unknown."
                ));
            }
        }
        if s.mem_total > 0 && s.mem_used as f64 / s.mem_total as f64 > 0.9 {
            s.findings
                .push("Observed: available memory below 10%; inspect memory pressure.".into());
        }
        if s.findings.is_empty() {
            s.findings.push("No CPU/memory threshold crossings in this sample. Trace-dependent health is unknown.".into());
        }
        s.views[11]=View::new(&["Observation"],s.findings.iter().map(|f|vec![f.clone()]).collect(),&["No verified root cause. /proc snapshots establish utilization, not causal chains.","Unavailable: scheduler wakeup percentiles, syscall traces, block completion paths and eBPF runtime.","Capture traces and compare equivalent windows before changing host configuration."]);
        self.enricher.enrich(&mut s);
        s.views[6] = unavailable(format!(
            "IRQ counters: {:?}",
            s.telemetry
                .capabilities
                .get("irq_counts")
                .unwrap_or(&crate::domain::Quality::Warming)
        ));
        if let Some(rows) = s.telemetry.details.get("irq_rows") {
            s.views[6]=View::new(&crate::irq::COLUMNS,rows.iter().filter_map(|(_,v)|serde_json::from_str(v).ok()).collect(),&["Observed CPU is based on interval counts; affinity changes have polling intervals, not known actors"]);
        }
        s.findings.retain(|s| !s.starts_with("Observed: CPU"));
        for c in &s.telemetry.cpus {
            if c.busy >= 90. {
                s.findings.push(format!(
                    "Observed: CPU{} utilization {:.1}% in the last sample; cause unknown",
                    c.id, c.busy
                ));
            }
        }
        s
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guest_not_counted_twice() {
        assert_eq!(cpu_usage(&[0; 10], &[50, 0, 0, 50, 0, 0, 0, 0, 50, 0]), 50.);
    }
    #[test]
    fn reset_safe() {
        assert_eq!(cpu_usage(&[100; 8], &[0; 8]), 0.);
    }
    #[test]
    fn process_names_with_parentheses() {
        let mut v = vec!["0"; 40];
        v[0] = "S";
        v[11] = "20";
        v[12] = "10";
        v[21] = "100";
        v[36] = "3";
        let p = parse_task(&format!("42 (a ) process) {}", v.join(" "))).unwrap();
        assert_eq!(p.0, "a ) process");
        assert_eq!(p.2, 3);
        assert_eq!(p.3, 30);
    }
}
