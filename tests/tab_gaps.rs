use kernwatch::{
    actions::{DemoHost, Plan},
    app::App,
    domain::*,
    model, ui,
};
use ratatui::{backend::TestBackend, Terminal};

fn screen(a: &App, w: u16, h: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| ui::draw(f, a)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[test]
fn verification_does_not_mutate_an_unrelated_action() {
    let mut a = App::new(model::demo());
    a.switch(11);
    let host = DemoHost::default();
    let target = std::path::PathBuf::from("fixture-target");
    host.0.borrow_mut().insert(target.clone(), "old".into());
    let mut plan = Plan::preview(&host, target, "new".into(), "test".into()).unwrap();
    plan.applied = true;
    plan.issue_id = Some("different-issue".into());
    plan.verification = Some("original verification".into());
    a.action_journal.push(plan);
    a.snapshot.telemetry.issues = vec![Issue {
        id: "sched".into(),
        subject: "cpu0".into(),
        ..Default::default()
    }];
    a.execute("verify");
    assert_eq!(
        a.action_journal[0].verification.as_deref(),
        Some("original verification")
    );
    assert!(a.status.contains("Verification"));
}

#[test]
fn unknown_cgroup_memory_roundtrips_without_becoming_zero() {
    let group = Cgroup::default();
    assert_eq!(model::cgroup_row(&group)[5], "—");
    let encoded = serde_json::to_string(&group).unwrap();
    let decoded: Cgroup = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.memory_bytes, None);
    let zero = Cgroup {
        memory_bytes: Some(0),
        ..Default::default()
    };
    assert_eq!(model::cgroup_row(&zero)[5], "0");
    // Older recordings stored an integer; Option accepts that representation.
    let legacy = encoded.replace("\"memory_bytes\":null", "\"memory_bytes\":42");
    assert_eq!(
        serde_json::from_str::<Cgroup>(&legacy)
            .unwrap()
            .memory_bytes,
        Some(42)
    );
}

#[test]
fn compact_uses_real_issue_and_memory_rows() {
    let mut a = App::new(model::demo());
    a.snapshot.telemetry.issues = vec![Issue {
        title: "actual typed finding".into(),
        ..Default::default()
    }];
    a.switch(11);
    assert!(screen(&a, 80, 24).contains("actual typed finding"));
    a.switch(3);
    assert!(screen(&a, 80, 24).contains("PSS MiB"));
}

#[test]
fn memory_sort_does_not_substitute_rss_for_missing_pss() {
    let mut a = App::new(model::demo());
    a.switch(3);
    a.snapshot.telemetry.tasks = vec![
        Task {
            name: "unknown".into(),
            pid: 1,
            tgid: 1,
            rss_bytes: 10000,
            ..Default::default()
        },
        Task {
            name: "measured".into(),
            pid: 2,
            tgid: 2,
            rss_bytes: 100,
            pss_bytes: Some(50),
            ..Default::default()
        },
    ];
    a.execute("sort PSS");
    assert_eq!(a.memory_tasks()[0].pid, 2);
    a.execute("sort RSS");
    assert_eq!(a.memory_tasks()[0].pid, 1);
}

#[test]
fn scheduler_subject_history_includes_task_generation() {
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.switch(2);
    a.mode = 1;
    a.snapshot.telemetry.tasks = vec![Task {
        pid: 123,
        start_ticks: 456,
        ..Default::default()
    }];
    assert_eq!(a.scheduler_subjects()[0].1, "task.123.456");
}

#[test]
fn cgroup_controller_modes_filter_actual_availability() {
    let mut a = App::new(model::demo());
    a.switch(7);
    a.snapshot.telemetry.cgroups = vec![
        Cgroup {
            path: "/memory".into(),
            memory_bytes: Some(0),
            ..Default::default()
        },
        Cgroup {
            path: "/io".into(),
            fields: vec![("io.stat".into(), "".into())],
            ..Default::default()
        },
    ];
    a.mode = 5;
    assert_eq!(a.rows().len(), 1);
    assert_eq!(a.rows()[0][0], "/memory");
    a.mode = 6;
    assert_eq!(a.rows().len(), 1);
    assert_eq!(a.rows()[0][0], "/io");
}

#[test]
fn missing_module_trust_metadata_stays_unknown() {
    let module = Module {
        taint: "unavailable: permission denied".into(),
        ..Default::default()
    };
    let row = model::module_row(&module);
    assert_eq!(&row[4..6], &["unknown", "unknown"]);
}

#[test]
fn outstanding_scheduler_wait_completes_and_is_invalidated_by_loss() {
    use kernwatch::tracing::{Correlator, TraceEvent};
    let mut c = Correlator::default();
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as u64;
    let mut t = Telemetry {
        at_ms: 1020,
        tasks: vec![Task {
            pid: 7,
            start_ticks: hz,
            cpu: 2,
            ..Default::default()
        }],
        ..Default::default()
    };
    c.task_starts.insert(7, 1_000_000_000);
    let event = |ns, name: &str, payload: &str| TraceEvent {
        at_ns: ns,
        cpu: 2,
        pid: 7,
        name: name.into(),
        payload: payload.into(),
    };
    c.consume(event(1_010_000_000, "sched_wakeup", "pid=7"));
    c.apply(&mut t);
    assert_eq!(t.runnable_waits.len(), 1);
    assert_eq!(t.runnable_waits[0].age_ms, 10.);
    c.consume(event(
        1_020_000_000,
        "sched_switch",
        "next_pid=7 prev_pid=0",
    ));
    c.apply(&mut t);
    assert!(t.runnable_waits.is_empty());
    c.consume(event(1_021_000_000, "sched_wakeup", "pid=7"));
    c.loss(1);
    c.apply(&mut t);
    assert!(t.runnable_waits.is_empty());
}

#[test]
fn every_table_row_matches_its_column_count() {
    // A short row silently shifts every later cell into the wrong column, which
    // is how a failure string once landed under the eBPF creator heading.
    let snapshot = model::demo();
    for (tab, view) in snapshot.views.iter().enumerate() {
        for (index, row) in view.rows.iter().enumerate() {
            assert_eq!(
                row.len(),
                view.columns.len(),
                "view {tab} row {index} has {} fields for {} columns: {row:?}",
                row.len(),
                view.columns.len()
            );
        }
    }
}

#[test]
fn a_denied_bpf_inventory_reports_the_permission_and_adds_no_rows() {
    let mut a = App::new(model::demo());
    a.switch(9);
    a.snapshot.demo = false;
    a.snapshot.views[9].rows.clear();
    a.snapshot.telemetry.capabilities.insert(
        "bpf".into(),
        Quality::Denied("program inventory requires CAP_BPF or root".into()),
    );
    let screen = screen(&a, 160, 40);
    assert!(
        screen.contains("requires CAP_BPF or root"),
        "the denial should name the permission it needs"
    );
    assert!(
        !screen.contains("bpf_prog_get_next_id"),
        "the refusing syscall belongs in the status detail, not the headline"
    );
    assert!(a.rows().is_empty(), "a denial must not synthesize a row");
}
