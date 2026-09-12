use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kernwatch::{
    app::App,
    domain::{Event, Quality},
    model, ui,
};
use ratatui::{backend::TestBackend, Terminal};
fn render(a: &App, width: u16, height: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
    t.draw(|f| ui::draw(f, a)).unwrap();
    t.backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}
#[test]
fn empty_live_tab_explains_capture_and_persists_failure_at_both_sizes() {
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.snapshot.views[5].rows.clear();
    a.snapshot
        .telemetry
        .capabilities
        .insert("syscalls".into(), Quality::Stopped);
    a.switch(5);
    let offer = "Press l";
    for (w, h) in [(80, 24), (160, 52)] {
        let screen = render(&a, w, h);
        assert!(
            screen.contains(offer),
            "an empty syscall tab should offer {offer:?} at {w}x{h}"
        );
        a.snapshot.telemetry.capabilities.insert(
            "syscalls".into(),
            Quality::Error("fixture attach failure".into()),
        );
        assert!(render(&a, w, h).contains("fixture attach failure"));
    }
}
#[test]
fn capture_shortcut_starts_live_capture_and_clears_hidden_display_state() {
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.switch(5);
    a.frozen = true;
    a.filter = "old scope".into();
    a.mode = 2;
    a.key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
    assert_eq!(a.probe_request.as_deref(), Some("syscalls seconds=30"));
    assert!(!a.frozen);
    assert!(a.filter.is_empty());
    assert_eq!(a.mode, 0);
    a.key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(a.probe_request.as_deref(), Some("stop"));
}
#[test]
fn demo_cannot_start_a_live_capture() {
    let mut a = App::new(model::demo());
    a.switch(5);
    a.key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
    assert!(a.probe_request.is_none());
}
#[test]
fn syscall_selection_survives_row_reordering_and_aggregate_filter_keeps_events() {
    let mut a = App::new(model::demo());
    a.switch(5);
    let name = a.rows()[0][0].clone();
    let mut next = a.snapshot.clone();
    next.views[5].rows.reverse();
    a.update(next);
    assert_eq!(a.rows()[a.selected][0], name);
    a.snapshot.views[5].rows = vec![vec![
        "openat".into(),
        "1".into(),
        "1".into(),
        "ENOENT".into(),
        "1ms".into(),
        "1ms".into(),
        "1ms".into(),
        "7".into(),
        "n=1".into(),
        "negative returns".into(),
    ]];
    a.selected = 0;
    a.filter = "ENOENT".into();
    a.snapshot.telemetry.events = vec![Event {
        source: "syscall trace".into(),
        message: "openat pid7 ret=-2 duration_ms=1".into(),
        ..Default::default()
    }];
    assert_eq!(a.visible_syscall_events().len(), 1);
    a.filter = "no such row".into();
    assert!(a.visible_syscall_events().is_empty());
}
