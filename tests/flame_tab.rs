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
fn an_empty_view_with_no_thread_chosen_offers_the_threads_to_choose_from() {
    let mut a = App::new(model::demo());
    a.snapshot.telemetry.profile = Default::default();
    a.switch(FLAME);
    assert!(a.flame_picking());
    let screen = render(&a, 160, 52);
    assert!(screen.contains("choose a process to profile"));
    assert!(screen.contains("TGID"), "the picker identifies processes");
    assert_eq!(
        a.flame_children(),
        0,
        "selection moves between threads here"
    );
    assert!(!a.visible_tasks().is_empty());
}

#[test]
fn an_empty_profile_for_a_chosen_thread_explains_itself() {
    let mut a = App::new(model::demo());
    a.snapshot.telemetry.profile = Default::default();
    a.switch(FLAME);
    a.flame_target = a.flame_subjects().first().cloned();
    assert!(!a.flame_picking());
    let screen = render(&a, 160, 52);
    assert!(screen.contains("No stacks have been collected"));
    let subject = a.flame_target.clone().unwrap();
    assert!(
        screen.contains(&subject.tid.to_string()),
        "an empty profile still says whose stacks are missing"
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
        // A thread has been chosen, so the view explains the empty profile
        // rather than offering the list again.
        a.flame_target = a.flame_subjects().first().cloned();
        // The fixture ships a syscall capability; each case sets its own.
        a.snapshot.telemetry.capabilities.remove("syscalls");
        setup(&mut a);
        a.switch(FLAME);
        render(&a, 160, 40)
    };

    // Nothing started: say how to start one, and that opening the view is not it.
    let screen = empty(&|_| {});
    assert!(screen.contains("Nothing has been captured yet"));
    assert!(screen.contains("choose a process or thread"));

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
    assert!(screen.contains("makes none produces nothing"));

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
fn the_view_can_start_a_profile_without_leaving_it() {
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.snapshot.telemetry.profile = Default::default();
    a.switch(FLAME);
    assert!(
        a.flame_picking(),
        "with no thread chosen it offers the list"
    );
    key(&mut a, KeyCode::Down);
    // The picker offers subjects, and a process is captured through its
    // busiest thread.
    let subject = a.flame_subjects()[a.selected].clone();
    let pid = subject.tid;
    key(&mut a, KeyCode::Enter);
    assert_eq!(
        a.probe_request.as_deref(),
        Some(format!("syscalls pid={pid} seconds=30 stack").as_str()),
        "Enter on the picker profiles the selected subject"
    );
    assert_eq!(a.tab, FLAME, "and stays on the profile it is filling");
    assert_eq!(a.flame_target.map(|s| s.tid), Some(pid));
}

#[test]
fn a_profile_names_the_thread_it_is_of() {
    let mut a = App::new(model::demo());
    a.switch(FLAME);
    a.flame_target = a.flame_subjects().first().cloned();
    let subject = a.flame_target.clone().unwrap();
    let screen = render(&a, 160, 52);
    assert!(
        screen.contains(&subject.name) && screen.contains(&subject.tid.to_string()),
        "the view should name the subject whose stacks it draws"
    );
    assert!(screen.contains("TGID"), "and the process it belongs to");
}

#[test]
fn profiling_again_returns_to_the_thread_list() {
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.switch(FLAME);
    a.flame_target = a.flame_subjects().first().cloned();
    assert!(!a.flame_picking());
    key(&mut a, KeyCode::Char('P'));
    assert!(
        a.flame_picking(),
        "P offers the list again once one is chosen"
    );
}

#[test]
fn the_timeline_trades_resolution_for_reach_rather_than_dropping_history() {
    // Frames on a busy host are megabytes, so a fixed budget spent at one per
    // second reaches only seconds. Thinning must keep the span, not the tail.
    let mut a = App::new(model::demo());
    let mut snapshot = model::demo();
    // Make each frame expensive enough that the budget binds quickly.
    let seed = snapshot.telemetry.tasks[0].clone();
    snapshot.telemetry.tasks = (0..4000)
        .map(|i| {
            let mut t = seed.clone();
            t.pid = 1000 + i;
            t.name = format!("worker-{i}-with-a-name-long-enough-to-cost-something");
            t
        })
        .collect();
    let base = a.snapshot.telemetry.at_ms + 1000;
    for second in 0..400u64 {
        snapshot.telemetry.at_ms = base + second * 1000;
        a.update(snapshot.clone());
    }
    let span = a.timeline_span().expect("frames were retained");
    assert!(
        span.1.saturating_sub(span.0) >= 300_000,
        "the timeline should still reach minutes back, spans {:?}",
        span
    );
    // The most recent history keeps its per-second resolution.
    assert_eq!(
        span.1,
        base + 399_000,
        "the newest frame is the one just recorded"
    );
    assert!(span.0 < span.1, "the span runs oldest to newest");
}

#[test]
fn the_picker_offers_only_subjects_a_capture_could_succeed_on() {
    let mut a = App::new(model::demo());
    a.snapshot.telemetry.profile = Default::default();
    a.switch(FLAME);
    let subjects = a.flame_subjects();
    assert!(!subjects.is_empty());
    // Stacks are walked in user space, so a kernel thread can never yield one.
    let kernel: Vec<&str> = a
        .snapshot
        .telemetry
        .tasks
        .iter()
        .filter(|t| t.kernel_thread)
        .map(|t| t.name.as_str())
        .collect();
    for subject in &subjects {
        assert!(
            !kernel.contains(&subject.name.as_str()),
            "{} is a kernel thread and cannot be profiled",
            subject.name
        );
    }
    // Processes by default, one row each, busiest first.
    let mut seen = std::collections::BTreeSet::new();
    for subject in &subjects {
        assert!(seen.insert(subject.tgid), "a process is listed once");
    }
    for pair in subjects.windows(2) {
        assert!(pair[0].cpu_pct >= pair[1].cpu_pct, "busiest first");
    }
}

#[test]
fn listing_threads_is_a_mode_of_the_same_picker() {
    let mut a = App::new(model::demo());
    a.snapshot.telemetry.profile = Default::default();
    a.switch(FLAME);
    let processes = a.flame_subjects().len();
    a.mode = 1;
    let threads = a.flame_subjects().len();
    assert!(
        threads >= processes,
        "every process has at least one thread ({threads} vs {processes})"
    );
    let screen = render(&a, 160, 52);
    assert!(screen.contains("choose a thread to profile"));
}

#[test]
fn a_multi_thread_process_says_which_thread_it_captured() {
    let mut a = App::new(model::demo());
    a.snapshot.telemetry.profile = Default::default();
    a.snapshot.demo = false;
    a.switch(FLAME);
    let multi = a.flame_subjects().into_iter().find(|s| s.threads > 1);
    let Some(subject) = multi else {
        return; // the fixture may be single-threaded
    };
    a.flame_target = Some(subject.clone());
    let screen = render(&a, 160, 52);
    assert!(
        screen.contains("the rest are not captured"),
        "a process captured through one thread must not imply it captured them all"
    );
}

#[test]
fn a_finished_capture_does_not_claim_it_never_started() {
    use kernwatch::domain::Quality;
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.snapshot.telemetry.profile = Default::default();
    a.switch(FLAME);
    a.flame_target = a.flame_subjects().first().cloned();
    a.snapshot
        .telemetry
        .capabilities
        .insert("syscalls".into(), Quality::Stopped);
    // A capture that actually ran identified itself.
    a.snapshot.telemetry.details.insert(
        "probe.capture_id".into(),
        vec![("id".into(), "1789".into())],
    );
    let screen = render(&a, 160, 52);
    assert!(
        screen.contains("finished"),
        "a capture that ran and stopped has finished, not gone missing"
    );
    assert!(!screen.contains("Nothing has been captured yet"));
}

#[test]
fn a_running_capture_shows_how_far_through_it_is() {
    use kernwatch::domain::Quality;
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.switch(FLAME);
    a.flame_target = a.flame_subjects().first().cloned();
    a.snapshot
        .telemetry
        .capabilities
        .insert("syscalls".into(), Quality::Available);
    a.snapshot.telemetry.details.insert(
        "probe.progress".into(),
        vec![
            ("elapsed seconds".into(), "18".into()),
            ("duration seconds".into(), "30".into()),
        ],
    );
    let screen = render(&a, 160, 52);
    assert!(
        screen.contains("18s of 30s"),
        "a quiet capture must be distinguishable from a broken one"
    );
}

#[test]
fn stopping_a_capture_works_from_the_view_that_shows_it() {
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.switch(FLAME);
    key(&mut a, KeyCode::Char('x'));
    assert_eq!(
        a.probe_request.as_deref(),
        Some("stop"),
        "the panel offers x, so x must stop the capture here"
    );
}

#[test]
fn the_view_says_what_its_stacks_do_not_cover() {
    let mut a = App::new(model::demo());
    a.switch(FLAME);
    let screen = render(&a, 160, 52);
    assert!(
        screen.contains("syscall entry"),
        "a syscall-entry profile must not be read as a CPU profile"
    );
}

#[test]
fn a_requested_capture_is_not_reported_as_one_that_finished() {
    use kernwatch::domain::Quality;
    // Between the request and the probe attaching the capability reads
    // Stopped, and briefly claimed the capture had finished.
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.snapshot.telemetry.profile = Default::default();
    a.switch(FLAME);
    a.flame_target = a.flame_subjects().first().cloned();
    a.snapshot
        .telemetry
        .capabilities
        .insert("syscalls".into(), Quality::Stopped);
    a.snapshot.telemetry.details.remove("probe.capture_id");
    let screen = render(&a, 160, 52);
    assert!(screen.contains("attaching"));
    assert!(
        !screen.contains("finished without collecting"),
        "nothing has run yet, so nothing has finished"
    );
}

#[test]
fn searching_marks_a_frame_everywhere_it_is_called_from() {
    let mut a = App::new(model::demo());
    a.switch(FLAME);
    // aesni_ctr32 is reached through both encrypt and decrypt in the fixture.
    a.filter = "aesni".into();
    let screen = render(&a, 160, 52);
    assert!(
        screen.contains("frames matching"),
        "a search should say what it found"
    );
    let profile = &a.snapshot.telemetry.profile;
    let reached: Vec<&str> = profile
        .root
        .children
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert!(!reached.is_empty());
    // Clearing the search leaves the profile as it was.
    a.filter.clear();
    let plain = render(&a, 160, 52);
    assert!(!plain.contains("frames matching"));
}
