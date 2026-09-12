use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kernwatch::{
    app::App,
    model::{self, TABS},
};
fn press(a: &mut App, k: KeyCode) {
    a.key(KeyEvent::new(k, KeyModifiers::NONE));
}
#[test]
fn stable_shortcuts_from_every_view() {
    for start in 0..13 {
        for (target, (_, key)) in TABS.iter().enumerate() {
            let mut a = App::new(model::demo());
            a.switch(start);
            press(&mut a, KeyCode::Char(*key));
            assert_eq!(a.tab, target);
        }
    }
}
#[test]
fn freeze_filter_and_drill() {
    let mut a = App::new(model::demo());
    a.switch(1);
    press(&mut a, KeyCode::Char('/'));
    for c in "envoy".chars() {
        press(&mut a, KeyCode::Char(c));
    }
    press(&mut a, KeyCode::Enter);
    assert_eq!(a.rows().len(), 2);
    press(&mut a, KeyCode::Down);
    press(&mut a, KeyCode::Enter);
    assert!(a.detail);
    press(&mut a, KeyCode::Esc);
    assert!(!a.detail);
    assert_eq!(a.rows().len(), 2);
    press(&mut a, KeyCode::Esc);
    assert_eq!(a.rows().len(), 10);
    press(&mut a, KeyCode::Char('f'));
    let original = a.snapshot.cpu.clone();
    let mut next = model::demo();
    next.cpu = vec![1.];
    a.update(next.clone());
    assert_eq!(a.snapshot.cpu, original);
    press(&mut a, KeyCode::Char('f'));
    a.update(next);
    assert_eq!(a.snapshot.cpu, vec![1.]);
}
#[test]
fn export_has_separate_markdown_headings_and_mode() {
    let a = App::new(model::demo());
    let path = a.export().unwrap();
    let report = std::fs::read_to_string(path.join("report.md")).unwrap();
    assert!(report.contains("\n\n## Observations\n\n"));
    assert!(report.contains("DEMO"));
    assert!(report.contains("hypothesis"));
    let snapshot: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path.join("snapshot.json")).unwrap())
            .unwrap();
    assert_eq!(snapshot["demo"], true);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn cpu_fixture_balances_and_overhead_arithmetic() {
    let s = model::demo();
    let sum: f64 = s.views[1]
        .rows
        .iter()
        .filter(|r| r[2] == "3")
        .map(|r| r[3].parse::<f64>().unwrap())
        .sum();
    assert_eq!(sum, s.cpu[3]);
    for r in &s.views[9].rows {
        let runs = r[3].replace(',', "").parse::<f64>().unwrap();
        let ns = r[4].replace(',', "").parse::<f64>().unwrap();
        let pct = r[5].parse::<f64>().unwrap();
        assert!((runs * ns / 10_000_000. - pct).abs() < 0.001);
    }
}

#[test]
fn trace_hydration_uses_named_quantiles_and_task_identity() {
    let mut s = model::demo();
    let task = s.telemetry.tasks.iter().find(|t| t.pid == 1188).unwrap();
    let start = task.start_ticks;
    s.telemetry.details.insert(
        "wake:1188".into(),
        vec![
            ("start ticks".into(), start.to_string()),
            ("p99".into(), "18".into()),
            ("p50".into(), "4.2".into()),
        ],
    );
    s.telemetry.details.insert(
        "wakecpu:3".into(),
        vec![("p50".into(), "4.2".into()), ("p99".into(), "18".into())],
    );
    kernwatch::probes::hydrate(&mut s);
    let task = s.telemetry.tasks.iter().find(|t| t.pid == 1188).unwrap();
    assert_eq!(task.wake_p50_ms, Some(4.2));
    assert_eq!(task.wake_p99_ms, Some(18.));
    assert_eq!(s.telemetry.cpus[3].wake_p99_ms, Some(18.));
    s.telemetry
        .tasks
        .iter_mut()
        .find(|t| t.pid == 1188)
        .unwrap()
        .start_ticks += 1;
    kernwatch::probes::hydrate(&mut s);
    assert_eq!(
        s.telemetry
            .tasks
            .iter()
            .find(|t| t.pid == 1188)
            .unwrap()
            .wake_p99_ms,
        None
    );
}

#[test]
fn scenarios_reconcile_cpu_and_group_runtime_at_shared_times() {
    for name in ["pre", "incident", "quota", "recovery"] {
        let current = kernwatch::fixture::scenario(name);
        for at in [300000, 522000, current.at_ms] {
            let t = kernwatch::fixture::at_time(name, at);
            for cpu in &t.cpus {
                assert!(
                    (cpu.busy - cpu.user - cpu.kernel - cpu.softirq - cpu.irq - cpu.steal).abs()
                        < 0.001,
                    "{name} CPU{} at {at}",
                    cpu.id
                );
            }
            for group in &t.cgroups {
                let runtime: f64 = t
                    .tasks
                    .iter()
                    .filter(|task| {
                        group.path == "/"
                            || task.cgroup == group.path
                            || task.cgroup.starts_with(&format!("{}/", group.path))
                    })
                    .filter_map(|task| task.cpu_pct)
                    .sum();
                assert!(
                    (group.runtime_pct.unwrap() - runtime).abs() < 0.001,
                    "{name} {} at {at}: group {:?} tasks {runtime}",
                    group.path,
                    group.runtime_pct
                );
            }
            assert!(t.events.iter().all(|e| e.at_ms <= t.at_ms));
            assert!(t
                .issues
                .iter()
                .flat_map(|i| &i.evidence)
                .all(|e| e.at_ms <= t.at_ms));
            if t.at_ms < current.at_ms {
                assert!(
                    t.histograms.is_empty(),
                    "historical distribution must not show future samples"
                );
            }
        }
    }
}

#[test]
fn trace_loss_clears_current_typed_quantiles() {
    let mut s = model::demo();
    s.telemetry.trace_drops = 1;
    kernwatch::probes::hydrate(&mut s);
    assert!(s.telemetry.tasks.iter().all(|t| t.wake_p99_ms.is_none()));
    assert!(s.telemetry.cpus.iter().all(|t| t.wake_p99_ms.is_none()));
}

#[test]
fn affinity_change_rule_requires_matching_placement_and_retains_causal_uncertainty() {
    let s = model::demo();
    let mut t = s.telemetry;
    t.events.clear();
    t.issues.clear();
    t.events.push(kernwatch::domain::Event {
        id: "affinity-test".into(),
        at_ms: 600000,
        source: "IRQ affinity poll".into(),
        severity: "info".into(),
        subject: "irq:47".into(),
        message: "configured placement changed during 599000..600000ms; actor unknown".into(),
    });
    t.details.insert(
        "irq_rows".into(),
        vec![(
            "47".into(),
            serde_json::to_string(&s.views[6].rows[0]).unwrap(),
        )],
    );
    let mut engine = kernwatch::diagnose::Engine::default();
    for at in [600000, 601000, 602000] {
        t.at_ms = at;
        engine.update(&mut t);
    }
    let issue = t
        .issues
        .iter()
        .find(|i| i.id == "irq-placement:47")
        .unwrap();
    assert!(issue.subject.contains("correlation only"));
    assert!(issue.causes.iter().all(|c| !c.observed));
}

#[test]
fn dense_is_zero_and_b_opens_bpf() {
    let mut a = App::new(model::demo());
    press(&mut a, KeyCode::Char('0'));
    assert_eq!(a.tab, 12);
    press(&mut a, KeyCode::Char('b'));
    assert_eq!(a.tab, 9);
}

#[test]
fn syscall_sort_compares_durations_and_stream_matches_exact_names() {
    let mut a = App::new(model::demo());
    a.switch(5);
    a.snapshot.views[5].rows = ["12ms", "900µs", "2ms"]
        .into_iter()
        .enumerate()
        .map(|(i, d)| {
            vec![
                format!("call{i}"),
                "1".into(),
                "0".into(),
                "—".into(),
                d.into(),
                d.into(),
                d.into(),
                "1".into(),
                "1".into(),
                "completed".into(),
            ]
        })
        .collect();
    a.sort = 6; // p99 is column 5; sort indexes are one-based.
    assert_eq!(
        a.rows().iter().map(|r| r[0].as_str()).collect::<Vec<_>>(),
        vec!["call1", "call2", "call0"]
    );
    a.mode = 2;
    assert_eq!(a.rows().len(), 2);
    a.mode = 0;
    a.sort = 0;
    a.snapshot.views[5].rows[0][0] = "read".into();
    a.snapshot.telemetry.events = [
        (1, "read", "info"),
        (3, "pread64", "error"),
        (2, "read", "error"),
    ]
    .into_iter()
    .map(|(at, name, severity)| kernwatch::domain::Event {
        id: at.to_string(),
        at_ms: at,
        source: "syscall trace".into(),
        message: format!("{name} PID=1 ret=-1 duration_ms=2"),
        subject: "task:1".into(),
        severity: severity.into(),
    })
    .collect();
    assert_eq!(
        a.visible_syscall_events()
            .iter()
            .map(|e| e.at_ms)
            .collect::<Vec<_>>(),
        vec![2, 1]
    );
    a.mode = 1;
    a.snapshot.views[5].rows[0][2] = "1".into();
    assert_eq!(a.visible_syscall_events().len(), 1);
}
