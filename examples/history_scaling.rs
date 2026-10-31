//! Virtual-clock history capacity/churn test. This does not simulate procfs cost.
use kernwatch::domain::Series;
use std::{collections::BTreeMap, time::Instant};
fn rss() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|l| {
            l.strip_prefix("VmRSS:")
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse().ok())
        })
        .unwrap()
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let count: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1000);
    let churn = args.get(2).is_some_and(|s| s == "churn");
    let mut keys: Vec<_> = (0..count).map(|n| format!("task.runtime{n}.1")).collect();
    let mut history: BTreeMap<String, Series> = keys
        .iter()
        .map(|k| (k.clone(), Series::default()))
        .collect();
    let mut consumer = history.clone();
    let mut frozen = None;
    let mut early = Vec::new();
    let mut late = Vec::new();
    let mut max_bytes = 0;
    let start = Instant::now();
    for tick in 0..1800u64 {
        let begin = Instant::now();
        if churn && tick > 0 && tick % 20 == 0 {
            for (n, key) in keys.iter_mut().enumerate().take(count / 10) {
                history.remove(key);
                *key = format!("task.runtime{n}.{}", tick + 1);
                history.insert(key.clone(), Series::default());
            }
        }
        for series in history.values_mut() {
            series.push(tick * 1000, (tick % 7 != 0).then_some(10.));
        }
        assert_eq!(history.len(), count);
        consumer = history.clone();
        if tick == 599 {
            frozen = Some(consumer.clone());
        }
        let ms = begin.elapsed().as_secs_f64() * 1000.;
        if (600..700).contains(&tick) {
            early.push(ms);
        }
        if tick >= 1700 {
            late.push(ms);
        }
        if tick % 100 == 0 {
            let bytes: usize = history.values().map(|s| s.samples.bytes()).sum();
            max_bytes = max_bytes.max(bytes);
            assert!(
                bytes <= count * (632 * std::mem::size_of::<kernwatch::domain::Sample>() + 4096)
            );
        }
    }
    assert!(consumer
        .values()
        .all(|s| s.samples.len() <= 600 && s.samples.last().unwrap().at_ms == 1799000));
    assert!(frozen
        .as_ref()
        .unwrap()
        .values()
        .all(|s| s.samples.last().unwrap().at_ms == 599000));
    early.sort_by(f64::total_cmp);
    late.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({"tasks":count,"churn":churn,"virtual_seconds":1800,"elapsed_seconds":start.elapsed().as_secs_f64(),"early_tick_median_ms":early[early.len()/2],"late_tick_median_ms":late[late.len()/2],"max_live_history_bytes":max_bytes,"rss_kib_including_frozen_history":rss()})
    );
}
