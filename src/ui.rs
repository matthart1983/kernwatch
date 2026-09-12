mod views;
mod widgets;
use crate::{app::App, model::TABS};
use ratatui::{prelude::*, widgets::*};
use widgets::*;
pub fn draw(f: &mut Frame, a: &App) {
    let area = f.size();
    f.render_widget(Block::default().style(Style::default().bg(BG).fg(FG)), area);
    if area.width < 80 || area.height < 24 {
        line(
            f,
            area,
            "kernwatch · resize to at least 80 × 24 · q quit",
            CYAN,
        );
        return;
    }
    let parts = vertical(
        area,
        &[
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(16),
            Constraint::Length(2),
        ],
    );
    chrome(f, parts[0], a);
    let verdict = a
        .snapshot
        .telemetry
        .issues
        .first()
        .map(|i| {
            format!(
                " ● {}   {}  ·  {} findings",
                i.title,
                i.subject,
                a.snapshot.telemetry.issues.len()
            )
        })
        .unwrap_or_else(|| {
            a.snapshot
                .findings
                .first()
                .cloned()
                .unwrap_or("Collecting baseline…".into())
        });
    line(f, parts[1], verdict, GOLD);
    if a.help {
        help(f, parts[2]);
    } else if a.palette {
        palette(f, parts[2], a);
    } else if a.detail {
        if a.expanded {
            views::expanded(f, parts[2], a);
        } else {
            detail(f, parts[2], a);
        }
    } else {
        views::draw(f, parts[2], a);
    }
    line(
        f,
        Rect::new(parts[3].x, parts[3].y, parts[3].width, 1),
        if a.filtering {
            format!(" /{}▏  Enter apply · Esc cancel", a.filter)
        } else {
            format!(" {}", a.status)
        },
        if a.filtering { GOLD } else { DIM },
    );
    let context = if a.detail {
        "↑↓ scroll · Esc close"
    } else {
        match a.tab {
            1 => "l latency capture · G group · w watch · a affinity",
            2 => "g scope · M metric · h histogram · z scale",
            3 => "g memory view · U units · ↵ cache/process",
            4 => "↵ queue IRQ · i IRQs",
            5 => "E errors · g latency · u stack · ↵ arguments",
            6 => "↵ effective CPU · a affinity",
            7 => "Space fold · p PSI · u unit · Q quota",
            8 => "B baseline · x mark · ↵ inspector",
            9 => "n probe · ↵ program/maps",
            10 => "t source/time · c diagnosis · ↵ record",
            11 => "Tab evidence/action · ↵ inspect/preview · v verify",
            12 => "l task latency · ↵ drill · [ ] views",
            _ => "↵ drill · [ ] views",
        }
    };
    line(f, Rect::new(parts[3].x, parts[3].y+1, parts[3].width, 1), format!(" {context} · : command · Tab focus · Esc back · / filter · f freeze · r record · e export · ? help"), FG);
    if a.terminal_theme {
        for cell in &mut f.buffer_mut().content {
            cell.bg = if cell.bg == SELECT {
                Color::DarkGray
            } else {
                Color::Reset
            };
            cell.fg = match cell.fg {
                c if c == CYAN => Color::Cyan,
                c if c == GOLD => Color::Yellow,
                c if c == GREEN => Color::Green,
                c if c == RED => Color::Red,
                c if c == PURPLE => Color::Magenta,
                c if c == DIM || c == BORDER => Color::DarkGray,
                _ => Color::White,
            };
        }
    }
}
fn chrome(f: &mut Frame, area: Rect, a: &App) {
    let t = &a.snapshot.telemetry;
    let mut status = format!(
        "{}  {}  {} cpu",
        if a.recording.is_some() {
            "● REC"
        } else if a.replay.is_some() {
            "REPLAY"
        } else {
            ""
        },
        if a.time_cursor.is_some() {
            "HISTORY"
        } else if a.frozen {
            "FROZEN"
        } else if a.snapshot.demo {
            "DEMO"
        } else {
            "LIVE"
        },
        a.snapshot.cpu.len()
    );
    if t.trace_drops > 0 {
        status.push_str(&format!(" lost:{}", t.trace_drops));
    } else if ["scheduler", "irq", "block", "syscalls"]
        .iter()
        .any(|key| t.capabilities.get(*key) == Some(&crate::domain::Quality::Available))
    {
        status.push_str(" trace:on");
    }
    let status_w = (status.chars().count() + 2) as u16;
    let nav_w = area.width.saturating_sub(status_w + 13);
    line(f, Rect::new(area.x, area.y, 12, 1), "◉ kernwatch", CYAN);
    let mut tabs = Vec::new();
    let mut used = 0;
    let order = [12, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 13, 10, 11];
    let position = order.iter().position(|i| *i == a.tab).unwrap_or(0);
    let start = if nav_w < 125 {
        position.saturating_sub(2)
    } else {
        0
    };
    for i in order.iter().skip(start) {
        let (name, key) = TABS[*i];
        let label = format!(
            " {key} {} ",
            if nav_w < 130 && *i != a.tab {
                match *i {
                    0 => "over",
                    2 => "sched",
                    3 => "mem",
                    5 => "sys",
                    7 => "cgrp",
                    8 => "mods",
                    10 => "log",
                    11 => "diag",
                    _ => name,
                }
            } else {
                name
            }
        );
        if used + label.len() > nav_w as usize {
            break;
        }
        used += label.len();
        tabs.push(Span::styled(
            label,
            if *i == a.tab {
                Style::default().fg(FG).bg(SELECT).bold()
            } else {
                Style::default().fg(DIM)
            },
        ));
    }
    f.render_widget(
        Paragraph::new(Line::from(tabs)),
        Rect::new(area.x + 12, area.y, nav_w, 1),
    );
    line(
        f,
        Rect::new(area.right() - status_w, area.y, status_w, 1),
        status,
        if a.frozen { GOLD } else { GREEN },
    );
    line(
        f,
        Rect::new(area.x, area.y + 1, area.width, 1),
        format!(
            " {} · {}   / {}   {}",
            t.hostname,
            t.kernel,
            TABS[a.tab].0,
            if a.routes.is_empty() {
                "".into()
            } else {
                format!(
                    "{} · Esc back",
                    a.routes
                        .iter()
                        .map(|r| TABS[r.tab].0)
                        .collect::<Vec<_>>()
                        .join(" › ")
                )
            }
        ),
        DIM,
    );
}
fn help(f: &mut Frame, area: Rect) {
    let inner = panel(f, area, 0, "Keyboard / investigation", "Esc close", true);
    text(
        f,
        inner,
        [
            "1–9 / 0 / m / d / b  Open a view · [ / ] cycle views",
            "Tab / Shift-Tab       Focus next / previous panel · Alt+1–9 focus directly",
            "↑↓ / j k              Select row · ←→ move timeline cursor",
            "Enter                 Drill selected subject · Esc restore previous context",
            "/                     Filter rows · s choose sort field · S reverse",
            "r                     Start/stop recording · f freeze display · e export incident",
            ":                     Command palette · type a command, Enter to execute",
            "v                     Evaluate selected issue verification",
            "g                     Cycle view mode · Space folds a cgroup subtree",
            "Space                 Inspect selection · p replay play/pause · ? help · q quit",
            "",
            "Palette: view NAME, filter TEXT, record, freeze, export, verify, inspect,",
            "capabilities, probe sched|irq|block|syscalls, stop-probe, replay FILE, baseline, theme",
            "",
            "Every measurement is source-labelled. Demo uses one fixed incident clock.",
            "Actions begin with an explicit dry-run; inspection never mutates host settings.",
        ]
        .iter()
        .map(|s| Line::raw(s.to_string()))
        .collect(),
    );
}
fn palette(f: &mut Frame, area: Rect, a: &App) {
    let inner = panel(
        f,
        area,
        0,
        "Command palette",
        "Enter execute · Esc close",
        true,
    );
    text(
        f,
        inner,
        vec![
            Line::styled(format!(":{}▏", a.command), Style::default().fg(CYAN)),
            Line::raw(""),
            Line::raw("view dense | overview | tasks | scheduler | memory | block | syscalls"),
            Line::raw("view irq | cgroups | modules | ebpf | dmesg | diagnose"),
            Line::raw("filter <text>    record    freeze    export    inspect    verify"),
            Line::raw("preview irq ID CPUS | quota GROUP QUOTA PERIOD | cpuset GROUP CPUS"),
            Line::raw("preview task PID START_TICKS CPUS | rps INTERFACE QUEUE HEX_MASK"),
            Line::raw("play    pause    speed 0.1..16   (replay)"),
            Line::raw("apply REVIEWED_ID   revert   actions   (reviewed changes / journal)"),
            Line::raw("probe sched | offcpu | irq | block | syscalls    stop-probe"),
            Line::raw("replay <file>    live    baseline    theme    scenario quota | incident"),
            Line::raw(""),
            Line::styled(a.status.clone(), Style::default().fg(GOLD)),
        ],
    );
}
fn detail(f: &mut Frame, area: Rect, a: &App) {
    let inner = panel(
        f,
        area,
        0,
        "Selected object / evidence",
        "↑↓ scroll · Esc back",
        true,
    );
    let mut lines = Vec::new();
    if a.capabilities_view {
        lines.push(Line::raw("Acquisition status · current snapshot"));
        for (source, quality) in &a.snapshot.telemetry.capabilities {
            lines.push(Line::styled(source.clone(), Style::default().fg(CYAN)));
            lines.push(Line::raw(format!("{quality:?}")));
            if let Some(fields) = a
                .snapshot
                .telemetry
                .details
                .get(&format!("source:{source}"))
            {
                lines.extend(fields.iter().map(|(k, v)| Line::raw(format!("{k}: {v}"))));
            }
            lines.push(Line::raw(""));
        }
        f.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((a.scroll, 0)),
            inner,
        );
        return;
    }
    if let Some(plan) = if a.reviewing_action {
        a.pending_action.as_ref().or(a.action_journal.last())
    } else {
        None
    } {
        for (key, value) in [
            ("DRY RUN", plan.description.clone()), ("Target", plan.target.display().to_string()),
            ("Identity",format!("inode {:?}; task start identity is embedded in its target",plan.identity)),
            ("Before",plan.before.trim().into()),("After",plan.after.clone()),
            ("Effective before",plan.effective_before.clone().unwrap_or("not captured".into())),
            ("Effective after",plan.effective_after.clone().unwrap_or("not applied".into())),
            ("Preconditions","Target identity and current value must still match; write permission is checked on apply".into()),
            ("Scope / risk","Placement affects target workloads; irqbalance and service managers may overwrite temporary changes".into()),
            ("Verification","Effective readback first; measure a complete 60-second post window under comparable load".into()),
            ("Rollback","Original value is journalled; revert refuses an externally changed target".into()),
            ("Next",if a.pending_action.is_some() { format!(": apply {}",plan.id) } else { ": verify · : revert · : export".into() }), ("Status",plan.outcome.clone())
        ] { lines.push(Line::styled(key,Style::default().fg(CYAN))); lines.push(Line::raw(value)); lines.push(Line::raw("")); }
        f.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((a.scroll, 0)),
            inner,
        );
        return;
    }
    let rows = a.rows();
    if let Some(r) = rows.get(a.selected.min(rows.len().saturating_sub(1))) {
        for (k, v) in a.snapshot.views[a.tab].columns.iter().zip(r) {
            lines.push(Line::from(vec![
                Span::styled(format!("{k:<20}"), Style::default().fg(CYAN)),
                Span::raw(v.clone()),
            ]));
        }
    }
    if a.tab == 5 {
        let selected = rows
            .get(a.selected)
            .and_then(|r| r.first())
            .cloned()
            .unwrap_or_default();
        lines.push(Line::raw(
            "Selected completed events: arguments, return, duration and captured stack",
        ));
        for event in a
            .snapshot
            .telemetry
            .events
            .iter()
            .filter(|e| e.source.contains("syscall") && e.message.contains(&selected))
            .rev()
            .take(32)
        {
            lines.push(Line::styled(
                format!("{}ms · {} · {}", event.at_ms, event.source, event.subject),
                Style::default().fg(CYAN),
            ));
            lines.push(Line::raw(event.message.clone()));
        }
        lines.push(Line::raw("u prepares a 10-second user-stack capture for the observed caller; addresses require symbolization."));
    }
    if a.tab == 10 {
        lines.clear();
        if let Some(e) = a.visible_events().get(a.selected) {
            lines.extend([
                Line::raw(format!("event {} · {}ms", e.id, e.at_ms)),
                Line::raw(format!("{} · {} · {}", e.source, e.severity, e.subject)),
                Line::raw(e.message.clone()),
                Line::raw(""),
                Line::raw("t source at captured time · c correlate in Diagnose"),
            ]);
        }
    }
    if a.tab == 1 {
        if let Some(task) = a.selected_task() {
            if let Some(fields) = a
                .snapshot
                .telemetry
                .details
                .get(&format!("task:{}", task.pid))
            {
                lines.extend(fields.iter().map(|(k, v)| Line::raw(format!("{k}: {v}"))));
            }
        }
    }
    let prefix = match a.tab {
        4 => Some("device"),
        7 => Some("cgroup"),
        8 => Some("module"),
        9 => Some("bpf"),
        _ => None,
    };
    if let Some(prefix) = prefix {
        if let Some(id) = rows.get(a.selected).and_then(|r| r.first()) {
            if let Some(fields) = a.snapshot.telemetry.details.get(&format!("{prefix}:{id}")) {
                for (key, value) in fields {
                    lines.push(Line::raw(format!("{key}: {value}")));
                }
            }
        }
    }
    if a.tab == 3 {
        let key = match a.focus {
            0 => "slab",
            1 => {
                if a.mode == 2 {
                    "hugepages"
                } else {
                    "numa"
                }
            }
            _ => "memory",
        };
        if let Some(fields) = a.snapshot.telemetry.details.get(key) {
            lines.extend(fields.iter().map(|(k, v)| Line::raw(format!("{k}: {v}"))));
        }
        if key == "slab" {
            if let Some((name, _)) = a
                .snapshot
                .telemetry
                .details
                .get("slab")
                .and_then(|v| v.get(a.selected))
            {
                if let Some(fields) = a.snapshot.telemetry.details.get(&format!("slab:{name}")) {
                    lines.extend(fields.iter().map(|(k, v)| Line::raw(format!("{k}: {v}"))));
                }
            }
        }
    }
    if a.tab == 1 {
        if let Some(task) = a.selected_task() {
            lines.push(Line::raw(format!(
                "identity PID {} / TGID {} / start ticks {}",
                task.pid, task.tgid, task.start_ticks
            )));
            lines.push(Line::raw(format!(
                "affinity {} · policy {} · cgroup {}",
                task.affinity, task.policy, task.cgroup
            )));
            lines.push(Line::raw(format!(
                "wchan {} · verdict {}",
                task.wchan, task.verdict
            )));
            if let Some(fields) = a
                .snapshot
                .telemetry
                .details
                .get(&format!("offcpu:{}", task.pid))
            {
                lines.extend(fields.iter().map(|(k, v)| Line::raw(format!("{k}: {v}"))));
            }
        }
    }
    lines.push(Line::raw(""));
    for note in &a.snapshot.views[a.tab].notes {
        lines.push(Line::raw(note.clone()));
        lines.push(Line::raw(""));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((a.scroll, 0)),
        inner,
    );
}
