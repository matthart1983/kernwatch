//! Deterministic observations, sustained thresholds, baseline readiness and verification.
use crate::domain::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Baseline {
    pub count: u64,
    pub mean: f64,
    pub variance: f64,
    pub since_ms: u64,
}
impl Baseline {
    pub fn add(&mut self, value: f64, now: u64) {
        if self.count == 0 {
            self.mean = value;
            self.since_ms = now;
        } else {
            let delta = value - self.mean;
            self.mean += 0.02 * delta;
            self.variance = 0.98 * (self.variance + 0.02 * delta * delta);
        }
        self.count += 1;
    }
    pub fn ready(&self, now: u64) -> bool {
        self.count >= 1800 && now.saturating_sub(self.since_ms) >= 1_800_000
    }
}
#[derive(Default, Serialize, Deserialize)]
pub struct Engine {
    pub baselines: BTreeMap<String, Baseline>,
    active: BTreeMap<String, Issue>,
    streak: BTreeMap<String, u32>,
    clear: BTreeMap<String, u64>,
    boot: String,
    #[serde(default)]
    scope: String,
}
impl Engine {
    pub fn load(path: &std::path::Path) -> std::io::Result<Self> {
        if std::fs::metadata(path)?.len() > 4 * 1024 * 1024 {
            return Err(std::io::Error::other("baseline state exceeds 4 MiB"));
        }
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("tmp");
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(&serde_json::to_vec(self)?)?;
        file.sync_all()?;
        std::fs::rename(temporary, path)
    }
    pub fn update(&mut self, t: &mut Telemetry) {
        if self.boot != t.boot_id {
            self.baselines.clear();
            self.active.clear();
            self.streak.clear();
            self.clear.clear();
            self.boot = t.boot_id.clone();
        }
        let scope = format!(
            "cpus={:?}; groups={:?}",
            t.cpus.iter().map(|c| c.id).collect::<Vec<_>>(),
            t.cgroups
                .iter()
                .map(|g| (&g.path, g.inode, &g.quota, &g.cpus))
                .collect::<Vec<_>>()
        );
        if self.scope != scope {
            self.baselines.clear();
            self.scope = scope;
        }
        let mut observed = Vec::new();
        for c in &t.cpus {
            observed.push((
                format!("cpu:{}", c.id),
                c.busy,
                90.,
                format!("CPU{} utilization", c.id),
                format!("CPU{} · {:.1}%", c.id, c.busy),
                "/proc/stat delta".to_string(),
            ));
        }
        for (key, limit, title) in [
            ("sched.p99", 5., "Scheduler wakeup latency"),
            ("disk.age", 1000., "Outstanding I/O age"),
            ("psi.memory", 10., "Memory pressure"),
            ("bpf.total", 20., "Instrumentation overhead"),
            ("slab.growth", 64., "Dentry slab growth"),
            ("module.untrusted", 0., "Loaded module trust flags"),
            ("module.changed", 0., "Modules outside accepted baseline"),
            ("softirq.peak", 70., "Concentrated softirq execution"),
        ] {
            if let Some(m) = t.metrics.get(key) {
                if let Some(v) = m.value.filter(|_| m.quality == Quality::Available) {
                    observed.push((
                        key.into(),
                        v,
                        limit,
                        title.into(),
                        format!("{v:.2}{}", m.unit),
                        m.source.clone(),
                    ));
                }
            }
        }
        if let Some(cpu) = t
            .cpus
            .iter()
            .filter(|c| c.softirq > 70.)
            .max_by(|a, b| a.softirq.total_cmp(&b.softirq))
        {
            for event in t.events.iter().filter(|e| {
                e.source == "IRQ affinity poll" && t.at_ms.saturating_sub(e.at_ms) < 60000
            }) {
                let id = event.subject.strip_prefix("irq:").unwrap_or("");
                let on_cpu = t
                    .details
                    .get("irq_rows")
                    .and_then(|rows| rows.iter().find(|(k, _)| k == id))
                    .and_then(|(_, r)| serde_json::from_str::<Vec<String>>(r).ok())
                    .is_some_and(|r| r.get(4) == Some(&cpu.id.to_string()));
                if on_cpu {
                    observed.push((
                        format!("irq-placement:{id}"),
                        cpu.softirq,
                        70.,
                        "Recent IRQ placement and concentrated execution".into(),
                        format!("IRQ {id} · CPU{}; correlation only", cpu.id),
                        event.message.clone(),
                    ));
                }
            }
        }
        for group in &t.cgroups {
            if let Some(value) = group.throttled_ms_s {
                observed.push((
                    format!("throttle:{}", group.path),
                    value,
                    50.,
                    "Cgroup CPU throttling".into(),
                    group.path.clone(),
                    "cpu.stat throttled_usec delta; inclusive descendants".into(),
                ));
            }
        }
        for task in &t.tasks {
            if let Some(value) = task.blocked_ms {
                observed.push((format!("blocked:{}:{}",task.pid,task.start_ticks),value,1000.,"Task observed in uninterruptible sleep".into(),format!("{} pid {}",task.name,task.pid),"consecutive /proc stat D observations; lower bound, not causal storage evidence".into()));
            }
        }
        let mut present = std::collections::BTreeSet::new();
        for (key, value, limit, title, subject, source) in observed {
            present.insert(key.clone());
            let base = self.baselines.entry(key.clone()).or_default();
            let anomalous = value > limit
                || (base.ready(t.at_ms)
                    && value > base.mean + 3. * base.variance.sqrt()
                    && value > base.mean * 1.5);
            if anomalous {
                let n = self.streak.entry(key.clone()).or_default();
                *n += 1;
                self.clear.remove(&key);
                if *n >= 3 {
                    let issue=self.active.entry(key.clone()).or_insert_with(||Issue{id:key.clone(),title:title.clone(),subject:subject.clone(),severity:"warn".into(),since_ms:t.at_ms.saturating_sub(2000),state:"open".into(),causes:vec![Cause{label:"Cause unconfirmed".into(),detail:"Inspect scope and acquire discriminating evidence".into(),observed:false}],steps:vec!["Inspect affected subject and effective configuration".into(),"Capture a bounded trace and compare equivalent workload windows".into()],verification:format!("Value below {limit} for at least 60 seconds with adequate samples; missing data is inconclusive."),..Default::default()});
                    issue.subject = subject;
                    issue.state = "open".into();
                    issue.evidence = vec![Evidence {
                        label: format!("{title}: {value:.2} · threshold {limit}"),
                        source,
                        at_ms: t.at_ms,
                        subject: key.clone(),
                        weight: 1.,
                        observed: true,
                    }];
                }
            } else {
                self.streak.remove(&key);
                base.add(value, t.at_ms);
                let since = self.clear.entry(key.clone()).or_insert(t.at_ms);
                if t.at_ms.saturating_sub(*since) >= 60000 {
                    if let Some(i) = self.active.get_mut(&key) {
                        i.state = "resolved".into();
                    }
                }
            }
        }
        for (key, issue) in &mut self.active {
            if !present.contains(key) {
                self.clear.remove(key);
                self.streak.remove(key);
                issue.state = "data unavailable".into();
            }
        }
        t.issues = self.active.values().cloned().collect();
        t.issues.sort_by_key(|i| {
            (
                i.state == "resolved",
                match i.severity.as_str() {
                    "error" | "critical" => 0,
                    "warn" | "warning" => 1,
                    _ => 2,
                },
                i.since_ms,
                i.id.clone(),
            )
        });
        t.details.insert(
            "baselines".into(),
            self.baselines
                .iter()
                .map(|(k, b)| {
                    (
                        k.clone(),
                        format!(
                            "n={} mean={:.2} σ={:.2} {}",
                            b.count,
                            b.mean,
                            b.variance.sqrt(),
                            if b.ready(t.at_ms) {
                                "ready"
                            } else {
                                "learning"
                            }
                        ),
                    )
                })
                .collect(),
        );
    }
}
#[derive(Debug, PartialEq)]
pub enum Verdict {
    Passed,
    Failed,
    Inconclusive,
}
pub fn verify(series: &Series, end: u64, threshold: f64) -> Verdict {
    let start = end.saturating_sub(60000);
    let values = series
        .samples
        .iter()
        .filter(|s| s.at_ms >= start && s.at_ms <= end)
        .collect::<Vec<_>>();
    if end < 60000
        || values.len() < 60
        || values
            .first()
            .map(|s| s.at_ms > start + 1000)
            .unwrap_or(true)
        || values
            .last()
            .map(|s| end.saturating_sub(s.at_ms) > 1000)
            .unwrap_or(true)
        || values
            .windows(2)
            .any(|w| w[1].at_ms.saturating_sub(w[0].at_ms) > 1500)
        || values.iter().any(|s| s.value.is_none())
    {
        return Verdict::Inconclusive;
    }
    if values.iter().any(|s| s.value.unwrap() > threshold) {
        Verdict::Failed
    } else {
        Verdict::Passed
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sustained_threshold_and_missing_evidence() {
        let mut engine = Engine::default();
        let mut t = Telemetry {
            boot_id: "boot".into(),
            ..Default::default()
        };
        for i in 0..3 {
            t.at_ms = i * 1000;
            t.cpus = vec![Cpu {
                id: 3,
                busy: 98.,
                ..Default::default()
            }];
            engine.update(&mut t);
            if i < 2 {
                assert!(t.issues.is_empty());
            }
        }
        assert_eq!(t.issues.len(), 1);
        t.cpus.clear();
        engine.update(&mut t);
        assert_eq!(t.issues[0].state, "data unavailable");
    }
    #[test]
    fn gaps_do_not_pass() {
        let s = Series {
            samples: (0..=60)
                .map(|i| Sample {
                    at_ms: i * 1000,
                    value: if i == 30 { None } else { Some(0.5) },
                })
                .collect(),
            ..Default::default()
        };
        assert_eq!(verify(&s, 60000, 1.), Verdict::Inconclusive);
    }
}
