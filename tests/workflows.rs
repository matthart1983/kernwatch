#![cfg(target_os = "linux")]
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kernwatch::{app::App, domain::*, model, recording};
fn key(a: &mut App, k: KeyCode) {
    a.key(KeyEvent::new(k, KeyModifiers::NONE));
}
#[test]
fn recording_recovers_partial_tail_and_rejects_bad_version() {
    let path = std::env::temp_dir().join(format!("kernwatch-test-{}.kwr", recording::stamp()));
    {
        let mut recorder = recording::Recorder::create(path.clone()).unwrap();
        for at in [1000, 2000] {
            let mut s = model::demo();
            s.telemetry.at_ms = at;
            recorder.push(&s).unwrap();
        }
    }
    assert_eq!(recording::read(&path).unwrap().len(), 2);
    use std::io::Write;
    {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(&100u32.to_le_bytes()).unwrap();
        file.write_all(b"{partial").unwrap();
    }
    let frames = recording::read(&path).unwrap();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[1].telemetry.at_ms, 2000);
    std::fs::write(&path, b"INVALID!").unwrap();
    assert!(recording::read(&path).is_err());
    std::fs::remove_file(path).unwrap();
}
#[test]
fn replay_clock_and_seek_change_the_entire_snapshot() {
    let mut a = App::new(model::demo());
    let mut second = model::demo();
    second.telemetry.at_ms += 1000;
    second.telemetry.tasks[0].name = "second-frame".into();
    a.replay = Some(vec![a.snapshot.clone(), second]);
    a.execute("speed 2");
    a.execute("play");
    a.tick(499);
    assert_eq!(a.replay_index, 0);
    a.tick(1);
    assert_eq!(a.replay_index, 1);
    assert_eq!(a.snapshot.telemetry.tasks[0].name, "second-frame");
    assert!(!a.replay_playing);
    key(&mut a, KeyCode::Left);
    assert_eq!(a.replay_index, 0);
    assert_ne!(a.snapshot.telemetry.tasks[0].name, "second-frame");
}
#[test]
fn selected_identity_survives_reordering_but_not_pid_reuse() {
    let mut a = App::new(model::demo());
    a.switch(1);
    a.execute("filter envoy");
    a.selected = 1;
    let identity = a.selected_task().map(|t| (t.pid, t.start_ticks)).unwrap();
    let mut next = a.snapshot.clone();
    next.telemetry.tasks.reverse();
    a.update(next);
    assert_eq!(
        a.selected_task().map(|t| (t.pid, t.start_ticks)),
        Some(identity)
    );
    key(&mut a, KeyCode::Enter);
    assert!(a.detail);
    key(&mut a, KeyCode::Esc);
    assert_eq!(a.filter, "envoy");
    let mut next = a.snapshot.clone();
    next.telemetry
        .tasks
        .iter_mut()
        .find(|t| t.pid == identity.0)
        .unwrap()
        .start_ticks += 1;
    a.update(next);
    assert!(a.status.contains("reused"));
    assert!(!a.detail);
}
#[test]
fn cgroup_fold_and_drill_keep_scope() {
    let mut a = App::new(model::demo());
    a.switch(7);
    assert_eq!(a.rows().len(), 3);
    key(&mut a, KeyCode::Char(' '));
    assert_eq!(a.rows().len(), 1);
    key(&mut a, KeyCode::Char(' '));
    assert_eq!(a.rows().len(), 3);
    a.selected = 2;
    let group = a.rows()[2][0].clone();
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.tab, 1);
    assert_eq!(a.filter, group);
    assert!(a.visible_tasks().iter().all(|t| t.cgroup.contains(&group)));
}
#[test]
fn export_manifest_matches_every_payload() {
    let a = App::new(model::demo());
    let path = a.export().unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path.join("manifest.json")).unwrap()).unwrap();
    let entries = manifest["files"].as_array().unwrap();
    assert_eq!(entries.len(), 8);
    assert!(entries.iter().any(|e| e["file"] == "actions.json"));
    assert!(entries.iter().any(|e| e["file"] == "stacks.folded"));
    for entry in entries {
        let file = path.join(entry["file"].as_str().unwrap());
        assert_eq!(
            std::fs::metadata(file).unwrap().len(),
            entry["bytes"].as_u64().unwrap()
        );
    }
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 9);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn missing_time_is_a_gap_instead_of_stretched_history() {
    let series = Series {
        samples: vec![
            Sample {
                at_ms: 0,
                value: Some(1.),
            },
            Sample {
                at_ms: 10000,
                value: Some(2.),
            },
        ],
        ..Default::default()
    };
    let window = series.window(10000, 10);
    assert_eq!(window.len(), 11);
    assert_eq!(window[5], None);
    assert_eq!(window[10], Some(2.));
}
#[test]
fn verification_uses_selected_issue_and_missing_tail_is_inconclusive() {
    let mut a = App::new(model::demo());
    a.switch(11);
    a.selected = 1;
    a.execute("verify");
    assert!(!a.status.contains("sched.p99"));
    let s = Series {
        samples: (0..60)
            .map(|i| Sample {
                at_ms: i * 500,
                value: Some(0.),
            })
            .collect(),
        ..Default::default()
    };
    assert_eq!(
        kernwatch::diagnose::verify(&s, 60000, 1.),
        kernwatch::diagnose::Verdict::Inconclusive
    );
}
#[test]
fn replay_cannot_start_a_host_action() {
    let mut a = App::new(model::demo());
    a.snapshot.demo = false;
    a.replay = Some(vec![a.snapshot.clone()]);
    a.execute("preview irq 47 0-7");
    assert!(a.pending_action.is_none());
    a.execute("apply 123");
    assert!(a.status.contains("read-only"));
}

#[test]
fn recovery_and_quota_fixtures_exercise_verification() {
    let mut a = App::new(model::demo());
    a.switch(11);
    a.execute("scenario recovery");
    a.execute("verify");
    assert!(a.status.contains("Passed"), "{}", a.status);
    a.execute("scenario quota");
    a.execute("verify");
    assert!(a.status.contains("Failed"), "{}", a.status);
    assert_eq!(
        a.snapshot
            .telemetry
            .cgroups
            .iter()
            .find(|g| g.path.ends_with("envoy.service"))
            .unwrap()
            .runtime_pct,
        Some(50.)
    );
}
#[test]
fn clipboard_encoding_and_focus_bounds() {
    assert_eq!(kernwatch::app::clipboard_base64(b"hello"), "aGVsbG8=");
    for tab in 0..13 {
        let mut a = App::new(model::demo());
        a.switch(tab);
        for _ in 0..a.panel_count() {
            key(&mut a, KeyCode::Tab);
        }
        assert_eq!(a.focus, 0);
    }
}

#[test]
fn history_cursor_keeps_tables_and_measurements_together() {
    let mut first = model::demo();
    first.demo = false;
    first.telemetry.at_ms = 1000;
    first.telemetry.tasks[0].name = "old-task".into();
    let mut a = App::new(first);
    let mut second = a.snapshot.clone();
    second.telemetry.at_ms = 2000;
    second.telemetry.tasks[0].name = "new-task".into();
    a.update(second);
    a.seek_time(1000);
    assert_eq!(a.snapshot.telemetry.tasks[0].name, "old-task");
    assert_eq!(a.cursor(), 1000);
    a.seek_time(2000);
    assert_eq!(a.snapshot.telemetry.tasks[0].name, "new-task");
    assert_eq!(a.time_cursor, None);
}

#[test]
fn demo_actions_are_reviewed_simulated_and_reversible() {
    let mut a = App::new(model::demo());
    a.journal_path =
        std::env::temp_dir().join(format!("kernwatch-demo-action-{}.json", recording::stamp()));
    a.execute("preview irq 47 0-2,4-7");
    let plan = a
        .pending_action
        .clone()
        .expect("fixture IRQ is represented");
    assert_eq!(plan.before, "3");
    assert!(a.detail);
    a.execute("apply 0");
    assert!(a.action_journal.is_empty());
    a.execute(&format!("apply {}", plan.id));
    assert!(a.action_journal[0].applied);
    assert!(a.action_journal[0].verified);
    assert!(a.status.contains("DEMO"));
    a.execute("revert");
    assert!(!a.action_journal[0].applied);
    let persisted: Vec<kernwatch::actions::Plan> =
        serde_json::from_slice(&std::fs::read(&a.journal_path).unwrap()).unwrap();
    assert!(persisted[0].outcome.contains("reverted"));
    std::fs::remove_file(&a.journal_path).unwrap();
}

#[test]
fn scheduler_modes_and_block_irq_tour_preserve_subjects() {
    let mut a = App::new(model::demo());
    a.switch(4);
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.rows()[a.selected][0], "52");
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.scheduler_subjects()[a.selected].0, "cpu3");
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.scope_cpu, Some(3));
    key(&mut a, KeyCode::Esc);
    assert_eq!(a.tab, 2);
    assert_eq!(a.scheduler_subjects()[a.selected].0, "cpu3");
    key(&mut a, KeyCode::Char('g'));
    assert!(a.scheduler_subjects()[0].1.starts_with("task."));
    key(&mut a, KeyCode::Char('g'));
    assert!(a.scheduler_subjects()[0].1.starts_with("sched.cgroup"));
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.tab, 1);
    assert_eq!(a.filter, "/");
}

#[test]
fn module_columns_and_selected_bpf_metadata_match_fixture() {
    let mut a = App::new(model::demo());
    a.switch(8);
    assert_eq!(a.rows()[0].len(), model::MODULE_COLUMNS.len());
    assert_eq!(a.rows()[0][4], "yes");
    a.switch(9);
    for row in a.rows() {
        let fields = &a.snapshot.telemetry.details[&format!("bpf:{}", row[0])];
        assert!(fields
            .iter()
            .any(|(key, value)| key == "owner" && value.starts_with(&row[7])));
    }
}

#[test]
fn memory_to_process_capture_preserves_back_route() {
    let mut a = App::new(model::demo());
    a.switch(3);
    a.focus = 0;
    key(&mut a, KeyCode::Enter);
    assert!(a.detail);
    key(&mut a, KeyCode::Esc);
    a.focus = 3;
    a.selected = 1;
    let pid = a.memory_tasks()[1].pid;
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.tab, 1);
    assert_eq!(a.selected_task().unwrap().pid, pid);
    key(&mut a, KeyCode::Char('t'));
    assert_eq!(a.tab, 5);
    assert!(a.palette);
    assert!(a.command.contains(&format!("pid={pid}")));
    assert!(
        a.probe_request.is_none(),
        "inspection does not attach a trace"
    );
    key(&mut a, KeyCode::Esc);
    key(&mut a, KeyCode::Esc);
    assert_eq!(a.tab, 1);
    key(&mut a, KeyCode::Esc);
    assert_eq!(a.tab, 3);
    assert_eq!(a.focus, 3);
    assert_eq!(a.selected, 1);
}

#[test]
fn evidence_drill_restores_issue_selection_and_cursor() {
    let mut a = App::new(model::demo());
    a.switch(11);
    a.focus = 1;
    key(&mut a, KeyCode::Down);
    key(&mut a, KeyCode::Down);
    assert_eq!(a.evidence_index, 2);
    assert_eq!(a.selected, 0);
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.tab, 1);
    assert_eq!(a.filter, "1188");
    assert_eq!(a.cursor(), 522000);
    key(&mut a, KeyCode::Esc);
    assert_eq!(a.tab, 11);
    assert_eq!(a.evidence_index, 2);
    assert_eq!(a.cursor(), 600000);
}

#[test]
fn cgroup_replacement_does_not_reuse_an_open_inspector() {
    let mut a = App::new(model::demo());
    a.switch(7);
    a.detail = true;
    let path = a.rows()[0][0].clone();
    let mut next = a.snapshot.clone();
    next.telemetry
        .cgroups
        .iter_mut()
        .find(|g| g.path == path)
        .unwrap()
        .inode += 1;
    next.telemetry.at_ms += 1000;
    a.update(next);
    assert!(!a.detail);
    assert!(a.status.contains("replaced"));
}

#[test]
fn metric_evidence_routes_to_its_source_screen() {
    for (subject, tab) in [
        ("sched.p99", 2),
        ("throttle:/system.slice/envoy.service", 7),
        ("blocked:1188:1", 1),
        ("irq-placement:47", 6),
        ("module.changed", 8),
        ("bpf.total", 9),
    ] {
        let mut a = App::new(model::demo());
        a.open_subject(subject, 600000);
        assert_eq!(a.tab, tab, "{subject}");
    }
}

#[test]
fn dense_scheduler_task_cgroup_tour_returns_to_each_subject() {
    let mut a = App::new(model::demo());
    a.focus = 4;
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.tab, 2);
    a.selected = a
        .scheduler_subjects()
        .iter()
        .position(|s| s.2 == Some(3))
        .unwrap();
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.tab, 1);
    a.selected = a
        .visible_tasks()
        .iter()
        .position(|t| t.pid == 1188)
        .unwrap();
    key(&mut a, KeyCode::Char('c'));
    assert_eq!(a.tab, 7);
    assert!(a.rows()[a.selected][0].contains("envoy.service"));
    key(&mut a, KeyCode::Esc);
    assert_eq!(a.selected_task().unwrap().pid, 1188);
    assert_eq!(a.scope_cpu, Some(3));
    key(&mut a, KeyCode::Esc);
    assert_eq!(a.tab, 2);
    assert_eq!(a.scheduler_subjects()[a.selected].2, Some(3));
    key(&mut a, KeyCode::Esc);
    assert_eq!(a.tab, 12);
    assert_eq!(a.focus, 4);
}

#[test]
fn syscall_latency_and_stack_controls_use_observed_caller() {
    let mut a = App::new(model::demo());
    a.switch(5);
    a.mode = 2;
    assert!(a
        .rows()
        .iter()
        .all(|r| r[5].trim_end_matches("ms").parse::<f64>().unwrap() > 1.));
    let caller = a.rows()[0][7].clone();
    key(&mut a, KeyCode::Char('u'));
    assert!(a.palette);
    assert_eq!(
        a.command,
        format!("probe syscalls pid={caller} seconds=10 stack")
    );
    assert!(a.probe_request.is_none());
}

#[test]
fn replacement_generations_reset_device_irq_module_and_program_inspection() {
    for tab in [4, 6, 8, 9] {
        let mut a = App::new(model::demo());
        a.switch(tab);
        a.detail = true;
        let id = a.rows()[0][0].clone();
        let mut next = a.snapshot.clone();
        match tab {
            4 => {
                next.telemetry
                    .devices
                    .iter_mut()
                    .find(|d| d.name == id)
                    .unwrap()
                    .major_minor = "259:99".into()
            }
            6 => next.views[6].rows[0][1] = "replacement controller".into(),
            8 => next
                .telemetry
                .modules
                .iter_mut()
                .find(|m| m.name == id)
                .unwrap()
                .fields
                .push(("source version".into(), "replacement".into())),
            9 => next
                .telemetry
                .details
                .get_mut(&format!("bpf:{id}"))
                .unwrap()
                .push(("tag".into(), "replacement".into())),
            _ => unreachable!(),
        }
        next.telemetry.at_ms += 1000;
        a.update(next);
        assert!(!a.detail, "tab {tab}");
        assert!(a.status.contains("replaced"), "{}", a.status);
    }
}

#[test]
fn log_diagnosis_preview_verification_report_tour() {
    let mut a = App::new(model::demo());
    a.switch(10);
    a.selected = a
        .visible_events()
        .iter()
        .position(|e| e.subject == "task:1188" && !e.source.contains("syscall"))
        .unwrap();
    key(&mut a, KeyCode::Char('c'));
    assert_eq!(a.tab, 11);
    a.focus = 2;
    key(&mut a, KeyCode::Enter);
    assert!(a.pending_action.is_some());
    assert!(a.detail);
    assert!(!a.pending_action.as_ref().unwrap().applied);
    key(&mut a, KeyCode::Esc);
    a.execute("scenario recovery");
    a.execute("verify");
    assert!(a.status.contains("Passed"), "{}", a.status);
    let path = a.export().unwrap();
    let report = std::fs::read_to_string(path.join("report.md")).unwrap();
    assert!(report.contains("Scheduler"));
    assert!(report.contains("verification") || report.contains("Verification"));
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn capabilities_inspector_displays_failures_and_closes_cleanly() {
    let mut a = App::new(model::demo());
    a.snapshot.telemetry.capabilities.insert(
        "storage_health".into(),
        Quality::Error("SMART permission denied".into()),
    );
    a.execute("capabilities");
    assert!(a.detail && a.capabilities_view);
    let backend = ratatui::backend::TestBackend::new(100, 35);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| kernwatch::ui::draw(f, &a)).unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("Acquisition status"));
    key(&mut a, KeyCode::Esc);
    assert!(!a.detail && !a.capabilities_view);
}

#[test]
fn latency_shortcut_scopes_capture_to_selected_live_task() {
    let mut a = App::new(model::demo());
    a.switch(1);
    a.snapshot.demo = false;
    let pid = a.selected_task().unwrap().pid;
    key(&mut a, KeyCode::Char('l'));
    assert_eq!(a.probe_request, Some(format!("sched pid={pid} seconds=30")));
    a.probe_request = None;
    a.snapshot.demo = true;
    key(&mut a, KeyCode::Char('l'));
    assert!(a.probe_request.is_none());
}
