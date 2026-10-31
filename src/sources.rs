//! Independent bounded topology workers. A slow smaps or device read cannot stop CPU/task updates.
use crate::{domain::*, inventory::Inventory};
use std::collections::BTreeMap;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
/// Only identities cross from the UI to slow collectors. No host records or
/// histories are cloned to express interest. Replay/frozen views clear demand.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Demand {
    pub tasks: std::collections::BTreeSet<(u32, u64)>,
    pub module: Option<String>,
    pub cgroup: Option<String>,
    pub device: Option<String>,
}
fn demand_slot() -> &'static std::sync::Mutex<Demand> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Demand>> = std::sync::OnceLock::new();
    SLOT.get_or_init(Default::default)
}
pub fn set_demand(demand: Demand) {
    *demand_slot().lock().unwrap() = demand;
}
pub fn demand() -> Demand {
    demand_slot().lock().unwrap().clone()
}
struct Source {
    name: &'static str,
    send: SyncSender<Telemetry>,
    receive: Receiver<Telemetry>,
    latest: Option<Telemetry>,
    /// A request is outstanding. Without this the caller rebuilds the payload
    /// every tick and throws it away when the worker is still busy, which for
    /// the task-carrying sources means cloning thousands of tasks per second
    /// for nothing.
    pending: bool,
    next_due: u64,
    demand: Demand,
}
impl Source {
    fn new(name: &'static str, collect: fn(&mut Inventory, &mut Telemetry)) -> Self {
        let (send, requests) = sync_channel::<Telemetry>(1);
        let (results, receive) = sync_channel(1);
        let _ = std::thread::Builder::new()
            .name(format!("kw-{name}"))
            .spawn(move || {
                let mut inventory = Inventory::default();
                let mut history = std::collections::BTreeMap::new();
                while let Ok(mut t) = requests.recv() {
                    let start = std::time::Instant::now();
                    t.at_ms = crate::enrich::monotonic_ms();
                    t.series = std::mem::take(&mut history);
                    {
                        let _cost = crate::cpu_cost::scope(name);
                        collect(&mut inventory, &mut t);
                    }
                    t.details.insert(
                        format!("source:{name}"),
                        vec![
                            ("sample boot ms".into(), t.at_ms.to_string()),
                            (
                                "collection ms".into(),
                                format!("{:.2}", start.elapsed().as_secs_f64() * 1000.),
                            ),
                        ],
                    );
                    history = t.series.clone();
                    let _ = results.try_send(t);
                }
            });
        Self {
            name,
            send,
            receive,
            latest: None,
            pending: false,
            next_due: 0,
            demand: Demand::default(),
        }
    }
    fn update(&mut self, t: &mut Telemetry) {
        while let Ok(result) = self.receive.try_recv() {
            self.latest = Some(result);
            self.pending = false;
        }
        let demand = demand();
        let changed = match self.name {
            "task_metadata" => demand.tasks != self.demand.tasks,
            "module_metadata" => demand.module != self.demand.module,
            "systemd" => demand.cgroup != self.demand.cgroup,
            "storage_health" => demand.device != self.demand.device,
            _ => false,
        };
        if changed {
            self.next_due = 0;
            self.demand = demand.clone();
        }
        let wanted = match self.name {
            "task_metadata" => !demand.tasks.is_empty(),
            "module_metadata" => demand.module.is_some(),
            "systemd" => demand.cgroup.is_some(),
            "storage_health" => demand.device.is_some(),
            _ => true,
        };
        if self.pending || t.at_ms < self.next_due || !wanted {
            self.publish(t);
            return;
        }
        let request = Telemetry {
            tasks: if self.name == "modules/pss" || self.name == "task_metadata" {
                t.tasks
                    .iter()
                    .filter(|t| {
                        if self.name == "task_metadata" {
                            demand.tasks.contains(&(t.pid, t.start_ticks))
                        } else {
                            t.rss_bytes > 0 && t.pid == t.tgid
                        }
                    })
                    .take(10000)
                    .map(|t| Task {
                        pid: t.pid,
                        tgid: t.tgid,
                        start_ticks: t.start_ticks,
                        rss_bytes: t.rss_bytes,
                        ..Default::default()
                    })
                    .collect()
            } else {
                Vec::new()
            },
            modules: if self.name == "module_metadata" {
                t.modules
                    .iter()
                    .filter(|m| Some(&m.name) == demand.module.as_ref())
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            },
            cgroups: if self.name == "systemd" {
                t.cgroups
                    .iter()
                    .filter(|g| Some(&g.path) == demand.cgroup.as_ref())
                    .map(|g| Cgroup {
                        path: g.path.clone(),
                        inode: g.inode,
                        ..Default::default()
                    })
                    .collect()
            } else {
                Vec::new()
            },
            devices: if self.name == "storage_health" {
                t.devices
                    .iter()
                    .filter(|d| Some(&d.name) == demand.device.as_ref())
                    .map(|d| Device {
                        name: d.name.clone(),
                        major_minor: d.major_minor.clone(),
                        partition: d.partition,
                        ..Default::default()
                    })
                    .collect()
            } else {
                Vec::new()
            },
            ..Default::default()
        };
        self.pending = self.send.try_send(request).is_ok();
        if self.pending {
            self.next_due = t.at_ms.saturating_add(self.interval());
        }
        self.publish(t);
    }

    fn interval(&self) -> u64 {
        match self.name {
            "modules/pss" | "task_metadata" | "journal" => 5000,
            "module_metadata" | "systemd" | "storage_health" => 30000,
            _ => 1000,
        }
    }
    fn stale_after(&self) -> u64 {
        (self.interval() * 2).max(10000)
    }

    /// Merge the worker's most recent result into the snapshot.
    fn publish(&mut self, t: &mut Telemetry) {
        let _cost = crate::cpu_cost::scope("source.publish");
        if let Some(source) = &self.latest {
            let age = t.at_ms.saturating_sub(source.at_ms);
            t.capabilities.insert(
                self.name.into(),
                if age < self.stale_after() {
                    source
                        .capabilities
                        .get(self.name)
                        .cloned()
                        .unwrap_or(Quality::Available)
                } else {
                    Quality::Stale
                },
            );
            if [
                "module_metadata",
                "systemd",
                "storage_health",
                "task_metadata",
            ]
            .contains(&self.name)
            {
                let old_tasks: BTreeMap<_, _> = source
                    .tasks
                    .iter()
                    .map(|p| (p.pid, p.start_ticks))
                    .collect();
                let tasks: BTreeMap<_, _> =
                    t.tasks.iter().map(|p| (p.pid, p.start_ticks)).collect();
                let old_modules: BTreeMap<_, _> = source
                    .modules
                    .iter()
                    .map(|m| {
                        (
                            m.name.as_str(),
                            crate::enrichment::named(&m.fields, "sysfs identity"),
                        )
                    })
                    .collect();
                let modules: BTreeMap<_, _> = t
                    .modules
                    .iter()
                    .map(|m| {
                        (
                            m.name.as_str(),
                            crate::enrichment::named(&m.fields, "sysfs identity"),
                        )
                    })
                    .collect();
                let old_groups: BTreeMap<_, _> = source
                    .cgroups
                    .iter()
                    .map(|g| (g.path.as_str(), g.inode))
                    .collect();
                let groups: BTreeMap<_, _> = t
                    .cgroups
                    .iter()
                    .map(|g| (g.path.as_str(), g.inode))
                    .collect();
                let old_devices: BTreeMap<_, _> = source
                    .devices
                    .iter()
                    .map(|d| (d.name.as_str(), d.major_minor.as_str()))
                    .collect();
                let devices: BTreeMap<_, _> = t
                    .devices
                    .iter()
                    .map(|d| (d.name.as_str(), d.major_minor.as_str()))
                    .collect();
                for (key, fields) in &source.details {
                    let valid = if let Some(pid) = key
                        .strip_prefix("task:")
                        .and_then(|v| v.parse::<u32>().ok())
                    {
                        old_tasks
                            .get(&pid)
                            .is_some_and(|old| tasks.get(&pid) == Some(old))
                    } else if let Some(name) = key.strip_prefix("module:") {
                        old_modules
                            .get(name)
                            .is_some_and(|old| modules.get(name) == Some(old))
                    } else if let Some(path) = key.strip_prefix("cgroup:") {
                        old_groups
                            .get(path)
                            .is_some_and(|old| groups.get(path) == Some(old))
                    } else if let Some(name) = key.strip_prefix("device:") {
                        old_devices
                            .get(name)
                            .is_some_and(|old| devices.get(name) == Some(old))
                    } else {
                        true
                    };
                    if !valid {
                        continue;
                    }
                    let target = t.details.entry(key.clone()).or_default();
                    target.extend(fields.clone());
                    if let Some(sampled) =
                        crate::enrichment::named(fields, "metadata sampled boot ms")
                            .and_then(|s| s.parse::<u64>().ok())
                    {
                        target.push((
                            "metadata age ms".into(),
                            t.at_ms.saturating_sub(sampled).to_string(),
                        ));
                    }
                }
            } else {
                t.details.extend(source.details.clone());
            }
            for event in &source.events {
                if !t.events.iter().any(|e| e.id == event.id) {
                    t.events.push(event.clone());
                }
            }

            t.series.extend(source.series.clone());
            match self.name {
                "devices" => t.devices = source.devices.clone(),
                "cgroups" => t.cgroups = source.cgroups.clone(),
                "modules/pss" => {
                    t.modules = source.modules.clone();
                    let tasks: BTreeMap<_, _> = source
                        .tasks
                        .iter()
                        .map(|p| ((p.pid, p.start_ticks), p))
                        .collect();
                    for task in &mut t.tasks {
                        let old = tasks.get(&(task.pid, task.start_ticks));
                        task.pss_at_ms = old.and_then(|p| p.pss_at_ms);
                        task.pss_bytes = old.and_then(|p| p.pss_bytes);
                    }
                }
                _ => {}
            }
            if age >= self.stale_after() {
                match self.name {
                    "devices" => {
                        for d in &mut t.devices {
                            d.read_mib_s = None;
                            d.write_mib_s = None;
                            d.iops = None;
                            d.await_ms = None;
                            d.busy_pct = None;
                        }
                    }
                    "cgroups" => {
                        for g in &mut t.cgroups {
                            g.runtime_pct = None;
                            g.throttled_ms_s = None;
                        }
                    }
                    "modules/pss" => {
                        for task in &mut t.tasks {
                            task.pss_bytes = None;
                        }
                    }
                    _ => {}
                }
            }
        } else {
            t.capabilities.insert(self.name.into(), Quality::Warming);
            t.details
                .entry(format!("source:{}", self.name))
                .or_insert_with(|| {
                    vec![(
                        "acquisition".into(),
                        if matches!(
                            self.name,
                            "module_metadata" | "systemd" | "storage_health" | "task_metadata"
                        ) {
                            "On demand: open the subject inspector to request metadata"
                        } else {
                            "Waiting for first observation"
                        }
                        .into(),
                    )]
                });
        }
    }
}
pub struct Topology {
    sources: Vec<Source>,
}
impl Default for Topology {
    fn default() -> Self {
        Self {
            sources: vec![
                Source::new("devices", Inventory::disks),
                Source::new("irq_counts", Inventory::irqs),
                Source::new("cgroups", Inventory::groups),
                Source::new("modules/pss", Inventory::modules),
                Source::new("module_metadata", Inventory::module_metadata),
                Source::new("systemd", Inventory::systemd),
                Source::new("storage_health", Inventory::storage_health),
                Source::new("task_metadata", Inventory::task_metadata),
                Source::new("journal", Inventory::journal),
            ],
        }
    }
}
impl Topology {
    pub fn sample(&mut self, t: &mut Telemetry) {
        let _cost = crate::cpu_cost::scope("topology.merge");
        for source in &mut self.sources {
            source.update(t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn task_metadata_rejects_recycled_pid() {
        let (send, _) = sync_channel(1);
        let (_, receive) = sync_channel(1);
        let mut old = Telemetry {
            at_ms: 1000,
            ..Default::default()
        };
        old.tasks
            .push(crate::model::demo().telemetry.tasks[0].clone());
        old.details.insert(
            format!("task:{}", old.tasks[0].pid),
            vec![("weight".into(), "42".into())],
        );
        let mut now = old.clone();
        now.details.clear();
        now.tasks[0].start_ticks += 1;
        let mut source = Source {
            name: "task_metadata",
            send,
            receive,
            latest: Some(old),
            pending: false,
            next_due: 0,
            demand: Demand::default(),
        };
        source.publish(&mut now);
        assert!(now.details.is_empty());
        now.tasks[0].start_ticks -= 1;
        source.publish(&mut now);
        assert_eq!(now.details.values().next().unwrap()[0].1, "42");
    }
}

#[cfg(test)]
mod scaling_tests {
    use super::*;
    fn source(name: &'static str, latest: Telemetry) -> Source {
        let (send, _) = sync_channel(1);
        let (_, receive) = sync_channel(1);
        Source {
            name,
            send,
            receive,
            latest: Some(latest),
            pending: false,
            next_due: 0,
            demand: Demand::default(),
        }
    }
    #[test]
    fn pss_merge_rejects_reuse_and_preserves_acquisition_time() {
        let old = Telemetry {
            at_ms: 1000,
            tasks: vec![Task {
                pid: 1,
                start_ticks: 7,
                pss_at_ms: Some(900),
                pss_bytes: Some(42),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut now = old.clone();
        now.tasks[0].start_ticks = 8;
        let mut source = source("modules/pss", old);
        source.publish(&mut now);
        assert_eq!(now.tasks[0].pss_bytes, None);
        now.tasks[0].start_ticks = 7;
        source.publish(&mut now);
        assert_eq!(
            (now.tasks[0].pss_at_ms, now.tasks[0].pss_bytes),
            (Some(900), Some(42))
        );
        now.at_ms = 11000;
        source.publish(&mut now);
        assert_eq!(now.tasks[0].pss_bytes, None);
        assert_eq!(now.capabilities["modules/pss"], Quality::Stale);
    }
    #[test]
    fn object_indexes_reject_replaced_cgroups_modules_and_devices() {
        for name in ["systemd", "module_metadata", "storage_health"] {
            let mut old = Telemetry {
                at_ms: 1000,
                ..Default::default()
            };
            old.cgroups.push(Cgroup {
                path: "/unit".into(),
                inode: 1,
                ..Default::default()
            });
            old.modules.push(Module {
                name: "module".into(),
                fields: vec![("sysfs identity".into(), "1".into())],
                ..Default::default()
            });
            old.devices.push(Device {
                name: "disk".into(),
                major_minor: "1:1".into(),
                ..Default::default()
            });
            let key = match name {
                "systemd" => "cgroup:/unit",
                "module_metadata" => "module:module",
                _ => "device:disk",
            };
            old.details
                .insert(key.into(), vec![("metadata".into(), "old".into())]);
            let mut now = old.clone();
            now.details.clear();
            now.cgroups[0].inode = 2;
            now.modules[0].fields[0].1 = "2".into();
            now.devices[0].major_minor = "2:2".into();
            source(name, old).publish(&mut now);
            assert!(!now.details.contains_key(key));
        }
    }
    #[test]
    #[ignore = "explicit release-mode scaling benchmark; no host acquisition"]
    fn metadata_join_scaling() {
        for count in [100, 1000, 3000, 10000] {
            let old = Telemetry {
                at_ms: 1000,
                tasks: (0..count)
                    .map(|pid| Task {
                        pid,
                        start_ticks: pid as u64 + 10,
                        pss_bytes: Some(42),
                        pss_at_ms: Some(900),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            };
            let mut source = source("modules/pss", old.clone());
            let mut times = Vec::new();
            for _ in 0..11 {
                let mut now = old.clone();
                for task in now.tasks.iter_mut().step_by(10) {
                    task.start_ticks += 1;
                }
                let start = std::time::Instant::now();
                source.publish(&mut now);
                times.push(start.elapsed().as_secs_f64() * 1000.);
                assert_eq!(
                    now.tasks.iter().filter(|t| t.pss_bytes == Some(42)).count(),
                    count as usize * 9 / 10
                );
            }
            times.sort_by(f64::total_cmp);
            println!(
                "METADATA_JOIN tasks={count} median_ms={:.3} p95_ms={:.3}",
                times[5], times[10]
            );
        }
    }
}
