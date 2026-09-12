use kernwatch::{
    app::App,
    flame::{ComparisonMode, Kind, Profile},
    model,
    symbols::demangle,
};
fn profile(paths: &[(&str, u64)]) -> Profile {
    let mut p = Profile::new("test CPU");
    p.metadata.kind = Kind::Cpu;
    p.metadata.unit = "cpu_samples".into();
    for (path, n) in paths {
        p.add(&path.split(';').map(str::to_owned).collect::<Vec<_>>(), *n);
    }
    p.sort();
    p
}
#[test]
fn comparisons_keep_removed_paths_and_normalize_only_when_requested() {
    let before = profile(&[("main;old", 20), ("main;common", 80)]);
    let after = profile(&[("main;new", 40), ("main;common", 160)]);
    let raw = after.compare(&before, ComparisonMode::Counts).unwrap();
    assert_eq!(
        raw.changes
            .iter()
            .find(|c| c.path.last().unwrap() == "old")
            .unwrap()
            .delta,
        -20.
    );
    assert_eq!(
        raw.changes
            .iter()
            .find(|c| c.path.last().unwrap() == "common")
            .unwrap()
            .delta,
        80.
    );
    let share = after.compare(&before, ComparisonMode::Share).unwrap();
    assert_eq!(
        share
            .changes
            .iter()
            .find(|c| c.path.last().unwrap() == "common")
            .unwrap()
            .delta,
        0.
    );
    assert!(before
        .compare(&before, ComparisonMode::Counts)
        .unwrap()
        .changes
        .iter()
        .all(|c| c.delta == 0.));
}
#[test]
fn comparison_rejects_unknown_or_incompatible_evidence() {
    let cpu = profile(&[("main", 1)]);
    let mut other = cpu.clone();
    other.metadata.kind = Kind::Unknown;
    assert!(cpu.compare(&other, ComparisonMode::Counts).is_err());
    other.metadata.kind = Kind::Syscalls;
    assert!(cpu.compare(&other, ComparisonMode::Counts).is_err());
    other.metadata.kind = Kind::Cpu;
    other.metadata.unit = "nanoseconds".into();
    assert!(cpu.compare(&other, ComparisonMode::Counts).is_err());
}
#[test]
fn user_quality_does_not_count_domain_or_kernel_frames() {
    let mut p = profile(&[]);
    p.observe(
        &["user".into()],
        &["kernel1".into(), "kernel2".into(), "kernel3".into()],
        7,
    );
    p.observe(&[], &["kernel".into()], 3);
    p.observe(&[], &[], 2);
    assert_eq!(p.shallow, 7);
    assert_eq!(p.shallow_share(), Some(100.));
    assert_eq!(p.root.samples, 10);
    assert_eq!(p.quality.failed, 2);
    assert_eq!(p.quality.partial, 3);
    let deep = vec!["frame".into(); 127];
    p.observe(&deep, &[], 5);
    assert_eq!(p.quality.depth_limit, 5);
}
#[test]
fn old_profiles_decode_with_unknown_provenance() {
    let p:Profile=serde_json::from_str(r#"{"root":{"name":"all","samples":1,"children":[]},"source":"old","shallow":0,"unresolved":0}"#).unwrap();
    assert_eq!(p.metadata.kind, Kind::Unknown);
    assert_eq!(p.quality.attempted, 0);
}
#[test]
fn demangling_parses_rust_and_cpp_and_preserves_invalid_input() {
    assert_eq!(demangle("_ZN4test4work17h0123456789abcdefE"), "test::work");
    assert_eq!(demangle("_Z3fooi"), "foo(int)");
    assert_eq!(demangle("_RNvCs_malformed"), "_RNvCs_malformed");
    assert_eq!(demangle("0x1234"), "0x1234");
}
#[test]
fn live_picker_profiles_whole_process_and_can_select_syscalls() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut snapshot = model::demo();
    snapshot.demo = false;
    snapshot.telemetry.profile = Profile::default();
    let mut a = App::new(snapshot);
    a.switch(13);
    let tgid = a.flame_subjects()[0].tgid;
    a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        a.probe_request,
        Some(format!("cpu tgid={tgid} seconds=30 hz=49"))
    );
    a.key(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::NONE));
    a.key(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::NONE));
    let tid = a.flame_subjects()[0].tid;
    a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        a.probe_request,
        Some(format!("syscalls pid={tid} seconds=30 stack"))
    );
}
#[test]
fn baseline_exports_are_complete_manifested_and_reloadable() {
    let mut a = App::new(model::demo());
    a.snapshot.telemetry.profile = profile(&[("main;before", 2)]);
    a.execute("profile-baseline");
    a.snapshot.telemetry.profile = profile(&[("main;after", 3)]);
    a.execute("profile-diff share");
    let path = a.export().unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path.join("manifest.json")).unwrap()).unwrap();
    for name in [
        "profile.json",
        "baseline.profile.json",
        "baseline.stacks.folded",
        "comparison.json",
    ] {
        assert!(manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["file"] == name));
    }
    a.execute(&format!(
        "profile-baseline {}",
        path.join("baseline.profile.json").display()
    ));
    assert_eq!(a.flame_baseline.as_ref().unwrap().root.samples, 2);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn differential_graph_preserves_removed_frames_and_scrolls() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};
    let mut a = App::new(model::demo());
    a.snapshot.telemetry.profile = profile(&[("main;removed", 20), ("main;kept", 80)]);
    a.execute("profile-baseline");
    a.snapshot.telemetry.profile = profile(&[("main;added", 40), ("main;kept", 60)]);
    a.execute("profile-diff counts");
    let union = a
        .snapshot
        .telemetry
        .profile
        .comparison_union(a.flame_baseline.as_ref().unwrap());
    assert_eq!(union.root.samples, 140);
    for (w, h) in [(80, 24), (160, 52)] {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| kernwatch::ui::draw(f, &a)).unwrap();
        let screen = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(screen.contains("removed"));
        assert!(screen.contains("added"));
    }
    a.key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    assert_eq!(a.scroll, 10);
    a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(a.flame_compare.is_none());
}
#[test]
fn readable_export_does_not_corrupt_delimiters_or_merge_identities() {
    use kernwatch::flame::FrameInfo;
    let mut p = profile(&[]);
    for id in ["image1!work", "image2!work"] {
        p.frames.insert(
            id.into(),
            FrameInfo {
                display: "work;with\nseparator".into(),
                ..Default::default()
            },
        );
        p.add(&[id.into()], 1);
    }
    assert_eq!(p.root.children.len(), 2);
    assert_eq!(
        p.folded(),
        vec!["work:with separator 1", "work:with separator 1"]
    );
}
