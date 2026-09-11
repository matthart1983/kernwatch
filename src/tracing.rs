//! Owned tracefs instance and bounded trace event correlation.
//! No global tracer state is changed. Source formats are inspected by the kernel.
use crate::domain::*;
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
};
#[derive(Clone, Debug)]
pub struct TraceEvent {
    pub at_ns: u64,
    pub cpu: u32,
    pub pid: u32,
    pub name: String,
    pub payload: String,
}
pub fn parse(line: &str) -> Option<TraceEvent> {
    let open = line.find('[')?;
    let close = line[open..].find(']')? + open;
    let cpu = line[open + 1..close].trim().parse().ok()?;
    let task = line[..open].trim();
    let pid = task.rsplit_once('-')?.1.trim().parse().ok()?;
    let rest = line[close + 1..].trim();
    let words = rest.split_whitespace().collect::<Vec<_>>();
    let index = words
        .iter()
        .position(|w| w.ends_with(':') && w.trim_end_matches(':').parse::<f64>().is_ok())?;
    let ts = words[index].trim_end_matches(':');
    let (seconds, fraction) = ts.split_once('.').unwrap_or((ts, "0"));
    let ns = seconds.parse::<u64>().ok()?.checked_mul(1_000_000_000)?
        + format!("{fraction:0<9}").get(..9)?.parse::<u64>().ok()?;
    let name = words.get(index + 1)?.trim_end_matches(':').to_string();
    let payload = words.get(index + 2..)?.join(" ");
    Some(TraceEvent {
        at_ns: ns,
        cpu,
        pid,
        name,
        payload,
    })
}
fn field(payload: &str, key: &str) -> Option<u64> {
    payload.split_whitespace().find_map(|w| {
        w.strip_prefix(&format!("{key}="))?
            .trim_end_matches(',')
            .parse()
            .ok()
    })
}
#[derive(Default, Clone)]
pub struct Correlator {
    pub task_migrations: BTreeMap<u32, u64>,
    pub group_migrations: BTreeMap<u64, u64>,
    pub task_starts: BTreeMap<u32, u64>,
    off: BTreeMap<u32, u64>,
    pub off_ms: BTreeMap<u32, Vec<f64>>,
    wake: BTreeMap<u32, (u64, u32)>,
    sys: BTreeMap<u32, (u64, u64, String)>,
    irq: BTreeMap<(u32, String), (u64, u64)>,
    pub wake_ms: BTreeMap<u32, Vec<f64>>,
    pub wake_cpu: BTreeMap<u32, Vec<f64>>,
    pub wake_cgroup: BTreeMap<u64, Vec<f64>>,
    pub migrations: BTreeMap<u32, u64>,
    pub switches: BTreeMap<u32, u64>,
    pub syscall_count: BTreeMap<u64, u64>,
    pub syscall_ms: BTreeMap<u64, Vec<f64>>,
    pub errors: BTreeMap<u64, u64>,
    pub vector_ns: BTreeMap<(u32, u64), u64>,
    pub vector_ms: BTreeMap<u64, Vec<f64>>,
    pub errno_counts: BTreeMap<(u64, i64), u64>,
    pub syscall_callers: BTreeMap<u64, BTreeMap<u32, u64>>,
    pub events: Vec<Event>,
    pub lost: u64,
    origin: Option<u64>,
    pub paired: u64,
}
impl Correlator {
    pub fn consume(&mut self, e: TraceEvent) {
        self.origin.get_or_insert(e.at_ns);
        match e.name.as_str() {
            "sched_wakeup" | "sched_wakeup_new" => {
                if let Some(pid) = field(&e.payload, "pid") {
                    self.wake.entry(pid as u32).or_insert((e.at_ns, e.cpu));
                }
            }
            "sched_switch" => {
                *self.switches.entry(e.cpu).or_default() += 1;
                if let Some(next) = field(&e.payload, "next_pid") {
                    if let Some(start) = field(&e.payload, "start_ns").filter(|n| *n > 0) {
                        if self
                            .task_starts
                            .get(&(next as u32))
                            .is_some_and(|old| *old != start)
                        {
                            self.wake.remove(&(next as u32));
                            self.wake_ms.remove(&(next as u32));
                            self.off.remove(&(next as u32));
                            self.off_ms.remove(&(next as u32));
                        }
                        self.task_starts.insert(next as u32, start);
                    }

                    if let Some(start) = self.off.remove(&(next as u32)) {
                        if e.at_ns >= start {
                            let values = self.off_ms.entry(next as u32).or_default();
                            values.push((e.at_ns - start) as f64 / 1e6);
                            if values.len() > 4096 {
                                values.remove(0);
                            }
                        }
                    }
                }
                if let Some(prev) = field(&e.payload, "prev_pid").filter(|p| *p != 0) {
                    self.off.insert(prev as u32, e.at_ns);
                }

                if let Some(pid) = field(&e.payload, "next_pid") {
                    if let Some((start, _)) = self.wake.remove(&(pid as u32)) {
                        if e.at_ns >= start {
                            let values = self.wake_ms.entry(pid as u32).or_default();
                            values.push((e.at_ns - start) as f64 / 1e6);
                            let cpu_values = self.wake_cpu.entry(e.cpu).or_default();
                            cpu_values.push((e.at_ns - start) as f64 / 1e6);
                            if cpu_values.len() > 4096 {
                                cpu_values.remove(0);
                            }
                            if values.len() > 4096 {
                                values.remove(0);
                            }
                            if let Some(group) = field(&e.payload, "cgroup").filter(|id| *id != 0) {
                                if self.wake_cgroup.len() < 512
                                    || self.wake_cgroup.contains_key(&group)
                                {
                                    let values = self.wake_cgroup.entry(group).or_default();
                                    values.push((e.at_ns - start) as f64 / 1e6);
                                    if values.len() > 4096 {
                                        values.remove(0);
                                    }
                                }
                            }
                            self.paired += 1;
                        }
                    }
                }
                if e.payload.contains("prev_state=R") {
                    if let Some(pid) = field(&e.payload, "prev_pid") {
                        if pid != 0 {
                            self.wake.entry(pid as u32).or_insert((e.at_ns, e.cpu));
                        }
                    }
                }
            }
            "sched_migrate_task" => {
                if let Some(pid) = field(&e.payload, "pid") {
                    *self.task_migrations.entry(pid as u32).or_default() += 1;
                }
                if let Some(id) = field(&e.payload, "cgroup").filter(|n| *n != 0) {
                    *self.group_migrations.entry(id).or_default() += 1;
                }

                if let Some(cpu) = field(&e.payload, "dest_cpu") {
                    *self.migrations.entry(cpu as u32).or_default() += 1;
                }
            }
            "sched_process_exit" => {
                let pid = field(&e.payload, "pid").unwrap_or(e.pid as u64) as u32;
                self.wake.remove(&pid);
                self.sys.remove(&pid);
                self.wake_ms.remove(&pid);
                self.task_starts.remove(&pid);
                self.task_migrations.remove(&pid);
                self.off.remove(&pid);
                self.off_ms.remove(&pid);
            }
            "sys_enter" => {
                if let Some(id) = field(&e.payload, "id")
                    .or_else(|| e.payload.split_whitespace().nth(1)?.parse().ok())
                {
                    self.sys.insert(e.pid, (e.at_ns, id, e.payload.clone()));
                }
            }
            "sys_exit" => {
                if let Some((start, id, arguments)) = self.sys.remove(&e.pid) {
                    if e.at_ns >= start {
                        *self.syscall_count.entry(id).or_default() += 1;
                        let v = self.syscall_ms.entry(id).or_default();
                        v.push((e.at_ns - start) as f64 / 1e6);
                        if v.len() > 4096 {
                            v.remove(0);
                        }
                        if e.payload.contains("ret=-") || e.payload.contains("-> -") {
                            *self.errors.entry(id).or_default() += 1;
                        }
                        let callers = self.syscall_callers.entry(id).or_default();
                        if callers.len() < 64 || callers.contains_key(&e.pid) {
                            *callers.entry(e.pid).or_default() += 1;
                        }
                        if let Some(ret) = e
                            .payload
                            .split_whitespace()
                            .find_map(|s| s.strip_prefix("ret=")?.parse::<i64>().ok())
                            .filter(|n| *n < 0)
                        {
                            *self.errno_counts.entry((id, -ret)).or_default() += 1;
                        }
                        self.events.push(Event {
                            id: format!("syscall-{}-{}", e.at_ns, e.pid),
                            at_ms: e.at_ns / 1_000_000,
                            source: "syscall trace".into(),
                            severity: if e.payload.contains("ret=-") {
                                "error"
                            } else {
                                "info"
                            }
                            .into(),
                            message: format!(
                                "{} pid{} {} {} duration_ms={:.6}",
                                crate::probes::syscall_name(id),
                                e.pid,
                                arguments,
                                e.payload,
                                (e.at_ns - start) as f64 / 1e6
                            ),
                            subject: format!("task:{}", e.pid),
                        });
                        self.paired += 1;
                    }
                }
            }
            "softirq_entry" | "irq_handler_entry" => {
                let id = field(&e.payload, "vec")
                    .or_else(|| field(&e.payload, "irq"))
                    .unwrap_or(0);
                self.irq.insert(
                    (
                        e.cpu,
                        if e.name.starts_with("soft") {
                            "soft"
                        } else {
                            "hard"
                        }
                        .into(),
                    ),
                    (e.at_ns, id),
                );
            }
            "softirq_exit" | "irq_handler_exit" => {
                let kind = if e.name.starts_with("soft") {
                    "soft"
                } else {
                    "hard"
                };
                if let Some((start, id)) = self.irq.remove(&(e.cpu, kind.into())) {
                    if e.at_ns >= start {
                        *self
                            .vector_ns
                            .entry((e.cpu, if kind == "hard" { id + 1_000_000 } else { id }))
                            .or_default() += e.at_ns - start;
                        let key = if kind == "hard" { id + 1_000_000 } else { id };
                        let values = self.vector_ms.entry(key).or_default();
                        values.push((e.at_ns - start) as f64 / 1e6);
                        if values.len() > 4096 {
                            values.remove(0);
                        }
                        self.paired += 1;
                    }
                }
            }
            _ => {}
        }
        if e.name.starts_with("block_") {
            self.events.push(Event {
                id: format!("trace-{}-{}", e.at_ns, e.cpu),
                at_ms: e.at_ns / 1_000_000,
                source: if e.name.starts_with("sys_") {
                    "syscall trace"
                } else {
                    "block trace"
                }
                .into(),
                severity: if e.payload.contains("ret=-") {
                    "error"
                } else {
                    "info"
                }
                .into(),
                message: format!("{} cpu{} pid{} {}", e.name, e.cpu, e.pid, e.payload),
                subject: format!("task:{}", e.pid),
            });
            if self.events.len() > 1000 {
                self.events.drain(..self.events.len() - 1000);
            }
        }
        if self.events.len() > 1000 {
            self.events.drain(..self.events.len() - 1000);
        }
        if self.task_migrations.len() > 16384 || self.group_migrations.len() > 512 {
            self.task_migrations.clear();
            self.group_migrations.clear();
            self.loss(1);
        }
        if self.task_starts.len() > 16384 {
            self.task_starts.clear();
            self.loss(1);
        }
        if self.off_ms.len() > 4096 {
            self.off_ms.clear();
            self.loss(1);
        }
        if self.wake_ms.len() > 4096 {
            self.wake_ms.clear();
            self.loss(1);
        }
        if self.wake.len() > 16384 || self.sys.len() > 16384 || self.off.len() > 16384 {
            self.loss(1);
        }
    }
    pub fn loss(&mut self, n: u64) {
        self.lost += n;
        self.wake.clear();
        self.off.clear();
        self.sys.clear();
        self.irq.clear();
    }
    pub fn apply(&self, t: &mut Telemetry) {
        t.runnable_waits.clear();
        if self.lost == 0 {
            for task in &t.tasks {
                let matching = self.task_starts.get(&task.pid).is_some_and(|ns| {
                    (*ns as u128 * unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u128
                        / 1_000_000_000) as u64
                        == task.start_ticks
                });
                if matching {
                    if let Some((at, _)) = self.wake.get(&task.pid) {
                        if let Some(age) = (t.at_ms as u128 * 1_000_000).checked_sub(*at as u128) {
                            t.runnable_waits.push(RunnableWait {
                                pid: task.pid,
                                start_ticks: task.start_ticks,
                                cpu: task.cpu,
                                age_ms: age as f64 / 1e6,
                            });
                        }
                    }
                }
            }
            t.runnable_waits
                .sort_by(|a, b| b.age_ms.total_cmp(&a.age_ms));
        }
        for task in &mut t.tasks {
            let matching = self.task_starts.get(&task.pid).is_some_and(|ns| {
                (*ns as u128 * unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u128
                    / 1_000_000_000) as u64
                    == task.start_ticks
            });
            if !matching {
                task.wake_p50_ms = None;
                task.wake_p99_ms = None;
                continue;
            }
            if let Some(values) = self.wake_ms.get(&task.pid) {
                task.wake_p50_ms = percentile(values, 0.5);
                task.wake_p99_ms = percentile(values, 0.99);
            }
        }
        let all = self
            .wake_cpu
            .values()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        if let Some(p99) = percentile(&all, 0.99) {
            t.metrics.insert(
                "sched.p99".into(),
                Measurement {
                    value: Some(p99),
                    unit: "ms".into(),
                    source: "sched wakeup/switch correlation · bounded capture samples".into(),
                    start_ms: self.origin.unwrap_or(0) / 1_000_000,
                    end_ms: t.at_ms,
                    count: all.len() as u64,
                    quality: if self.lost == 0 {
                        Quality::Available
                    } else {
                        Quality::Stale
                    },
                },
            );
            t.record("sched.p99", Some(p99), "ms", p99.max(20.));
        }
        t.trace_drops = self.lost;
        t.events
            .extend(self.events.iter().rev().take(100).rev().cloned());
    }
}
pub fn percentile(values: &[f64], q: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    v.get(
        ((v.len() as f64 * q).ceil() as usize)
            .saturating_sub(1)
            .min(v.len() - 1),
    )
    .copied()
}
pub struct TraceSession {
    path: PathBuf,
    pipe: Option<File>,
    partial: String,
    pub mode: String,
    pub correlator: Correlator,
}
impl TraceSession {
    pub fn start(mode: &str) -> io::Result<Self> {
        let events: &[&str] = match mode {
            "sched" => &[
                "sched/sched_wakeup",
                "sched/sched_wakeup_new",
                "sched/sched_switch",
                "sched/sched_process_exit",
            ],
            "irq" => &[
                "irq/softirq_entry",
                "irq/softirq_exit",
                "irq/irq_handler_entry",
                "irq/irq_handler_exit",
            ],
            "syscalls" => &[
                "raw_syscalls/sys_enter",
                "raw_syscalls/sys_exit",
                "sched/sched_process_exit",
            ],
            "block" => &["block/block_rq_issue", "block/block_rq_complete"],
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "probe must be sched, irq, block or syscalls",
                ))
            }
        };
        let path = PathBuf::from(format!(
            "/sys/kernel/tracing/instances/kernwatch-{}-{}",
            std::process::id(),
            crate::recording::stamp()
        ));
        fs::create_dir(&path)?;
        let result = (|| {
            fs::write(path.join("tracing_on"), "0")?;
            fs::write(path.join("buffer_size_kb"), "256")?;
            fs::write(path.join("trace_clock"), "mono")?;
            for event in events {
                fs::write(path.join("events").join(event).join("enable"), "1")?;
            }
            let pipe = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(path.join("trace_pipe"))?;
            fs::write(path.join("tracing_on"), "1")?;
            Ok(Self {
                path: path.clone(),
                pipe: Some(pipe),
                partial: String::new(),
                mode: mode.into(),
                correlator: Correlator::default(),
            })
        })();
        if result.is_err() {
            let _ = fs::write(path.join("tracing_on"), "0");
            let _ = fs::remove_dir(&path);
        }
        result
    }
    pub fn poll(&mut self) -> io::Result<()> {
        let mut buf = [0u8; 65536];
        for _ in 0..8 {
            match self
                .pipe
                .as_mut()
                .ok_or_else(|| io::Error::other("trace pipe closed"))?
                .read(&mut buf)
            {
                Ok(0) => break,
                Ok(n) => {
                    self.partial.push_str(&String::from_utf8_lossy(&buf[..n]));
                    while let Some(end) = self.partial.find('\n') {
                        let line = self.partial[..end].to_string();
                        self.partial.drain(..=end);
                        if line.contains("LOST") {
                            self.correlator.loss(1);
                        } else if let Some(e) = parse(&line) {
                            self.correlator.consume(e);
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        if self.partial.len() > 1_048_576 {
            self.partial.clear();
            self.correlator.loss(1);
        }
        Ok(())
    }
}
impl Drop for TraceSession {
    fn drop(&mut self) {
        let _ = fs::write(self.path.join("tracing_on"), "0");
        let _ = fs::write(self.path.join("events/enable"), "0");
        drop(self.pipe.take());
        let _ = fs::remove_dir(&self.path);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offcpu_is_distinct_from_wakeup_and_loss_breaks_pairing() {
        let mut c = Correlator::default();
        let event = |at_ns, payload: &str| TraceEvent {
            at_ns,
            cpu: 0,
            pid: 7,
            name: "sched_switch".into(),
            payload: payload.into(),
        };
        c.consume(event(1_000_000, "prev_pid=7 next_pid=8 prev_state=S"));
        c.consume(TraceEvent {
            at_ns: 9_000_000,
            cpu: 0,
            pid: 7,
            name: "sched_wakeup".into(),
            payload: "pid=7".into(),
        });
        c.consume(event(
            10_000_000,
            "prev_pid=8 next_pid=7 prev_state=R cgroup=100",
        ));
        assert_eq!(c.off_ms[&7], vec![9.]);
        assert_eq!(c.wake_ms[&7], vec![1.]);
        assert_eq!(c.wake_cgroup[&100], vec![1.]);
        c.consume(event(11_000_000, "prev_pid=7 next_pid=8 prev_state=S"));
        c.loss(1);
        c.consume(event(20_000_000, "prev_pid=8 next_pid=7 prev_state=R"));
        assert_eq!(c.off_ms[&7], vec![9.]);
    }
    #[test]
    fn parse_and_pair() {
        let mut c = Correlator::default();
        c.consume(parse("task-10 [003] d..2 100.000000: sched_wakeup: comm=envoy pid=42 prio=120 target_cpu=3").unwrap());
        c.consume(parse("idle-0 [003] d..2 100.018000: sched_switch: prev_comm=idle prev_pid=0 prev_state=S ==> next_comm=envoy next_pid=42").unwrap());
        assert_eq!(c.wake_ms[&42], vec![18.]);
    }
    #[test]
    fn loss_invalidates_pending() {
        let mut c = Correlator::default();
        c.consume(parse("task-10 [003] .... 1.0: sys_enter: id=9").unwrap());
        c.loss(3);
        c.consume(parse("task-10 [003] .... 1.1: sys_exit: id=9 ret=0").unwrap());
        assert!(c.syscall_ms.is_empty());
        assert_eq!(c.lost, 3);
    }
    #[test]
    fn quantiles() {
        assert_eq!(percentile(&[1., 3., 2., 4.], 0.5), Some(2.));
        assert_eq!(percentile(&[], 0.99), None);
    }
}
