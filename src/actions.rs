//! Reviewed, journalled file changes. UI preview has no write side effects.
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    #[serde(default)]
    pub effective_before: Option<String>,
    #[serde(default)]
    pub effective_after: Option<String>,
    #[serde(default)]
    pub applied_at_ms: Option<u64>,
    #[serde(default)]
    pub verification: Option<String>,
    pub id: u128,
    pub target: PathBuf,
    pub before: String,
    pub after: String,
    pub description: String,
    pub applied: bool,
    pub verified: bool,
    #[serde(default)]
    pub identity: Option<u64>,
    #[serde(default)]
    pub outcome: String,
}
pub trait Host {
    fn identity(&self, _: &Path) -> io::Result<Option<u64>> {
        Ok(None)
    }
    fn read(&self, path: &Path) -> io::Result<String>;
    fn write(&self, path: &Path, value: &str) -> io::Result<()>;
    fn effective(&self, path: &Path) -> io::Result<String> {
        self.read(path)
    }
}
/// In-memory action target used only by the labelled fixture tour.
#[derive(Default)]
pub struct DemoHost(pub std::cell::RefCell<std::collections::BTreeMap<PathBuf, String>>);
impl Host for DemoHost {
    fn read(&self, path: &Path) -> io::Result<String> {
        self.0
            .borrow()
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::other("target is not present in the demo fixture"))
    }
    fn write(&self, path: &Path, value: &str) -> io::Result<()> {
        let mut values = self.0.borrow_mut();
        let target = values
            .get_mut(path)
            .ok_or_else(|| io::Error::other("demo target disappeared"))?;
        *target = value.into();
        Ok(())
    }
}
pub struct LinuxHost;
impl Host for LinuxHost {
    fn effective(&self, path: &Path) -> io::Result<String> {
        match path.file_name().and_then(|s| s.to_str()) {
            Some("smp_affinity_list") => {
                fs::read_to_string(path.with_file_name("effective_affinity_list"))
            }
            Some("cpuset.cpus") => fs::read_to_string(path.with_file_name("cpuset.cpus.effective")),
            _ => self.read(path),
        }
    }

    fn identity(&self, path: &Path) -> io::Result<Option<u64>> {
        use std::os::unix::fs::MetadataExt;
        if task_target(path).is_some() {
            return Ok(None);
        }
        Ok(Some(fs::metadata(path)?.ino()))
    }
    fn read(&self, path: &Path) -> io::Result<String> {
        if let Some((pid, start)) = task_target(path) {
            validate_task(pid, start)?;
            return fs::read_to_string(format!("/proc/{pid}/status"))?
                .lines()
                .find_map(|l| {
                    l.strip_prefix("Cpus_allowed_list:")
                        .map(|v| v.trim().to_owned())
                })
                .ok_or_else(|| io::Error::other("task affinity unavailable"));
        }
        fs::read_to_string(path)
    }
    fn write(&self, path: &Path, value: &str) -> io::Result<()> {
        if let Some((pid, start)) = task_target(path) {
            validate_task(pid, start)?;
            validate_cpus(value)?;
            let mut mask: libc::cpu_set_t = unsafe { std::mem::zeroed() };
            for part in value.trim().split(',') {
                let mut range = part.split('-');
                let lo = range
                    .next()
                    .unwrap()
                    .parse::<usize>()
                    .map_err(io::Error::other)?;
                let hi = range
                    .next()
                    .map(str::parse::<usize>)
                    .transpose()
                    .map_err(io::Error::other)?
                    .unwrap_or(lo);
                if hi >= libc::CPU_SETSIZE as usize {
                    return Err(io::Error::other("CPU exceeds affinity mask capacity"));
                }
                for cpu in lo..=hi {
                    unsafe { libc::CPU_SET(cpu, &mut mask) };
                }
            }
            if unsafe { libc::sched_setaffinity(pid, std::mem::size_of_val(&mask), &mask) } != 0 {
                return Err(io::Error::last_os_error());
            }
            return Ok(());
        }
        fs::write(path, format!("{}\n", value.trim()))
    }
}
fn task_target(path: &Path) -> Option<(i32, u64)> {
    let value = path.to_str()?.strip_prefix("task-affinity:")?;
    let (pid, start) = value.split_once(':')?;
    Some((pid.parse().ok()?, start.parse().ok()?))
}
pub(crate) fn validate_task(pid: i32, start: u64) -> io::Result<()> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let end = stat
        .rfind(')')
        .ok_or_else(|| io::Error::other("invalid task stat"))?;
    let actual = stat[end + 2..]
        .split_whitespace()
        .nth(19)
        .and_then(|v| v.parse::<u64>().ok());
    if actual != Some(start) {
        return Err(io::Error::other("task identity changed after preview"));
    }
    Ok(())
}
impl Plan {
    pub fn preview(
        host: &impl Host,
        target: PathBuf,
        after: String,
        description: String,
    ) -> io::Result<Self> {
        let before = host.read(&target)?;
        let identity = host.identity(&target)?;
        let effective_before = Some(host.effective(&target)?);
        Ok(Self {
            effective_before,
            effective_after: None,
            applied_at_ms: None,
            verification: None,
            id: crate::recording::stamp(),
            target,
            before,
            after,
            description,
            applied: false,
            verified: false,
            identity,
            outcome: "reviewed; not applied".into(),
        })
    }
    pub fn apply(&mut self, host: &impl Host) -> io::Result<()> {
        let result = self.apply_inner(host);
        self.outcome = match &result {
            Ok(()) => "applied; configured readback verified; effective value captured; performance unverified".into(),
            Err(e) => format!("apply failed: {e}"),
        };
        result
    }
    fn apply_inner(&mut self, host: &impl Host) -> io::Result<()> {
        if host.identity(&self.target)? != self.identity {
            return Err(io::Error::other("target identity changed after preview"));
        }
        if host.read(&self.target)? != self.before {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "target changed after preview; create a new plan",
            ));
        }
        host.write(&self.target, &self.after)?;
        self.applied = true;
        self.effective_after = Some(host.effective(&self.target)?);
        self.verified = equivalent(&self.target, &host.read(&self.target)?, &self.after)
            && effective_allowed(
                &self.target,
                self.effective_after.as_deref().unwrap_or(""),
                &self.after,
            );
        if !self.verified {
            return Err(io::Error::other(
                "write completed but effective readback differs; inspect journal before rollback",
            ));
        }
        Ok(())
    }
    pub fn revert(&mut self, host: &impl Host) -> io::Result<()> {
        let result = self.revert_inner(host);
        self.outcome = match &result {
            Ok(()) => "reverted; readback verified".into(),
            Err(e) => format!("rollback failed: {e}"),
        };
        result
    }
    fn revert_inner(&mut self, host: &impl Host) -> io::Result<()> {
        if host.identity(&self.target)? != self.identity {
            return Err(io::Error::other("target identity changed after apply"));
        }
        if !self.applied {
            return Err(io::Error::other("action was not applied"));
        }
        if !equivalent(&self.target, &host.read(&self.target)?, &self.after) {
            return Err(io::Error::other(
                "target changed since apply; automatic rollback refused",
            ));
        }
        host.write(&self.target, &self.before)?;
        self.effective_after = Some(host.effective(&self.target)?);
        self.verified = equivalent(&self.target, &host.read(&self.target)?, &self.before)
            && effective_allowed(
                &self.target,
                self.effective_after.as_deref().unwrap_or(""),
                &self.before,
            );
        if !self.verified {
            return Err(io::Error::other(
                "rollback readback differs from original value",
            ));
        }
        self.applied = false;
        Ok(())
    }
}
fn effective_allowed(path: &Path, actual: &str, requested: &str) -> bool {
    if !matches!(
        path.file_name().and_then(|s| s.to_str()),
        Some("smp_affinity_list" | "cpuset.cpus")
    ) {
        return true;
    }
    if requested.trim().is_empty() {
        return !actual.trim().is_empty();
    }
    !actual.trim().is_empty()
        && equivalent(
            path,
            &format!("{},{}", requested.trim(), actual.trim()),
            requested,
        )
}
/// Persist the reviewed intent before touching a host control, then persist readback.
pub fn save_journal(path: &Path, plans: &[Plan]) -> io::Result<()> {
    use std::io::Write;
    let temporary = path.with_extension(format!("{}.tmp", crate::recording::stamp()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(plans)?)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    fs::File::open(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?
    .sync_all()?;
    Ok(())
}
fn equivalent(path: &Path, a: &str, b: &str) -> bool {
    let path = path.to_string_lossy();
    if path.ends_with("rps_cpus") {
        let normalize = |s: &str| {
            let mut words = s
                .trim()
                .split(',')
                .map(|w| u32::from_str_radix(w, 16))
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            while words.len() > 1 && words[0] == 0 {
                words.remove(0);
            }
            Some(words)
        };
        return normalize(a).zip(normalize(b)).is_some_and(|(a, b)| a == b);
    }
    if path.ends_with("smp_affinity_list")
        || path.ends_with("cpuset.cpus")
        || path.starts_with("task-affinity:")
    {
        let normalize = |s: &str| -> Option<Vec<(u32, u32)>> {
            if s.trim().is_empty() {
                return Some(Vec::new());
            }
            let mut ranges = Vec::new();
            for part in s.trim().split(',') {
                let mut bounds = part.split('-');
                let lo = bounds.next()?.parse().ok()?;
                let hi = bounds
                    .next()
                    .map(str::parse)
                    .transpose()
                    .ok()?
                    .unwrap_or(lo);
                ranges.push((lo, hi));
            }
            ranges.sort_unstable();
            let mut merged: Vec<(u32, u32)> = Vec::new();
            for (lo, hi) in ranges {
                if let Some(last) = merged.last_mut() {
                    if lo <= last.1.saturating_add(1) {
                        last.1 = last.1.max(hi);
                        continue;
                    }
                }
                merged.push((lo, hi));
            }
            Some(merged)
        };
        return normalize(a).zip(normalize(b)).is_some_and(|(a, b)| a == b);
    }
    a.trim() == b.trim()
}
pub fn parse_target(command: &str) -> io::Result<(PathBuf, String, String)> {
    let args = command.split_whitespace().collect::<Vec<_>>();
    match args.as_slice() {
        ["task", pid, start, cpus] => {
            let pid = pid.parse::<u32>().map_err(io::Error::other)?;
            let start = start.parse::<u64>().map_err(io::Error::other)?;
            if pid == 0 {
                return Err(io::Error::other("PID must be positive"));
            }
            validate_cpus(cpus)?;
            Ok((format!("task-affinity:{pid}:{start}").into(),cpus.to_string(),format!("Set task {pid} (start {start}) affinity to {cpus}; effective cpuset still applies")))
        }
        ["rps", interface, queue, mask] => {
            if interface.is_empty()
                || !interface
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
                || *interface == "."
                || *interface == ".."
            {
                return Err(io::Error::other("invalid interface"));
            }
            let queue = queue.parse::<u32>().map_err(io::Error::other)?;
            if mask.is_empty()
                || mask.split(',').any(|s| {
                    s.is_empty() || s.len() > 8 || !s.chars().all(|c| c.is_ascii_hexdigit())
                })
            {
                return Err(io::Error::other(
                    "RPS mask must be comma-separated hex words, high word first",
                ));
            }
            Ok((format!("/sys/class/net/{interface}/queues/rx-{queue}/rps_cpus").into(),mask.to_string(),format!("Set {interface} RX queue {queue} RPS CPU mask to {mask}; may change cross-CPU packet processing")))
        }
        ["irq", id, cpus] => {
            id.parse::<u32>()
                .map_err(|_| io::Error::other("IRQ must be numeric"))?;
            validate_cpus(cpus)?;
            Ok((
                PathBuf::from(format!("/proc/irq/{id}/smp_affinity_list")),
                cpus.to_string(),
                format!("Set IRQ {id} affinity to CPUs {cpus}; irqbalance may overwrite this"),
            ))
        }
        ["quota", group, quota, period] => {
            let period = period
                .parse::<u64>()
                .map_err(|_| io::Error::other("invalid period"))?;
            if !(1000..=1_000_000).contains(&period) {
                return Err(io::Error::other(
                    "period must be 1000..1000000 microseconds",
                ));
            }
            if *quota != "max"
                && quota
                    .parse::<u64>()
                    .map_err(|_| io::Error::other("invalid quota"))?
                    < 1000
            {
                return Err(io::Error::other(
                    "quota must be max or at least 1000 microseconds",
                ));
            }
            let target = cgroup_path(group, "cpu.max")?;
            Ok((
                target,
                format!("{quota} {period}"),
                format!("Change {group} runtime quota; may affect every descendant task"),
            ))
        }
        ["cpuset", group, cpus] => {
            validate_cpus(cpus)?;
            Ok((
                cgroup_path(group, "cpuset.cpus")?,
                cpus.to_string(),
                format!("Change {group} cpuset; tasks may still have narrower affinity"),
            ))
        }
        _ => Err(io::Error::other(
            "preview irq ID CPUS | quota GROUP QUOTA PERIOD | cpuset GROUP CPUS",
        )),
    }
}
fn cgroup_path(group: &str, file: &str) -> io::Result<PathBuf> {
    let relative = group.trim_start_matches('/');
    if relative.split('/').any(|s| s == ".." || s == ".") {
        return Err(io::Error::other("invalid cgroup path"));
    }
    Ok(PathBuf::from("/sys/fs/cgroup").join(relative).join(file))
}
pub fn validate_cpus(s: &str) -> io::Result<()> {
    if s.is_empty() {
        return Err(io::Error::other("empty CPU set"));
    }
    for part in s.split(',') {
        let bounds = part
            .split('-')
            .map(|s| s.parse::<u32>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| io::Error::other("invalid CPU list"))?;
        if bounds.len() > 2
            || bounds.len() == 2 && bounds[0] > bounds[1]
            || bounds.iter().any(|n| *n > 1_048_575)
        {
            return Err(io::Error::other("invalid CPU range"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    struct Fake(RefCell<String>);
    impl Host for Fake {
        fn read(&self, _: &Path) -> io::Result<String> {
            Ok(self.0.borrow().clone())
        }
        fn write(&self, _: &Path, v: &str) -> io::Result<()> {
            *self.0.borrow_mut() = v.into();
            Ok(())
        }
    }
    #[test]
    fn preview_apply_and_rollback() {
        let h = Fake(RefCell::new("3".into()));
        let mut p = Plan::preview(&h, "mock".into(), "0-7".into(), "test".into()).unwrap();
        assert_eq!(*h.0.borrow(), "3");
        p.apply(&h).unwrap();
        assert!(p.verified);
        p.revert(&h).unwrap();
        assert_eq!(*h.0.borrow(), "3");
    }
    #[test]
    fn stale_target_refused() {
        let h = Fake(RefCell::new("3".into()));
        let mut p = Plan::preview(&h, "mock".into(), "0".into(), "test".into()).unwrap();
        *h.0.borrow_mut() = "2".into();
        assert!(p.apply(&h).is_err());
    }
    #[test]
    fn failed_rollback_keeps_action_outstanding() {
        struct RefuseRestore(RefCell<String>);
        impl Host for RefuseRestore {
            fn read(&self, _: &Path) -> io::Result<String> {
                Ok(self.0.borrow().clone())
            }
            fn write(&self, _: &Path, v: &str) -> io::Result<()> {
                if v != "3" {
                    *self.0.borrow_mut() = v.into();
                }
                Ok(())
            }
        }
        let host = RefuseRestore(RefCell::new("3".into()));
        let mut p = Plan::preview(&host, "mock".into(), "0".into(), "test".into()).unwrap();
        p.apply(&host).unwrap();
        assert!(p.revert(&host).is_err());
        assert!(p.applied);
        assert!(!p.verified);
        assert!(p.outcome.contains("rollback failed"));
    }
    #[test]
    fn cpu_list_readback_allows_kernel_canonicalization() {
        assert!(equivalent(
            Path::new("/proc/irq/47/smp_affinity_list"),
            "0,1,2,4,5,6,7\n",
            "0-2,4-7"
        ));
        assert!(!equivalent(
            Path::new("/proc/irq/47/smp_affinity_list"),
            "0-7",
            "0-2,4-7"
        ));
    }
    #[test]
    fn traversal_and_bad_ranges() {
        assert!(parse_target("cpuset ../../tmp 0").is_err());
        assert!(validate_cpus("7-2").is_err());
        assert!(validate_cpus("0-3,7").is_ok());
    }
}
