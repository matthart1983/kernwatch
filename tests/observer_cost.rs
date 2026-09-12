use kernwatch::{
    app::App,
    domain::{Sample, Samples, Series},
    flame::{ComparisonMode, Kind, Profile},
    model,
};

#[test]
fn shared_history_retains_versions_and_flat_wire_format_across_wraps() {
    let mut series = Series::default();
    for i in 0..600 {
        series.push(i * 1000, Some(i as f64));
    }
    let frozen = series.clone();
    let bytes = series.samples.bytes();
    for i in 600..3600 {
        let previous = series.clone();
        series.push(i * 1000, (i % 7 != 0).then_some(i as f64));
        assert_eq!(previous.samples.last().unwrap().at_ms, (i - 1) * 1000);
        assert_eq!(series.samples.len(), 600);
        assert_eq!(series.samples[0].at_ms, (i - 599) * 1000);
        assert!(series.samples.bytes() <= bytes + 2048);
    }
    assert_eq!(frozen.samples[0].at_ms, 0);
    assert_eq!(frozen.samples.last().unwrap().at_ms, 599000);
    let wire = serde_json::to_value(&series.samples).unwrap();
    assert!(wire.is_array());
    let restored: Samples = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(restored).unwrap(), wire);
    assert_eq!(series.window(3599000, 10).len(), 11);
}

#[test]
fn partial_chunks_and_mutable_fixture_edits_do_not_change_other_versions() {
    let mut values: Samples = (0..45)
        .map(|at_ms| Sample {
            at_ms,
            value: Some(1.),
        })
        .collect();
    let saved = values.clone();
    for _ in 0..37 {
        values.pop_front();
    }
    for v in values.iter_mut() {
        v.value = None;
    }
    values.push(Sample {
        at_ms: 45,
        value: Some(2.),
    });
    assert_eq!(values[0].at_ms, 37);
    assert_eq!(values[8].value, Some(2.));
    assert!(saved.iter().all(|s| s.value == Some(1.)));
    values.retain(|s| s.at_ms >= 40);
    assert_eq!(values[0].at_ms, 40);
}

fn profile() -> Profile {
    let mut p = Profile::new("test");
    p.metadata.kind = Kind::Cpu;
    p.metadata.unit = "samples".into();
    p.add(&["main".into(), "work".into()], 10);
    p
}
#[test]
fn comparison_cache_tracks_direct_mutations_and_baseline_replacement() {
    let mut a = App::new(model::demo());
    a.snapshot.telemetry.profile = profile();
    let mut baseline = profile();
    let current = a.snapshot.telemetry.profile.clone();
    {
        let first = a
            .prepared_comparison(&baseline, ComparisonMode::Counts)
            .unwrap();
        assert!(first.comparison.changes.iter().all(|c| c.delta == 0.));
    }
    a.snapshot.telemetry.profile.root.samples += 10;
    a.snapshot.telemetry.profile.root.children[0].samples += 10;
    a.snapshot.telemetry.profile.root.children[0].children[0].samples += 10;
    assert_eq!(current.root.samples, 10);
    assert!(a
        .prepared_comparison(&baseline, ComparisonMode::Counts)
        .unwrap()
        .comparison
        .changes
        .iter()
        .all(|c| c.delta == 10.));
    assert!(a
        .prepared_comparison(&baseline, ComparisonMode::Share)
        .unwrap()
        .comparison
        .changes
        .iter()
        .all(|c| c.delta == 0.));
    baseline.add(&["removed".into()], 5);
    let prepared = a
        .prepared_comparison(&baseline, ComparisonMode::Counts)
        .unwrap();
    assert!(prepared.union.root.at(&["removed".into()]).is_some());
    assert!(prepared
        .comparison
        .changes
        .iter()
        .any(|c| c.path == ["removed"] && c.delta == -5.));
}

#[test]
fn freeze_and_history_keep_latest_without_mutating_held_evidence() {
    let mut s = model::demo();
    s.demo = false;
    s.telemetry.at_ms = 1000;
    s.telemetry.profile = profile();
    let mut a = App::new(s.clone());
    a.frozen = true;
    s.telemetry.at_ms = 2000;
    s.telemetry.profile.add(&["later".into()], 7);
    a.update(s.clone());
    assert_eq!(a.snapshot.telemetry.at_ms, 1000);
    assert_eq!(a.snapshot.telemetry.profile.root.samples, 10);
    a.seek_time(2000);
    assert_eq!(a.snapshot.telemetry.profile.root.samples, 17);
    a.frozen = false;
    a.seek_time(1000);
    assert_eq!(a.snapshot.telemetry.at_ms, 1000);
    s.telemetry.at_ms = 3000;
    a.update(s);
    assert_eq!(a.snapshot.telemetry.at_ms, 1000);
    a.seek_time(3000);
    assert_eq!(a.snapshot.telemetry.at_ms, 3000);
    assert!(a.time_cursor.is_none());
}

#[test]
fn retained_budget_includes_long_symbols() {
    let mut s = model::demo();
    let before = App::frame_bytes(&s);
    s.telemetry.profile.frames.insert(
        "long".into(),
        kernwatch::flame::FrameInfo {
            display: "x".repeat(1_000_000),
            ..Default::default()
        },
    );
    assert!(App::frame_bytes(&s) >= before + 1_000_000);
}

#[test]
fn inspector_demand_uses_real_tab_and_identity_and_replay_disables_it() {
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    for (tab, kind) in [(4, "device"), (7, "cgroup"), (8, "module")] {
        a.switch(tab);
        a.detail = true;
        let d = a.collection_demand();
        assert!(match kind {
            "device" => d.device.is_some(),
            "cgroup" => d.cgroup.is_some(),
            _ => d.module.is_some(),
        });
    }
    a.frozen = true;
    assert_eq!(a.collection_demand(), Default::default());
}
