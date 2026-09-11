//! Kernel BPF inventory; statistics are deltas, never inferred from run count alone.
use crate::{
    domain::{Quality, Telemetry},
    model::View,
};
use std::{
    collections::BTreeMap,
    os::fd::{AsFd, AsRawFd},
    time::Instant,
};
struct Collector {
    previous: BTreeMap<u32, (u64, u128)>,
    last: Instant,
    metadata: BTreeMap<u32, (Instant, String)>,
}
impl Default for Collector {
    fn default() -> Self {
        Self {
            previous: BTreeMap::new(),
            last: Instant::now(),
            metadata: BTreeMap::new(),
        }
    }
}
impl Collector {
    pub fn sample(&mut self, t: &mut Telemetry) -> View {
        let dt = self.last.elapsed().as_secs_f64().max(0.001);
        self.last = Instant::now();
        let mut rows = Vec::new();
        let mut next = BTreeMap::new();
        let mut failure = None;
        t.details.insert(
            "bpf.status".into(),
            [
                ("JIT", "/proc/sys/net/core/bpf_jit_enable"),
                (
                    "unprivileged disabled",
                    "/proc/sys/kernel/unprivileged_bpf_disabled",
                ),
                ("global stats", "/proc/sys/kernel/bpf_stats_enabled"),
            ]
            .into_iter()
            .map(|(name, path)| {
                (
                    name.into(),
                    std::fs::read_to_string(path)
                        .map(|v| v.trim().to_owned())
                        .unwrap_or("unavailable".into()),
                )
            })
            .collect(),
        );
        let links = crate::bpf_metadata::links();
        let mut inspections = 0;
        let enabled = crate::probes::statistics_active()
            || std::fs::read_to_string("/proc/sys/kernel/bpf_stats_enabled")
                .map(|s| s.trim() == "1")
                .unwrap_or(false);
        for result in aya::programs::loaded_programs().take(4096) {
            match result {
                Err(e) => {
                    failure = Some(e.to_string());
                    break;
                }
                Ok(info) => {
                    let id = info.id();
                    let runs = info.run_count();
                    let ns = info.run_time().as_nanos();
                    let (rate, mean, cpu) = if enabled {
                        self.previous
                            .get(&id)
                            .map(|(old_runs, old_ns)| {
                                let n = runs.saturating_sub(*old_runs);
                                let time = ns.saturating_sub(*old_ns);
                                (
                                    format!("{:.0}", n as f64 / dt),
                                    if n == 0 {
                                        "—".into()
                                    } else {
                                        format!("{:.0}", time as f64 / n as f64)
                                    },
                                    format!("{:.3}", time as f64 / 1e9 / dt * 100.),
                                )
                            })
                            .unwrap_or(("warming".into(), "warming".into(), "warming".into()))
                    } else {
                        ("stats off".into(), "—".into(), "—".into())
                    };
                    let mapids = info.map_ids().ok().flatten().unwrap_or_default();
                    let mut details = vec![
                        ("id".into(), id.to_string()),
                        ("tag".into(), format!("{:016x}", info.tag())),
                        (
                            "loaded at".into(),
                            info.loaded_at()
                                .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| format!("{} Unix seconds", d.as_secs()))
                                .unwrap_or("unknown".into()),
                        ),
                        (
                            "name".into(),
                            info.name_as_str().unwrap_or("non-UTF8 name").into(),
                        ),
                        ("JIT bytes".into(), info.size_jitted().to_string()),
                        (
                            "instructions".into(),
                            info.size_translated()
                                .map(|n| (n / 8).to_string())
                                .unwrap_or("not available".into()),
                        ),
                        (
                            "verifier processed instructions".into(),
                            info.verified_instruction_count()
                                .map(|n| n.to_string())
                                .unwrap_or("not exposed by kernel".into()),
                        ),
                        (
                            "translated bytes".into(),
                            info.size_translated()
                                .map(|n| n.to_string())
                                .unwrap_or("unknown".into()),
                        ),
                        (
                            "runtime statistics".into(),
                            if enabled {
                                "enabled"
                            } else {
                                "disabled; mean/p99 cannot be inferred"
                            }
                            .into(),
                        ),
                        (
                            "creator UID".into(),
                            info.created_by_uid()
                                .map(|n| n.to_string())
                                .unwrap_or("not exposed by kernel".into()),
                        ),
                    ];
                    let attachment = match &links {
                        Ok(links) => links.get(&id).map(|v| v.join("; ")).unwrap_or(
                            "No BPF link observed; legacy attachments are not enumerable here"
                                .into(),
                        ),
                        Err(e) => format!("link inventory: {e}"),
                    };
                    details.push(("attachments".into(), attachment.clone()));
                    if inspections < 8
                        && !self
                            .metadata
                            .get(&id)
                            .is_some_and(|(at, _)| at.elapsed().as_secs() < 60)
                    {
                        let helpers = info
                            .fd()
                            .map_err(|e| e.to_string())
                            .and_then(|fd| {
                                crate::bpf_metadata::helpers(
                                    fd.as_fd().as_raw_fd(),
                                    info.size_translated().unwrap_or(0),
                                )
                                .map_err(|e| e.to_string())
                            })
                            .unwrap_or_else(|e| format!("helper acquisition: {e}"));
                        self.metadata.insert(id, (Instant::now(), helpers));
                        inspections += 1;
                    }
                    if let Some((at, helpers)) = self.metadata.get(&id) {
                        details.push((
                            "helpers".into(),
                            format!("{helpers}; sampled {}s ago", at.elapsed().as_secs()),
                        ));
                    }
                    for mid in &mapids {
                        let description = match aya::maps::MapInfo::from_id(*mid) {
                            Ok(m) => format!("{} {} · max_entries={} · key {}B / value {}B · occupancy unavailable",
                                m.name_as_str().unwrap_or("?"),
                                m.map_type().map(|kind|format!("{kind:?}")).unwrap_or("unknown".into()),
                                m.max_entries(), m.key_size(), m.value_size()),
                            Err(error) => format!("metadata unavailable: {error}"),
                        };
                        details.push((format!("map {mid}"), description));
                    }
                    t.details.insert(format!("bpf:{id}"), details.clone());
                    if rows.is_empty() {
                        t.details.insert("bpf".into(), details);
                    }
                    t.record(
                        &format!("bpf.program{id}"),
                        cpu.parse::<f64>().ok(),
                        "% / one CPU",
                        100.,
                    );
                    rows.push(vec![
                        id.to_string(),
                        info.name_as_str().unwrap_or("?").into(),
                        info.program_type()
                            .map(|kind| format!("{kind:?}"))
                            .unwrap_or("unknown".into()),
                        rate,
                        mean,
                        cpu,
                        mapids.len().to_string(),
                        info.created_by_uid()
                            .map(|n| format!("uid {n}"))
                            .unwrap_or("unknown".into()),
                        attachment,
                    ]);
                    next.insert(id, (runs, ns));
                }
            }
        }
        if enabled && failure.is_none() {
            let measured = rows
                .iter()
                .filter_map(|r| r.get(5)?.parse::<f64>().ok())
                .collect::<Vec<_>>();
            if measured.len() == rows.len() && !measured.is_empty() {
                let total = measured.iter().sum::<f64>();
                t.metrics.insert(
                    "bpf.total".into(),
                    crate::domain::Measurement::known(
                        total,
                        "%",
                        "BPF program runtime delta / one CPU",
                        t.at_ms,
                    ),
                );
                t.record("bpf.total", Some(total), "%", total.max(10.));
            }
        }
        self.metadata.retain(|id, _| next.contains_key(id));
        self.previous = next;
        if let Some(e) = failure {
            t.capabilities
                .insert("bpf".into(), Quality::Denied(e.clone()));
            if rows.is_empty() {
                rows.push(vec![
                    "—".into(),
                    e,
                    "—".into(),
                    "—".into(),
                    "—".into(),
                    "—".into(),
                    "—".into(),
                ]);
            }
        } else {
            t.capabilities.insert("bpf".into(), Quality::Available);
        }
        View::new(&["id","program","type","runs/s","mean ns","CPU %","maps","creator","attachment"],rows,&["Source: BPF program info. CPU% normalized to one core. Runtime counters require kernel statistics.","Kernel program info does not expose loader PID, p99 runtime, helper costs or ring-buffer consumer lag."])
    }
}

/// Inventory and enrichment never block the fast procfs collector.
pub struct Inventory {
    send: std::sync::mpsc::SyncSender<u64>,
    receive: std::sync::mpsc::Receiver<(Telemetry, View)>,
    latest: Option<(Telemetry, View)>,
}
impl Default for Inventory {
    fn default() -> Self {
        let (send, requests) = std::sync::mpsc::sync_channel(1);
        let (results, receive) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut collector = Collector::default();
            let mut history = BTreeMap::new();
            while let Ok(at_ms) = requests.recv() {
                let mut t = Telemetry {
                    at_ms,
                    series: std::mem::take(&mut history),
                    ..Default::default()
                };
                let view = collector.sample(&mut t);
                history = t.series.clone();
                let _ = results.try_send((t, view));
            }
        });
        Self {
            send,
            receive,
            latest: None,
        }
    }
}
impl Inventory {
    pub fn sample(&mut self, t: &mut Telemetry) -> View {
        while let Ok(result) = self.receive.try_recv() {
            self.latest = Some(result)
        }
        let _ = self.send.try_send(t.at_ms);
        if let Some((source, view)) = &self.latest {
            t.details.extend(source.details.clone());
            t.series.extend(source.series.clone());
            if t.at_ms.saturating_sub(source.at_ms) < 10000 {
                t.capabilities.extend(source.capabilities.clone());
                t.metrics.extend(source.metrics.clone());
            } else {
                t.capabilities.insert("bpf".into(), Quality::Stale);
                let mut view = view.clone();
                for row in &mut view.rows {
                    for value in row.iter_mut().skip(3).take(3) {
                        *value = "stale".into();
                    }
                }
                return view;
            }
            view.clone()
        } else {
            t.capabilities.insert("bpf".into(), Quality::Warming);
            View::new(&["id", "program"], Vec::new(), &["BPF inventory warming"])
        }
    }
}
