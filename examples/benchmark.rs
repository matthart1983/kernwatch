use kernwatch::{app::App, model, ui};
use ratatui::{backend::TestBackend, Terminal};
fn main() {
    let mut snapshot = model::demo();
    let seed = snapshot.telemetry.tasks[2].clone();
    snapshot.telemetry.tasks = (0..10_000)
        .map(|i| {
            let mut t = seed.clone();
            t.pid = 1000 + i;
            t.name = format!("worker-{i}");
            t.cpu = i % 256;
            t
        })
        .collect();
    let cpu = snapshot.telemetry.cpus[0].clone();
    snapshot.telemetry.cpus = (0..256)
        .map(|id| {
            let mut c = cpu.clone();
            c.id = id;
            c
        })
        .collect();
    let group = snapshot.telemetry.cgroups[0].clone();
    snapshot.telemetry.cgroups = (0..1000)
        .map(|i| {
            let mut g = group.clone();
            g.path = format!("/group-{i}");
            g
        })
        .collect();
    let mut a = App::new(snapshot);
    let mut terminal = Terminal::new(TestBackend::new(160, 60)).unwrap();
    for tab in [12, 1, 2, 7] {
        a.switch(tab);
        let mut times = Vec::new();
        for _ in 0..100 {
            let start = std::time::Instant::now();
            terminal.draw(|f| ui::draw(f, &a)).unwrap();
            times.push(start.elapsed().as_secs_f64() * 1000.);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{}: median={:.2}ms p95={:.2}ms max={:.2}ms",
            model::TABS[tab].0,
            times[50],
            times[95],
            times[99]
        );
    }
}
