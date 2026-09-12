use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kernwatch::{app::App, model, ui};
use ratatui::{backend::TestBackend, Terminal};

const FLAME: usize = 13;

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
fn key(a: &mut App, code: KeyCode) {
    a.key(KeyEvent::new(code, KeyModifiers::NONE));
}

#[test]
fn the_demo_profile_draws_its_hottest_path_at_both_sizes() {
    let mut a = App::new(model::demo());
    a.switch(FLAME);
    for (w, h) in [(80, 24), (160, 52)] {
        let screen = render(&a, w, h);
        assert!(
            screen.contains("envoy_main"),
            "the root of the captured stacks should be drawn at {w}x{h}"
        );
        assert!(
            screen.contains("samples"),
            "the panel should say how many samples it is drawing at {w}x{h}"
        );
    }
}

#[test]
fn an_empty_profile_explains_how_to_collect_one_instead_of_drawing_nothing() {
    let mut a = App::new(model::demo());
    a.snapshot.telemetry.profile = Default::default();
    a.switch(FLAME);
    let screen = render(&a, 160, 52);
    assert!(screen.contains("No stacks have been collected"));
    assert!(
        screen.contains("stack"),
        "the empty state should name the capture that would populate it"
    );
}

#[test]
fn zooming_narrows_to_the_selected_frame_and_escape_widens_again() {
    let mut a = App::new(model::demo());
    a.switch(FLAME);
    assert!(a.flame_zoom.is_empty());
    // The only child of the root is the process frame.
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.flame_zoom, ["envoy_main"]);
    // Selecting the heaviest child and zooming again goes one level deeper.
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.flame_zoom, ["envoy_main", "worker_loop"]);
    let screen = render(&a, 160, 52);
    assert!(
        screen.contains("envoy_main › worker_loop"),
        "the panel title should show the path that was zoomed into"
    );
    key(&mut a, KeyCode::Esc);
    assert_eq!(a.flame_zoom, ["envoy_main"]);
    key(&mut a, KeyCode::Esc);
    assert!(a.flame_zoom.is_empty());
}

#[test]
fn selection_moves_between_the_zoomed_frames_children() {
    let mut a = App::new(model::demo());
    a.switch(FLAME);
    key(&mut a, KeyCode::Enter);
    key(&mut a, KeyCode::Enter);
    let children = a.flame_children();
    assert!(children > 1, "worker_loop calls several frames");
    key(&mut a, KeyCode::Down);
    assert_eq!(a.selected, 1);
    // Selection stops at the last child rather than running past it.
    for _ in 0..children + 5 {
        key(&mut a, KeyCode::Down);
    }
    assert_eq!(a.selected, children - 1);
}

#[test]
fn leaving_the_view_drops_the_zoom() {
    let mut a = App::new(model::demo());
    a.switch(FLAME);
    key(&mut a, KeyCode::Enter);
    assert!(!a.flame_zoom.is_empty());
    a.switch(0);
    assert!(a.flame_zoom.is_empty(), "zoom is state of the flame view");
}

#[test]
fn the_truncated_share_is_reported_rather_than_repaired() {
    let mut a = App::new(model::demo());
    a.switch(FLAME);
    let profile = &a.snapshot.telemetry.profile;
    assert!(profile.shallow > 0, "the fixture includes truncated stacks");
    let screen = render(&a, 160, 52);
    assert!(
        screen.contains("truncated"),
        "a profile with truncated stacks must say so"
    );
}

#[test]
fn an_exported_report_carries_the_folded_stacks() {
    let a = App::new(model::demo());
    let folded = a.snapshot.telemetry.profile.folded();
    assert!(!folded.is_empty());
    // One line per leaf path, ending in a count, as flamegraph.pl reads.
    let line = folded
        .iter()
        .find(|l| l.contains("epoll_wait"))
        .expect("the hottest path is present");
    let (path, count) = line.rsplit_once(' ').expect("a path and a count");
    assert!(path.contains(';'), "frames are separated by semicolons");
    assert!(count.parse::<u64>().is_ok(), "the line ends in a count");
}

#[test]
fn a_captures_stacks_survive_polls_that_fold_nothing() {
    // The probe thread rebuilds its telemetry every poll. A poll that folded
    // no stacks must not erase what earlier polls collected, which is how the
    // profile reached the view empty on a live capture.
    let mut retained = kernwatch::flame::Profile::default();
    let mut collected = kernwatch::flame::Profile::new("syscalls · TID 42");
    collected.add(&["main".into(), "read".into()], 1);

    let mut s = model::demo();
    s.telemetry.profile = Default::default();
    kernwatch::probes::merge_profile(&mut retained, &collected, true, &mut s);
    assert_eq!(s.telemetry.profile.root.samples, 1);

    // The next poll folded nothing; the view keeps what was collected.
    let mut s = model::demo();
    s.telemetry.profile = Default::default();
    kernwatch::probes::merge_profile(&mut retained, &Default::default(), false, &mut s);
    assert_eq!(s.telemetry.profile.root.samples, 1);
    assert_eq!(s.telemetry.profile.source, "syscalls · TID 42");

    // A new capture starts a new profile rather than adding to the old one.
    let mut s = model::demo();
    s.telemetry.profile = Default::default();
    kernwatch::probes::merge_profile(&mut retained, &Default::default(), true, &mut s);
    assert!(
        s.telemetry.profile.is_empty(),
        "stacks from two captures are not one profile"
    );
}

#[test]
fn an_empty_profile_says_which_reason_it_is() {
    use kernwatch::domain::Quality;
    let empty = |setup: &dyn Fn(&mut App)| {
        let mut a = App::new(model::demo());
        a.snapshot.demo = false;
        a.snapshot.telemetry.profile = Default::default();
        // The fixture ships a syscall capability; each case sets its own.
        a.snapshot.telemetry.capabilities.remove("syscalls");
        setup(&mut a);
        a.switch(FLAME);
        render(&a, 160, 40)
    };

    // Nothing started: say how to start one, and that opening the view is not it.
    let screen = empty(&|_| {});
    assert!(screen.contains("No capture has been started"));
    assert!(screen.contains("thread id, not a process id"));

    // Refused: repeat the refusal rather than the instructions.
    let screen = empty(&|a| {
        a.snapshot.telemetry.capabilities.insert(
            "syscalls".into(),
            Quality::Denied("operation not permitted".into()),
        );
    });
    assert!(screen.contains("did not run"));
    assert!(screen.contains("operation not permitted"));
    assert!(screen.contains("restart as root"));

    // Running but nothing recorded yet.
    let screen = empty(&|a| {
        a.snapshot
            .telemetry
            .capabilities
            .insert("syscalls".into(), Quality::Available);
    });
    assert!(screen.contains("capture is running"));
    assert!(screen.contains("makes no syscalls"));

    // Running, and the walk is failing for a nameable reason.
    let screen = empty(&|a| {
        a.snapshot
            .telemetry
            .capabilities
            .insert("syscalls".into(), Quality::Available);
        a.snapshot.telemetry.details.insert(
            "probe.stacks".into(),
            vec![(
                "walk failed (errno 14, no frame pointer)".into(),
                "812".into(),
            )],
        );
    });
    assert!(screen.contains("812"));
    assert!(screen.contains("no frame pointer"));
    assert!(
        screen.contains("fno-omit-frame-pointer"),
        "a reader who cannot walk a binary should be told what would fix it"
    );
}

#[test]
fn profiling_the_selected_thread_starts_a_stack_capture_for_it() {
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.switch(1);
    let pid = a.selected_task().expect("the fixture lists threads").pid;
    key(&mut a, KeyCode::Char('P'));
    assert_eq!(
        a.probe_request.as_deref(),
        Some(format!("syscalls pid={pid} seconds=30 stack").as_str()),
        "P should request the capture the Flame view needs, scoped to the selection"
    );
    assert_eq!(a.tab, FLAME, "and show the profile it is filling");
    assert!(a.status.contains(&pid.to_string()));
}

#[test]
fn moving_the_selection_profiles_the_thread_that_is_selected() {
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.switch(1);
    key(&mut a, KeyCode::Down);
    key(&mut a, KeyCode::Down);
    let pid = a.selected_task().expect("a later thread").pid;
    key(&mut a, KeyCode::Char('P'));
    assert_eq!(
        a.probe_request.as_deref(),
        Some(format!("syscalls pid={pid} seconds=30 stack").as_str())
    );
}

#[test]
fn a_demo_or_replay_profile_refuses_to_start_a_host_capture() {
    let mut a = App::new(model::demo());
    a.switch(1);
    key(&mut a, KeyCode::Char('P'));
    assert!(
        a.probe_request.is_none(),
        "demo data must never start a capture on the host"
    );
    assert!(a.status.contains("live mode"));
    assert_eq!(a.tab, 1, "and the view does not move");
}

#[test]
fn profiling_from_the_flame_view_goes_where_a_thread_can_be_chosen() {
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.switch(FLAME);
    key(&mut a, KeyCode::Char('P'));
    assert_eq!(a.tab, 1, "the profile itself has no thread list");
    assert!(
        a.probe_request.is_none(),
        "nothing is captured until one is picked"
    );
}
