//! Independent bounded topology workers. A slow smaps or device read cannot stop CPU/task updates.
use crate::{domain::*, inventory::Inventory};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
struct Source {
    name: &'static str,
    send: SyncSender<Telemetry>,
    receive: Receiver<Telemetry>,
    latest: Option<Telemetry>,
}
impl Source {
    fn new(name: &'static str, collect: fn(&mut Inventory, &mut Telemetry)) -> Self {
        let (send, requests) = sync_channel::<Telemetry>(1);
        let (results, receive) = sync_channel(1);
        std::thread::spawn(move || {
            let mut inventory = Inventory::default();
            let mut history = std::collections::BTreeMap::new();
            while let Ok(mut t) = requests.recv() {
                let start = std::time::Instant::now();
                t.at_ms = crate::enrich::monotonic_ms();
                t.series = std::mem::take(&mut history);
                collect(&mut inventory, &mut t);
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
        }
    }
    fn update(&mut self, t: &mut Telemetry) {
        while let Ok(result) = self.receive.try_recv() {
            self.latest = Some(result);
        }
        let request = Telemetry {
            tasks: if self.name == "modules/pss" || self.name == "task_metadata" {
                t.tasks
                    .iter()
                    .filter(|t| {
                        self.name == "task_metadata" || (t.rss_bytes > 0 && t.pid == t.tgid)
                    })
                    .take(if self.name == "task_metadata" {
                        10000
                    } else {
                        32
                    })
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            },
            modules: if self.name == "module_metadata" {
                t.modules.clone()
            } else {
                Vec::new()
            },
            cgroups: if self.name == "systemd" {
                t.cgroups.clone()
            } else {
                Vec::new()
            },
            devices: if self.name == "storage_health" {
                t.devices.clone()
            } else {
                Vec::new()
            },
            ..Default::default()
        };
        let _ = self.send.try_send(request);
        if let Some(source) = &self.latest {
            let age = t.at_ms.saturating_sub(source.at_ms);
            t.capabilities.insert(
                self.name.into(),
                if age < 10000 {
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
                for (key, fields) in &source.details {
                    let valid = if let Some(pid) = key
                        .strip_prefix("task:")
                        .and_then(|v| v.parse::<u32>().ok())
                    {
                        source
                            .tasks
                            .iter()
                            .find(|p| p.pid == pid)
                            .is_some_and(|old| {
                                t.tasks
                                    .iter()
                                    .any(|p| p.pid == pid && p.start_ticks == old.start_ticks)
                            })
                    } else if let Some(name) = key.strip_prefix("module:") {
                        source
                            .modules
                            .iter()
                            .find(|m| m.name == name)
                            .is_some_and(|old| {
                                t.modules.iter().any(|m| {
                                    m.name == name
                                        && crate::enrichment::named(&m.fields, "sysfs identity")
                                            == crate::enrichment::named(
                                                &old.fields,
                                                "sysfs identity",
                                            )
                                })
                            })
                    } else if let Some(path) = key.strip_prefix("cgroup:") {
                        source
                            .cgroups
                            .iter()
                            .find(|g| g.path == path)
                            .is_some_and(|old| {
                                t.cgroups
                                    .iter()
                                    .any(|g| g.path == path && g.inode == old.inode)
                            })
                    } else if let Some(name) = key.strip_prefix("device:") {
                        source
                            .devices
                            .iter()
                            .find(|d| d.name == name)
                            .is_some_and(|old| {
                                t.devices
                                    .iter()
                                    .any(|d| d.name == name && d.major_minor == old.major_minor)
                            })
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
                    for task in &mut t.tasks {
                        task.pss_at_ms = source
                            .tasks
                            .iter()
                            .find(|p| p.pid == task.pid && p.start_ticks == task.start_ticks)
                            .and_then(|p| p.pss_at_ms);
                        task.pss_bytes = source
                            .tasks
                            .iter()
                            .find(|p| p.pid == task.pid && p.start_ticks == task.start_ticks)
                            .and_then(|p| p.pss_bytes);
                    }
                }
                _ => {}
            }
            if age >= 10000 {
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
        };
        source.update(&mut now);
        assert!(now.details.is_empty());
        now.tasks[0].start_ticks -= 1;
        source.update(&mut now);
        assert_eq!(now.details.values().next().unwrap()[0].1, "42");
    }
}
