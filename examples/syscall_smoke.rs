#[cfg(target_os = "linux")]
fn main() {
    use kernwatch::{app::App, domain::*, model, probes::Probes, ui};
    use ratatui::{backend::TestBackend, Terminal};
    let tid = unsafe { libc::syscall(libc::SYS_gettid) };
    let mut p = Probes::start(&format!("syscalls pid={tid} seconds=5")).expect("syscall attach");
    for _ in 0..30 {
        let _ = std::fs::metadata("/kernwatch-nonexistent-syscall-test");
        std::thread::sleep(std::time::Duration::from_millis(5));
        p.poll().expect("poll");
    }
    let mut captured = Telemetry {
        at_ms: kernwatch::enrich::monotonic_ms(),
        ..Default::default()
    };
    p.apply(&mut captured);
    let mut snapshot = model::demo();
    snapshot.demo = false;
    snapshot.telemetry = captured;
    snapshot.views[5].rows.clear();
    kernwatch::probes::hydrate(&mut snapshot);
    assert!(
        !snapshot.views[5].rows.is_empty(),
        "no syscall rows after capture"
    );
    assert!(
        snapshot
            .telemetry
            .metrics
            .keys()
            .any(|k| k.starts_with("syscall.") && k.ends_with(".p99")),
        "no live history measurements"
    );
    assert!(!snapshot.telemetry.histograms.is_empty(), "no histogram");
    let mut a = App::new(snapshot);
    a.switch(5);
    let index = a
        .rows()
        .iter()
        .position(|r| r.get(3).is_some_and(|v| v == "ENOENT"))
        .expect("missing expected errno row");
    a.selected = index;
    assert!(
        !a.visible_syscall_events().is_empty(),
        "empty selected stream"
    );
    let mut terminal = Terminal::new(TestBackend::new(160, 52)).unwrap();
    terminal.draw(|f| ui::draw(f, &a)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(text.contains("ENOENT"));
    println!(
        "SYSCALL_TAB_OK rows={} events={} paired={} lost={} selected={}",
        a.rows().len(),
        a.visible_syscall_events().len(),
        p.correlator.paired,
        p.correlator.lost,
        a.rows()[index][0]
    );
    assert_eq!(p.correlator.lost, 0);
}
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Linux syscall capture validation only");
}
