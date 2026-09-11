use kernwatch::{app::App, model, ui};
use ratatui::{backend::TestBackend, Terminal};
#[test]
fn all_screens_render_at_breakpoints() {
    for (w, h) in [
        (79, 23),
        (80, 24),
        (110, 32),
        (120, 40),
        (160, 52),
        (160, 68),
        (240, 80),
    ] {
        for tab in 0..13 {
            let mut a = App::new(model::demo());
            a.switch(tab);
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|f| ui::draw(f, &a)).unwrap();
        }
    }
}
#[test]
fn dense_has_all_eight_panels_and_real_graphs() {
    let a = App::new(model::demo());
    let mut t = Terminal::new(TestBackend::new(160, 52)).unwrap();
    t.draw(|f| ui::draw(f, &a)).unwrap();
    let text = t
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    for name in [
        "cpu",
        "mem",
        "block",
        "irq / softirq",
        "sched",
        "taint",
        "tasks by concern",
        "timeline",
    ] {
        assert!(text.contains(name), "missing {name}");
    }
    assert!(text.chars().any(|c| ('\u{2801}'..='\u{28ff}').contains(&c)));
}
#[test]
fn diagnose_contains_graph_and_workflow_panels() {
    let mut a = App::new(model::demo());
    a.switch(11);
    let mut t = Terminal::new(TestBackend::new(160, 50)).unwrap();
    t.draw(|f| ui::draw(f, &a)).unwrap();
    let text = t
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    for name in [
        "issues",
        "cause chain / evidence",
        "remediation / verification",
        "report preview",
        "IRQ placement",
        "NET_RX 91%",
        "Envoy pinned",
        "Storage coupling?",
    ] {
        assert!(text.contains(name), "missing {name}");
    }
}

#[test]
fn every_focus_expands_at_compact_size() {
    for tab in 0..13 {
        let mut app = App::new(model::demo());
        app.switch(tab);
        for focus in 0..app.panel_count() {
            app.focus = focus;
            app.expanded = true;
            app.detail = true;
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal.draw(|f| ui::draw(f, &app)).unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(text.contains("Esc"), "tab {tab} focus {focus}");
        }
    }
}

#[test]
fn reference_buffers_match_reviewed_captures() {
    for (i, name) in [
        "dense",
        "overview",
        "tasks",
        "scheduler",
        "memory",
        "block",
        "syscalls",
        "irq",
        "cgroups",
        "modules",
        "ebpf",
        "dmesg",
        "diagnose",
    ]
    .iter()
    .enumerate()
    {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("screenshots/current/{i:02}-{name}.json"));
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let w = expected["width"].as_u64().unwrap() as u16;
        let h = expected["height"].as_u64().unwrap() as u16;
        let mut a = App::new(model::demo());
        a.switch(if i == 0 { 12 } else { i - 1 });
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| ui::draw(f, &a)).unwrap();
        let cells = expected["cells"].as_array().unwrap();
        for (index, cell) in terminal.backend().buffer().content.iter().enumerate() {
            assert_eq!(
                cell.symbol(),
                cells[index][0].as_str().unwrap(),
                "{name} cell {},{} changed; review capture before accepting",
                index % w as usize,
                index / w as usize
            );
            if let ratatui::style::Color::Rgb(r, g, b) = cell.fg {
                assert_eq!(
                    format!("#{r:02x}{g:02x}{b:02x}"),
                    cells[index][1].as_str().unwrap(),
                    "{name} foreground changed"
                );
            }
        }
    }
}

#[test]
fn dense_keeps_irq_and_scheduler_visible_without_tracing_at_compact_and_full_sizes() {
    for (w, h) in [(80, 24), (100, 30), (160, 52)] {
        let mut a = App::new(model::demo());
        a.switch(12);
        a.snapshot.demo = false;
        a.snapshot.telemetry.metrics.clear();
        a.snapshot.telemetry.series.clear();
        for (key, value) in [
            ("irq.rate", 42.),
            ("sched.switches", 73.),
            ("runqueue", 2.),
            ("psi.cpu", 3.),
        ] {
            a.snapshot.telemetry.record(key, Some(value), "/s", 100.);
            a.snapshot.telemetry.metrics.insert(
                key.into(),
                kernwatch::domain::Measurement::known(value, "/s", "test live counter", a.cursor()),
            );
        }
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| ui::draw(f, &a)).unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        for expected in [
            "irq / softirq",
            "sched",
            "IRQ/s 42",
            "switches/s 73",
            "runnable/CPU",
        ] {
            assert!(text.contains(expected), "{w}x{h} missing {expected}");
        }
    }
}

#[test]
fn syscall_histogram_and_bpf_metadata_never_use_another_subject() {
    let mut a = App::new(model::demo());
    a.switch(5);
    let name = a.rows()[a.selected][0].clone();
    a.snapshot
        .telemetry
        .histograms
        .remove(&format!("syscall:{name}"));
    for expanded in [false, true] {
        a.expanded = expanded;
        a.focus = 2;
        let mut terminal = Terminal::new(TestBackend::new(160, 60)).unwrap();
        terminal.draw(|f| ui::draw(f, &a)).unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("Histogram acquisition unavailable"));
    }
    a.expanded = false;
    a.switch(9);
    let id = a.rows()[a.selected][0].clone();
    a.snapshot.telemetry.details.remove(&format!("bpf:{id}"));
    a.snapshot.telemetry.details.insert(
        "bpf".into(),
        vec![("wrong".into(), "UNRELATED PROGRAM".into())],
    );
    let mut terminal = Terminal::new(TestBackend::new(160, 60)).unwrap();
    terminal.draw(|f| ui::draw(f, &a)).unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(!text.contains("UNRELATED PROGRAM"));
    assert!(text.contains("Selected source detail unavailable"));
}
