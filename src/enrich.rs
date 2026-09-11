//! Typed live source enrichment and history; demo-only values never enter this path.
use crate::{domain::*, model::Snapshot};
use std::{collections::BTreeMap, fs, time::Instant};
pub struct Enricher {
    topology: crate::sources::Topology,
    inventory: crate::bpf::Inventory,
    logs: crate::logs::KernelLog,
    previous: BTreeMap<String, Vec<u64>>,
    last: Instant,
    history: BTreeMap<String, Series>,

    boot: String,
}
impl Default for Enricher {
    fn default() -> Self {
        Self {
            topology: crate::sources::Topology::default(),
            inventory: crate::bpf::Inventory::default(),
            logs: crate::logs::KernelLog::default(),
            previous: BTreeMap::new(),
            last: Instant::now(),
            history: BTreeMap::new(),

            boot: read("/proc/sys/kernel/random/boot_id"),
        }
    }
}
fn read(p: &str) -> String {
    fs::read_to_string(p).unwrap_or_default().trim().into()
}
fn kv(p: &str) -> BTreeMap<String, String> {
    read(p)
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            Some((k.into(), v.trim().into()))
        })
        .collect()
}
fn leading(s: &str) -> Option<f64> {
    s.split_whitespace().next()?.parse().ok()
}
impl Enricher {
    pub fn enrich(&mut self, s: &mut Snapshot) {
        let dt = self.last.elapsed().as_secs_f64().max(0.001);
        self.last = Instant::now();
        let mut t = Telemetry {
            at_ms: monotonic_ms(),
            hostname: read("/proc/sys/kernel/hostname"),
            kernel: read("/proc/sys/kernel/osrelease"),
            boot_id: self.boot.clone(),
            series: std::mem::take(&mut self.history),
            ..Default::default()
        };
        for l in read("/proc/stat")
            .lines()
            .filter(|l| l.starts_with("cpu") && !l.starts_with("cpu "))
        {
            let mut words = l.split_whitespace();
            let key = words.next().unwrap();
            let id = key[3..].parse().unwrap_or(0);
            let values = words
                .filter_map(|v| v.parse::<u64>().ok())
                .take(8)
                .collect::<Vec<_>>();
            if values.len() < 8 {
                continue;
            }
            if let Some(prev) = self.previous.get(key) {
                let delta = values
                    .iter()
                    .zip(prev)
                    .map(|(v, p)| v.saturating_sub(*p))
                    .collect::<Vec<_>>();
                let total = delta.iter().sum::<u64>().max(1) as f64;
                let pct = |i: usize| delta[i] as f64 / total * 100.;
                let c = Cpu {
                    id,
                    user: pct(0) + pct(1),
                    kernel: pct(2),
                    irq: pct(5),
                    softirq: pct(6),
                    steal: pct(7),
                    busy: 100. - pct(3) - pct(4),
                    ..Default::default()
                };
                t.record(&format!("cpu.{id}"), Some(c.busy), "%", 100.);
                t.record(
                    &format!("softirq.cpu{id}"),
                    Some(c.softirq),
                    "% CPU execution",
                    100.,
                );
                t.cpus.push(c);
            }
            self.previous.insert(key.into(), values);
        }
        let user = t.cpus.iter().map(|c| c.user).sum::<f64>() / t.cpus.len().max(1) as f64;
        let kernel = t
            .cpus
            .iter()
            .map(|c| c.kernel + c.irq + c.softirq)
            .sum::<f64>()
            / t.cpus.len().max(1) as f64;
        let busy = t.cpus.iter().map(|c| c.busy).sum::<f64>() / t.cpus.len().max(1) as f64;
        t.record("cpu.busy", Some(busy), "%", 100.);
        t.record("cpu.user", Some(user), "%", 100.);
        t.record("cpu.kernel", Some(kernel), "%", 100.);
        let memory = kv("/proc/meminfo");
        for (raw, key) in [
            ("MemAvailable", "memory.available"),
            ("Cached", "memory.cache"),
            ("Slab", "memory.slab"),
            ("MemTotal", "memory.total"),
        ] {
            if let Some(n) = memory.get(raw).and_then(|s| leading(s)) {
                let value = n / 1048576.;
                t.metrics.insert(
                    key.into(),
                    Measurement::known(value, "GiB", "/proc/meminfo", t.at_ms),
                );
                t.record(key, Some(value), "GiB", s.mem_total as f64 / 1024.);
            }
        }
        let used = s.mem_used as f64 / 1024.;
        t.metrics.insert(
            "memory.used".into(),
            Measurement::known(used, "GiB", "MemTotal - MemAvailable", t.at_ms),
        );
        t.record("memory.used", Some(used), "GiB", s.mem_total as f64 / 1024.);
        for (resource, key) in [
            ("cpu", "psi.cpu"),
            ("memory", "psi.memory"),
            ("io", "psi.io"),
        ] {
            let text = read(&format!("/proc/pressure/{resource}"));
            if let Some(value) = text.lines().find(|l| l.starts_with("some ")).and_then(|l| {
                l.split_whitespace()
                    .find_map(|s| s.strip_prefix("avg10=")?.parse::<f64>().ok())
            }) {
                t.metrics.insert(
                    key.into(),
                    Measurement::known(value, "%", "PSI some avg10", t.at_ms),
                );
                t.record(key, Some(value), "%", 100.);
            }
        }
        for l in read("/proc/vmstat").lines() {
            let v = l.split_whitespace().collect::<Vec<_>>();
            if v.len() != 2 {
                continue;
            }
            let name = match v[0] {
                "pgfault" => "fault.total",
                "pgmajfault" => "fault.major",
                _ => continue,
            };
            let n = v[1].parse::<u64>().unwrap_or(0);
            if let Some(p) = self.previous.get(name) {
                let rate = n.saturating_sub(p[0]) as f64 / dt;
                t.metrics.insert(
                    name.into(),
                    Measurement::known(rate, "/s", "/proc/vmstat delta", t.at_ms),
                );
                t.record(name, Some(rate), "/s", rate.max(1.) * 1.5);
            }
            self.previous.insert(name.into(), vec![n]);
        }
        if let (Some(total), Some(major)) = (t.value("fault.total"), t.value("fault.major")) {
            let minor = (total - major).max(0.);
            t.metrics.insert(
                "fault.minor".into(),
                Measurement::known(
                    minor,
                    "/s",
                    "/proc/vmstat pgfault - pgmajfault delta",
                    t.at_ms,
                ),
            );
            t.record("fault.minor", Some(minor), "/s", minor.max(1.) * 1.5);
        }
        let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as f64;
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(1) as u64;
        let mut current_tasks = BTreeMap::new();
        let mut thread_ids = std::collections::BTreeSet::new();
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|s| s.parse::<u32>().ok())
                else {
                    continue;
                };
                if let Ok(threads) = fs::read_dir(format!("/proc/{pid}/task")) {
                    for thread in threads.flatten() {
                        if let Some(tid) = thread
                            .file_name()
                            .to_str()
                            .and_then(|s| s.parse::<u32>().ok())
                        {
                            thread_ids.insert((pid, tid));
                            if thread_ids.len() >= 10000 {
                                break;
                            }
                        }
                    }
                }
                if thread_ids.len() >= 10000 {
                    break;
                }
            }
        }
        {
            for (tgid, pid) in thread_ids {
                let path = format!("/proc/{tgid}/task/{pid}");
                let stat = read(&format!("{path}/stat"));
                let Some((name, state, cpu, ticks, rss)) = crate::collect::parse_task(&stat) else {
                    continue;
                };
                let Some(end) = stat.rfind(')') else { continue };
                let fields = stat[end + 2..].split_whitespace().collect::<Vec<_>>();
                let start_ticks = fields
                    .get(19)
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                let key = format!("task:{pid}:{start_ticks}");
                let pct = self
                    .previous
                    .get(&key)
                    .map(|p| ticks.saturating_sub(p[0]) as f64 / hz / dt * 100.);
                let status = kv(&format!("{path}/status"));
                let ctx = |name: &str| status.get(name).and_then(|v| v.parse::<u64>().ok());
                let voluntary = ctx("voluntary_ctxt_switches");
                let involuntary = ctx("nonvoluntary_ctxt_switches");
                let ctx_rate = |n: Option<u64>, i: usize| {
                    n.and_then(|n| {
                        self.previous
                            .get(&key)
                            .and_then(|v| v.get(i))
                            .map(|p| n.saturating_sub(*p) as f64 / dt)
                    })
                };
                let voluntary_s = ctx_rate(voluntary, 1);
                let involuntary_s = ctx_rate(involuntary, 2);
                let blocked_since = if state == "D" {
                    self.previous
                        .get(&key)
                        .and_then(|v| v.get(3))
                        .copied()
                        .filter(|n| *n > 0)
                        .unwrap_or(t.at_ms)
                } else {
                    0
                };
                let blocked_ms =
                    (blocked_since > 0).then(|| t.at_ms.saturating_sub(blocked_since) as f64);
                let minor = fields.get(7).and_then(|v| v.parse::<u64>().ok());
                let minor_faults_s = minor.and_then(|n| {
                    self.previous
                        .get(&key)
                        .and_then(|v| v.get(4))
                        .map(|old| n.saturating_sub(*old) as f64 / dt)
                });
                let rss_growth_bytes_s = self
                    .previous
                    .get(&key)
                    .and_then(|v| v.get(5))
                    .map(|old| (rss as f64 - *old as f64) * page as f64 / dt);
                current_tasks.insert(
                    key,
                    vec![
                        ticks,
                        voluntary.unwrap_or(0),
                        involuntary.unwrap_or(0),
                        blocked_since,
                        minor.unwrap_or(0),
                        rss,
                    ],
                );
                let affinity = status.get("Cpus_allowed_list").cloned().unwrap_or_default();
                let cgroup = read(&format!("{path}/cgroup"))
                    .lines()
                    .find_map(|l| l.strip_prefix("0::"))
                    .unwrap_or("")
                    .to_string();
                let memory_field = |key: &str| {
                    status
                        .get(key)
                        .and_then(|v| v.split_whitespace().next())
                        .and_then(|v| v.parse::<u64>().ok())
                        .map(|v| v.saturating_mul(1024))
                };
                t.tasks.push(Task {
                    nice: fields.get(16).and_then(|v| v.parse().ok()),
                    age_ms: Some(
                        t.at_ms
                            .saturating_sub((start_ticks as f64 / hz * 1000.) as u64),
                    ),
                    anon_bytes: memory_field("RssAnon"),
                    file_bytes: memory_field("RssFile"),
                    shmem_bytes: memory_field("RssShmem"),
                    swap_bytes: memory_field("VmSwap"),
                    minor_faults_s,
                    rss_growth_bytes_s,
                    uid: status
                        .get("Uid")
                        .and_then(|v| v.split_whitespace().next())
                        .and_then(|v| v.parse().ok()),
                    parent_pid: fields.get(1).and_then(|v| v.parse().ok()).unwrap_or(0),
                    tgid,
                    kernel_thread: fields
                        .get(6)
                        .and_then(|s| s.parse::<u64>().ok())
                        .is_some_and(|flags| flags & 0x00200000 != 0),
                    pid,
                    start_ticks,
                    name,
                    state: state.clone(),
                    cpu: cpu as u32,
                    cpu_pct: pct,
                    voluntary_s,
                    involuntary_s,
                    blocked_ms,
                    rss_bytes: rss * page,
                    cgroup,
                    affinity,
                    policy: fields
                        .get(38)
                        .map(|v| {
                            match *v {
                                "0" => "OTHER",
                                "1" => "FIFO",
                                "2" => "RR",
                                "3" => "BATCH",
                                "5" => "IDLE",
                                "6" => "DEADLINE",
                                _ => "unknown",
                            }
                            .into()
                        })
                        .unwrap_or_default(),
                    wchan: read(&format!("{path}/wchan")),
                    verdict: if state == "D" {
                        "blocked"
                    } else if pct.unwrap_or(0.) > 90. {
                        "CPU busy"
                    } else {
                        "—"
                    }
                    .into(),
                    ..Default::default()
                });
                if t.tasks.len() >= 10_000 {
                    break;
                }
            }
        }
        self.previous.retain(|k, _| !k.starts_with("task:"));
        self.previous.extend(current_tasks);
        t.tasks.sort_by(|a, b| {
            let severity = |x: &Task| {
                if x.state == "D" {
                    1000.
                } else {
                    x.cpu_pct.unwrap_or(0.)
                }
            };
            severity(b).total_cmp(&severity(a))
        });
        let slab = read("/proc/slabinfo");
        for line in slab.lines().skip(2) {
            let v = line.split_whitespace().collect::<Vec<_>>();
            if v.len() >= 6 {
                t.details.insert(format!("slab:{}",v[0]),vec![("cache".into(),v[0].into()),("active objects".into(),v[1].into()),("total objects".into(),v[2].into()),("object bytes".into(),v[3].into()),("objects / slab".into(),v[4].into()),("pages / slab".into(),v[5].into()),("attribution".into(),"Cache size does not identify allocating process; inspect allocation/free evidence".into())]);
            }
        }
        let mut caches = slab
            .lines()
            .skip(2)
            .filter_map(|l| {
                let v = l.split_whitespace().collect::<Vec<_>>();
                Some((
                    v.first()?.to_string(),
                    v.get(2)?
                        .parse::<u64>()
                        .ok()?
                        .saturating_mul(v.get(3)?.parse().ok()?),
                ))
            })
            .collect::<Vec<_>>();
        caches.sort_by_key(|x| std::cmp::Reverse(x.1));
        if !caches.is_empty() {
            t.details.insert(
                "slab".into(),
                caches
                    .iter()
                    .take(12)
                    .map(|(k, v)| (k.clone(), format!("{:.1} MiB", *v as f64 / 1048576.)))
                    .collect(),
            );
        }
        if let Some((_, bytes)) = caches.iter().find(|(k, _)| k == "dentry") {
            if let Some(prev) = self.previous.get("slab:dentry") {
                let growth = (*bytes as f64 - prev[0] as f64) / 1048576. / dt * 3600.;
                t.metrics.insert(
                    "slab.growth".into(),
                    Measurement::known(
                        growth,
                        "MiB/h",
                        "dentry allocated bytes delta, extrapolated hourly",
                        t.at_ms,
                    ),
                );
                t.record("slab.growth", Some(growth), "MiB/h", growth.abs().max(1.));
            }
            self.previous.insert("slab:dentry".into(), vec![*bytes]);
        }
        let vm = read("/proc/vmstat");
        for (key, prefixes) in [
            ("alloc", vec!["pgalloc_"]),
            ("reclaim", vec!["pgsteal_kswapd ", "pgsteal_direct "]),
        ] {
            let count = vm
                .lines()
                .filter(|l| prefixes.iter().any(|p| l.starts_with(p)))
                .filter_map(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
                .sum::<u64>();
            if let Some(prev) = self.previous.get(key) {
                let rate = count.saturating_sub(prev[0]) as f64 / dt;
                t.record(key, Some(rate), "pages/s", rate.max(1.));
            }
            self.previous.insert(key.into(), vec![count]);
        }
        let mut numa = Vec::new();
        if let Ok(nodes) = fs::read_dir("/sys/devices/system/node") {
            for n in nodes.flatten() {
                let name = n.file_name().to_string_lossy().into_owned();
                if name
                    .strip_prefix("node")
                    .and_then(|s| s.parse::<u32>().ok())
                    .is_some()
                {
                    numa.push((
                        name,
                        format!("CPUs {}", read(&format!("{}/cpulist", n.path().display()))),
                    ));
                }
            }
        }
        let mut huge = memory
            .iter()
            .filter(|(k, _)| {
                k.starts_with("Huge") || k.starts_with("AnonHuge") || k.starts_with("ShmemHuge")
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        huge.push((
            "KSM shared pages".into(),
            read("/sys/kernel/mm/ksm/pages_shared"),
        ));
        huge.push((
            "THP enabled".into(),
            read("/sys/kernel/mm/transparent_hugepage/enabled"),
        ));
        t.details.insert("hugepages".into(), huge);
        let zones = read("/proc/zoneinfo");
        let mut zone_name = String::new();
        for line in zones.lines() {
            let words = line.split_whitespace().collect::<Vec<_>>();
            if line.starts_with("Node ") {
                zone_name = line.trim().to_string();
            } else if words.len() == 2
                && ["min", "low", "high", "managed", "present"].contains(&words[0])
                && numa.len() < 40
            {
                numa.push((
                    format!("{} {}", zone_name, words[0]),
                    format!("{} pages", words[1]),
                ));
            }
        }
        t.details.insert("numa".into(), numa);
        for line in read("/proc/stat").lines() {
            let mut fields = line.split_whitespace();
            let key = match fields.next() {
                Some("intr") => "irq.rate",
                Some("ctxt") => "sched.switches",
                _ => continue,
            };
            if let Some(n) = fields.next().and_then(|v| v.parse::<u64>().ok()) {
                let rate = self
                    .previous
                    .insert(key.into(), vec![n])
                    .filter(|p| n >= p[0])
                    .map(|p| (n - p[0]) as f64 / dt);
                if let Some(rate) = rate {
                    t.metrics.insert(
                        key.into(),
                        Measurement::known(rate, "/s", "/proc/stat counter delta", t.at_ms),
                    );
                }
                t.record(key, rate, "/s", rate.unwrap_or(1.).max(1.));
            }
        }
        let soft = read("/proc/softirqs");
        for (cpu, name, n) in softirq_counts(&soft) {
            let key = format!("softirq.count.cpu{cpu}.{name}");
            if let Some(previous) = self
                .previous
                .insert(key.clone(), vec![n])
                .filter(|p| n >= p[0])
            {
                let rate = (n - previous[0]) as f64 / dt;
                t.metrics.insert(
                    key,
                    Measurement::known(
                        rate,
                        "events/s",
                        "/proc/softirqs counter delta; not execution time",
                        t.at_ms,
                    ),
                );
            }
        }
        t.details.insert(
            "softirqs".into(),
            soft.lines()
                .skip(1)
                .filter_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    Some((
                        k.trim().into(),
                        format!(
                            "{} cumulative events (not execution %)",
                            v.split_whitespace()
                                .filter_map(|v| v.parse::<u64>().ok())
                                .sum::<u64>()
                        ),
                    ))
                })
                .collect(),
        );
        for (name, why) in [
            ("scheduler", "Start : probe sched"),
            ("irq", "Start : probe irq"),
            ("block", "Start : probe block"),
            ("syscalls", "Start : probe syscalls"),
        ] {
            t.capabilities.insert(name.into(), Quality::Stopped);
            t.details
                .insert(format!("source:{name}"), vec![("start".into(), why.into())]);
        }
        t.capabilities.insert(
            "bpf".into(),
            Quality::Unsupported("BPF inventory requires kernel permission".into()),
        );
        for c in &t.cpus {
            if c.busy > 90. {
                t.issues.push(Issue{id:format!("cpu:{}",c.id),title:"CPU utilization".into(),subject:format!("CPU{} · {:.1}%",c.id,c.busy),severity:"warn".into(),since_ms:t.at_ms,state:"observation".into(),evidence:vec![Evidence{label:format!("CPU busy {:.1}%",c.busy),source:"/proc/stat delta".into(),at_ms:t.at_ms,subject:format!("cpu:{}",c.id),weight:1.,observed:true}],causes:vec![Cause{label:"Cause unknown".into(),detail:"Inspect runnable tasks and trace latency".into(),observed:false}],steps:vec!["Inspect tasks on this CPU".into(),"Capture scheduler latency".into()],verification:"Compare equivalent windows; high utilization alone does not imply harmful contention.".into()});
            }
        }
        self.topology.sample(&mut t);
        let await_max = t
            .devices
            .iter()
            .filter(|d| !d.partition)
            .filter_map(|d| d.await_ms)
            .reduce(f64::max);
        t.record(
            "disk.await_max",
            await_max,
            "ms",
            await_max.unwrap_or(1.).max(1.),
        );
        let untrusted = t
            .modules
            .iter()
            .filter(|m| m.taint.contains('O') || m.taint.contains('E'))
            .count() as f64;
        t.metrics.insert(
            "module.untrusted".into(),
            Measurement::known(
                untrusted,
                "modules",
                "/sys/module taint flags; trust only, not a performance cause",
                t.at_ms,
            ),
        );
        if t.capabilities.get("modules/pss") != Some(&Quality::Available) {
            if let Some(m) = t.metrics.get_mut("module.untrusted") {
                m.value = None;
                m.quality = t
                    .capabilities
                    .get("modules/pss")
                    .cloned()
                    .unwrap_or(Quality::Warming);
            }
        }
        if t.capabilities.get("modules/pss") == Some(&Quality::Available)
            && !t.modules.is_empty()
            && t.modules
                .iter()
                .all(|m| m.fields.iter().any(|(k, _)| k == "baseline"))
        {
            let changed = t
                .modules
                .iter()
                .filter(|m| m.fields.iter().any(|(k, v)| k == "baseline" && v == "new"))
                .count();
            t.metrics.insert(
                "module.changed".into(),
                Measurement::known(
                    changed as f64,
                    "modules",
                    "accepted module-name baseline versus loaded modules",
                    t.at_ms,
                ),
            );
        }
        let peak = t.cpus.iter().map(|c| c.softirq).reduce(f64::max);
        if let Some(peak) = peak {
            t.metrics.insert(
                "softirq.peak".into(),
                Measurement::known(peak, "%", "/proc/stat softirq CPU delta", t.at_ms),
            );
            t.record("softirq.peak", Some(peak), "%", 100.);
        }

        s.views[4].columns = crate::model::DEVICE_COLUMNS
            .iter()
            .map(|s| s.to_string())
            .collect();
        s.views[7].columns = crate::model::CGROUP_COLUMNS
            .iter()
            .map(|v| v.to_string())
            .collect();
        for measurement in t.metrics.values_mut() {
            if measurement.source == "PSI some avg10" {
                measurement.start_ms = t.at_ms.saturating_sub(10000);
            } else if measurement.source.contains("delta") {
                measurement.start_ms = t.at_ms.saturating_sub((dt * 1000.) as u64);
            }
        }
        let running = t.tasks.iter().filter(|x| x.state == "R").count() as f64;
        let blocked = t.tasks.iter().filter(|x| x.state == "D").count() as f64;
        t.metrics.insert(
            "runqueue".into(),
            Measurement::known(
                running / t.cpus.len().max(1) as f64,
                "/cpu",
                "sampled runnable tasks / CPUs",
                t.at_ms,
            ),
        );
        t.record(
            "runqueue",
            Some(running / t.cpus.len().max(1) as f64),
            "/cpu",
            4.,
        );
        t.record("dstate", Some(blocked), "tasks", blocked.max(1.));
        let runtime = t
            .tasks
            .iter()
            .map(|x| (x.pid, x.cpu_pct))
            .collect::<Vec<_>>();
        for (pid, cpu) in runtime {
            t.record(&format!("task.runtime{pid}"), cpu, "% / one CPU", 100.);
        }

        let prior_event = self.logs.events.last().map(|e| e.at_ms).unwrap_or(0);
        self.logs.poll();
        let rate = self
            .logs
            .events
            .iter()
            .filter(|e| e.at_ms > prior_event)
            .count() as f64
            / dt;
        t.record("events.rate", Some(rate), "events/s", rate.max(1.));
        for event in &self.logs.events {
            if !t.events.iter().any(|e| e.id == event.id) {
                t.events.push(event.clone());
            }
        }
        t.capabilities
            .insert("logs".into(), self.logs.quality.clone());
        s.views[10] = crate::model::View::new(
            &["time (boot ms)", "severity", "source", "message"],
            t.events
                .iter()
                .map(|e| {
                    vec![
                        e.at_ms.to_string(),
                        e.severity.clone(),
                        e.source.clone(),
                        e.message.clone(),
                    ]
                })
                .collect(),
            &["Source: /dev/kmsg; nonblocking bounded stream. Last 300 records retained."],
        );
        if s.views[10].rows.is_empty() {
            s.views[10]
                .notes
                .push(format!("Kernel log: {:?}", self.logs.quality));
        }
        s.views[9] = self.inventory.sample(&mut t);
        s.views[1] = crate::model::View::new(
            &["Task / PID", "State", "CPU", "CPU %", "RSS", "Wake p99"],
            t.tasks
                .iter()
                .map(|x| {
                    vec![
                        format!("{} {}", x.name, x.pid),
                        x.state.clone(),
                        x.cpu.to_string(),
                        x.cpu_pct.map(|v| format!("{v:.1}")).unwrap_or("—".into()),
                        format!("{:.0} MiB", x.rss_bytes as f64 / 1048576.),
                        "—".into(),
                    ]
                })
                .collect(),
            &["Runtime is normalized to one CPU; identity includes task start ticks."],
        );
        s.views[4].rows = t.devices.iter().map(crate::model::device_row).collect();
        s.views[7].rows = t.cgroups.iter().map(crate::model::cgroup_row).collect();
        s.views[8] = crate::model::View::new(
            &crate::model::MODULE_COLUMNS,
            t.modules.iter().map(crate::model::module_row).collect(),
            &["Loaded-module metadata; trust flags alone do not establish a performance cause."],
        );
        self.history = t.series.clone();
        s.telemetry = t;
    }
}

pub fn monotonic_ms() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1000 + ts.tv_nsec as u64 / 1_000_000
}

fn softirq_counts(raw: &str) -> Vec<(u32, String, u64)> {
    let mut lines = raw.lines();
    let cpus = lines
        .next()
        .unwrap_or("")
        .split_whitespace()
        .map(|s| s.strip_prefix("CPU").and_then(|v| v.parse::<u32>().ok()))
        .collect::<Vec<_>>();
    lines
        .filter_map(|line| line.split_once(':'))
        .flat_map(|(name, values)| {
            cpus.iter()
                .zip(values.split_whitespace())
                .filter_map(move |(cpu, value)| {
                    Some(((*cpu)?, name.trim().to_string(), value.parse().ok()?))
                })
        })
        .collect()
}
#[cfg(test)]
mod softirq_tests {
    #[test]
    fn retains_sparse_cpu_ids_and_rejects_invalid_values() {
        assert_eq!(
            super::softirq_counts("CPU0 CPU7\nNET_RX: 10 20\nTIMER: bad 30\n"),
            vec![
                (0, "NET_RX".into(), 10),
                (7, "NET_RX".into(), 20),
                (7, "TIMER".into(), 30)
            ]
        );
    }
}
