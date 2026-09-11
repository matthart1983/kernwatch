//! Independently scheduled, bounded IRQ counters and affinity observations.
use crate::domain::*;
use std::{collections::BTreeMap, fs, path::Path};
pub const COLUMNS: [&str; 7] = [
    "IRQ",
    "Name",
    "Rate/s",
    "Configured",
    "Effective",
    "Observed CPU",
    "Handler p99",
];
#[derive(Default)]
pub struct Collector {
    previous: BTreeMap<String, (u64, Vec<u64>)>,
    affinity: BTreeMap<String, (String, u64)>,
    events: Vec<Event>,
}
impl Collector {
    pub fn sample(&mut self, t: &mut Telemetry, root: &Path) {
        let text = match fs::read_to_string(root.join("interrupts")) {
            Ok(s) => s,
            Err(e) => {
                t.capabilities
                    .insert("irq_counts".into(), Quality::Denied(e.to_string()));
                return;
            }
        };
        let cpus = text
            .lines()
            .next()
            .unwrap_or("")
            .split_whitespace()
            .filter_map(|v| v.strip_prefix("CPU").and_then(|v| v.parse::<u32>().ok()))
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for line in text.lines().skip(1).take(4096) {
            let Some((id, rest)) = line.split_once(':') else {
                continue;
            };
            let id = id.trim();
            let words = rest.split_whitespace().collect::<Vec<_>>();
            let counts = words
                .iter()
                .take(cpus.len())
                .map(|v| v.parse::<u64>())
                .collect::<Result<Vec<_>, _>>();
            let Ok(counts) = counts else { continue };
            if counts.len() != cpus.len() || counts.is_empty() {
                continue;
            }
            let key = format!("{id}:{cpus:?}");
            let previous = self.previous.insert(key, (t.at_ms, counts.clone()));
            let delta = previous.as_ref().filter(|(at, v)| {
                *at < t.at_ms
                    && v.len() == counts.len()
                    && counts.iter().zip(v).all(|(n, p)| n >= p)
            });
            let dt = delta.map(|(at, _)| (t.at_ms - at) as f64 / 1000.);
            let rate = delta.zip(dt).map(|((_, v), dt)| {
                counts.iter().zip(v).map(|(n, p)| n - p).sum::<u64>() as f64 / dt
            });
            let observed = delta
                .and_then(|(_, v)| {
                    counts
                        .iter()
                        .zip(v)
                        .enumerate()
                        .map(|(i, (n, p))| (i, n - p))
                        .max_by_key(|(_, n)| *n)
                })
                .map(|(i, n)| {
                    if n == 0 {
                        "no events".into()
                    } else {
                        format!("cpu{} · {n} events", cpus[i])
                    }
                })
                .unwrap_or("warming".into());
            let read = |file: &str| {
                fs::read_to_string(root.join(format!("irq/{id}/{file}")))
                    .map(|v| v.trim().to_owned())
                    .unwrap_or("—".into())
            };
            let configured = read("smp_affinity_list");
            let effective = read("effective_affinity_list");
            if id.parse::<u32>().is_ok() && configured != "—" {
                let value = format!("configured {configured} / effective {effective}");
                if let Some((old, at)) = self.affinity.insert(id.into(), (value.clone(), t.at_ms)) {
                    if old != value {
                        self.events.push(Event{id:format!("irq-affinity-{id}-{}",t.at_ms),at_ms:t.at_ms,source:"IRQ affinity poll".into(),severity:"warn".into(),subject:format!("irq:{id}"),message:format!("IRQ {id} changed during {at}..{}ms: {old} → {value}; actor and exact time unknown",t.at_ms)});
                    }
                }
            }
            t.record(
                &format!("irq.{id}.rate"),
                rate,
                "events/s",
                rate.unwrap_or(1.).max(1.),
            );
            rows.push((
                id.to_string(),
                serde_json::to_string(&vec![
                    id.to_string(),
                    words[cpus.len()..].join(" "),
                    rate.map(|v| format!("{v:.0}")).unwrap_or("sampling".into()),
                    configured,
                    effective,
                    observed,
                    "—".into(),
                ])
                .unwrap(),
            ));
        }
        self.previous
            .retain(|_, (at, _)| t.at_ms.saturating_sub(*at) < 600000);
        self.affinity
            .retain(|_, (_, at)| t.at_ms.saturating_sub(*at) < 600000);
        if self.events.len() > 64 {
            self.events.drain(..self.events.len() - 64);
        }
        t.events.extend(self.events.clone());
        t.capabilities.insert(
            "irq_counts".into(),
            if rows.is_empty() {
                Quality::Unsupported("No visible IRQ counters in this namespace".into())
            } else {
                Quality::Available
            },
        );
        t.details.insert("irq_rows".into(), rows);
        t.series.retain(|_, s| {
            s.samples
                .last()
                .is_some_and(|s| t.at_ms.saturating_sub(s.at_ms) < 600000)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hotplug_ids_counter_resets_and_affinity_intervals() {
        let root =
            std::env::temp_dir().join(format!("kernwatch-irq-{}", crate::recording::stamp()));
        fs::create_dir_all(root.join("irq/47")).unwrap();
        fs::write(
            root.join("interrupts"),
            " CPU0 CPU7\n47: 10 20 PCI-MSI fixture\n",
        )
        .unwrap();
        for name in ["smp_affinity_list", "effective_affinity_list"] {
            fs::write(root.join("irq/47").join(name), "0").unwrap();
        }
        let mut c = Collector::default();
        let mut t = Telemetry {
            at_ms: 1000,
            ..Default::default()
        };
        c.sample(&mut t, &root);
        fs::write(
            root.join("interrupts"),
            " CPU0 CPU7\n47: 12 50 PCI-MSI fixture\n",
        )
        .unwrap();
        fs::write(root.join("irq/47/effective_affinity_list"), "7").unwrap();
        t.at_ms = 2000;
        c.sample(&mut t, &root);
        let row: Vec<String> = serde_json::from_str(&t.details["irq_rows"][0].1).unwrap();
        assert_eq!(row[2], "32");
        assert!(row[5].starts_with("cpu7"));
        assert!(t.events.iter().any(|e| e.message.contains("1000..2000ms")));
        fs::write(
            root.join("interrupts"),
            " CPU0 CPU7\n47: 1 2 PCI-MSI fixture\n",
        )
        .unwrap();
        t.at_ms = 3000;
        c.sample(&mut t, &root);
        assert_eq!(t.series["irq.47.rate"].samples.last().unwrap().value, None);
        fs::write(
            root.join("interrupts"),
            " CPU0 CPU3\n47: 30 40 PCI-MSI fixture\n",
        )
        .unwrap();
        t.at_ms = 4000;
        c.sample(&mut t, &root);
        let row: Vec<String> = serde_json::from_str(&t.details["irq_rows"][0].1).unwrap();
        assert_eq!(row[5], "warming");
        fs::remove_dir_all(root).unwrap();
    }
}
