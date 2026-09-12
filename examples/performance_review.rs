//! Repeatable profile scaling measurements, without requiring BPF permissions.
use kernwatch::{app::App, flame, model, ui};
use ratatui::{backend::TestBackend, Terminal};
use serde_json::{json, Value};
use std::{hint::black_box, time::Instant};

fn rss() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")
                .map(|v| v.split_whitespace().next().unwrap().parse().unwrap())
        })
        .unwrap()
}

fn measure(mut f: impl FnMut()) -> Value {
    f();
    let start = Instant::now();
    let mut times = Vec::new();
    while times.len() < 7 || (times.len() < 100 && start.elapsed().as_millis() < 300) {
        let t = Instant::now();
        f();
        times.push(t.elapsed().as_secs_f64() * 1000.);
    }
    times.sort_by(f64::total_cmp);
    json!({"iterations":times.len(), "median_ms":times[times.len()/2],
           "p95_ms":times[(times.len()*95/100).min(times.len()-1)]})
}

fn profile(stacks: usize, shape: &str) -> flame::Profile {
    let mut p = flame::Profile::new("generated scaling workload");
    p.metadata.kind = flame::Kind::Cpu;
    p.metadata.unit = "cpu_samples".into();
    for i in 0..stacks {
        let path: Vec<String> = if shape == "wide" {
            vec!["main".into(), format!("handler_{i:05}")]
        } else {
            let mut path = vec!["main".into(), format!("group_{:03}", i / 64)];
            path.extend((0..10).map(|depth| format!("frame_{i:05}_{depth}")));
            path
        };
        for id in &path {
            p.frames
                .entry(id.clone())
                .or_insert_with(|| flame::FrameInfo {
                    raw: format!("_RNvNtCs0123456789abcdef_14profile_review8workload_{}", id),
                    display: format!("profile_review::service::request::dispatch::{id}"),
                    image: "/usr/local/bin/profile-review-workload@0123456789abcdef".into(),
                    ..Default::default()
                });
        }
        p.add(&path, (i % 17 + 1) as u64);
    }
    p.sort();
    p.quality.attempted = p.root.samples;
    p.quality.user_stacks = p.root.samples;
    p
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let shape = args.get(1).map(String::as_str).unwrap_or("wide");
    let count: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000);
    let start = Instant::now();
    let current = profile(count, shape);
    let build_ms = start.elapsed().as_secs_f64() * 1000.;
    let mut baseline = current.clone();
    baseline.add(&["main".into(), "removed_path".into()], 10);
    let mut app = App::new(model::demo());
    app.switch(13);
    app.snapshot.telemetry.profile = current.clone();
    app.flame_baseline = Some(baseline.clone());
    let mut terminal = Terminal::new(TestBackend::new(160, 48)).unwrap();
    let single = measure(|| {
        terminal.draw(|f| ui::draw(f, &app)).unwrap();
    });
    app.flame_compare = Some(flame::ComparisonMode::Share);
    let first = Instant::now();
    terminal.draw(|f| ui::draw(f, &app)).unwrap();
    let initial_comparison_ms = first.elapsed().as_secs_f64() * 1000.;
    let comparison = measure(|| {
        terminal.draw(|f| ui::draw(f, &app)).unwrap();
    });
    let union = measure(|| {
        black_box(current.comparison_union(&baseline));
    });
    let clone_sort = measure(|| {
        let mut p = current.clone();
        p.sort();
        black_box(p);
    });
    let mut snapshot = model::demo();
    snapshot.telemetry.profile = current.clone();
    let estimated_with_symbols = App::frame_bytes(&snapshot);
    let symbols_json_bytes = serde_json::to_vec(&current.frames).unwrap().len();
    snapshot.telemetry.profile.frames.clear();
    let estimated_without_symbols = App::frame_bytes(&snapshot);
    snapshot.telemetry.profile.frames = current.frames.clone();
    snapshot.telemetry.at_ms = 0;
    let mut history = App::new(snapshot.clone());
    let rss_before = rss();
    for tick in 1..=60 {
        snapshot.telemetry.at_ms = tick * 1000;
        history.update(snapshot.clone());
    }
    let rss_after = rss();
    println!(
        "{}",
        json!({"shape":shape,"stacks":count,"symbols":current.frames.len(),
        "build_ms":build_ms,"render":single,"render_comparison":comparison,
        "initial_comparison_ms":initial_comparison_ms,"comparison_union":union,"clone_sort_publish":clone_sort,
        "frame_estimate_with_symbols":estimated_with_symbols,
        "frame_estimate_without_symbols":estimated_without_symbols,
        "symbol_json_bytes":symbols_json_bytes,
        "history_rss_before_kib":rss_before,"history_rss_after_kib":rss_after,
        "history_span_ms":history.timeline_span()})
    );
}
