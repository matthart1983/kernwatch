//! Bounded procfs/sysfs topology enrichment. Rates use object identity and elapsed time.
use crate::domain::*;
use std::{collections::BTreeMap, fs, os::unix::fs::MetadataExt, path::Path};
#[derive(Default)]
pub struct Inventory {
    extra: crate::enrichment::Extra,
    journal: crate::logs::Journal,
    irq: crate::irq::Collector,
    module_quality: Quality,
    module_events: Vec<Event>,
    previous: BTreeMap<String, (u64, Vec<u64>)>,
    slow_at: u64,
    modules: Vec<Module>,
    pss: BTreeMap<(u32, u64), u64>,
}
fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path)
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|e| format!("unavailable: {e}"))
}
fn counters(text: &str) -> BTreeMap<&str, u64> {
    text.lines()
        .filter_map(|l| {
            let mut v = l.split_whitespace();
            Some((v.next()?, v.next()?.parse().ok()?))
        })
        .collect()
}
impl Inventory {
    pub fn journal(&mut self, t: &mut Telemetry) {
        self.journal.sample(t);
    }
    pub fn module_metadata(&mut self, t: &mut Telemetry) {
        self.extra.modules(t);
    }
    pub fn systemd(&mut self, t: &mut Telemetry) {
        self.extra.systemd(t);
    }
    pub fn storage_health(&mut self, t: &mut Telemetry) {
        self.extra.storage(t);
    }
    pub fn task_metadata(&mut self, t: &mut Telemetry) {
        self.extra.tasks(t);
    }
    pub fn irqs(&mut self, t: &mut Telemetry) {
        self.irq.sample(t, Path::new("/proc"));
    }
    fn rates(&mut self, key: String, at: u64, values: Vec<u64>) -> Option<Vec<f64>> {
        let prev = self.previous.insert(key, (at, values.clone()))?;
        if at <= prev.0
            || prev.1.len() != values.len()
            || values.iter().zip(&prev.1).any(|(n, p)| n < p)
        {
            return None;
        }
        let dt = (at - prev.0) as f64 / 1000.;
        Some(
            values
                .iter()
                .zip(prev.1)
                .map(|(n, p)| (n - p) as f64 / dt)
                .collect(),
        )
    }
    pub fn disks(&mut self, t: &mut Telemetry) {
        let at = t.at_ms;
        let raw = match fs::read_to_string("/proc/diskstats") {
            Ok(v) => v,
            Err(e) => {
                t.capabilities.insert(
                    "devices".into(),
                    Quality::Error(format!("/proc/diskstats: {e}")),
                );
                return;
            }
        };
        for line in raw.lines() {
            let v = line.split_whitespace().collect::<Vec<_>>();
            if v.len() < 14 || v[2].starts_with("loop") || v[2].starts_with("ram") {
                continue;
            }
            let n = v[3..]
                .iter()
                .map(|s| s.parse::<u64>().unwrap_or(0))
                .collect::<Vec<_>>();
            let name = v[2];
            let major_minor = format!("{}:{}", v[0], v[1]);
            let root = format!("/sys/class/block/{name}");
            let identity = fs::metadata(&root).map(|m| m.ino()).unwrap_or(0);
            let rate = self.rates(
                format!("disk:{major_minor}:{name}:{identity}"),
                at,
                n.clone(),
            );
            let rv = |i| rate.as_ref().map(|v| v[i]);
            let mut d = Device {
                name: name.into(),
                major_minor,
                partition: Path::new(&format!("{root}/partition")).exists(),
                read_mib_s: rv(2).map(|v| v / 2048.),
                write_mib_s: rv(6).map(|v| v / 2048.),
                iops: rate.as_ref().map(|v| v[0] + v[4]),
                inflight: n[8],
                busy_pct: rv(9).map(|v| v / 10.),
                ..Default::default()
            };
            d.fields
                .push(("sysfs identity".into(), identity.to_string()));
            d.await_ms = rate.as_ref().and_then(|v| {
                if v[0] + v[4] > 0. {
                    Some((v[3] + v[7]) / (v[0] + v[4]))
                } else {
                    None
                }
            });
            d.fields = vec![
                ("device".into(), format!("/dev/{name} · {}", d.major_minor)),
                (
                    "read/write".into(),
                    format!("{} / {} MiB/s", fmt(d.read_mib_s), fmt(d.write_mib_s)),
                ),
                ("in flight".into(), d.inflight.to_string()),
                (
                    "mean completed await".into(),
                    format!("{} ms", fmt(d.await_ms)),
                ),
            ];
            for (label, file) in [
                ("scheduler", "queue/scheduler"),
                ("rotational", "queue/rotational"),
                ("logical block bytes", "queue/logical_block_size"),
                ("queue requests", "queue/nr_requests"),
                ("model", "device/model"),
            ] {
                d.fields
                    .push((label.into(), read(format!("{root}/{file}"))));
            }
            if let Ok(queues) = fs::read_dir(format!("{root}/mq")) {
                for queue in queues.flatten().take(256) {
                    d.fields.push((
                        format!("queue {} CPUs", queue.file_name().to_string_lossy()),
                        read(queue.path().join("cpu_list")),
                    ));
                }
            }
            if let Ok(real) = fs::canonicalize(&root) {
                for ancestor in real.ancestors().take(8) {
                    if let Ok(entries) = fs::read_dir(ancestor.join("msi_irqs")) {
                        let mut ids = entries
                            .flatten()
                            .filter_map(|e| e.file_name().to_string_lossy().parse::<u32>().ok())
                            .collect::<Vec<_>>();
                        ids.sort_unstable();
                        d.fields.push((
                            "controller IRQ candidates".into(),
                            ids.iter().map(u32::to_string).collect::<Vec<_>>().join(","),
                        ));
                        d.fields.push(("IRQ mapping scope".into(),"Controller MSI IRQs; exact queue-to-vector assignment needs driver evidence".into()));
                        break;
                    }
                }
            }
            let mounts = read("/proc/self/mountinfo")
                .lines()
                .filter_map(|l| {
                    let v = l.split_whitespace().collect::<Vec<_>>();
                    (v.get(2) == Some(&d.major_minor.as_str()))
                        .then(|| v.get(4).unwrap_or(&"?").to_string())
                })
                .collect::<Vec<_>>()
                .join(", ");
            d.fields.push((
                "mounts (this namespace)".into(),
                if mounts.is_empty() {
                    "none observed".into()
                } else {
                    mounts
                },
            ));
            for (suffix, value, unit) in [
                ("read", d.read_mib_s, "MiB/s"),
                ("write", d.write_mib_s, "MiB/s"),
                ("iops", d.iops, "/s"),
                ("queue", Some(d.inflight as f64), "requests"),
            ] {
                let key = format!("device:{name}:{suffix}");
                t.record(&key, value, unit, value.unwrap_or(1.).max(1.));
            }
            t.details.insert(format!("device:{name}"), d.fields.clone());
            t.devices.push(d);
        }
        self.previous
            .retain(|_, (then, _)| at.saturating_sub(*then) < 60000);
        t.series.retain(|_, series| {
            series
                .samples
                .last()
                .is_some_and(|s| at.saturating_sub(s.at_ms) < 600000)
        });
    }
    pub fn groups(&mut self, t: &mut Telemetry) {
        let at = t.at_ms;
        if let Err(e) = fs::read_to_string("/sys/fs/cgroup/cgroup.controllers") {
            t.capabilities
                .insert("cgroups".into(), Quality::Error(format!("cgroup v2: {e}")));
            return;
        }
        let mut pending = vec![std::path::PathBuf::from("/sys/fs/cgroup")];
        let mut i = 0;
        while i < pending.len() && i < 512 {
            let root = pending[i].clone();
            i += 1;
            if let Ok(es) = fs::read_dir(&root) {
                for e in es.flatten() {
                    if pending.len() < 512 && e.file_type().is_ok_and(|t| t.is_dir()) {
                        pending.push(e.path());
                    }
                }
            }
            let Ok(meta) = fs::metadata(&root) else {
                continue;
            };
            let path = format!(
                "/{}",
                root.strip_prefix("/sys/fs/cgroup").unwrap().display()
            );
            let stat = read(root.join("cpu.stat"));
            let counts = counters(&stat);
            let usage = counts.get("usage_usec").copied();
            let throttle = counts.get("throttled_usec").copied();
            let rate = usage
                .zip(throttle)
                .and_then(|(u, h)| self.rates(format!("cgroup:{}", meta.ino()), at, vec![u, h]));
            let mut group = Cgroup {
                path: path.clone(),
                inode: meta.ino(),
                runtime_pct: rate.as_ref().map(|v| v[0] / 10000.),
                throttled_ms_s: rate.as_ref().map(|v| v[1] / 1000.),
                memory_bytes: read(root.join("memory.current")).parse().unwrap_or(0),
                quota: read(root.join("cpu.max")),
                cpus: read(root.join("cpuset.cpus.effective")),
                ..Default::default()
            };
            for file in [
                "cpu.max",
                "cpu.weight",
                "cpuset.cpus.effective",
                "memory.current",
                "memory.high",
                "memory.max",
                "pids.current",
                "pids.max",
                "cpu.pressure",
                "memory.pressure",
                "io.pressure",
            ] {
                group
                    .fields
                    .push((file.into(), read(root.join(file)).replace('\n', " · ")));
            }
            group.fields.push((
                "runtime (one CPU)".into(),
                format!("{}%", fmt(group.runtime_pct)),
            ));
            group.fields.push((
                "throttled time".into(),
                format!("{} ms/s", fmt(group.throttled_ms_s)),
            ));
            for kind in ["cpu", "io"] {
                let value = group
                    .fields
                    .iter()
                    .find(|(k, _)| k == &format!("{kind}.pressure"))
                    .and_then(|(_, v)| {
                        v.split_whitespace().find_map(|v| {
                            v.strip_prefix("avg10=").and_then(|v| v.parse::<f64>().ok())
                        })
                    });
                t.record(
                    &format!("cgroup:{path}:psi.{kind}"),
                    value,
                    "% some avg10",
                    100.,
                );
            }
            t.record(
                &format!("cgroup:{path}:cpu"),
                group.runtime_pct,
                "%",
                100f64.max(group.runtime_pct.unwrap_or(0.) * 1.2),
            );
            t.record(
                &format!("cgroup:{path}:throttled"),
                group.throttled_ms_s,
                "ms/s",
                1000.,
            );
            t.details
                .insert(format!("cgroup:{path}"), group.fields.clone());
            t.cgroups.push(group);
        }
        t.cgroups.sort_by(|a, b| a.path.cmp(&b.path));
        self.previous
            .retain(|_, (then, _)| at.saturating_sub(*then) < 60000);
        t.series.retain(|_, series| {
            series
                .samples
                .last()
                .is_some_and(|s| at.saturating_sub(s.at_ms) < 600000)
        });
    }
    pub fn modules(&mut self, t: &mut Telemetry) {
        let at = t.at_ms;
        if self.slow_at == 0 || at.saturating_sub(self.slow_at) >= 5000 {
            let observed_before = self.slow_at;
            self.slow_at = at;
            let previous_names = self
                .modules
                .iter()
                .map(|m| m.name.clone())
                .collect::<std::collections::BTreeSet<_>>();

            let raw = match fs::read_to_string("/proc/modules") {
                Ok(v) => v,
                Err(e) => {
                    t.capabilities.insert(
                        "modules/pss".into(),
                        Quality::Error(format!("/proc/modules: {e}")),
                    );
                    self.module_quality = Quality::Error(format!("/proc/modules: {e}"));
                    return;
                }
            };
            self.modules.clear();
            self.module_quality = Quality::Available;
            for line in raw.lines() {
                let v = line.split_whitespace().collect::<Vec<_>>();
                if v.len() < 6 {
                    continue;
                }
                let root = format!("/sys/module/{}", v[0]);
                let taint = read(format!("{root}/taint"));
                let mut m = Module {
                    name: v[0].into(),
                    bytes: v[1].parse().unwrap_or(0),
                    refs: v[2].parse().unwrap_or(0),
                    state: v[4].into(),
                    taint: taint.clone(),
                    fields: vec![
                        ("name".into(), v[0].into()),
                        (
                            "sysfs identity".into(),
                            fs::metadata(&root)
                                .map(|m| m.ino().to_string())
                                .unwrap_or_default(),
                        ),
                        (
                            "taint flags".into(),
                            if taint.is_empty() {
                                "none".into()
                            } else {
                                taint
                            },
                        ),
                        ("dependent modules".into(), v[3].into()),
                        ("version".into(), read(format!("{root}/version"))),
                        ("source version".into(), read(format!("{root}/srcversion"))),
                        (
                            "signer / load time".into(),
                            "not exposed by loaded-module sysfs metadata".into(),
                        ),
                    ],
                };
                m.fields.extend([
                    (
                        "loader".into(),
                        "not captured; polling does not identify the loading process".into(),
                    ),
                    (
                        "hooks".into(),
                        "not exposed by loaded-module sysfs metadata".into(),
                    ),
                    (
                        "loaded".into(),
                        self.module_events
                            .iter()
                            .rev()
                            .find(|e| e.subject == format!("module:{}", m.name))
                            .map(|e| format!("observed {}ms", e.at_ms))
                            .unwrap_or("present at first sample; exact load time unknown".into()),
                    ),
                ]);
                if let Ok(parameters) = fs::read_dir(format!("{root}/parameters")) {
                    for entry in parameters.flatten().take(16) {
                        if fs::metadata(entry.path())
                            .map(|m| m.len() <= 4096)
                            .unwrap_or(false)
                        {
                            m.fields.push((
                                format!("parameter {}", entry.file_name().to_string_lossy()),
                                read(entry.path()).chars().take(512).collect(),
                            ));
                        }
                    }
                }
                self.modules.push(m);
            }
            if !previous_names.is_empty() {
                let names = self
                    .modules
                    .iter()
                    .map(|m| m.name.clone())
                    .collect::<std::collections::BTreeSet<_>>();
                for (name, change) in names
                    .difference(&previous_names)
                    .map(|n| (n, "appeared"))
                    .chain(
                        previous_names
                            .difference(&names)
                            .map(|n| (n, "disappeared")),
                    )
                {
                    self.module_events.push(Event {id:format!("module-{name}-{at}"),at_ms:at,source:"module poll".into(),severity:"info".into(),subject:format!("module:{name}"),message:format!("{name} {change} between {observed_before}..{at}ms; exact load/unload time and actor unknown")});
                }
                if self.module_events.len() > 64 {
                    self.module_events.drain(..self.module_events.len() - 64);
                }
            }
            self.pss.clear();
            for task in t
                .tasks
                .iter()
                .filter(|x| x.rss_bytes > 0 && x.pid == x.tgid)
                .take(32)
            {
                if crate::actions::validate_task(task.pid as i32, task.start_ticks).is_err() {
                    continue;
                }
                let path = format!("/proc/{}/smaps_rollup", task.pid);
                if let Ok(text) = fs::read_to_string(path) {
                    if let Some(pss) = text.lines().find_map(|l| {
                        l.strip_prefix("Pss:")?
                            .split_whitespace()
                            .next()?
                            .parse::<u64>()
                            .ok()
                    }) {
                        if crate::actions::validate_task(task.pid as i32, task.start_ticks).is_ok()
                        {
                            self.pss
                                .insert((task.pid, task.start_ticks), pss.saturating_mul(1024));
                        }
                    }
                }
            }
        }
        for task in &mut t.tasks {
            task.pss_bytes = self.pss.get(&(task.pid, task.start_ticks)).copied();
            task.pss_at_ms = task.pss_bytes.map(|_| self.slow_at);
        }
        t.capabilities
            .insert("modules/pss".into(), self.module_quality.clone());
        t.modules = self.modules.clone();
        t.events.extend(self.module_events.clone());
        if let Ok(bytes) = fs::read("kernwatch-baseline.json") {
            if bytes.len() < 4 * 1024 * 1024 {
                if let Ok(meta) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    t.details.insert(
                        "module baseline".into(),
                        vec![
                            (
                                "accepted unix ms".into(),
                                meta["accepted_at_unix_ms"].to_string(),
                            ),
                            (
                                "intent".into(),
                                "Accepted inventory; historical taint is retained".into(),
                            ),
                        ],
                    );
                }
                if let Ok(baseline) = serde_json::from_slice::<crate::model::View>(&bytes) {
                    for module in &mut t.modules {
                        let known = baseline
                            .rows
                            .iter()
                            .any(|row| row.first() == Some(&module.name));
                        module.fields.push((
                            "baseline".into(),
                            if known { "accepted" } else { "new" }.into(),
                        ));
                    }
                }
            }
        }

        for m in &t.modules {
            t.details
                .insert(format!("module:{}", m.name), m.fields.clone());
        }
        t.details.insert(
            "taint".into(),
            vec![
                (
                    "kernel taint bitmask".into(),
                    read("/proc/sys/kernel/tainted"),
                ),
                (
                    "interpretation".into(),
                    "Historical kernel state; a trust flag does not prove a performance cause"
                        .into(),
                ),
            ],
        );
    }
}
fn fmt(value: Option<f64>) -> String {
    value
        .map(|v| format!("{v:.2}"))
        .unwrap_or_else(|| "—".into())
}
