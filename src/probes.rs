//! Compiled CO-RE raw tracepoint acquisition, attached only on explicit request.
use crate::{
    domain::*,
    tracing::{Correlator, TraceEvent},
};
use aya::{
    maps::{MapData, PerCpuArray, RingBuf, StackTraceMap},
    programs::RawTracePoint,
    Ebpf,
};
use std::{collections::BTreeMap, io};
static ACTIVE_STATS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub fn statistics_active() -> bool {
    ACTIVE_STATS.load(std::sync::atomic::Ordering::Relaxed) > 0
}
fn thread_cpu_ns() -> u64 {
    let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Event {
    ns: u64,
    key: u64,
    a: u64,
    b: u64,
    kind: u32,
    cpu: u32,
    pid: u32,
    pad: u32,
    args: [u64; 6],
    text: [u8; 80],
}
pub struct Probes {
    capture_id: String,
    owned_attachments: BTreeMap<String, String>,
    _bpf: Ebpf,
    stats: Option<std::os::fd::OwnedFd>,
    stats_error: Option<String>,
    started: std::time::Instant,
    cpu_started: u64,
    ring: RingBuf<MapData>,
    losses: PerCpuArray<MapData, u64>,
    stacks: StackTraceMap<MapData>,
    pub correlator: Correlator,
    pending: BTreeMap<u64, (u64, u64, u64)>,
    pub block_ms: Vec<f64>,
    pub device_ms: BTreeMap<u64, Vec<f64>>,
    last_loss: u64,
    pub mode: String,
    /// Thread the capture is scoped to; 0 when unscoped.
    pub target_pid: u32,
    /// Folded stacks from this capture, resolved as they arrive.
    pub profile: crate::flame::Profile,
    symbols: crate::symbols::Symbols,
    pub start_ns: Option<u64>,
    pub last_ns: u64,
    pub processed: u64,
    pub duration_seconds: u64,
    pub scope: String,
}
fn err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}
impl Probes {
    pub fn start(mode: &str) -> io::Result<Self> {
        let mut words = mode.split_whitespace();
        let mode = words.next().ok_or_else(|| err("missing probe type"))?;
        let mut capture_stack = 0u32;
        let mut pid = 0u32;
        let mut cgroup = 0u64;
        let mut duration_seconds = 30;
        for option in words {
            if option == "stack" {
                capture_stack = 1;
                continue;
            }
            if let Some(value) = option.strip_prefix("pid=") {
                pid = value.parse().map_err(err)?;
                if !["sched", "offcpu", "syscalls"].contains(&mode) {
                    return Err(err("PID scope supports sched/syscalls"));
                }
            } else if let Some(value) = option.strip_prefix("seconds=") {
                duration_seconds = value.parse::<u64>().map_err(err)?;
                if !(1..=60).contains(&duration_seconds) {
                    return Err(err("duration must be 1..60 seconds"));
                }
            } else if let Some(value) = option.strip_prefix("cgroup=") {
                use std::os::unix::fs::MetadataExt;
                if mode != "syscalls" || value.split('/').any(|s| s == ".." || s == ".") {
                    return Err(err(
                        "cgroup scope requires syscalls and a valid cgroup path",
                    ));
                }
                cgroup = std::fs::metadata(
                    std::path::Path::new("/sys/fs/cgroup").join(value.trim_start_matches('/')),
                )?
                .ino();
            } else {
                return Err(err(format!("unknown probe option {option}")));
            }
        }
        if capture_stack != 0 && (mode != "syscalls" || pid == 0) {
            return Err(err("user stack capture requires syscalls pid=TID stack"));
        }
        let scope =
            format!("{mode} · TID {pid} (0=all) · cgroup id {cgroup} · {duration_seconds}s");
        let programs: &[(&str, &str)] = match mode {
            "all" => &[
                ("kw_wake", "sched_wakeup"),
                ("kw_wake_new", "sched_wakeup_new"),
                ("kw_switch", "sched_switch"),
                ("kw_migrate", "sched_migrate_task"),
                ("kw_exit", "sched_process_exit"),
                ("kw_soft_enter", "softirq_entry"),
                ("kw_soft_exit", "softirq_exit"),
                ("kw_irq_enter", "irq_handler_entry"),
                ("kw_irq_exit", "irq_handler_exit"),
                ("kw_sys_enter", "sys_enter"),
                ("kw_sys_exit", "sys_exit"),
                ("kw_block_issue", "block_rq_issue"),
                ("kw_block_done", "block_rq_complete"),
                ("kw_block_requeue", "block_rq_requeue"),
            ],
            "sched" | "offcpu" => &[
                ("kw_wake", "sched_wakeup"),
                ("kw_wake_new", "sched_wakeup_new"),
                ("kw_switch", "sched_switch"),
                ("kw_migrate", "sched_migrate_task"),
                ("kw_exit", "sched_process_exit"),
            ],
            "irq" => &[
                ("kw_soft_enter", "softirq_entry"),
                ("kw_soft_exit", "softirq_exit"),
                ("kw_irq_enter", "irq_handler_entry"),
                ("kw_irq_exit", "irq_handler_exit"),
            ],
            "syscalls" => &[
                ("kw_sys_enter", "sys_enter"),
                ("kw_sys_exit", "sys_exit"),
                ("kw_exit", "sched_process_exit"),
            ],
            "block" => &[
                ("kw_block_issue", "block_rq_issue"),
                ("kw_block_done", "block_rq_complete"),
                ("kw_block_requeue", "block_rq_requeue"),
            ],
            _ => return Err(err("probe must be sched, offcpu, irq, block or syscalls")),
        };
        let (stats, stats_error) = match aya::sys::enable_stats(aya::sys::Stats::RunTime) {
            Ok(fd) => (Some(fd), None),
            Err(e) => (None, Some(e.to_string())),
        };
        #[cfg(target_arch = "aarch64")]
        let object = aya::include_bytes_aligned!("../probes/kernwatch-aarch64.bpf.o");
        #[cfg(not(target_arch = "aarch64"))]
        let object = aya::include_bytes_aligned!("../probes/kernwatch.bpf.o");
        let mut bpf = aya::EbpfLoader::new()
            .set_global("capture_stack", &capture_stack, true)
            .set_global("target_pid", &pid, true)
            .set_global("target_cgroup", &cgroup, true)
            .load(object)
            .map_err(err)?;
        let mut owned_attachments = BTreeMap::new();
        for (name, attach) in programs {
            let p: &mut RawTracePoint = bpf
                .program_mut(name)
                .ok_or_else(|| err("compiled program missing"))?
                .try_into()
                .map_err(err)?;
            p.load().map_err(err)?;
            p.attach(attach).map_err(err)?;
            owned_attachments.insert(p.info().map_err(err)?.id().to_string(), attach.to_string());
        }
        let mut symbols = crate::symbols::Symbols::default();
        if capture_stack != 0 {
            symbols.load_kernel();
        }
        let stack_source = format!("{mode} · TID {pid} · user stacks");
        let stacks = StackTraceMap::try_from(
            bpf.take_map("stacks")
                .ok_or_else(|| err("missing stack map"))?,
        )
        .map_err(err)?;
        let ring = RingBuf::try_from(
            bpf.take_map("events")
                .ok_or_else(|| err("missing events map"))?,
        )
        .map_err(err)?;
        let losses = PerCpuArray::try_from(
            bpf.take_map("losses")
                .ok_or_else(|| err("missing loss map"))?,
        )
        .map_err(err)?;
        if stats.is_some() {
            ACTIVE_STATS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(Self {
            capture_id: crate::recording::stamp().to_string(),
            owned_attachments,
            stats,
            stats_error,
            started: std::time::Instant::now(),
            cpu_started: thread_cpu_ns(),
            _bpf: bpf,
            ring,
            losses,
            stacks,
            correlator: Correlator::default(),
            pending: BTreeMap::new(),
            block_ms: Vec::new(),
            device_ms: BTreeMap::new(),
            last_loss: 0,
            mode: mode.into(),
            target_pid: pid,
            profile: crate::flame::Profile::new(&stack_source),
            symbols,
            start_ns: None,
            last_ns: 0,
            processed: 0,
            duration_seconds,
            scope,
        })
    }
    pub fn poll(&mut self) -> io::Result<()> {
        let lost = self.losses.get(&0, 0).map_err(err)?.iter().sum::<u64>();
        if lost > self.last_loss {
            self.correlator.loss(lost - self.last_loss);
            self.pending.clear();
            self.last_loss = lost;
        }
        let mut batch = Vec::new();
        for _ in 0..100_000 {
            let Some(item) = self.ring.next() else { break };
            if item.len() == std::mem::size_of::<Event>() {
                let event = unsafe { std::ptr::read_unaligned(item.as_ptr() as *const Event) };
                batch.push(event);
            }
        }
        for e in batch {
            self.processed += 1;
            self.start_ns.get_or_insert(e.ns);
            self.last_ns = e.ns;
            let (name, payload) = match e.kind {
                1 => ("sched_wakeup", format!("pid={}", e.key)),
                2 => (
                    "sched_switch",
                    format!(
                        "next_pid={} prev_pid={} prev_state={} cgroup={} start_ns={}",
                        e.key,
                        e.a,
                        if e.b != 0 { "R" } else { "S" },
                        e.args[0],
                        e.args[1]
                    ),
                ),
                11 => (
                    "sched_migrate_task",
                    format!("pid={} dest_cpu={} cgroup={}", e.key, e.a, e.b),
                ),
                3 => ("sched_process_exit", format!("pid={}", e.key)),
                4 => {
                    let end = e.text.iter().position(|b| *b == 0).unwrap_or(e.text.len());
                    let stack = if e.pad == u32::MAX {
                        String::new()
                    } else if (e.pad as i32) < 0 {
                        format!(" user_stack_errno={}", -(e.pad as i32))
                    } else {
                        match self.stacks.get(&e.pad, 0) {
                            Ok(trace) => {
                                // Outermost frame first, which is the order a
                                // profile folds; the kernel returns the
                                // innermost caller first.
                                let mut frames = trace
                                    .frames()
                                    .iter()
                                    .map(|f| self.symbols.frame(e.pid, f.ip, false))
                                    .collect::<Vec<_>>();
                                frames.reverse();
                                self.profile.add(&frames, 1);
                                format!(" user_stack={}", frames.join(";"))
                            }
                            Err(error) => format!(" user_stack_unavailable={error}"),
                        }
                    };
                    (
                        "sys_enter",
                        format!(
                            "id={} path={} args={:x?}{stack}",
                            e.key,
                            String::from_utf8_lossy(&e.text[..end]),
                            e.args
                        ),
                    )
                }
                5 => ("sys_exit", format!("ret={}", e.a as i64)),
                6 => (
                    if e.a == 0 {
                        "softirq_entry"
                    } else {
                        "irq_handler_entry"
                    },
                    format!("{}={}", if e.a == 0 { "vec" } else { "irq" }, e.key),
                ),
                7 => (
                    if e.a == 0 {
                        "softirq_exit"
                    } else {
                        "irq_handler_exit"
                    },
                    format!("{}={}", if e.a == 0 { "vec" } else { "irq" }, e.key),
                ),
                8 => {
                    self.pending.insert(e.key, (e.ns, e.a, e.b));
                    if self.pending.len() > 16384 {
                        self.pending.clear();
                        self.correlator.loss(1);
                    }
                    continue;
                }
                9 => {
                    if let Some((start, remaining, device)) = self.pending.get_mut(&e.key) {
                        if e.a >= *remaining {
                            let start = *start;
                            let device = *device;
                            let latencies = self.device_ms.entry(device).or_default();
                            latencies.push(e.ns.saturating_sub(start) as f64 / 1e6);
                            if latencies.len() > 4096 {
                                latencies.remove(0);
                            }
                            self.pending.remove(&e.key);
                            self.block_ms.push(e.ns.saturating_sub(start) as f64 / 1e6);
                            if self.block_ms.len() > 4096 {
                                self.block_ms.remove(0);
                            }
                        } else {
                            *remaining -= e.a;
                        }
                    }
                    continue;
                }
                10 => {
                    self.pending.remove(&e.key);
                    continue;
                }
                _ => continue,
            };
            self.correlator.consume(TraceEvent {
                at_ns: e.ns,
                cpu: e.cpu,
                pid: e.pid,
                name: name.into(),
                payload,
            });
        }
        Ok(())
    }
    pub fn apply(&self, t: &mut Telemetry) {
        self.correlator.apply(t);
        if !self.profile.is_empty() {
            let mut profile = self.profile.clone();
            profile.sort();
            t.profile = profile;
        }
        t.details.insert(
            "probe.capture_id".into(),
            vec![("id".into(), self.capture_id.clone())],
        );
        t.details.insert(
            "owned_program_ids".into(),
            self._bpf
                .programs()
                .filter_map(|(name, p)| {
                    p.info()
                        .ok()
                        .map(|i| (i.id().to_string(), name.to_string()))
                })
                .collect(),
        );

        t.details.insert(
            "owned_attachments".into(),
            self.owned_attachments
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );
        let elapsed = self.started.elapsed().as_secs_f64().max(0.001);
        let ingest_cpu =
            thread_cpu_ns().saturating_sub(self.cpu_started) as f64 / 1e9 / elapsed * 100.;
        t.metrics.insert(
            "trace.ingest_cpu".into(),
            Measurement::known(
                ingest_cpu,
                "%",
                "trace worker thread CPU time / capture; one CPU",
                t.at_ms,
            ),
        );
        if self.stats.is_some() {
            let ns = self
                ._bpf
                .programs()
                .filter_map(|(_, p)| p.info().ok())
                .map(|i| i.run_time().as_nanos())
                .sum::<u128>();
            t.metrics.insert(
                "bpf.own".into(),
                Measurement::known(
                    ns as f64 / 1e9 / elapsed * 100.,
                    "%",
                    "owned BPF program runtime / capture; one CPU",
                    t.at_ms,
                ),
            );
            t.capabilities
                .insert("bpf.runtime".into(), Quality::Available);
        } else {
            t.capabilities.insert(
                "bpf.runtime".into(),
                Quality::Denied(self.stats_error.clone().unwrap_or_default()),
            );
        }

        t.details.insert(
            "probe.scope".into(),
            vec![
                (
                    "backend".into(),
                    format!(
                        "CO-RE raw tracepoints · {} · {}s limit",
                        self.mode, self.duration_seconds
                    ),
                ),
                ("scope".into(), self.scope.clone()),
                (
                    "buffer".into(),
                    "16 MiB ring; occupancy not measured".into(),
                ),
                ("events".into(), self.processed.to_string()),
            ],
        );
        if self.mode == "all" {
            for name in ["scheduler", "irq", "block", "syscalls"] {
                t.capabilities.insert(name.into(), Quality::Available);
            }
        } else {
            t.capabilities.insert(
                if self.mode == "sched" || self.mode == "offcpu" {
                    "scheduler".into()
                } else {
                    self.mode.clone()
                },
                Quality::Available,
            );
        }
        let elapsed = self
            .last_ns
            .saturating_sub(self.start_ns.unwrap_or(self.last_ns)) as f64
            / 1e9;
        if elapsed > 0. {
            for (key, n) in self
                .correlator
                .task_migrations
                .iter()
                .map(|(id, n)| (format!("sched.migrations.task{id}"), n))
                .chain(
                    self.correlator
                        .group_migrations
                        .iter()
                        .map(|(id, n)| (format!("sched.migrations.cgroup{id}"), n)),
                )
            {
                t.metrics.insert(
                    key,
                    Measurement::known(
                        *n as f64 / elapsed,
                        "/s",
                        "BPF migration count / capture; membership observed at migration",
                        t.at_ms,
                    ),
                );
            }
            for (cpu, n) in &self.correlator.switches {
                t.metrics.insert(
                    format!("sched.switches.cpu{cpu}"),
                    Measurement::known(
                        *n as f64 / elapsed,
                        "/s",
                        "BPF sched_switch count / capture",
                        t.at_ms,
                    ),
                );
            }
            for (cpu, n) in &self.correlator.migrations {
                t.metrics.insert(
                    format!("sched.migrations.cpu{cpu}"),
                    Measurement::known(
                        *n as f64 / elapsed,
                        "/s",
                        "BPF migrations to destination CPU / capture",
                        t.at_ms,
                    ),
                );
            }
        }
        for (key, values) in [
            (
                "sched",
                self.correlator
                    .wake_ms
                    .values()
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>(),
            ),
            (
                "syscall",
                self.correlator
                    .syscall_ms
                    .values()
                    .flatten()
                    .copied()
                    .collect(),
            ),
            ("block", self.block_ms.clone()),
        ] {
            if !values.is_empty() {
                let bounds = vec![
                    0.001, 0.004, 0.016, 0.064, 0.256, 1., 4., 16., 64., 256., 1024.,
                ];
                let mut counts = vec![0; bounds.len() + 1];
                for value in values {
                    let bin = bounds
                        .iter()
                        .position(|b| value <= *b)
                        .unwrap_or(bounds.len());
                    counts[bin] += 1;
                }
                t.histograms.insert(
                    key.into(),
                    Histogram {
                        bounds,
                        counts,
                        unit: "ms".into(),
                    },
                );
            }
        }
        for (key, values) in self
            .correlator
            .wake_ms
            .iter()
            .map(|(pid, v)| (format!("task.{pid}"), v))
            .chain(
                self.correlator
                    .wake_cpu
                    .iter()
                    .map(|(cpu, v)| (format!("sched.cpu{cpu}"), v)),
            )
            .chain(
                self.correlator
                    .wake_cgroup
                    .iter()
                    .map(|(id, v)| (format!("sched.cgroup{id}"), v)),
            )
        {
            let bounds = vec![
                0.001, 0.004, 0.016, 0.064, 0.256, 1., 4., 16., 64., 256., 1024.,
            ];
            let mut counts = vec![0; bounds.len() + 1];
            for value in values {
                counts[bounds
                    .iter()
                    .position(|b| value <= b)
                    .unwrap_or(bounds.len())] += 1;
            }
            t.histograms.insert(
                key,
                Histogram {
                    bounds,
                    counts,
                    unit: "ms".into(),
                },
            );
        }
        for (pid, values) in &self.correlator.off_ms {
            t.record(
                &format!("offcpu.{pid}"),
                crate::tracing::percentile(values, 0.99),
                "ms",
                1000.,
            );
            t.details.insert(format!("offcpu:{pid}"),vec![("off-CPU p99 ms".into(),crate::tracing::percentile(values,0.99).unwrap_or(0.).to_string()),("scope".into(),"switch-out to switch-in; includes sleeping and runnable waiting; distinct from wake-to-run".into())]);
        }
        for (group, values) in &self.correlator.wake_cgroup {
            t.record(
                &format!("sched.cgroup{group}"),
                crate::tracing::percentile(values, 0.99),
                "ms",
                20.,
            );
            t.details.insert(
                format!("wakegroup:{group}"),
                vec![
                    (
                        "p99".into(),
                        crate::tracing::percentile(values, 0.99)
                            .unwrap_or(0.)
                            .to_string(),
                    ),
                    ("sample count".into(), values.len().to_string()),
                    (
                        "scope".into(),
                        "direct cgroup membership observed at switch-in; last 4096 completed waits"
                            .into(),
                    ),
                ],
            );
        }
        for (pid, values) in &self.correlator.wake_ms {
            if let Some(ns) = self.correlator.task_starts.get(pid) {
                let ticks = (*ns as u128
                    * unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u128
                    / 1_000_000_000) as u64;
                t.record(
                    &format!("task.{pid}.{ticks}"),
                    crate::tracing::percentile(values, 0.99),
                    "ms",
                    20.,
                );
            }
            t.record(
                &format!("task.{pid}"),
                crate::tracing::percentile(values, 0.99),
                "ms",
                20.,
            );
            t.details.insert(
                format!("wake:{pid}"),
                vec![
                    (
                        "start ticks".into(),
                        self.correlator
                            .task_starts
                            .get(pid)
                            .map(|ns| {
                                ((*ns as u128
                                    * unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u128)
                                    / 1_000_000_000)
                                    .to_string()
                            })
                            .unwrap_or("unknown".into()),
                    ),
                    (
                        "p50".into(),
                        crate::tracing::percentile(values, 0.5)
                            .unwrap_or(0.)
                            .to_string(),
                    ),
                    (
                        "p99".into(),
                        crate::tracing::percentile(values, 0.99)
                            .unwrap_or(0.)
                            .to_string(),
                    ),
                ],
            );
        }
        for (cpu, values) in &self.correlator.wake_cpu {
            t.record(
                &format!("sched.cpu{cpu}"),
                crate::tracing::percentile(values, 0.99),
                "ms",
                20.,
            );
            t.details.insert(
                format!("wakecpu:{cpu}"),
                vec![
                    (
                        "p50".into(),
                        crate::tracing::percentile(values, 0.5)
                            .unwrap_or(0.)
                            .to_string(),
                    ),
                    (
                        "p99".into(),
                        crate::tracing::percentile(values, 0.99)
                            .unwrap_or(0.)
                            .to_string(),
                    ),
                ],
            );
        }
        for (id, values) in &self.correlator.syscall_ms {
            let bounds = vec![
                0.001, 0.004, 0.016, 0.064, 0.256, 1., 4., 16., 64., 256., 1024.,
            ];
            let mut counts = vec![0; bounds.len() + 1];
            for value in values {
                let i = bounds
                    .iter()
                    .position(|b| value <= b)
                    .unwrap_or(bounds.len());
                counts[i] += 1;
            }
            t.histograms.insert(
                format!("syscall:{}", syscall_name(*id)),
                Histogram {
                    bounds,
                    counts,
                    unit: "ms".into(),
                },
            );
        }
        let rows = self
            .correlator
            .syscall_ms
            .iter()
            .map(|(id, values)| {
                let n = self.correlator.syscall_count.get(id).copied().unwrap_or(0);
                vec![
                    syscall_name(*id),
                    format!("{:.1}", n as f64 / elapsed.max(0.001)),
                    format!(
                        "{:.1}",
                        self.correlator.errors.get(id).copied().unwrap_or(0) as f64
                            / elapsed.max(0.001)
                    ),
                    self.correlator
                        .errno_counts
                        .iter()
                        .filter(|((syscall, _), _)| syscall == id)
                        .max_by_key(|(_, n)| *n)
                        .map(|((_, errno), _)| errno_name(*errno))
                        .unwrap_or("—".into()),
                    format!(
                        "{:.6}ms",
                        values.iter().sum::<f64>() / values.len().max(1) as f64
                    ),
                    format!(
                        "{:.6}ms",
                        crate::tracing::percentile(values, 0.99).unwrap_or(0.)
                    ),
                    format!(
                        "{:.6}ms",
                        values.iter().copied().reduce(f64::max).unwrap_or(0.)
                    ),
                    self.correlator
                        .syscall_callers
                        .get(id)
                        .and_then(|callers| callers.iter().max_by_key(|(_, n)| *n))
                        .map(|(pid, _)| pid.to_string())
                        .unwrap_or("—".into()),
                    format!("n={}", values.len()),
                    if self.correlator.errors.get(id).copied().unwrap_or(0) > 0 {
                        "negative returns"
                    } else {
                        "completed"
                    }
                    .into(),
                ]
            })
            .collect::<Vec<_>>();
        for row in &rows {
            let value = row[5].trim_end_matches("ms").parse::<f64>().ok();
            t.metrics.insert(
                format!("syscall.{}.p99", row[0]),
                Measurement {
                    value,
                    unit: "ms".into(),
                    source: "syscall enter/exit; last 4096 completed calls".into(),
                    end_ms: t.at_ms,
                    quality: if self.correlator.lost == 0 {
                        Quality::Available
                    } else {
                        Quality::Stale
                    },
                    ..Default::default()
                },
            );
            t.record(
                &format!("syscall.{}.p99", row[0]),
                value,
                "ms",
                value.unwrap_or(1.).max(1.),
            );
        }
        if self.mode == "syscalls" || self.mode == "all" {
            t.details.insert(
                "syscall_rows".into(),
                rows.iter()
                    .enumerate()
                    .map(|(i, r)| (i.to_string(), r.join("\t")))
                    .collect(),
            );
        }
        if (self.mode == "irq" || self.mode == "all") && elapsed > 0. {
            for ((cpu, vector), ns) in &self.correlator.vector_ns {
                let value = *ns as f64 / 1e9 / elapsed * 100.;
                let key = if *vector < 1_000_000 {
                    format!("softirq.cpu{cpu}.vec{vector}")
                } else {
                    format!("irq.cpu{cpu}.irq{}", vector - 1_000_000)
                };
                t.metrics.insert(
                    key.clone(),
                    Measurement::known(value, "%", "BPF entry/exit duration / capture", t.at_ms),
                );
                t.record(&key, Some(value), "%", 100.);
            }
            let values = self
                .correlator
                .vector_ns
                .iter()
                .map(|((cpu, vec), ns)| {
                    (
                        format!("cpu{cpu} vector {vec}"),
                        format!("{:.2}% execution", *ns as f64 / 1e9 / elapsed * 100.),
                    )
                })
                .collect();
            t.details.insert("softirqs".into(), values);
            let max = self
                .correlator
                .vector_ns
                .iter()
                .filter(|((_, vec), _)| *vec == 3)
                .map(|(_, ns)| *ns as f64 / 1e9 / elapsed * 100.)
                .reduce(f64::max);
            if let Some(v) = max {
                t.metrics.insert(
                    "softirq".into(),
                    Measurement::known(v, "%", "BPF NET_RX duration / capture", t.at_ms),
                );
            }
        }
        if let Some(v) = crate::tracing::percentile(&self.block_ms, 0.99) {
            t.metrics.insert(
                "disk.p99".into(),
                Measurement {
                    value: Some(v),
                    unit: "ms".into(),
                    source: "BPF request identity / completion bytes".into(),
                    count: self.block_ms.len() as u64,
                    start_ms: self.start_ns.unwrap_or(0) / 1_000_000,
                    end_ms: t.at_ms,
                    quality: Quality::Available,
                },
            );
            t.record("disk.p99", Some(v), "ms", v.max(5.));
        } else if (self.mode == "block" || self.mode == "all") && elapsed > 0. {
            // The block probe is attached and has seen no completion yet. That
            // is an observed idle disk, so hold the baseline instead of leaving
            // the history blank until the first request lands.
            t.record("disk.p99", Some(0.), "ms", 5.);
        }
        if let Some(age) = self
            .pending
            .values()
            .map(|(ns, _, _)| t.at_ms.saturating_mul(1_000_000).saturating_sub(*ns) as f64 / 1e6)
            .reduce(f64::max)
        {
            t.metrics.insert(
                "disk.age".into(),
                Measurement::known(age, "ms", "BPF oldest pending request", t.at_ms),
            );
        }
        for (vector, values) in &self.correlator.vector_ms {
            if *vector >= 1_000_000 {
                if let Some(value) = crate::tracing::percentile(values, 0.99) {
                    t.metrics.insert(
                        format!("irq.vec{}.p99", vector - 1_000_000),
                        Measurement::known(
                            value,
                            "ms",
                            "IRQ handler entry/exit wall duration p99",
                            t.at_ms,
                        ),
                    );
                }
            }
        }
        for (device, values) in &self.device_ms {
            if let Some(value) = crate::tracing::percentile(values, 0.99) {
                let key = format!("block.dev{}:{}.p99", device >> 32, device & 0xffffffff);
                t.metrics.insert(
                    key.clone(),
                    Measurement::known(
                        value,
                        "ms",
                        "BPF completion / whole-disk major:minor",
                        t.at_ms,
                    ),
                );
                t.record(&key, Some(value), "ms", value.max(5.));
            }
        }
        for (key, series) in t.series.clone() {
            if !t.metrics.contains_key(&key) {
                if let Some(value) = series.at(t.at_ms) {
                    t.metrics.insert(
                        key,
                        Measurement::known(value, &series.unit, "BPF capture percentile", t.at_ms),
                    );
                }
            }
        }
        for m in t.metrics.values_mut() {
            m.start_ms = self.start_ns.unwrap_or(0) / 1_000_000;
            if self.correlator.lost > 0 {
                m.quality = Quality::Stale;
                m.value = None;
            }
        }
    }
}

pub fn errno_name(errno: i64) -> String {
    match errno as i32 {
        libc::EPERM => "EPERM".into(),
        libc::ENOENT => "ENOENT".into(),
        libc::ESRCH => "ESRCH".into(),
        libc::EINTR => "EINTR".into(),
        libc::EIO => "EIO".into(),
        libc::EBADF => "EBADF".into(),
        libc::EAGAIN => "EAGAIN".into(),
        libc::ENOMEM => "ENOMEM".into(),
        libc::EACCES => "EACCES".into(),
        libc::EFAULT => "EFAULT".into(),
        libc::EEXIST => "EEXIST".into(),
        libc::EINVAL => "EINVAL".into(),
        libc::ENOSPC => "ENOSPC".into(),
        libc::EPIPE => "EPIPE".into(),
        libc::ENOSYS => "ENOSYS".into(),
        libc::ETIMEDOUT => "ETIMEDOUT".into(),
        libc::ECONNREFUSED => "ECONNREFUSED".into(),
        _ => format!("errno {errno}"),
    }
}
pub fn syscall_name(id: u64) -> String {
    let names = [
        (libc::SYS_read, "read"),
        (libc::SYS_write, "write"),
        (libc::SYS_close, "close"),
        (libc::SYS_mmap, "mmap"),
        (libc::SYS_nanosleep, "nanosleep"),
        (libc::SYS_fsync, "fsync"),
        (libc::SYS_futex, "futex"),
        (libc::SYS_clock_nanosleep, "clock_nanosleep"),
        (libc::SYS_epoll_pwait, "epoll_pwait"),
        (libc::SYS_openat, "openat"),
        (libc::SYS_newfstatat, "newfstatat"),
    ];
    #[cfg(target_arch = "x86_64")]
    if id == libc::SYS_epoll_wait as u64 {
        return "epoll_wait".into();
    }
    names
        .iter()
        .find(|(number, _)| *number as u64 == id)
        .map(|(_, name)| (*name).to_owned())
        .unwrap_or_else(|| format!("syscall_{id}"))
}

/// Carry a capture's folded stacks into the snapshot the views read.
///
/// The probe thread rebuilds its telemetry every poll, so a poll that folded
/// nothing must not erase what earlier polls collected. `retained` holds the
/// last profile that had samples; a new capture resets it, because stacks from
/// two captures are not one profile.
pub fn merge_profile(
    retained: &mut crate::flame::Profile,
    captured: &crate::flame::Profile,
    new_capture: bool,
    s: &mut crate::model::Snapshot,
) {
    if new_capture {
        *retained = crate::flame::Profile::default();
    }
    if !captured.is_empty() {
        *retained = captured.clone();
    }
    if !retained.is_empty() {
        s.telemetry.profile = retained.clone();
    }
}

pub fn hydrate(s: &mut crate::model::Snapshot) {
    for row in &mut s.views[9].rows {
        if row.len() >= 9
            && s.telemetry
                .details
                .get("owned_program_ids")
                .is_some_and(|ids| ids.iter().any(|(id, _)| row.first() == Some(id)))
        {
            row[7] = "kernwatch".into();
            if let Some((_, attach)) = s
                .telemetry
                .details
                .get("owned_attachments")
                .and_then(|v| v.iter().find(|(id, _)| row.first() == Some(id)))
            {
                row[8] = attach.clone();
            }
            if let Some(fields) = s.telemetry.details.get_mut(&format!("bpf:{}", row[0])) {
                fields.retain(|(k, _)| k != "owner" && k != "attachment");
                fields.push((
                    "owner".into(),
                    "kernwatch · owned handle in this capture".into(),
                ));
                fields.push(("attachment".into(), row[8].clone()));
            }
        }
    }

    if s.telemetry.trace_drops > 0 {
        s.telemetry
            .details
            .retain(|k, _| !k.starts_with("wake:") && !k.starts_with("wakecpu:"));
        for task in &mut s.telemetry.tasks {
            task.wake_p50_ms = None;
            task.wake_p99_ms = None;
        }
        for cpu in &mut s.telemetry.cpus {
            cpu.wake_p50_ms = None;
            cpu.wake_p99_ms = None;
        }
    }
    for device in &mut s.telemetry.devices {
        device.p99_ms = s
            .telemetry
            .metrics
            .get(&format!("block.dev{}.p99", device.major_minor))
            .and_then(|m| m.value);
    }
    for device in &s.telemetry.devices {
        if let Some(fields) = s
            .telemetry
            .details
            .get_mut(&format!("device:{}", device.name))
        {
            fields.push((
                "completed p99".into(),
                device
                    .p99_ms
                    .map(|v| format!("{v:.3} ms · whole-disk request completion"))
                    .unwrap_or("not captured".into()),
            ));
        }
    }
    for row in &mut s.views[6].rows {
        if let Some(id) = row.first() {
            let value = s
                .telemetry
                .metrics
                .get(&format!("irq.vec{id}.p99"))
                .and_then(|m| m.value);
            if row.len() > 6 {
                row[6] = value.map(|v| format!("{v:.3}ms")).unwrap_or("—".into());
            }
        }
    }
    for task in &mut s.telemetry.tasks {
        if let Some(v) = s.telemetry.details.get(&format!("wake:{}", task.pid)) {
            if v.iter()
                .find(|(k, _)| k == "start ticks")
                .and_then(|(_, v)| v.parse::<u64>().ok())
                != Some(task.start_ticks)
            {
                task.wake_p50_ms = None;
                task.wake_p99_ms = None;
                continue;
            }

            task.wake_p50_ms = v
                .iter()
                .find(|(k, _)| k == "p50")
                .and_then(|(_, v)| v.parse().ok());
            task.wake_p99_ms = v
                .iter()
                .find(|(k, _)| k == "p99")
                .and_then(|(_, v)| v.parse().ok());
        }
    }
    for cpu in &mut s.telemetry.cpus {
        cpu.switches_s = s
            .telemetry
            .metrics
            .get(&format!("sched.switches.cpu{}", cpu.id))
            .and_then(|m| m.value);
        cpu.migrations_s = s
            .telemetry
            .metrics
            .get(&format!("sched.migrations.cpu{}", cpu.id))
            .and_then(|m| m.value);
        if let Some(v) = s.telemetry.details.get(&format!("wakecpu:{}", cpu.id)) {
            cpu.wake_p50_ms = v
                .iter()
                .find(|(k, _)| k == "p50")
                .and_then(|(_, v)| v.parse().ok());
            cpu.wake_p99_ms = v
                .iter()
                .find(|(k, _)| k == "p99")
                .and_then(|(_, v)| v.parse().ok());
        }
    }
    if let Some(v) = s.telemetry.details.get("syscall_rows") {
        s.views[5]=crate::model::View::new(&crate::model::SYSCALL_COLUMNS,v.iter().map(|(_,v)|v.split('\t').map(String::from).collect()).collect(),&["BPF syscall enter/exit capture. Quantiles use the last 4096 completed events per syscall; rates use the capture interval."]);
    }
}

impl Drop for Probes {
    fn drop(&mut self) {
        if self.stats.is_some() {
            ACTIVE_STATS.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod architecture_tests {
    #[test]
    fn syscall_names_follow_the_target_abi() {
        assert_eq!(super::syscall_name(libc::SYS_read as u64), "read");
        assert_eq!(super::syscall_name(libc::SYS_openat as u64), "openat");
        assert_eq!(
            super::syscall_name(u64::MAX),
            format!("syscall_{}", u64::MAX)
        );
    }
}
