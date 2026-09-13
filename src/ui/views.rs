use super::widgets::*;
use crate::{app::App, domain::Telemetry};
use ratatui::{prelude::*, widgets::*};
fn p(f: &mut Frame, r: Rect, a: &App, n: usize, title: &str, meta: &str) -> Rect {
    panel(f, r, n, title, meta, a.focus == n.saturating_sub(1))
}
fn t(a: &App) -> &Telemetry {
    &a.snapshot.telemetry
}
fn value(a: &App, key: &str, unit: &str) -> String {
    number(t(a).value(key), unit)
}
fn rows(a: &App, tab: usize) -> Vec<Vec<String>> {
    if a.tab == tab {
        a.rows()
    } else {
        a.snapshot.views[tab].rows.clone()
    }
}
fn hist(f: &mut Frame, r: Rect, a: &App, key: &str, alarm: bool) {
    history(f, r, t(a).series.get(key), a.cursor(), CYAN, alarm)
}
fn note(f: &mut Frame, r: Rect, texts: &[&str]) {
    text(
        f,
        r,
        texts.iter().map(|s| Line::raw(s.to_string())).collect(),
    );
}
fn controls(f: &mut Frame, r: Rect, label: &str, a: &App) {
    line(
        f,
        r,
        format!(
            " {label}    · g mode {}    / filter {}",
            match a.tab {
                1 => ["all", "runnable", "D-state", "kernel", "threads"]
                    .get(a.mode)
                    .unwrap_or(&"all")
                    .to_string(),
                2 => ["CPU", "task", "cgroup"]
                    .get(a.mode)
                    .unwrap_or(&"CPU")
                    .to_string(),
                3 => ["overview", "slab", "hugepages", "cgroup"]
                    .get(a.mode)
                    .unwrap_or(&"overview")
                    .to_string(),
                7 => ["tree", "flat", "throttled", "pressure", "cpu", "mem", "io"]
                    .get(a.mode)
                    .unwrap_or(&"tree")
                    .to_string(),
                5 => ["all", "negative returns", "p99 >1ms"]
                    .get(a.mode)
                    .unwrap_or(&"all")
                    .to_string(),
                9 => ["all programs", "this capture", "other programs"]
                    .get(a.mode)
                    .unwrap_or(&"all programs")
                    .to_string(),
                _ => a.mode.to_string(),
            },
            if a.filter.is_empty() {
                "—"
            } else {
                &a.filter
            }
        ),
        DIM,
    );
}
fn task_latency_key(a: &App, task: &crate::domain::Task) -> String {
    if a.snapshot.demo {
        format!("task.{}", task.pid)
    } else {
        format!("task.{}.{}", task.pid, task.start_ticks)
    }
}
fn task_latency_history(f: &mut Frame, r: Rect, a: &App, task: &crate::domain::Task) {
    let key = task_latency_key(a, task);
    if t(a).series.get(&key).is_some_and(|s| {
        s.window(a.cursor(), u64::from(r.width.saturating_sub(1)).min(600))
            .iter()
            .any(Option::is_some)
    }) {
        hist(f, r, a, &key, false);
    } else {
        let state = match t(a).capabilities.get("scheduler") {
            Some(crate::domain::Quality::Available) => "no wakes yet",
            Some(crate::domain::Quality::Stale) => "capture loss",
            _ => "l capture",
        };
        line(f, r, state, DIM);
    }
}
fn task_rows(tasks: &[&crate::domain::Task]) -> Vec<Vec<String>> {
    tasks
        .iter()
        .map(|x| {
            vec![
                format!("{} {}", x.name, x.pid),
                x.state.clone(),
                x.cpu.to_string(),
                number(x.cpu_pct, ""),
                if x.rss_bytes == 0 {
                    "—".into()
                } else {
                    format!("{:.0}M", x.rss_bytes as f64 / 1048576.)
                },
                latency(x.wake_p99_ms),
                number(x.voluntary_s, ""),
                x.wchan.clone(),
                x.verdict.clone(),
            ]
        })
        .collect()
}
fn tasks_panel(f: &mut Frame, r: Rect, a: &App, id: usize) {
    let tasks = a.visible_tasks();
    let inner = p(
        f,
        r,
        a,
        id,
        "tasks by concern",
        &format!(
            "{} total · {} matched · {} shown · s sort",
            t(a).tasks.len(),
            tasks.len(),
            tasks.len().min(r.height.saturating_sub(4) as usize)
        ),
    );
    let with_history = inner.width >= 70;
    let cols = if with_history {
        horizontal(inner, &[Constraint::Min(45), Constraint::Length(13)])
    } else {
        horizontal(inner, &[Constraint::Percentage(100)])
    };
    let compact = inner.width < 100;
    let offset = a
        .selected
        .saturating_sub(inner.height.saturating_sub(3) as usize);
    let visible = tasks
        .iter()
        .skip(offset)
        .take(inner.height.saturating_sub(2) as usize)
        .copied()
        .collect::<Vec<_>>();
    let mut data = task_rows(&visible);
    for (row, task) in data.iter_mut().zip(&visible) {
        if a.watched.contains(&(task.pid, task.start_ticks)) {
            row[0] = format!("★ {}", row[0]);
        }
    }
    if compact {
        for row in &mut data {
            row.remove(7);
            row.remove(6);
        }
    } else {
        for (row, task) in data.iter_mut().zip(&visible) {
            row.insert(3, task.policy.replace("SCHED_", ""));
            row.insert(6, latency(task.wake_p50_ms));
            row.insert(9, number(task.involuntary_s, ""));
        }
    }
    let headers: &[&str] = if compact {
        &["task / pid", "st", "cpu", "cpu%", "rss", "p99", "verdict"]
    } else {
        &[
            "task / pid",
            "st",
            "cpu",
            "policy",
            "cpu%",
            "rss",
            "wake p50",
            "wake p99",
            "vol/s",
            "invol/s",
            "wchan",
            "verdict",
        ]
    };
    let widths: &[u16] = if compact {
        &[28, 4, 5, 7, 9, 11, 25]
    } else {
        &[24, 3, 4, 9, 6, 7, 8, 8, 7, 7, 13, 20]
    };
    let mut headers = headers.to_vec();
    let mut widths = widths.to_vec();
    if a.tab == 1 && a.grouping > 0 {
        headers.insert(1, ["", "cgroup", "UID", "parent"][a.grouping]);
        widths.insert(1, 18);
        let mut previous = String::new();
        for (row, task) in data.iter_mut().zip(&visible) {
            let group = match a.grouping {
                1 => task.cgroup.clone(),
                2 => task.uid.map(|v| v.to_string()).unwrap_or("unknown".into()),
                _ => task.parent_pid.to_string(),
            };
            row.insert(
                1,
                if group != previous {
                    group.clone()
                } else {
                    String::new()
                },
            );
            previous = group;
        }
    }
    table(
        f,
        cols[0],
        &headers,
        &data,
        &widths,
        a.selected.saturating_sub(offset),
    );
    if with_history {
        line(f, Rect::new(cols[1].x, cols[1].y, 13, 1), "1s/column", DIM);
        for (i, task) in visible.iter().enumerate() {
            task_latency_history(
                f,
                Rect::new(cols[1].x, cols[1].y + 2 + i as u16, 12, 1),
                a,
                task,
            );
        }
    }
}
fn cpu_panel(f: &mut Frame, r: Rect, a: &App, id: usize, side: bool) {
    let u = t(a).cpus.iter().map(|c| c.user).sum::<f64>() / t(a).cpus.len().max(1) as f64;
    let k = t(a)
        .cpus
        .iter()
        .map(|c| c.kernel + c.softirq + c.irq)
        .sum::<f64>()
        / t(a).cpus.len().max(1) as f64;
    let inner = p(
        f,
        r,
        a,
        id,
        "cpu",
        &format!("▲ user {u:.1}%   ▼ kernel+irq {k:.1}%"),
    );
    let regions = if side {
        horizontal(
            inner,
            &[Constraint::Percentage(72), Constraint::Percentage(28)],
        )
    } else {
        vertical(inner, &[Constraint::Min(4), Constraint::Length(3)])
    };
    graph(
        f,
        regions[0],
        t(a).series.get("cpu.user"),
        t(a).series.get("cpu.kernel"),
        a.cursor(),
    );
    if side {
        for (i, c) in t(a)
            .cpus
            .iter()
            .enumerate()
            .take(regions[1].height as usize)
        {
            let y = regions[1].y + i as u16;
            line(
                f,
                Rect::new(regions[1].x, y, 5, 1),
                format!("c{}", c.id),
                if c.busy > 90. { GOLD } else { DIM },
            );
            let w = regions[1].width.saturating_sub(13);
            hist(
                f,
                Rect::new(regions[1].x + 5, y, w, 1),
                a,
                &format!("cpu.{}", c.id),
                true,
            );
            line(
                f,
                Rect::new(regions[1].right().saturating_sub(7), y, 7, 1),
                format!("{:>5.1}%", c.busy),
                if c.busy > 90. { GOLD } else { FG },
            );
        }
    } else {
        let n = t(a).cpus.len().clamp(1, 16);
        let sizes = vec![Constraint::Ratio(1, n as u32); n];
        let cells = horizontal(regions[1], &sizes);
        for (c, r) in t(a).cpus.iter().zip(cells.iter()) {
            line(
                f,
                Rect::new(r.x, r.y, r.width, 1),
                format!("cpu{}", c.id),
                if c.busy > 90. { GOLD } else { DIM },
            );
            hist(
                f,
                Rect::new(r.x, r.y + 1, r.width.saturating_sub(1), 1),
                a,
                &format!("cpu.{}", c.id),
                true,
            );
            line(
                f,
                Rect::new(r.x, r.y + 2, r.width, 1),
                format!("{:.0}%", c.busy),
                if c.busy > 90. { GOLD } else { FG },
            );
        }
    }
}
fn timeline_rows(t: &crate::domain::Telemetry) -> [(&'static str, &'static str); 5] {
    let has = |key: &str| {
        t.series
            .get(key)
            .is_some_and(|s| s.samples.iter().any(|v| v.value.is_some()))
    };
    [
        if has("cpu.busy") {
            ("CPU busy", "cpu.busy")
        } else {
            ("CPU user", "cpu.user")
        },
        if has("softirq.peak") {
            ("softirq peak", "softirq.peak")
        } else {
            ("NET_RX peak", "softirq")
        },
        if has("sched.p99") {
            ("sched p99", "sched.p99")
        } else {
            ("runnable/CPU", "runqueue")
        },
        if has("disk.p99") {
            ("I/O p99", "disk.p99")
        } else {
            ("I/O max mean", "disk.await_max")
        },
        ("D-state", "dstate"),
    ]
}
fn timeline(f: &mut Frame, r: Rect, a: &App, id: usize) {
    let rows = timeline_rows(t(a));
    let seconds = 600;
    let subtitle = "1s / column · ← → cursor";
    let inner = p(f, r, a, id, "timeline", subtitle);
    for (i, (label, key)) in rows.iter().enumerate() {
        if i as u16 >= inner.height.saturating_sub(1) {
            break;
        }
        let y = inner.y + i as u16;
        line(
            f,
            Rect::new(inner.x, y, 13.min(inner.width), 1),
            *label,
            DIM,
        );
        let series = t(a).series.get(*key);
        let current = series
            .and_then(|s| s.samples.iter().rev().find(|v| v.at_ms <= a.cursor()))
            .filter(|v| a.cursor().saturating_sub(v.at_ms) <= 1500)
            .and_then(|v| v.value);
        let value = current
            .map(|v| format!("{v:.1}{}", series.map(|s| s.unit.as_str()).unwrap_or("")))
            .unwrap_or("warming".into());
        line(
            f,
            Rect::new(inner.x + 14, y, 10.min(inner.width.saturating_sub(14)), 1),
            value,
            CYAN,
        );
        let area = Rect::new(inner.x + 25, y, inner.width.saturating_sub(25), 1);
        if series.is_some_and(|s| {
            s.window(
                a.cursor(),
                u64::from(area.width.saturating_sub(1)).min(seconds),
            )
            .iter()
            .any(Option::is_some)
        }) {
            history_window(f, area, series, a.cursor(), CYAN, i > 0, seconds);
        } else {
            line(f, area, "waiting for samples", DIM);
        }
    }
    if inner.height > 5 {
        let plot_x = inner.x + 25;
        let width = inner.width.saturating_sub(25);
        line(
            f,
            Rect::new(inner.x, inner.bottom() - 1, 24.min(inner.width), 1),
            "events · 1s/col",
            DIM,
        );
        if width > 0 {
            let end_second = a.cursor() / 1000;
            for event in &t(a).events {
                if event.at_ms > a.cursor() || event.source.contains("syscall") {
                    continue;
                }
                let age = end_second.saturating_sub(event.at_ms / 1000);
                if age > seconds {
                    continue;
                }
                if age >= u64::from(width) {
                    continue;
                }
                let x = width - 1 - age as u16;
                line(
                    f,
                    Rect::new(plot_x + x, inner.bottom() - 1, 1, 1),
                    "▲",
                    GOLD,
                );
            }
        }
    }
}
fn clock(a: &App, ms: u64) -> String {
    if !a.snapshot.demo {
        return format!("boot+{:.3}s", ms as f64 / 1000.);
    }
    let secs = 9 * 3600 + 3 * 60 + 22 + ms / 1000;
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}
fn summary_fields(f: &mut Frame, r: Rect, a: &App, key: &str) {
    let selected = if ["device", "module", "cgroup", "bpf"].contains(&key) {
        a.rows()
            .get(a.selected)
            .and_then(|r| r.first())
            .map(|id| format!("{key}:{id}"))
    } else {
        None
    };
    let values = selected
        .as_ref()
        .and_then(|k| t(a).details.get(k))
        .or_else(|| {
            if selected.is_some() {
                None
            } else {
                t(a).details.get(key)
            }
        });
    if let Some(v) = values {
        if key == "bpf" && !a.expanded {
            let mut fields = v.clone();
            fields.sort_by_key(|(name, _)| match name.as_str() {
                "owner" | "creator UID" => 0,
                "attachment" | "attach" => 1,
                "runtime statistics" => 2,
                "helpers" => 3,
                name if name.starts_with("map") => 4,
                "JIT bytes" | "translated bytes" | "code" => 5,
                _ => 6,
            });
            let height = r.height.saturating_sub(1);
            fields_widget(f, Rect::new(r.x, r.y, r.width, height), &fields, 0);
            if fields.len() > height as usize {
                line(
                    f,
                    Rect::new(r.x, r.y + height, r.width, 1),
                    format!(
                        "{} more fields · focus this panel, Space expand · ↑↓ scroll",
                        fields.len() - height as usize
                    ),
                    DIM,
                );
            }
        } else {
            fields_widget(f, r, v, if a.expanded { a.scroll } else { 0 });
        }
    } else {
        line(
            f,
            r,
            "Selected source detail unavailable · : capabilities",
            DIM,
        );
    }
}
pub fn draw(f: &mut Frame, r: Rect, a: &App) {
    if a.tab == 5 && !a.snapshot.views[5].rows.iter().any(|row| row.len() >= 10) {
        syscall_empty(f, r, a);
        return;
    }
    if r.height < 26 || r.width < 110 {
        compact(f, r, a);
        return;
    }
    match a.tab {
        12 => dense(f, r, a),
        0 => overview(f, r, a),
        1 => tasks(f, r, a),
        2 => scheduler(f, r, a),
        3 => memory(f, r, a),
        4 => block(f, r, a),
        5 => syscalls(f, r, a),
        6 => irq(f, r, a),
        7 => cgroups(f, r, a),
        8 => modules(f, r, a),
        9 => ebpf(f, r, a),
        10 => logs(f, r, a),
        11 => diagnose(f, r, a),
        13 => flame(f, r, a),
        _ => {}
    }
}
fn softirq_traced(a: &App) -> bool {
    a.snapshot.demo
        || t(a).metrics.iter().any(|(key, m)| {
            key.starts_with("softirq.cpu")
                && key.contains(".vec")
                && m.value.is_some()
                && m.quality == crate::domain::Quality::Available
        })
}
fn softirq_caption(a: &App) -> &'static str {
    if softirq_traced(a) {
        "softirq execution % by CPU"
    } else {
        "softirq events/s by CPU"
    }
}
fn dense_irq(f: &mut Frame, r: Rect, a: &App) {
    let tracing = softirq_traced(a);
    let ir = p(
        f,
        r,
        a,
        4,
        "irq / softirq",
        if tracing {
            "softirq execution %"
        } else {
            "softirq events/s"
        },
    );
    if ir.height == 0 {
        return;
    }
    line(f, Rect::new(ir.x, ir.y, ir.width.min(8), 1), "IRQ/s", CYAN);
    right_line(
        f,
        Rect::new(ir.x + 8, ir.y, ir.width.saturating_sub(8), 1),
        value(a, "irq.rate", ""),
        CYAN,
    );
    let col = a.snapshot.views[6]
        .columns
        .iter()
        .position(|c| c.to_lowercase().contains("rate"))
        .unwrap_or(2);
    let mut rows = rows(a, 6);
    let rate = |row: &Vec<String>| {
        row.get(col)
            .and_then(|s| s.replace(',', "").parse::<f64>().ok())
    };
    rows.sort_by(|a, b| rate(b).unwrap_or(-1.).total_cmp(&rate(a).unwrap_or(-1.)));
    for (i, row) in rows
        .iter()
        .filter(|row| rate(row).is_some())
        .take(2)
        .enumerate()
    {
        let y = ir.y + 1 + i as u16;
        if y >= ir.bottom() {
            break;
        }
        let value_width = 12.min(ir.width);
        right_line(
            f,
            Rect::new(ir.x, y, ir.width.saturating_sub(value_width + 1), 1),
            format!(
                "{} {}",
                row.first().map(String::as_str).unwrap_or(""),
                row.get(1).map(String::as_str).unwrap_or("")
            ),
            DIM,
        );
        right_line(
            f,
            Rect::new(ir.right() - value_width, y, value_width, 1),
            format!("{}/s", number(rate(row), "")),
            FG,
        );
    }
    let offset = if rows.iter().any(|row| rate(row).is_some()) {
        3
    } else {
        1
    };
    if ir.height > offset {
        softirq_matrix(
            f,
            Rect::new(ir.x, ir.y + offset, ir.width, ir.height - offset),
            a,
            true,
        );
    }
}
fn dense_scheduler(f: &mut Frame, r: Rect, a: &App) {
    let sc = p(f, r, a, 5, "sched", "live counters / capture latency");
    let traced = t(a)
        .series
        .get("sched.p99")
        .is_some_and(|s| s.at(a.cursor()).is_some());
    for (i, (label, key)) in [
        if traced {
            ("wake p99", "sched.p99")
        } else {
            ("runnable/CPU", "runqueue")
        },
        ("switches/s", "sched.switches"),
        ("CPU PSI", "psi.cpu"),
    ]
    .iter()
    .enumerate()
    {
        let y = sc.y + i as u16 * 2;
        if y >= sc.bottom() {
            break;
        }
        let series = t(a).series.get(*key);
        let latest = series
            .and_then(|s| s.at(a.cursor()))
            .or_else(|| t(a).value(key))
            .or_else(|| {
                if *key == "sched.switches" && !t(a).cpus.is_empty() {
                    t(a).cpus
                        .iter()
                        .map(|c| c.switches_s)
                        .collect::<Option<Vec<_>>>()
                        .map(|v| v.iter().sum())
                } else {
                    None
                }
            });
        line(
            f,
            Rect::new(sc.x, y, sc.width, 1),
            format!(
                "{label} {}",
                number(latest, series.map(|s| s.unit.as_str()).unwrap_or(""))
            ),
            CYAN,
        );
        if y + 1 < sc.bottom() {
            hist(f, Rect::new(sc.x, y + 1, sc.width, 1), a, key, false);
        }
    }
}
fn dense(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(11),
            Constraint::Min(16),
            Constraint::Length(8),
        ],
    );
    cpu_panel(f, bands[0], a, 1, true);
    let cols = horizontal(
        bands[1],
        &[
            Constraint::Percentage(24),
            Constraint::Percentage(26),
            Constraint::Percentage(50),
        ],
    );
    let left = vertical(
        cols[0],
        &[Constraint::Percentage(60), Constraint::Percentage(40)],
    );
    let middle = vertical(
        cols[1],
        &[
            Constraint::Percentage(45),
            Constraint::Percentage(35),
            Constraint::Percentage(20),
        ],
    );
    let m = p(
        f,
        left[0],
        a,
        2,
        "mem",
        &format!("{:.1} GiB total", a.snapshot.mem_total as f64 / 1024.),
    );
    for (i, (key, label)) in [
        ("memory.used", "used"),
        ("memory.cache", "cache"),
        ("memory.slab", "slab"),
        ("memory.available", "avail"),
    ]
    .iter()
    .enumerate()
    {
        let y = m.y + i as u16 * 2;
        if y >= m.bottom() {
            break;
        }
        line(f, Rect::new(m.x, y, 7, 1), *label, DIM);
        let w = m.width.saturating_sub(17);
        meter(
            f,
            Rect::new(m.x + 7, y, w, 1),
            t(a).value(key).unwrap_or(0.),
            a.snapshot.mem_total as f64 / 1024.,
            if *label == "slab" { GOLD } else { CYAN },
        );
        line(
            f,
            Rect::new(m.right().saturating_sub(9), y, 9, 1),
            value(a, key, "G"),
            FG,
        );
    }
    if m.height > 8 {
        line(
            f,
            Rect::new(m.x, m.bottom() - 1, m.width, 1),
            format!("▲ dentry {}", value(a, "slab.growth", " MiB/h")),
            GOLD,
        );
    }
    let b = p(
        f,
        left[1],
        a,
        3,
        "block",
        &format!("{} devices", t(a).devices.len()),
    );
    for (i, d) in t(a).devices.iter().take(2).enumerate() {
        let y = b.y + i as u16 * 2;
        line(
            f,
            Rect::new(b.x, y, b.width, 1),
            format!(
                "{}  {}/{} MiB/s",
                d.name,
                number(d.read_mib_s, ""),
                number(d.write_mib_s, "")
            ),
            CYAN,
        );
        hist(
            f,
            Rect::new(b.x + 10, y + 1, b.width.saturating_sub(11), 1),
            a,
            &format!("device:{}:read", d.name),
            false,
        );
    }
    if b.height > 4 {
        line(
            f,
            Rect::new(b.x, b.bottom() - 1, b.width, 1),
            format!(
                "{} age {}",
                if a.snapshot.demo {
                    "FLUSH"
                } else {
                    "oldest request"
                },
                value(a, "disk.age", "ms")
            ),
            GOLD,
        );
    }
    dense_irq(f, middle[0], a);
    dense_scheduler(f, middle[1], a);
    let ta = p(f, middle[2], a, 6, "taint · eBPF", "trust / overhead");
    let taint = t(a)
        .details
        .get("taint")
        .and_then(|v| v.first())
        .map(|v| v.1.as_str())
        .unwrap_or("unavailable");
    note(
        f,
        ta,
        &[
            &format!("Taint {taint}"),
            &format!(
                "BPF own {} · total {}",
                value(a, "bpf.own", "%"),
                value(a, "bpf.total", "%")
            ),
        ],
    );
    tasks_panel(f, cols[2], a, 7);
    timeline(f, bands[2], a, 8);
}
fn overview(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(5),
            Constraint::Length(15),
            Constraint::Length(11),
            Constraint::Min(8),
            Constraint::Length(8),
        ],
    );
    let cards = horizontal(bands[0], &[Constraint::Ratio(1, 5); 5]);
    for (i, (title, key, unit, sub, warn)) in [
        (
            "sched latency p99",
            "sched.p99",
            " ms",
            "measured wake-to-run",
            true,
        ),
        (
            "run queue",
            "runqueue",
            " /cpu",
            "visible runnable / logical CPU",
            false,
        ),
        (
            "softirq peak CPU",
            if a.snapshot.demo {
                "softirq"
            } else {
                "softirq.peak"
            },
            "%",
            if a.snapshot.demo {
                "NET_RX execution time"
            } else {
                "all softirq · CPU time delta"
            },
            true,
        ),
        ("PSI some", "psi.cpu", "%", "10s avg · CPU", false),
        (
            "major faults",
            "fault.major",
            "/s",
            "/proc/vmstat delta",
            false,
        ),
    ]
    .iter()
    .enumerate()
    {
        card(
            f,
            cards[i],
            title,
            &value(a, key, unit),
            sub,
            t(a).series.get(*key),
            a.cursor(),
            *warn
                && t(a)
                    .concern(if *key == "sched.p99" { "sched" } else { "irq" })
                    .is_some(),
        );
    }
    cpu_panel(f, bands[1], a, 1, false);
    let health = p(
        f,
        bands[2],
        a,
        2,
        "health",
        &format!("{} findings", t(a).issues.len()),
    );
    for (i, (label, key, unit)) in [
        ("sched", "sched.p99", "ms"),
        (
            "irq",
            if a.snapshot.demo {
                "softirq"
            } else {
                "softirq.peak"
            },
            "%",
        ),
        ("memory", "memory.used", "GiB"),
        ("block I/O", "disk.p99", "ms"),
        ("D-state", "dstate", ""),
    ]
    .iter()
    .enumerate()
    {
        line(
            f,
            Rect::new(health.x, health.y + i as u16, 20, 1),
            format!("● {label:<9} {}", value(a, key, unit)),
            if t(a)
                .concern(["sched", "irq", "memory", "block", "dstate"][i])
                .is_some()
            {
                GOLD
            } else if t(a).value(key).is_none() {
                DIM
            } else {
                GREEN
            },
        );
        hist(
            f,
            Rect::new(
                health.x + 21,
                health.y + i as u16,
                health.width.saturating_sub(21),
                1,
            ),
            a,
            key,
            true,
        );
    }
    if health.height > 6 {
        text(
            f,
            Rect::new(health.x, health.y + 6, health.width, health.height - 6),
            t(a).issues
                .iter()
                .take(3)
                .map(|i| Line::raw(format!("▲ {} · {} · {}", i.title, i.subject, i.state)))
                .collect(),
        );
    }
    tasks_panel(f, bands[3], a, 3);
    timeline(f, bands[4], a, 4);
}
fn tasks(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(1),
            Constraint::Length(14),
            Constraint::Length(10),
            Constraint::Min(7),
            Constraint::Length(5),
        ],
    );
    controls(
        f,
        bands[0],
        &format!(
            "g all / runnable / D / kernel / threads    G group {}",
            ["none", "cgroup", "UID", "parent"][a.grouping]
        ),
        a,
    );
    tasks_panel(f, bands[1], a, 1);
    let task = a.selected_task();
    let title = task
        .map(|x| format!("{} · pid {}", x.name, x.pid))
        .unwrap_or("selected task".into());
    let detail = p(f, bands[2], a, 2, &title, "task context");
    if let Some(x) = task {
        fields(
            f,
            detail,
            &[
                (
                    "on CPU".into(),
                    format!("{} · affinity {}", x.cpu, x.affinity),
                ),
                (
                    "policy / nice".into(),
                    format!(
                        "{} / {}",
                        x.policy,
                        x.nice.map(|v| v.to_string()).unwrap_or("unknown".into())
                    ),
                ),
                (
                    "type / uptime".into(),
                    format!(
                        "{} · {}",
                        if x.kernel_thread {
                            "kernel thread"
                        } else if x.pid != x.tgid {
                            "user thread"
                        } else {
                            "process leader"
                        },
                        x.age_ms
                            .map(|ms| format!("{}s", ms / 1000))
                            .unwrap_or("unknown".into())
                    ),
                ),
                (
                    "CPU time".into(),
                    format!("{} of one CPU", number(x.cpu_pct, "%")),
                ),
                ("wchan".into(), x.wchan.clone()),
                ("cgroup".into(), x.cgroup.clone()),
                (
                    "UID / parent".into(),
                    format!(
                        "{} / {}",
                        x.uid.map(|v| v.to_string()).unwrap_or("unknown".into()),
                        x.parent_pid
                    ),
                ),
                ("wakeup p99".into(), latency(x.wake_p99_ms)),
                ("blocked age".into(), number(x.blocked_ms, "ms")),
            ],
        );
    }
    let why = p(f, bands[3], a, 3, "why waiting", "1s / column");
    let plots = vertical(
        why,
        &[
            Constraint::Length(1),
            Constraint::Min(2),
            Constraint::Length(1),
            Constraint::Min(2),
            Constraint::Length(2),
        ],
    );
    line(f, plots[0], "Selected task runtime · % of one CPU", DIM);
    hist(
        f,
        plots[1],
        a,
        &task
            .map(|x| {
                if a.snapshot.demo {
                    format!("task.runtime{}", x.pid)
                } else {
                    format!("task.runtime{}.{}", x.pid, x.start_ticks)
                }
            })
            .unwrap_or_default(),
        false,
    );
    line(
        f,
        plots[2],
        "Wake-to-run p99 · l capture selected task (30s)",
        DIM,
    );
    if let Some(task) = task {
        task_latency_history(f, plots[3], a, task);
    }

    line(f,plots[4],"Compare the selected task against CPU and IRQ activity. Shared timing alone does not establish causality.",FG);
    let actions = p(
        f,
        bands[4],
        a,
        4,
        "actions",
        "Enter follows selected subject",
    );
    note(
        f,
        actions,
        &[
            "↵ scheduler on selected CPU    i IRQ affinity    a affinity dry-run",
            "c cgroup · P profile stacks · y copy identity · w watch",
        ],
    );
}
fn scheduler(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(1),
            Constraint::Length(13),
            Constraint::Min(8),
            Constraint::Length(11),
        ],
    );
    controls(
        f,
        bands[0],
        "g per CPU / per task / per cgroup     M wakeup latency / runtime / CPU migrations",
        a,
    );
    let subjects = a.scheduler_subjects();
    let cp = p(
        f,
        bands[1],
        a,
        1,
        match a.mode {
            1 => "tasks",
            2 => "cgroups",
            _ => "cpus",
        },
        "g change subject · ↵ inspect",
    );
    // One index for the whole table rather than a scan per row: the subject
    // list is one entry per task in this mode, so the scan was quadratic. It
    // also never matched live, where the key carries start_ticks, so every row
    // fell through to the unmeasured branch.
    let by_key: std::collections::BTreeMap<String, &crate::domain::Task> = if a.mode == 1 {
        t(a).tasks
            .iter()
            .map(|x| (task_latency_key(a, x), x))
            .collect()
    } else {
        Default::default()
    };
    let data = subjects
        .iter()
        .map(|(label, key, cpu, group)| {
            if a.mode == 1 {
                if let Some(x) = by_key.get(key.as_str()) {
                    return vec![
                        label.clone(),
                        x.state.clone(),
                        latency(x.wake_p99_ms),
                        number(x.cpu_pct, "%"),
                        x.cpu.to_string(),
                        x.affinity.clone(),
                        x.cgroup.clone(),
                    ];
                }
            } else if a.mode == 2 {
                let tasks = t(a)
                    .tasks
                    .iter()
                    .filter(|x| Some(&x.cgroup) == group.as_ref())
                    .collect::<Vec<_>>();
                let measured = t(a)
                    .series
                    .get(key)
                    .and_then(|s| s.samples.iter().rev().find(|v| v.at_ms <= a.cursor()))
                    .and_then(|v| v.value);
                let cg = t(a)
                    .cgroups
                    .iter()
                    .find(|x| Some(&x.path) == group.as_ref());
                return vec![
                    label.clone(),
                    tasks.iter().filter(|x| x.state == "R").count().to_string(),
                    number(measured, "ms"),
                    number(cg.and_then(|x| x.runtime_pct), "%"),
                    number(cg.and_then(|x| x.throttled_ms_s), "ms/s"),
                    cg.map(|x| x.cpus.clone()).unwrap_or_default(),
                    "sampled direct members".into(),
                ];
            }
            let c = t(a).cpus.iter().find(|c| Some(c.id) == *cpu).unwrap();
            vec![
                label.clone(),
                c.runnable.map(|n| n.to_string()).unwrap_or("—".into()),
                number(c.wake_p50_ms, "ms"),
                number(c.wake_p99_ms, "ms"),
                number(c.switches_s, ""),
                number(c.migrations_s, ""),
                format!("{:.0}", c.irq),
                format!("{:.0}", c.softirq),
                format!("{:.0}", c.steal),
                t(a).tasks
                    .iter()
                    .filter(|x| x.cpu == c.id)
                    .max_by(|a, b| a.cpu_pct.unwrap_or(0.).total_cmp(&b.cpu_pct.unwrap_or(0.)))
                    .map(|x| x.name.clone())
                    .unwrap_or("—".into()),
                if c.softirq > 80. {
                    "softirq storm"
                } else {
                    "ok"
                }
                .into(),
            ]
        })
        .collect::<Vec<_>>();
    let headers: &[&str] = match a.mode {
        1 => &[
            "task", "state", "wake p99", "runtime", "CPU", "affinity", "cgroup",
        ],
        2 => &[
            "cgroup",
            "nr_run",
            "wake p99",
            "runtime",
            "throttle",
            "cpuset",
            "aggregation",
        ],
        _ => &[
            "cpu", "nr_run", "wake p50", "wake p99", "switch/s", "migr/s", "irq%", "soft%",
            "steal%", "top task", "verdict",
        ],
    };
    history_table(
        f,
        cp,
        a,
        &headers.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        &data,
        if a.mode == 0 {
            &[6, 7, 10, 10, 10, 10, 7, 7, 7, 20, 18]
        } else {
            &[24, 7, 12, 12, 12, 18, 28]
        },
        ("scheduler", ""),
    );
    let subject = subjects.get(a.selected);
    let c = subject.and_then(|x| x.2).unwrap_or(0);
    let label = subject.map(|x| x.0.as_str()).unwrap_or("no subject");
    let key = subject.map(|x| x.1.as_str()).unwrap_or("");
    let selected_key = match a.metric {
        1 => match a.mode {
            1 => subject
                .map(|x| x.1.replace("task.", "task.runtime"))
                .unwrap_or_default(),
            2 => format!("cgroup:{label}:cpu"),
            _ => format!("cpu.{c}"),
        },
        2 => match a.mode {
            1 => subject
                .map(|x| x.1.replace("task.", "sched.migrations.task"))
                .unwrap_or_default(),
            2 => subject
                .map(|x| x.1.replace("sched.cgroup", "sched.migrations.cgroup"))
                .unwrap_or_default(),
            _ => format!("sched.migrations.cpu{c}"),
        },
        _ => key.to_string(),
    };
    let key = selected_key.as_str();
    let metric_label = match a.metric {
        1 => "runtime · % of one CPU",
        2 => "migration rate · capture average",
        _ => "wakeup latency",
    };

    let plot = p(
        f,
        bands[2],
        a,
        2,
        &format!("{label} · {metric_label}"),
        if a.auto_scale {
            "h histogram · z fixed scale"
        } else {
            "h histogram · z auto scale"
        },
    );
    if a.histogram && a.metric == 0 {
        histogram(f, plot, a, key);
    } else if a.auto_scale {
        let series = t(a).series.get(key).cloned().map(|mut s| {
            s.max = s
                .window(a.cursor(), u64::from(plot.width.saturating_sub(1)).min(600))
                .into_iter()
                .flatten()
                .reduce(f64::max)
                .unwrap_or(1.)
                .max(0.001)
                * 1.1;
            s
        });
        history(f, plot, series.as_ref(), a.cursor(), CYAN, true);
    } else {
        hist(f, plot, a, key, true);
    }
    let waiting = p(
        f,
        bands[3],
        a,
        3,
        &format!("placement · {label}"),
        if a.snapshot.demo {
            "resident tasks · last sampled wakeup"
        } else {
            "observed runnable waits · sampled placement"
        },
    );
    if !a.snapshot.demo {
        let waits: Vec<_> = t(a)
            .runnable_waits
            .iter()
            .filter(|wait| {
                if a.mode == 2 {
                    t(a).tasks.iter().any(|task| {
                        task.pid == wait.pid
                            && task.start_ticks == wait.start_ticks
                            && subject.and_then(|s| s.3.as_ref()) == Some(&task.cgroup)
                    })
                } else if a.mode == 1 {
                    subject
                        .is_some_and(|s| s.1 == format!("task.{}.{}", wait.pid, wait.start_ticks))
                } else {
                    wait.cpu == c
                }
            })
            .collect();
        if waits.is_empty() {
            line(
                f,
                waiting,
                if matches!(
                    t(a).capabilities.get("scheduler"),
                    Some(crate::domain::Quality::Available)
                ) {
                    "No identity-confirmed waits observed; capture is not a complete run queue"
                } else {
                    "Waiting evidence unavailable · l capture; loss invalidates outstanding waits"
                },
                DIM,
            );
        } else {
            for (i, wait) in waits.iter().take(waiting.height as usize).enumerate() {
                line(
                    f,
                    Rect::new(waiting.x, waiting.y + i as u16, waiting.width, 1),
                    format!(
                        "PID {} · CPU {} · waiting {}",
                        wait.pid,
                        wait.cpu,
                        latency(Some(wait.age_ms))
                    ),
                    GOLD,
                );
            }
        }
        return;
    }
    for (i, x) in t(a)
        .tasks
        .iter()
        .filter(|x| {
            if a.mode == 2 {
                subject.and_then(|s| s.3.as_ref()) == Some(&x.cgroup)
            } else {
                x.cpu == c
            }
        })
        .enumerate()
        .take(6)
    {
        let y = waiting.y + i as u16;
        line(f, Rect::new(waiting.x, y, 24, 1), &x.name, FG);
        meter(
            f,
            Rect::new(waiting.x + 25, y, waiting.width.saturating_sub(39), 1),
            x.wake_p99_ms.unwrap_or(0.),
            20.,
            if x.state == "R" { GOLD } else { CYAN },
        );
        line(
            f,
            Rect::new(waiting.right() - 13, y, 13, 1),
            if x.state == "R" {
                "runnable".into()
            } else {
                latency(x.wake_p99_ms)
            },
            FG,
        );
    }
    if waiting.height > 7 {
        line(f,Rect::new(waiting.x,waiting.bottom()-2,waiting.width,2),"Affinity and cpuset constrain placement independently. Inspect effective masks before changing either.\na preview affinity · i IRQs · ↵ selected CPU tasks",DIM);
    }
}
fn memory(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Min(6),
            Constraint::Length(9),
        ],
    );
    controls(
        f,
        bands[0],
        "view overview / slab / hugepages / per cgroup",
        a,
    );
    let cards = horizontal(bands[1], &[Constraint::Ratio(1, 6); 6]);
    for (i, (label, key, unit)) in [
        ("used", "memory.used", " GiB"),
        ("available", "memory.available", " GiB"),
        ("page cache", "memory.cache", " GiB"),
        ("slab", "memory.slab", " GiB"),
        ("PSI mem", "psi.memory", "%"),
        ("faults", "fault.minor", "/s"),
    ]
    .iter()
    .enumerate()
    {
        card(
            f,
            cards[i],
            label,
            &if key.starts_with("memory.") {
                number(
                    t(a).value(key)
                        .map(|v| if a.memory_gib { v } else { v * 1024. }),
                    if a.memory_gib { " GiB" } else { " MiB" },
                )
            } else {
                value(a, key, unit)
            },
            if i == 3 {
                "growth; cause unconfirmed"
            } else {
                "10s window"
            },
            t(a).series.get(*key),
            a.cursor(),
            i == 3,
        );
    }
    let slab = p(
        f,
        bands[2],
        a,
        1,
        "slab · top by size",
        "growth requires allocation evidence",
    );
    if let Some(values) = t(a).details.get("slab") {
        let size = |val: &str| {
            val.split_whitespace()
                .next()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.)
                * if val.contains("GiB") { 1024. } else { 1. }
        };
        let max = values.iter().map(|(_, v)| size(v)).fold(1., f64::max);
        let capacity = slab.height as usize;
        let offset = if a.mode == 1 {
            a.selected.saturating_sub(capacity.saturating_sub(1))
        } else {
            0
        };
        for (i, (label, val)) in values.iter().skip(offset).take(capacity).enumerate() {
            line(f, Rect::new(slab.x, slab.y + i as u16, 20, 1), label, FG);
            let n = val
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.);
            meter(
                f,
                Rect::new(
                    slab.x + 21,
                    slab.y + i as u16,
                    slab.width.saturating_sub(35),
                    1,
                ),
                if val.contains("GiB") { n * 1024. } else { n },
                max,
                if i == 0 { GOLD } else { CYAN },
            );
            line(
                f,
                Rect::new(slab.right() - 13, slab.y + i as u16, 13, 1),
                memory_text(val, a.memory_gib),
                FG,
            );
        }
    } else {
        line(f, slab, "Slab cache access unavailable", DIM);
    }
    let numa = p(f, bands[3], a, 2, "NUMA / zones", "locality and watermarks");
    if a.mode == 1 {
        memory_fields(f, numa, a, "slab");
    } else if a.mode == 2 {
        memory_fields(f, numa, a, "hugepages");
    } else if a.mode == 3 {
        let data = t(a)
            .cgroups
            .iter()
            .map(|g| {
                vec![
                    g.path.clone(),
                    number(
                        g.memory_bytes
                            .map(|v| v as f64 / if a.memory_gib { 1073741824. } else { 1048576. }),
                        if a.memory_gib { " GiB" } else { " MiB" },
                    ),
                    g.fields
                        .iter()
                        .find(|(k, _)| k == "memory.max")
                        .map(|(_, v)| v.clone())
                        .unwrap_or("—".into()),
                ]
            })
            .collect::<Vec<_>>();
        table(
            f,
            numa,
            &["cgroup", "memory current", "memory limit"],
            &data,
            &[60, 20, 20],
            a.selected,
        );
    } else {
        memory_fields(f, numa, a, "numa");
    }
    let graph_r = p(
        f,
        bands[4],
        a,
        3,
        "allocation / reclaim",
        "▲ alloc · ▼ reclaim pages/s",
    );
    graph(
        f,
        graph_r,
        t(a).series.get("alloc"),
        t(a).series.get("reclaim"),
        a.cursor(),
    );
    let rss = p(
        f,
        bands[5],
        a,
        4,
        "process memory",
        "PSS rotating batch ≤32 / 5s · age per process",
    );
    let divisor = if a.memory_gib { 1073741824. } else { 1048576. };
    let memory = |v: Option<u64>| {
        v.map(|v| format!("{:.2}", v as f64 / divisor))
            .unwrap_or("—".into())
    };
    let data = a
        .memory_tasks()
        .iter()
        .map(|x| {
            vec![
                format!("{} {}", x.name, x.pid),
                memory(Some(x.rss_bytes)),
                memory(x.pss_bytes),
                memory(x.anon_bytes),
                memory(x.file_bytes),
                memory(x.shmem_bytes),
                memory(x.swap_bytes),
                number(x.minor_faults_s, ""),
                number(x.rss_growth_bytes_s.map(|v| v / divisor), ""),
                x.pss_at_ms
                    .map(|at| format!("{}s", t(a).at_ms.saturating_sub(at) / 1000))
                    .unwrap_or("—".into()),
                x.verdict.clone(),
            ]
        })
        .collect::<Vec<_>>();
    table(
        f,
        rss,
        &[
            "process",
            if a.memory_gib { "RSS GiB" } else { "RSS MiB" },
            if a.memory_gib { "PSS GiB" } else { "PSS MiB" },
            "anon",
            "file",
            "shmem",
            "swap",
            "minor/s",
            if a.memory_gib { "GiB/s Δ" } else { "MiB/s Δ" },
            "PSS age",
            "verdict",
        ],
        &data,
        &[24, 9, 9, 8, 8, 8, 8, 10, 10, 9, 18],
        a.selected,
    );
}
fn block(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(1),
            Constraint::Length(10),
            Constraint::Min(15),
        ],
    );
    controls(
        f,
        bands[0],
        "show devices / partitions / dm / mounts    latency block request completion",
        a,
    );
    let dev = p(
        f,
        bands[1],
        a,
        1,
        "devices",
        "completed latency ≠ pending age",
    );
    history_table(
        f,
        dev,
        a,
        &crate::model::DEVICE_COLUMNS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        &rows(a, 4),
        &[14, 12, 9, 9, 9, 11, 11, 7, 8, 10],
        ("device:", ":read"),
    );
    let cols = horizontal(
        bands[2],
        &[Constraint::Percentage(38), Constraint::Percentage(62)],
    );
    let detail = p(f, cols[0], a, 2, "selected block device", "queue topology");
    summary_fields(f, detail, a, "device");
    let graphs = p(
        f,
        cols[1],
        a,
        3,
        "throughput / request lifecycle",
        "▲ read · ▼ write",
    );
    let plots = vertical(
        graphs,
        &[
            Constraint::Min(6),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
        ],
    );
    graph(
        f,
        plots[0],
        selected_device_series(a, "read"),
        selected_device_series(a, "write"),
        a.cursor(),
    );
    for (i, (key, label)) in [
        ("disk.iops", "IOPS"),
        ("disk.p99", "completed latency p99 · ms"),
        ("disk.queue", "queue depth"),
    ]
    .iter()
    .enumerate()
    {
        line(f, plots[1 + i * 2], *label, DIM);
        if i == 1 {
            let device = rows(a, 4)
                .get(a.selected)
                .and_then(|r| r.first())
                .and_then(|name| t(a).devices.iter().find(|d| d.name == *name));
            let key = device
                .map(|d| format!("block.dev{}.p99", d.major_minor))
                .unwrap_or_else(|| key.to_string());
            hist(f, plots[2 + i * 2], a, &key, true);
        } else {
            history(
                f,
                plots[2 + i * 2],
                selected_device_series(a, if i == 0 { "iops" } else { "queue" }),
                a.cursor(),
                CYAN,
                false,
            );
        }
    }
}
fn histogram(f: &mut Frame, r: Rect, a: &App, key: &str) {
    if let Some(h) = t(a).histograms.get(key) {
        let bars = r.height.saturating_sub(2);
        let vals = h.counts.iter().map(|n| Some(*n as f64)).collect::<Vec<_>>();
        dots(
            f,
            Rect::new(r.x, r.y, r.width, bars),
            &vals,
            h.counts.iter().copied().max().unwrap_or(1) as f64,
            GOLD,
            false,
            true,
        );
        line(
            f,
            Rect::new(r.x, r.bottom().saturating_sub(1), r.width, 1),
            format!(
                "log buckets ({})  {}   · n={}",
                h.unit,
                h.bounds
                    .iter()
                    .map(|v| v.to_string())
                    .chain(
                        (h.counts.len() > h.bounds.len())
                            .then(|| format!(">{}", h.bounds.last().unwrap_or(&0.)))
                    )
                    .collect::<Vec<_>>()
                    .join("  "),
                h.counts.iter().sum::<u64>()
            ),
            DIM,
        );
    } else {
        line(f, r, "Histogram acquisition unavailable", DIM);
    }
}
fn syscall_empty(f: &mut Frame, r: Rect, a: &App) {
    use crate::domain::Quality;
    let area = p(
        f,
        r,
        a,
        1,
        "Syscalls",
        "l capture · x stop · : scoped capture",
    );
    let quality = t(a).capabilities.get("syscalls");
    let mut lines = match quality {
        Some(Quality::Available) => vec![
            "Capture active — waiting for completed syscalls.".to_string(),
            "A call appears after both its entry and return are observed.".into(),
        ],
        Some(Quality::Error(error) | Quality::Denied(error) | Quality::Unsupported(error)) => {
            vec!["Syscall capture unavailable".into(), error.clone()]
        }
        _ => vec![
            "No syscall capture has been collected.".into(),
            "This tab needs kernel tracing; procfs cannot provide syscall events.".into(),
        ],
    };
    lines.push(String::new());
    if a.replay.is_some() {
        lines.push("This recording contains no syscall rows. Open a recording made during syscall capture.".into());
    } else if !a.snapshot.demo {
        lines.extend([
            "Press l to capture all visible system syscalls for 30 seconds.".into(),
            "Capture requires BPF privileges. If denied, restart kernwatch as root and press l."
                .into(),
            "For a quieter capture: :probe syscalls pid=TID seconds=30".into(),
            "pid= selects one thread. x stops capture; collected rows remain available.".into(),
        ]);
    }
    if let Some(scope) = t(a).details.get("probe.scope") {
        lines.push(String::new());
        lines.extend(scope.iter().map(|(k, v)| format!("{k}: {v}")));
    }
    f.render_widget(
        Paragraph::new(lines.join("\n"))
            .style(Style::default().fg(FG))
            .wrap(Wrap { trim: false }),
        area,
    );
}
fn syscalls(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(2),
            Constraint::Length(12),
            Constraint::Min(10),
            Constraint::Length(10),
        ],
    );
    controls(
        f,
        bands[0],
        "l capture 30s · x stop · P profile stacks · g all / errors / >1ms · E errors",
        a,
    );
    capture_status(
        f,
        Rect::new(bands[0].x, bands[0].y + 1, bands[0].width, 1),
        a,
        "syscalls",
    );
    let tab = p(
        f,
        bands[1],
        a,
        1,
        "syscalls · completed calls",
        "s sort by duration",
    );
    let mut display_rows = rows(a, 5);
    for row in &mut display_rows {
        for value in row.iter_mut().skip(4).take(3) {
            if let Some(ms) = crate::app::duration_ms(value) {
                *value = latency(Some(ms));
            }
        }
    }
    history_table(
        f,
        tab,
        a,
        &a.snapshot.views[5].columns,
        &display_rows,
        &[18, 10, 10, 12, 10, 10, 10, 10, 10, 16],
        ("syscall.", ".p99"),
    );
    let stream = p(
        f,
        bands[2],
        a,
        2,
        "live event stream",
        "pause follows display freeze",
    );
    let events = a
        .visible_syscall_events()
        .into_iter()
        .take(stream.height.saturating_sub(3) as usize)
        .map(|e| Line::raw(format!("{} {}", event_clock(a, e.at_ms), e.message)))
        .collect();
    let mut events: Vec<Line<'static>> = events;
    if events.is_empty() {
        events.push(Line::styled(
            "No completed events match this syscall and filter",
            Style::default().fg(DIM),
        ));
    }
    events.push(Line::raw(""));
    events.push(Line::styled(
        "Completed duration includes blocking; use scheduler tracing to isolate wakeup delay.",
        Style::default().fg(DIM),
    ));
    events.push(Line::styled("Lookup errors do not establish slab allocation or a leak. : stop-probe · f pause · e export",Style::default().fg(DIM)));
    text(f, stream, events);
    let selected = rows(a, 5)
        .get(a.selected)
        .and_then(|r| r.first())
        .cloned()
        .unwrap_or_default();
    let h = p(
        f,
        bands[3],
        a,
        3,
        "latency distribution",
        &format!("{selected} · completed duration"),
    );
    let key = format!("syscall:{selected}");
    histogram(f, h, a, &key);
}
fn irq(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(13),
            Constraint::Length(14),
            Constraint::Min(8),
        ],
    );
    let tab = p(
        f,
        bands[0],
        a,
        1,
        "hardware IRQs",
        "rate · affinity · handler duration",
    );
    let mut data = rows(a, 6);
    for row in &mut data {
        row.push(
            if row.get(4).is_some_and(|v| v.parse::<u32>().is_ok()) {
                "single CPU"
            } else if row
                .get(4)
                .is_some_and(|v| crate::actions::validate_cpus(v).is_ok())
            {
                "multiple CPUs"
            } else {
                "unknown"
            }
            .into(),
        );
    }
    let mut columns = a.snapshot.views[6].columns.clone();
    columns.push("placement".into());
    history_table(
        f,
        tab,
        a,
        &columns,
        &data,
        &[6, 26, 12, 12, 12, 18, 12, 14],
        ("irq.", ".rate"),
    );
    let matrix = p(
        f,
        bands[1],
        a,
        2,
        softirq_caption(a),
        "10s avg · counts are not time",
    );
    softirq_matrix(f, matrix, a, true);
    let gr = p(
        f,
        bands[2],
        a,
        3,
        "IRQ rate / softirq execution",
        "▲ IRQ/s · ▼ softirq %",
    );
    graph(
        f,
        gr,
        t(a).series.get(
            &a.rows()
                .get(a.selected)
                .and_then(|r| r.first())
                .map(|id| format!("irq.{id}.rate"))
                .unwrap_or_default(),
        ),
        t(a).series.get(
            &a.rows()
                .get(a.selected)
                .and_then(|r| r.get(4))
                .and_then(|v| v.parse::<u32>().ok())
                .map(|cpu| format!("softirq.cpu{cpu}"))
                .unwrap_or_default(),
        ),
        a.cursor(),
    );
}
fn cgroups(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(1),
            Constraint::Length(13),
            Constraint::Length(11),
            Constraint::Min(8),
            Constraint::Length(5),
        ],
    );
    controls(
        f,
        bands[0],
        "g tree / flat / throttled / pressure / cpu / mem / io",
        a,
    );
    let tr = p(
        f,
        bands[1],
        a,
        1,
        "cgroups",
        "Space fold · hierarchy inclusive",
    );
    history_table(
        f,
        tr,
        a,
        &crate::model::CGROUP_COLUMNS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        &rows(a, 7),
        &[30, 6, 10, 16, 12, 11, 10, 9, 9, 8, 13],
        ("cgroup:", ":cpu"),
    );
    let detail = p(f, bands[2], a, 2, "selected cgroup", "cgroup v2");
    summary_fields(f, detail, a, "cgroup");
    let why = p(
        f,
        bands[3],
        a,
        3,
        "quota / runtime / throttling",
        "CPU waiting does not consume quota",
    );
    let plots = vertical(
        why,
        &[
            Constraint::Length(1),
            Constraint::Min(2),
            Constraint::Length(1),
            Constraint::Min(2),
            Constraint::Length(2),
        ],
    );
    line(
        f,
        plots[0],
        if a.pressure_view {
            "CPU pressure · some avg10 %"
        } else {
            "CPU runtime · % of one logical CPU"
        },
        DIM,
    );
    let group = rows(a, 7)
        .get(a.selected)
        .and_then(|r| r.first())
        .cloned()
        .unwrap_or_default();
    let runtime_key = format!("cgroup:{group}:cpu");
    if a.pressure_view {
        hist(f, plots[1], a, &format!("cgroup:{group}:psi.cpu"), false);
        line(f, plots[2], "IO pressure · some avg10 %", DIM);
        hist(f, plots[3], a, &format!("cgroup:{group}:psi.io"), false);
    } else {
        hist(f, plots[1], a, &runtime_key, false);
        if let Some(g) = t(a).cgroups.iter().find(|g| g.path == group) {
            let numbers = g
                .quota
                .split_whitespace()
                .filter_map(|v| v.parse::<f64>().ok())
                .collect::<Vec<_>>();
            if numbers.len() == 2 && numbers[1] > 0. && plots[1].height > 0 {
                let quota = numbers[0] / numbers[1] * 100.;
                let max = t(a).series.get(&runtime_key).map(|s| s.max).unwrap_or(100.);
                let y = plots[1].bottom()
                    - 1
                    - ((quota / max).clamp(0., 1.) * (plots[1].height - 1) as f64) as u16;
                for x in plots[1].x..plots[1].right() {
                    if x % 2 == 0 {
                        f.buffer_mut()[(x, y)].set_char('─').set_fg(GOLD);
                    }
                }
                line(
                    f,
                    Rect::new(plots[1].x, y, plots[1].width.min(24), 1),
                    format!("quota {quota:.1}% / one CPU"),
                    GOLD,
                );
            }
        }
        line(f, plots[2], "throttled milliseconds / second", DIM);
        hist(f, plots[3], a, &format!("cgroup:{group}:throttled"), true);
    }
    line(
        f,
        plots[4],
        "Compare measured runtime against quota; inspect ancestor limits independently.",
        FG,
    );
    let ac = p(f, bands[4], a, 4, "actions", "dry-run before apply");
    note(
        f,
        ac,
        &[
            "↵ tasks in group    c cpuset preview    Q quota preview",
            "u inspect unit · p pressure history · y copy path",
        ],
    );
}
fn modules(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(1),
            Constraint::Length(13),
            Constraint::Min(12),
            Constraint::Length(9),
        ],
    );
    controls(
        f,
        bands[0],
        "show all / out-of-tree / unsigned / new / unused    B baseline · x mark",
        a,
    );
    let tb = p(f, bands[1], a, 1, "modules", "baseline and trust");
    table(
        f,
        tb,
        &a.snapshot.views[8]
            .columns
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        &rows(a, 8),
        &[25, 11, 5, 20, 12, 14, 13, 14, 18],
        a.selected,
    );
    let de = p(
        f,
        bands[2],
        a,
        2,
        "module inspector",
        "unknown metadata stays unknown",
    );
    summary_fields(f, de, a, "module");
    let ta = p(
        f,
        bands[3],
        a,
        3,
        "taint / module events",
        "historical state persists",
    );
    let mut content = t(a)
        .details
        .get("taint")
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| Line::raw(format!("{k}: {v}")))
        .collect::<Vec<_>>();
    if let Some(baseline) = t(a).details.get("module baseline") {
        content.extend(baseline.iter().map(|(k, v)| Line::raw(format!("{k}: {v}"))));
    }
    content.extend(
        t(a).events
            .iter()
            .filter(|e| e.subject.starts_with("module:") && e.at_ms <= a.cursor())
            .rev()
            .take(4)
            .map(|e| {
                Line::raw(format!(
                    "{} · {} · {}",
                    clock(a, e.at_ms),
                    e.source,
                    e.message
                ))
            }),
    );
    text(f, ta, content);
}
fn ebpf(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(2),
            Constraint::Length(20),
            Constraint::Length(11),
            Constraint::Min(9),
        ],
    );
    controls(
        f,
        bands[0],
        "show all / this capture / other programs   / filter name or type",
        a,
    );
    capture_status(
        f,
        Rect::new(bands[0].x, bands[0].y + 1, bands[0].width, 1),
        a,
        "bpf",
    );
    let tb = p(
        f,
        bands[1],
        a,
        1,
        "programs",
        ": probe … enables runtime stats · one-core %",
    );
    history_table(
        f,
        tb,
        a,
        &a.snapshot.views[9].columns.clone(),
        &rows(a, 9),
        &[6, 21, 20, 12, 11, 10, 7, 17, 22],
        ("bpf.program", ""),
    );
    let de = p(f, bands[2], a, 2, "program / maps", "selected program");
    summary_fields(f, de, a, "bpf");
    let ov = p(
        f,
        bands[3],
        a,
        3,
        "overhead by owner / creator UID",
        &format!(
            "total {} · own {}",
            value(a, "bpf.total", "%"),
            value(a, "bpf.own", "%")
        ),
    );
    let mut owners = std::collections::BTreeMap::<String, Option<f64>>::new();
    for row in &a.snapshot.views[9].rows {
        if row.len() < 8 {
            continue;
        }
        let entry = owners.entry(row[7].clone()).or_insert(Some(0.));
        *entry = entry.zip(row[5].parse::<f64>().ok()).map(|(a, b)| a + b);
    }
    let mut owners: Vec<_> = owners.into_iter().collect();
    owners.sort_by(|a, b| b.1.unwrap_or(-1.).total_cmp(&a.1.unwrap_or(-1.)));
    let count = owners.len().min(5).min(ov.height as usize / 2);
    let maximum = owners
        .iter()
        .filter_map(|(_, v)| *v)
        .reduce(f64::max)
        .unwrap_or(1.)
        .max(1.);
    for (i, (owner, value)) in owners.iter().take(count).enumerate() {
        let y = ov.y + i as u16 * 2;
        line(f, Rect::new(ov.x, y, 20, 1), owner, DIM);
        if let Some(v) = value {
            meter(
                f,
                Rect::new(ov.x + 21, y, ov.width.saturating_sub(31), 1),
                *v,
                maximum,
                CYAN,
            );
        } else {
            line(
                f,
                Rect::new(ov.x + 21, y, ov.width.saturating_sub(31), 1),
                "runtime statistics unavailable",
                DIM,
            );
        }
        line(
            f,
            Rect::new(ov.right() - 9, y, 9, 1),
            number(*value, "%"),
            FG,
        );
    }
    let used = count as u16 * 2 + 1;
    if ov.height > count as u16 * 2 {
        line(
            f,
            Rect::new(ov.x, ov.y + count as u16 * 2, ov.width, 1),
            format!(
                "{} / {} owner groups shown · own probe history · {} drops",
                count,
                owners.len(),
                t(a).trace_drops
            ),
            DIM,
        );
    }
    if ov.height > used + 2 {
        hist(
            f,
            Rect::new(ov.x, ov.y + used, ov.width, ov.height - used),
            a,
            "bpf.own",
            false,
        );
    }
}
fn logs(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(1),
            Constraint::Min(12),
            Constraint::Length(7),
            Constraint::Length(10),
        ],
    );
    controls(
        f,
        bands[0],
        "level all / warning / error   / filter message or source   follow newest",
        a,
    );
    let log = p(
        f,
        bands[1],
        a,
        1,
        "kernel / trace log",
        "timestamp · source · event",
    );
    let events = a.visible_events();
    let capacity = log.height as usize / 2;
    let selected = a.selected.min(events.len().saturating_sub(1));
    let offset = selected.saturating_sub(capacity.saturating_sub(1));
    for (row, e) in events.iter().skip(offset).take(capacity).enumerate() {
        let rect = Rect::new(log.x, log.y + row as u16 * 2, log.width, 2);
        f.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    format!("{} · {} · {}", clock(a, e.at_ms), e.source, e.severity),
                    Style::default().fg(DIM),
                ),
                Line::styled(
                    e.message.clone(),
                    Style::default().fg(severity(&e.severity)),
                ),
            ])
            .style(Style::default().bg(if offset + row == selected {
                SELECT
            } else {
                BG
            })),
            rect,
        );
    }
    let rate = p(
        f,
        bands[2],
        a,
        2,
        "kernel kmsg event rate",
        "1s / column · source-specific",
    );
    hist(f, rate, a, "events.rate", false);
    let trace = p(
        f,
        bands[3],
        a,
        3,
        "tracing / correlation",
        "acquisition health",
    );
    let mut lines = vec![Line::raw(format!(
        "events retained {} · lost {}",
        t(a).events.len(),
        t(a).trace_drops
    ))];
    if let Some(scope) = t(a).details.get("probe.scope") {
        lines.extend(scope.iter().map(|(k, v)| Line::raw(format!("{k}: {v}"))));
    }
    for chunk in t(a).capabilities.iter().collect::<Vec<_>>().chunks(3) {
        lines.push(Line::raw(
            chunk
                .iter()
                .map(|(k, v)| format!("{k}: {v:?}"))
                .collect::<Vec<_>>()
                .join(" · "),
        ));
    }
    lines.push(Line::raw("↵ source evidence · c correlate in Diagnose"));
    text(f, trace, lines);
}
fn diagnose(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(
        r,
        &[
            Constraint::Length(1),
            Constraint::Min(16),
            Constraint::Length(9),
            Constraint::Length(9),
        ],
    );
    line(
        f,
        bands[0],
        if t(a).issues.is_empty() {
            " analysis · no active findings · inspect source readiness"
        } else {
            " analysis · findings available · causes require evidence · verify selected issue"
        },
        DIM,
    );
    let cols = horizontal(
        bands[1],
        &[Constraint::Percentage(31), Constraint::Percentage(69)],
    );
    let issues = p(f, cols[0], a, 1, "issues", "active first · severity");
    let capacity = (issues.height as usize / 4).max(1);
    let offset = a.selected.saturating_sub(capacity - 1);
    for (i, issue) in t(a).issues.iter().enumerate().skip(offset) {
        let y = issues.y + (i - offset) as u16 * 4;
        if y + 2 >= issues.bottom() {
            break;
        }
        let rect = Rect::new(issues.x, y, issues.width, 3);
        f.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    format!("▲ {}", issue.title),
                    Style::default().fg(GOLD).bold(),
                ),
                Line::raw(issue.subject.clone()),
                Line::styled(
                    format!("since {} · {}", clock(a, issue.since_ms), issue.state),
                    Style::default().fg(DIM),
                ),
            ])
            .style(Style::default().bg(if i == a.selected { SELECT } else { BG }))
            .wrap(Wrap { trim: false }),
            rect,
        );
    }
    let selected = t(a)
        .issues
        .get(a.selected.min(t(a).issues.len().saturating_sub(1)));
    let cause = p(
        f,
        cols[1],
        a,
        2,
        "cause chain / evidence",
        "observed vs hypothesis",
    );
    if let Some(issue) = selected {
        let n = issue.causes.len().max(1);
        let card_w = 23u16;
        let per_row = (cause.width / (card_w + 1)).max(1) as usize;
        for (i, c) in issue.causes.iter().enumerate() {
            let row = i / per_row;
            let col = i % per_row;
            let rect = Rect::new(
                cause.x + col as u16 * (card_w + 1),
                cause.y + row as u16 * 5,
                card_w.min(cause.width),
                4,
            );
            if rect.bottom() > cause.bottom() {
                break;
            }
            let inner = panel(
                f,
                rect,
                0,
                &c.label,
                if c.observed { "seen" } else { "?" },
                false,
            );
            text(
                f,
                inner,
                vec![Line::styled(
                    c.detail.clone(),
                    Style::default().fg(if c.observed { FG } else { GOLD }),
                )],
            );
        }
        let used = n.div_ceil(per_row) as u16 * 5;
        if cause.height > used + 3 {
            let evidence = Rect::new(cause.x, cause.y + used, cause.width, cause.height - used);
            let data = issue
                .evidence
                .iter()
                .map(|e| {
                    vec![
                        e.label.clone(),
                        e.source.clone(),
                        clock(a, e.at_ms),
                        format!("{:.2}", e.weight),
                    ]
                })
                .collect::<Vec<_>>();
            table(
                f,
                evidence,
                &["evidence", "source", "seen", "weight"],
                &data,
                &[50, 25, 15, 10],
                a.evidence_index,
            );
        }
    }
    let remediation = p(
        f,
        bands[2],
        a,
        3,
        "remediation / verification",
        "Enter dry-run · v verify",
    );
    if let Some(issue) = selected {
        let mut lines = issue
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| Line::raw(format!("{}  {s}", i + 1)))
            .collect::<Vec<_>>();
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("verify  {}", issue.verification),
            Style::default().fg(CYAN),
        ));
        text(f, remediation, lines);
    }
    let report = p(
        f,
        bands[3],
        a,
        4,
        "report preview",
        "whole snapshot · Markdown · e export bundle",
    );
    f.render_widget(
        Paragraph::new(
            crate::recording::report(&a.snapshot)
                .lines()
                .skip_while(|line| !line.starts_with("## Observations"))
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join(
                    "
",
                ),
        )
        .wrap(Wrap { trim: false }),
        report,
    );
}
fn compact(f: &mut Frame, r: Rect, a: &App) {
    if a.tab == 12 {
        let bands = vertical(
            r,
            &[
                Constraint::Length(5),
                Constraint::Min(7),
                Constraint::Length(6),
            ],
        );
        cpu_panel(f, bands[0], a, 1, true);
        let middle = horizontal(
            bands[1],
            &[Constraint::Percentage(50), Constraint::Percentage(50)],
        );
        dense_irq(f, middle[0], a);
        dense_scheduler(f, middle[1], a);
        timeline(f, bands[2], a, 8);
        return;
    }
    if a.tab == 13 {
        flame(f, r, a);
        return;
    }
    let bands = vertical(r, &[Constraint::Min(8), Constraint::Length(6)]);
    if a.tab == 0 || a.tab == 12 {
        cpu_panel(f, bands[0], a, 1, true);
    } else if a.tab == 1 {
        tasks_panel(f, bands[0], a, 1);
    } else {
        let inner = p(
            f,
            bands[0],
            a,
            1,
            crate::model::TABS[a.tab].0,
            "compact · Space expand",
        );
        let (columns, data) = match a.tab {
            11 => (
                vec!["Issue".into(), "Subject".into(), "State".into()],
                t(a).issues
                    .iter()
                    .map(|i| vec![i.title.clone(), i.subject.clone(), i.state.clone()])
                    .collect(),
            ),
            3 => (
                vec![
                    "Process".into(),
                    "RSS MiB".into(),
                    "PSS MiB".into(),
                    "PSS age".into(),
                ],
                a.memory_tasks()
                    .iter()
                    .map(|x| {
                        vec![
                            format!("{} {}", x.pid, x.name),
                            format!("{:.1}", x.rss_bytes as f64 / 1048576.),
                            number(x.pss_bytes.map(|v| v as f64 / 1048576.), ""),
                            x.pss_at_ms
                                .map(|at| format!("{}s", t(a).at_ms.saturating_sub(at) / 1000))
                                .unwrap_or("—".into()),
                        ]
                    })
                    .collect(),
            ),
            2 => (
                vec!["Subject".into(), "Wake p99".into(), "Capture".into()],
                a.scheduler_subjects()
                    .iter()
                    .map(|x| {
                        vec![
                            x.0.clone(),
                            latency(t(a).series.get(&x.1).and_then(|s| s.at(a.cursor()))),
                            format!(
                                "{:?}",
                                t(a).capabilities
                                    .get("scheduler")
                                    .cloned()
                                    .unwrap_or_default()
                            ),
                        ]
                    })
                    .collect(),
            ),
            _ => (a.snapshot.views[a.tab].columns.clone(), a.rows()),
        };
        let headers = columns.iter().map(String::as_str).collect::<Vec<_>>();
        table(
            f,
            inner,
            &headers,
            &data,
            &vec![1; headers.len().max(1)],
            a.selected,
        );
    }
    let inner = p(f, bands[1], a, 2, "findings / context", "d diagnose");
    text(
        f,
        inner,
        a.snapshot
            .findings
            .iter()
            .take(3)
            .map(|s| Line::raw(s.clone()))
            .collect(),
    );
}

fn selected_device_series<'a>(a: &'a App, suffix: &str) -> Option<&'a crate::domain::Series> {
    let name = rows(a, 4)
        .get(a.selected)
        .and_then(|r| r.first())
        .cloned()?;
    t(a).series.get(&format!("device:{name}:{suffix}"))
}

fn softirq_matrix(f: &mut Frame, r: Rect, a: &App, header: bool) {
    let traced = softirq_traced(a);
    let count = ((r.width.saturating_sub(9)) / 4)
        .min(t(a).cpus.len() as u16)
        .min(16);
    let offset = if header { 1 } else { 0 };
    if header {
        line(
            f,
            Rect::new(r.x, r.y, 8.min(r.width), 1),
            if traced { "exec %" } else { "events/s" },
            DIM,
        );
        for (i, cpu) in t(a).cpus.iter().take(count as usize).enumerate() {
            right_line(
                f,
                Rect::new(r.x + 9 + i as u16 * 4, r.y, 4, 1),
                cpu.id.to_string(),
                DIM,
            );
        }
    }
    for (row, (vector, name)) in [
        (3, "NET_RX"),
        (4, "BLOCK"),
        (1, "TIMER"),
        (2, "NET_TX"),
        (7, "SCHED"),
        (9, "RCU"),
    ]
    .iter()
    .enumerate()
    {
        if row as u16 + offset >= r.height {
            break;
        }
        let y = r.y + offset + row as u16;
        right_line(f, Rect::new(r.x, y, 8, 1), *name, DIM);
        for (i, cpu) in t(a).cpus.iter().take(count as usize).enumerate() {
            let value = if a.snapshot.demo {
                Some(if *vector == 3 { cpu.softirq } else { 0. })
            } else if traced {
                t(a).value(&format!("softirq.cpu{}.vec{vector}", cpu.id))
            } else {
                t(a).value(&format!("softirq.count.cpu{}.{name}", cpu.id))
            };
            let rect = Rect::new(r.x + 9 + i as u16 * 4, y, 4, 1);
            right_line(
                f,
                rect,
                value
                    .map(|v| {
                        if v >= 1000. {
                            format!("{:>3.0}k", v / 1000.)
                        } else {
                            format!("{v:>3.0}")
                        }
                    })
                    .unwrap_or("  —".into()),
                if traced && value.unwrap_or(0.) > 50. {
                    GOLD
                } else {
                    FG
                },
            );
        }
    }
}

/// Full-screen inspection of each numbered panel, including compact terminals.
pub fn expanded(f: &mut Frame, r: Rect, a: &App) {
    let key = (a.tab, a.focus);
    match key {
        (12, 0) | (0, 0) => cpu_panel(f, r, a, 1, true),
        (12, 6) | (0, 2) | (1, 0) => tasks_panel(f, r, a, a.focus + 1),
        (12, 7) | (0, 3) => timeline(f, r, a, a.focus + 1),
        (6, 1) | (12, 3) => {
            let inner = p(f, r, a, a.focus + 1, softirq_caption(a), "Esc close");
            softirq_matrix(f, inner, a, true);
        }
        (12, 2) => {
            let inner = p(f, r, a, 3, "block devices", "Esc close");
            let data = t(a)
                .devices
                .iter()
                .map(|d| {
                    vec![
                        d.name.clone(),
                        number(d.read_mib_s, "MiB/s"),
                        number(d.write_mib_s, "MiB/s"),
                        d.inflight.to_string(),
                        number(d.await_ms, "ms"),
                    ]
                })
                .collect::<Vec<_>>();
            table(
                f,
                inner,
                &["device", "read", "write", "in flight", "mean await"],
                &data,
                &[25, 20, 20, 15, 20],
                a.selected,
            );
        }
        (1, 3) | (7, 3) => {
            let inner = p(f, r, a, 4, "reviewed actions", "Esc close");
            if let Some(plan) = &a.pending_action {
                fields(
                    f,
                    inner,
                    &[
                        ("review id".into(), plan.id.to_string()),
                        ("target".into(), plan.target.display().to_string()),
                        ("before".into(), plan.before.clone()),
                        ("after".into(), plan.after.clone()),
                        ("scope / impact".into(), plan.description.clone()),
                        ("outcome".into(), plan.outcome.clone()),
                    ],
                );
            } else {
                note(
                    f,
                    inner,
                    &[
                        ": preview task PID START_TICKS CPUS",
                        ": preview irq ID CPUS",
                        ": preview quota GROUP QUOTA PERIOD",
                        ": preview cpuset GROUP CPUS",
                        "Apply requires the exact reviewed ID; no action is implied by inspection.",
                    ],
                );
            }
        }
        (4, 1) | (7, 1) | (8, 1) | (9, 1) => {
            let inner = p(f, r, a, a.focus + 1, "selected object", "Esc close");
            summary_fields(
                f,
                inner,
                a,
                match a.tab {
                    4 => "device",
                    7 => "cgroup",
                    8 => "module",
                    _ => "bpf",
                },
            );
        }
        (3, 0) | (3, 1) | (8, 2) | (12, 5) => {
            let inner = p(f, r, a, a.focus + 1, "source detail", "Esc close");
            summary_fields(
                f,
                inner,
                a,
                match key {
                    (3, 0) => "slab",
                    (3, 1) => "numa",
                    _ => "taint",
                },
            );
        }
        (4, 2) => {
            let inner = p(f, r, a, 3, "device throughput", "Esc close");
            graph(
                f,
                inner,
                selected_device_series(a, "read"),
                selected_device_series(a, "write"),
                a.cursor(),
            );
        }
        (3, 2) => {
            let inner = p(f, r, a, 3, "allocation / reclaim", "pages/s");
            graph(
                f,
                inner,
                t(a).series.get("alloc"),
                t(a).series.get("reclaim"),
                a.cursor(),
            );
        }
        (6, 2) => {
            let inner = p(f, r, a, 3, "IRQ rate / NET_RX duration", "Esc close");
            graph(
                f,
                inner,
                t(a).series.get("irq.rate"),
                t(a).series.get("softirq"),
                a.cursor(),
            );
        }
        (1, 2) | (2, 1) | (12, 4) => {
            let inner = p(
                f,
                r,
                a,
                a.focus + 1,
                "wakeup latency",
                "Esc close · h histogram",
            );
            if a.histogram {
                histogram(f, inner, a, "sched");
            } else {
                let key = if a.tab == 1 {
                    a.selected_task()
                        .map(|t| task_latency_key(a, t))
                        .unwrap_or_default()
                } else {
                    format!(
                        "sched.cpu{}",
                        t(a).cpus.get(a.selected).map(|c| c.id).unwrap_or(0)
                    )
                };
                graph(f, inner, t(a).series.get(&key), None, a.cursor());
            }
        }
        (5, 2) => {
            let inner = p(f, r, a, 3, "syscall latency histogram", "completed calls");
            let selected = rows(a, 5)
                .get(a.selected)
                .and_then(|r| r.first())
                .cloned()
                .unwrap_or_default();
            histogram(f, inner, a, &format!("syscall:{selected}"));
        }
        (7, 2) => {
            let inner = p(f, r, a, 3, "cgroup runtime / throttling", "Esc close");
            let group = rows(a, 7)
                .get(a.selected)
                .and_then(|r| r.first())
                .cloned()
                .unwrap_or_default();
            graph(
                f,
                inner,
                t(a).series.get(&format!("cgroup:{group}:cpu")),
                t(a).series.get(&format!("cgroup:{group}:throttled")),
                a.cursor(),
            );
        }
        (10, 0) | (5, 1) => {
            let inner = p(
                f,
                r,
                a,
                a.focus + 1,
                "event records",
                "↑↓ scroll · Esc close",
            );
            let events = if a.tab == 5 {
                a.visible_syscall_events()
            } else {
                t(a).events.iter().collect()
            };
            let lines = events
                .into_iter()
                .filter(|e| e.message.to_lowercase().contains(&a.filter.to_lowercase()))
                .map(|e| Line::raw(format!("{} {} {}", clock(a, e.at_ms), e.source, e.message)))
                .collect::<Vec<_>>();
            f.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .scroll((a.scroll, 0)),
                inner,
            );
        }
        (10, 1) | (9, 2) => {
            let inner = p(f, r, a, a.focus + 1, "measurement history", "Esc close");
            graph(
                f,
                inner,
                t(a).series.get(if a.tab == 10 {
                    "events.rate"
                } else {
                    "bpf.own"
                }),
                None,
                a.cursor(),
            );
        }
        (11, _) => {
            let inner = p(
                f,
                r,
                a,
                a.focus + 1,
                "diagnosis / report",
                "↑↓ scroll · Esc close",
            );
            let report = crate::recording::report(&a.snapshot);
            f.render_widget(
                Paragraph::new(report)
                    .wrap(Wrap { trim: false })
                    .scroll((a.scroll, 0)),
                inner,
            );
        }
        (1, 1) | (2, 2) => {
            let inner = p(
                f,
                r,
                a,
                a.focus + 1,
                "task placement / latency",
                "Esc close",
            );
            if let Some(task) = a.selected_task() {
                fields(
                    f,
                    inner,
                    &[
                        ("task".into(), format!("{} pid {}", task.name, task.pid)),
                        (
                            "identity".into(),
                            format!("tgid {} start {}", task.tgid, task.start_ticks),
                        ),
                        ("state".into(), task.state.clone()),
                        ("CPU".into(), task.cpu.to_string()),
                        ("affinity".into(), task.affinity.clone()),
                        ("cgroup".into(), task.cgroup.clone()),
                        ("wchan".into(), task.wchan.clone()),
                        ("wakeup p99".into(), number(task.wake_p99_ms, "ms")),
                    ],
                );
            }
        }
        (12, 1) | (0, 1) => {
            let inner = p(f, r, a, a.focus + 1, "memory / health", "Esc close");
            let fields = t(a)
                .metrics
                .iter()
                .map(|(key, m)| {
                    (
                        key.clone(),
                        format!(
                            "{} · {} · {:?}",
                            number(m.value, &m.unit),
                            m.source,
                            m.quality
                        ),
                    )
                })
                .collect::<Vec<_>>();
            fields_widget(f, inner, &fields, a.scroll);
        }
        (10, 2) | (9, 3) => {
            let inner = p(f, r, a, a.focus + 1, "probe health / controls", "Esc close");
            let fields = t(a)
                .capabilities
                .iter()
                .map(|(key, v)| (key.clone(), format!("{v:?}")))
                .collect::<Vec<_>>();
            fields_widget(f, inner, &fields, a.scroll);
        }
        _ => {
            let inner = p(f, r, a, a.focus + 1, "focused table / actions", "Esc close");
            let view = &a.snapshot.views[a.tab];
            table(
                f,
                inner,
                &view.columns.iter().map(String::as_str).collect::<Vec<_>>(),
                &a.rows(),
                &vec![1; view.columns.len().max(1)],
                a.selected,
            );
        }
    }
}
/// Detail fields as two aligned columns rather than `name: value` prose.
///
/// Names share one dim column sized to the widest of them, so every value
/// starts at the same offset and the column can be scanned downward. A value
/// too long for the remaining width wraps under itself instead of returning to
/// the left edge, where a continuation reads as a nameless field.
fn fields_widget(f: &mut Frame, r: Rect, items: &[(String, String)], scroll: u16) {
    if r.width == 0 || r.height == 0 {
        return;
    }
    let widest = items
        .iter()
        .map(|(k, _)| Line::raw(k.as_str()).width())
        .max()
        .unwrap_or(0);
    // Never let one long name push the values off a narrow panel.
    let names = widest.clamp(6, (r.width as usize / 2).max(6));
    let values = (r.width as usize).saturating_sub(names + 2);
    if values < 4 {
        let lines = items
            .iter()
            .map(|(k, v)| Line::raw(format!("{k}: {v}")))
            .collect::<Vec<_>>();
        f.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0)),
            r,
        );
        return;
    }
    let mut lines = Vec::new();
    for (name, value) in items {
        for (index, part) in wrapped(value, values).into_iter().enumerate() {
            let label = if index == 0 {
                format!("{:<names$}  ", ellipsize(name, names))
            } else {
                " ".repeat(names + 2)
            };
            lines.push(Line::from(vec![
                Span::styled(label, Style::default().fg(DIM)),
                Span::styled(part, Style::default().fg(severity(value))),
            ]));
        }
    }
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), r);
}

/// Break text on whitespace to fit `width`, keeping over-long words intact.
fn wrapped(text: &str, width: usize) -> Vec<String> {
    let text = text.replace(['\n', '\t'], " ");
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if Line::raw(&candidate).width() <= width || current.is_empty() {
            current = candidate;
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_owned();
        }
    }
    lines.push(current);
    lines
}

fn history_table(
    f: &mut Frame,
    area: Rect,
    a: &App,
    columns: &[String],
    data: &[Vec<String>],
    widths: &[u16],
    (prefix, suffix): (&str, &str),
) {
    let parts = horizontal(area, &[Constraint::Min(40), Constraint::Length(12)]);
    let mut table_area = parts[0];
    if columns
        .first()
        .is_some_and(|s| s.eq_ignore_ascii_case("irq"))
    {
        table_area.width = table_area.width.saturating_sub(1);
    }
    table(
        f,
        table_area,
        &columns.iter().map(String::as_str).collect::<Vec<_>>(),
        data,
        widths,
        a.selected,
    );
    line(f, Rect::new(parts[1].x, parts[1].y, 12, 1), "history", DIM);
    let count = area.height.saturating_sub(2) as usize;
    let offset = a
        .selected
        .min(data.len().saturating_sub(1))
        .saturating_sub(count.saturating_sub(1));
    for (i, row) in data.iter().skip(offset).take(count).enumerate() {
        if let Some(id) = row.first() {
            hist(
                f,
                Rect::new(parts[1].x, parts[1].y + 2 + i as u16, 12, 1),
                a,
                &if prefix == "scheduler" {
                    a.scheduler_subjects()
                        .get(offset + i)
                        .map(|s| s.1.clone())
                        .unwrap_or_default()
                } else {
                    format!("{prefix}{id}{suffix}")
                },
                false,
            );
        }
    }
}

fn memory_text(value: &str, gib: bool) -> String {
    let mut tokens = value
        .split_whitespace()
        .map(String::from)
        .collect::<Vec<_>>();
    for i in 1..tokens.len() {
        if let Ok(number) = tokens[i - 1].parse::<f64>() {
            if tokens[i] == "GiB" && !gib {
                tokens[i - 1] = format!("{:.1}", number * 1024.);
                tokens[i] = "MiB".into();
            } else if tokens[i] == "MiB" && gib {
                tokens[i - 1] = format!("{:.3}", number / 1024.);
                tokens[i] = "GiB".into();
            }
        }
    }
    tokens.join(" ")
}
fn memory_fields(f: &mut Frame, area: Rect, a: &App, key: &str) {
    if let Some(values) = t(a).details.get(key) {
        // Same two-column layout as every other detail panel; only the unit
        // conversion is particular to memory.
        let converted = values
            .iter()
            .map(|(k, v)| (k.clone(), memory_text(v, a.memory_gib)))
            .collect::<Vec<_>>();
        fields_widget(f, area, &converted, 0);
    } else {
        summary_fields(f, area, a, key);
    }
}

fn event_clock(a: &App, ms: u64) -> String {
    if a.snapshot.demo {
        format!("{}.{:03}", clock(a, ms), ms % 1000)
    } else {
        clock(a, ms)
    }
}
fn capture_status(f: &mut Frame, r: Rect, a: &App, source: &str) {
    let state = if a.snapshot.demo {
        "demo fixture".to_owned()
    } else {
        format!(
            "{:?}",
            t(a).capabilities
                .get(source)
                .unwrap_or(&crate::domain::Quality::Stopped)
        )
    };
    let scope = t(a)
        .details
        .get("probe.scope")
        .and_then(|fields| fields.iter().find(|(k, _)| k == "scope"))
        .map(|(_, v)| v.as_str())
        .unwrap_or("no active capture scope");
    let extra = if source == "bpf" {
        t(a).details
            .get("bpf.status")
            .map(|fields| {
                fields
                    .iter()
                    .map(|(k, v)| format!("{k} {v}"))
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_else(|| {
                if a.snapshot.demo {
                    "synthetic inventory".into()
                } else {
                    "inventory status unavailable".into()
                }
            })
    } else {
        scope.into()
    };
    line(
        f,
        r,
        format!(
            "{state} · {extra} · drops {} · probe CPU {} · ingest {}",
            t(a).trace_drops,
            value(a, "bpf.own", "%"),
            value(a, "trace.ingest_cpu", "%")
        ),
        DIM,
    );
}

#[cfg(test)]
mod timeline_tests {
    use super::*;
    #[test]
    fn event_markers_use_one_column_per_second_after_resize() {
        let mut a = App::new(crate::model::demo());
        a.snapshot.telemetry.at_ms = 600000;
        a.snapshot.telemetry.events = [600000, 597000, 588000]
            .into_iter()
            .map(|at_ms| crate::domain::Event {
                at_ms,
                source: "test".into(),
                ..Default::default()
            })
            .collect();
        for width in [80, 160] {
            let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(width, 8)).unwrap();
            terminal.draw(|f| timeline(f, f.area(), &a, 8)).unwrap();
            let columns: Vec<_> = (0..width)
                .filter(|x| terminal.backend().buffer()[(*x, 6)].symbol() == "▲")
                .collect();
            assert_eq!(columns, vec![width - 14, width - 5, width - 2]);
        }
    }
    #[test]
    fn newly_started_timeline_displays_all_five_measured_lanes() {
        let mut a = App::new(crate::model::demo());
        a.snapshot.telemetry.series.clear();
        a.snapshot.telemetry.at_ms = 50_000;
        for key in [
            "cpu.busy",
            "softirq.peak",
            "runqueue",
            "disk.await_max",
            "dstate",
        ] {
            a.snapshot.telemetry.record(key, Some(2.), "", 100.);
        }
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(100, 8)).unwrap();
        terminal.draw(|f| timeline(f, f.area(), &a, 8)).unwrap();
        for y in 1..=5 {
            let line = (0..100)
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<String>();
            assert!(line.contains("2.0"), "missing value: {line}");
            assert!(
                line.chars().any(|c| ('\u{2801}'..='\u{28ff}').contains(&c)),
                "missing plotted data: {line}"
            );
        }
    }
    #[test]
    fn timeline_uses_live_counters_without_probes_and_preserves_captured_percentiles() {
        let mut t = crate::domain::Telemetry::default();
        for key in [
            "cpu.busy",
            "softirq.peak",
            "runqueue",
            "disk.await_max",
            "dstate",
        ] {
            t.record(key, Some(2.), "", 100.);
        }
        assert_eq!(
            timeline_rows(&t).map(|(_, key)| key),
            [
                "cpu.busy",
                "softirq.peak",
                "runqueue",
                "disk.await_max",
                "dstate"
            ]
        );
        t.record("sched.p99", Some(3.), "ms", 5.);
        t.record("disk.p99", Some(4.), "ms", 5.);
        assert_eq!(timeline_rows(&t)[2], ("sched p99", "sched.p99"));
        assert_eq!(timeline_rows(&t)[3], ("I/O p99", "disk.p99"));
    }
}

#[cfg(test)]
mod field_tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    fn row(buffer: &ratatui::buffer::Buffer, y: u16, width: u16) -> String {
        (0..width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
    }
    #[test]
    fn long_values_break_on_whitespace_and_keep_unbreakable_words_whole() {
        assert_eq!(wrapped("one two three", 7), vec!["one two", "three"]);
        assert_eq!(wrapped("", 10), vec![""]);
        // A word wider than the column is never chopped into fragments.
        assert_eq!(
            wrapped("short supercalifragilistic", 6),
            vec!["short", "supercalifragilistic"]
        );
        for line in wrapped("alpha beta gamma delta epsilon", 11) {
            assert!(line.len() <= 11, "{line:?} exceeds the value column");
        }
    }
    #[test]
    fn field_values_share_one_column_and_continuations_hang_under_it() {
        let items = [
            ("id".to_string(), "7".to_string()),
            ("a much longer name".to_string(), "value".to_string()),
            (
                "note".to_string(),
                "a value long enough that it has to wrap onto a second line".to_string(),
            ),
        ];
        let mut terminal = Terminal::new(TestBackend::new(40, 6)).unwrap();
        terminal
            .draw(|f| fields_widget(f, Rect::new(0, 0, 40, 6), &items, 0))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let lines: Vec<String> = (0..6).map(|y| row(buffer, y, 40)).collect();
        let start = |text: &str| text.find(|c: char| !c.is_whitespace()).unwrap_or(0);
        // Every value begins at the same offset, whatever its name's length.
        let column = lines[0].trim_end().rfind("7").unwrap();
        assert_eq!(lines[1].trim_end().rfind("value"), Some(column));
        assert!(lines[1].starts_with("a much longer name"));
        // The wrapped remainder hangs under the value, not at the left edge.
        assert_eq!(start(&lines[3]), column);
        assert!(
            !lines[3].trim().is_empty(),
            "the second line of a wrapped value should be rendered"
        );
    }
}

/// The subtree currently in view, and the path that reached it.
fn flame_root(a: &App) -> (&crate::flame::Node, Vec<String>) {
    let profile = &t(a).profile;
    let zoom = a.flame_zoom.clone();
    match profile.root.at(&zoom) {
        Some(node) => (node, zoom),
        // A zoom that no longer matches the capture falls back to the whole
        // profile rather than showing an empty panel.
        None => (&profile.root, Vec::new()),
    }
}

fn flame(f: &mut Frame, r: Rect, a: &App) {
    if let (Some(mode), Some(baseline)) = (a.flame_compare, &a.flame_baseline) {
        flame_difference(f, r, a, baseline, mode);
        return;
    }
    if a.flame_picking() {
        flame_picker(f, r, a);
        return;
    }
    let profile = &t(a).profile;
    // An empty profile explains itself in prose, which needs more rows than a
    // shallow graph would.
    let deepest = flame_root(a).0.depth();
    let wanted = if profile.is_empty() { 16 } else { deepest + 3 };
    let graph = wanted.clamp(8, r.height.saturating_sub(10).max(8));
    let bands = vertical(
        r,
        &[
            Constraint::Length(1),
            Constraint::Length(graph),
            Constraint::Min(6),
        ],
    );
    line(
        f,
        bands[0],
        format!(
            " {} samples · ←→ sibling · ↓ callee · ↑ caller · h hottest · ↵ zoom · Esc back · / find{}",
            profile.root.samples,
            if a.snapshot.demo {
                " · demo fixture"
            } else {
                ""
            }
        ),
        DIM,
    );
    let (root, zoom) = flame_root(a);
    let subtitle = if profile.is_empty() {
        match flame_progress(a) {
            Some((elapsed, total)) if matches!(capture_state(a), Capture::Running) => {
                format!("capturing · {elapsed}s of {total}s")
            }
            _ => "no stacks collected".to_string(),
        }
    } else {
        let mut parts = Vec::new();
        if let (Capture::Running, Some((elapsed, total))) = (capture_state(a), flame_progress(a)) {
            parts.push(format!("{elapsed}s of {total}s"));
        }
        parts.push(format!("{} samples", root.samples));
        if let Some(share) = profile.shallow_share().filter(|s| *s > 0.) {
            parts.push(format!("{share:.0}% shallow user stacks"));
        }
        if profile.unresolved > 0 {
            parts.push(format!("{} unresolved", profile.unresolved));
        }
        if !a.filter.is_empty() {
            let needle = a.filter.to_lowercase();
            let (frames, samples) = matching_frames(root, &needle, profile);
            parts.push(format!(
                "{frames} frames matching {:?} · {samples} stacks",
                a.filter
            ));
        }
        parts.join(" · ")
    };
    // Name the subject in the title: a profile is unreadable without knowing
    // whose stacks it is.
    let subject = flame_subject(a);
    let title = if zoom.is_empty() {
        subject.clone()
    } else {
        format!("{subject} · {}", zoom.join(" › "))
    };
    let area = p(f, bands[1], a, 1, &title, &subtitle);
    let mut cells = crate::flame::layout(root, area.width, area.height, &a.flame_cursor);
    for cell in &mut cells.cells {
        cell.name = profile.label(&cell.name).to_owned();
    }
    // The cursor can sit on any frame, so the highlighted cell is the one
    // whose path matches it rather than a child of the root.
    let selected = cells
        .cells
        .iter()
        .position(|c| c.path == a.flame_cursor)
        .unwrap_or(usize::MAX);
    if profile.is_empty() {
        text(
            f,
            area,
            flame_empty_lines(a).into_iter().map(Line::raw).collect(),
        );
    } else {
        icicle(f, area, &cells.cells, selected, &a.filter.to_lowercase());
    }
    let detail = p(
        f,
        bands[2],
        a,
        2,
        "subject / frame",
        if profile.metadata.kind == crate::flame::Kind::Cpu {
            "CPU samples"
        } else {
            "syscall-entry stacks"
        },
    );
    // Lead with the frame under the cursor: that is what the reader is
    // investigating. The subject and its caveats follow it.
    let mut fields = Vec::new();
    match a.flame_frame() {
        Some(node) if !a.flame_cursor.is_empty() => {
            let share = node.samples as f64 / root.samples.max(1) as f64 * 100.;
            fields.push(("frame".into(), profile.label(&node.name).to_owned()));
            if let Some(info) = profile.frames.get(&node.name) {
                fields.push(("image".into(), info.image.clone()));
                fields.push(("raw symbol".into(), info.raw.clone()));
            }
            fields.push((
                "stacks".into(),
                format!("{} · {share:.1}% of {}", node.samples, root.name),
            ));
            fields.push((
                "in this frame".into(),
                format!(
                    "{} not accounted for by callees ({:.1}%)",
                    node.own(),
                    node.own() as f64 / root.samples.max(1) as f64 * 100.
                ),
            ));
            fields.push((
                "callees".into(),
                if node.children.is_empty() {
                    "none; this is where the stack ends".into()
                } else {
                    let heaviest = &node.children[0];
                    format!(
                        "{} · heaviest {} at {:.1}%",
                        node.children.len(),
                        profile.label(&heaviest.name),
                        heaviest.samples as f64 / node.samples.max(1) as f64 * 100.
                    )
                },
            ));
            fields.push((
                "path".into(),
                a.flame_path()
                    .iter()
                    .map(|id| profile.label(id))
                    .collect::<Vec<_>>()
                    .join(" › "),
            ));
        }
        _ => {
            fields.push((
                "frame".into(),
                "none selected — ↓ enters the profile, h jumps to the hottest path".into(),
            ));
        }
    }
    // Name what a stand-in is standing in for, so the frames folded into it are
    // reachable rather than merely counted.
    if let Some(parent) = a.flame_frame() {
        let drawn: std::collections::BTreeSet<&str> = cells
            .cells
            .iter()
            .filter(|c| c.path.len() == a.flame_cursor.len() + 1)
            .map(|c| c.name.as_str())
            .collect();
        let folded: Vec<String> = parent
            .children
            .iter()
            .filter(|c| !drawn.contains(profile.label(&c.name)))
            .map(|c| {
                format!(
                    "{} {:.1}%",
                    profile.label(&c.name),
                    c.samples as f64 / root.samples.max(1) as f64 * 100.
                )
            })
            .collect();
        if !folded.is_empty() {
            fields.push((
                "folded here".into(),
                format!(
                    "{} callees below one column: {}",
                    folded.len(),
                    folded.join(" · ")
                ),
            ));
        }
    }
    if cells.hidden > 0 {
        fields.push((
            "not drawn".into(),
            format!(
                "{} frames below one column, with no room for a stand-in",
                cells.hidden
            ),
        ));
    }
    fields.push((String::new(), String::new()));
    fields.extend(flame_subject_fields(a));
    fields_widget(f, detail, &fields, 0);
}

/// What the profile is of.
fn flame_subject(a: &App) -> String {
    if t(a).profile.metadata.kind == crate::flame::Kind::Cpu {
        return t(a).profile.source.clone();
    }
    match &a.flame_target {
        Some(s) if s.threads > 1 => format!("{} · TID {} of {}", s.name, s.tid, s.threads),
        Some(s) => format!("{} · TID {}", s.name, s.tid),
        None if !t(a).profile.source.is_empty() => t(a).profile.source.clone(),
        None => "stacks".into(),
    }
}

/// Elapsed and total seconds of the capture in progress, if one is running.
fn flame_progress(a: &App) -> Option<(u64, u64)> {
    let fields = t(a).details.get("probe.progress")?;
    let read = |name: &str| {
        fields
            .iter()
            .find(|(key, _)| key == name)
            .and_then(|(_, v)| v.parse::<u64>().ok())
    };
    Some((read("elapsed seconds")?, read("duration seconds")?))
}

/// Whether a capture is running, finished, or never started.
enum Capture {
    Starting,
    Running,
    Finished,
    Refused(String),
    None,
}
fn capture_state(a: &App) -> Capture {
    match t(a).capabilities.get(
        if match t(a).profile.metadata.kind {
            crate::flame::Kind::Cpu => true,
            crate::flame::Kind::Syscalls => false,
            crate::flame::Kind::Unknown => a.flame_cpu,
        } {
            "cpu"
        } else {
            "syscalls"
        },
    ) {
        Some(crate::domain::Quality::Available) => Capture::Running,
        Some(
            crate::domain::Quality::Denied(why)
            | crate::domain::Quality::Error(why)
            | crate::domain::Quality::Unsupported(why),
        ) => Capture::Refused(why.clone()),
        // A capture that ran and stopped leaves its capability Stopped. Without
        // this it read as one that never started. Between the request and the
        // probe attaching the capability is also Stopped, and no capture has
        // identified itself yet: that is starting, not finished.
        Some(crate::domain::Quality::Stopped)
            if a.flame_target.is_some()
                || t(a).profile.metadata.kind == crate::flame::Kind::Cpu =>
        {
            if t(a).details.contains_key("probe.capture_id") {
                Capture::Finished
            } else {
                Capture::Starting
            }
        }
        _ => Capture::None,
    }
}

/// The subject being profiled, as detail rows.
fn flame_subject_fields(a: &App) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    if let Some(s) = &a.flame_target {
        fields.push(("process".into(), format!("{} · TGID {}", s.name, s.tgid)));
        // The probe filters on a thread id, so a process with many threads is
        // not captured whole. Saying so beats letting the graph imply it.
        fields.push((
            "captured thread".into(),
            if t(a).profile.metadata.kind == crate::flame::Kind::Cpu {
                t(a).profile.metadata.scope.clone()
            } else if s.threads > 1 {
                format!(
                    "{} · TID {} — the busiest of {} threads; the rest are not captured",
                    s.busiest, s.tid, s.threads
                )
            } else {
                format!("{} · TID {}", s.busiest, s.tid)
            },
        ));
        fields.push((
            "cpu / memory".into(),
            format!(
                "{:.1}% · {:.0} MiB RSS",
                s.cpu_pct,
                s.rss_bytes as f64 / 1048576.
            ),
        ));
        if !s.cgroup.is_empty() {
            fields.push(("cgroup".into(), s.cgroup.clone()));
        }
    } else if !t(a).profile.source.is_empty() {
        fields.push(("source".into(), t(a).profile.source.clone()));
    }
    fields.push((
        "capture".into(),
        match capture_state(a) {
            Capture::Starting => "requested · waiting for the probe to attach".into(),
            Capture::Running => match flame_progress(a) {
                Some((elapsed, total)) => format!(
                    "running · {elapsed}s of {total}s · x stops it and keeps what was collected"
                ),
                None => "running · x stops it".into(),
            },
            Capture::Finished => "finished · the profile below is what it collected".into(),
            Capture::Refused(why) => format!("did not run · {why}"),
            Capture::None => "none started · P chooses a subject".into(),
        },
    ));
    fields.push((
        "scope".into(),
        if t(a).profile.metadata.kind == crate::flame::Kind::Cpu { format!("CPU observations · {} · requested {} Hz · {} CPUs", t(a).profile.metadata.scope, t(a).profile.metadata.frequency_hz, t(a).profile.metadata.cpus.len()) } else { "stacks are taken at syscall entry; time spent on CPU between syscalls is not represented".into() },
    ));
    let profile = &t(a).profile;
    if profile.shallow > 0 {
        fields.push((
            "shallow user stacks".into(),
            format!(
                "{} of {} samples; possible incomplete walk, not proof of truncation",
                profile.shallow,
                if profile.metadata.kind == crate::flame::Kind::Cpu {
                    profile.quality.user_stacks
                } else {
                    profile.root.samples
                }
            ),
        ));
    }
    if profile.metadata.kind == crate::flame::Kind::Unknown {
        fields.push((
            "quality".into(),
            "capture diagnostics unavailable in this recording".into(),
        ));
    } else {
        fields.push(("quality".into(), format!("{} attempted · {} user · {} kernel · {} failed · {} partial · {} depth limit · {} map failures", profile.quality.attempted, profile.quality.user_stacks, profile.quality.kernel_stacks, profile.quality.failed, profile.quality.partial, profile.quality.depth_limit, profile.quality.map_failures)));
    }
    fields.extend(
        profile
            .quality
            .errors
            .iter()
            .map(|(reason, count)| (reason.clone(), count.to_string())),
    );
    fields.extend(
        profile
            .metadata
            .warnings
            .iter()
            .map(|warning| ("warning".into(), warning.clone())),
    );
    fields
}

/// The subjects a profile can be taken of, so one can be chosen here.
fn flame_picker(f: &mut Frame, r: Rect, a: &App) {
    let bands = vertical(r, &[Constraint::Length(1), Constraint::Min(8)]);
    let threads = a.mode == 1;
    line(
        f,
        bands[0],
        format!(
            " {} · ↑↓ select · ↵ or P 30s capture · C kind · g {} · / filter",
            if a.flame_cpu {
                "CPU 49 Hz"
            } else {
                "syscall entries"
            },
            if threads {
                "group by process"
            } else {
                "list every thread"
            }
        ),
        DIM,
    );
    let subjects = a.flame_subjects();
    let subtitle = if a.snapshot.demo {
        "demo fixture · capture requires live mode".to_string()
    } else {
        format!(
            "{} {} · C toggles CPU/syscall capture · B baseline · D compare",
            subjects.len(),
            if threads { "threads" } else { "processes" }
        )
    };
    let area = p(
        f,
        bands[1],
        a,
        1,
        if threads {
            "choose a thread to profile"
        } else {
            "choose a process to profile"
        },
        &subtitle,
    );
    let rows = subjects
        .iter()
        .map(|s| {
            vec![
                s.name.clone(),
                if threads {
                    s.tid.to_string()
                } else {
                    s.tgid.to_string()
                },
                if threads {
                    "—".into()
                } else {
                    s.threads.to_string()
                },
                format!("{:.1}", s.cpu_pct),
                format!("{:.0}", s.rss_bytes as f64 / 1048576.),
                s.cgroup.clone(),
            ]
        })
        .collect::<Vec<_>>();
    table(
        f,
        area,
        &[
            if threads { "thread" } else { "process" },
            if threads { "TID" } else { "TGID" },
            "threads",
            "CPU %",
            "RSS MiB",
            "cgroup",
        ],
        &rows,
        &[26, 9, 9, 9, 9, 34],
        a.selected,
    );
}

/// Why a profile is empty, in the terms the reader can act on.
fn flame_empty_lines(a: &App) -> Vec<String> {
    let mut lines = vec!["No stacks have been collected.".to_string(), String::new()];
    if t(a).profile.metadata.kind == crate::flame::Kind::Cpu {
        lines.push(format!(
            "CPU attempts: {} · failed walks: {} · map failures: {}",
            t(a).profile.quality.attempted,
            t(a).profile.quality.failed,
            t(a).profile.quality.map_failures
        ));
        lines.extend(
            t(a).profile
                .quality
                .errors
                .iter()
                .map(|(reason, n)| format!("{n} {reason}")),
        );
        match capture_state(a) {
            Capture::Refused(why) => lines.push(format!("CPU capture did not run: {why}")),
            Capture::Running => lines
                .push("CPU sampling is running; an idle target may yield no observations.".into()),
            Capture::Finished => {
                lines.push("CPU capture finished; P selects another target.".into())
            }
            _ => lines.push("CPU sampler is waiting to attach.".into()),
        }
        return lines;
    }
    match capture_state(a) {
        Capture::Starting => {
            lines.push("The capture has been requested and the probe is attaching.".into());
        }
        Capture::Running => {
            match flame_progress(a) {
                Some((elapsed, total)) => {
                    lines.push(format!("The capture is running: {elapsed}s of {total}s."))
                }
                None => lines.push("The capture is running.".into()),
            }
            match t(a).details.get("probe.stacks") {
                Some(tally) => {
                    lines.push("What it has seen so far:".into());
                    lines.extend(
                        tally
                            .iter()
                            .map(|(reason, count)| format!("  {count} {reason}")),
                    );
                    lines.push(String::new());
                    lines.push(
                        "A walk failure can mean no frame pointer; rebuilding with \
                         -fno-omit-frame-pointer may help. The errno does not prove the cause."
                            .into(),
                    );
                }
                None => lines.push(
                    "It has recorded no syscall yet. Stacks are taken at syscall entry, so a \
                     thread that makes none produces nothing."
                        .into(),
                ),
            }
        }
        Capture::Finished => {
            lines.push("The capture finished without collecting a stack.".into());
            lines.push(String::new());
            match t(a).details.get("probe.stacks") {
                Some(tally) => {
                    lines.push("What it saw:".into());
                    lines.extend(
                        tally
                            .iter()
                            .map(|(reason, count)| format!("  {count} {reason}")),
                    );
                }
                None => lines.push(
                    "It recorded no syscall at all. Pick a busier thread, or one that does I/O."
                        .into(),
                ),
            }
            lines.push(String::new());
            lines.push("P chooses another subject.".into());
        }
        Capture::Refused(why) => {
            lines.push("The last capture did not run:".into());
            lines.push(format!("  {why}"));
            lines.push(String::new());
            lines.push("Stack capture needs BPF privileges; restart as root.".into());
        }
        Capture::None => {
            lines.push("Nothing has been captured yet.".into());
            lines.push("  P  choose a process or thread and profile it for 30s".into());
            lines.push(String::new());
            lines.push("Opening this view collects nothing on its own.".into());
        }
    }
    lines
}

/// How many frames match a search, and how many stacks pass through them.
fn matching_frames(
    node: &crate::flame::Node,
    needle: &str,
    profile: &crate::flame::Profile,
) -> (usize, u64) {
    let mut frames = 0;
    let mut samples = 0;
    if profile.label(&node.name).to_lowercase().contains(needle) {
        frames += 1;
        samples += node.samples;
    }
    for child in &node.children {
        let (f, s) = matching_frames(child, needle, profile);
        frames += f;
        if !profile.label(&node.name).to_lowercase().contains(needle) {
            samples += s;
        }
    }
    (frames, samples)
}

fn flame_difference(
    f: &mut Frame,
    r: Rect,
    a: &App,
    baseline: &crate::flame::Profile,
    mode: crate::flame::ComparisonMode,
) {
    let current = &t(a).profile;
    let area = p(
        f,
        r,
        a,
        1,
        "profile comparison",
        "red grew · green shrank · :profile-diff counts|share|off",
    );
    let comparison = match current.compare(baseline, mode) {
        Ok(c) => c,
        Err(e) => {
            text(f, area, vec![Line::raw(e)]);
            return;
        }
    };
    let unit = if mode == crate::flame::ComparisonMode::Share {
        "percentage points"
    } else {
        "samples"
    };
    let bands = vertical(
        area,
        &[Constraint::Percentage(55), Constraint::Percentage(45)],
    );
    let union = current.comparison_union(baseline);
    let mut layout = crate::flame::layout(&union.root, bands[0].width, bands[0].height, &[]);
    for cell in &mut layout.cells {
        let mut node = &union.root;
        let mut path = Vec::new();
        for index in &cell.path {
            node = &node.children[*index];
            path.push(node.name.clone());
        }
        if cell.folded == 0 {
            let delta = comparison
                .changes
                .iter()
                .find(|c| c.path == path)
                .map_or(0., |c| c.delta);
            cell.delta = Some(delta);
            cell.name = format!("{} {delta:+.1}", union.label(&cell.name));
        }
    }
    icicle(
        f,
        bands[0],
        &layout.cells,
        usize::MAX,
        &a.filter.to_lowercase(),
    );
    let mut changes = comparison.changes;
    changes.sort_by(|x, y| {
        y.delta
            .abs()
            .total_cmp(&x.delta.abs())
            .then_with(|| x.path.cmp(&y.path))
    });
    let mut lines = vec![
        Line::raw(format!(
            "{} baseline → {} current · delta in {unit}; widths = union path counts",
            baseline.root.samples, current.root.samples
        )),
        Line::raw(format!(
            "quality: baseline {} failed / {} attempts; current {} failed / {} attempts",
            baseline.quality.failed,
            baseline.quality.attempted,
            current.quality.failed,
            current.quality.attempted
        )),
    ];
    lines.extend(comparison.warnings.into_iter().map(Line::raw));
    lines.push(Line::raw(
        "   baseline    current       delta  call path (inclusive counts)",
    ));
    let needle = a.filter.to_lowercase();
    for change in changes {
        let path = change
            .path
            .iter()
            .map(|id| {
                current
                    .frames
                    .get(id)
                    .or_else(|| baseline.frames.get(id))
                    .map_or(id.as_str(), |f| f.display.as_str())
            })
            .collect::<Vec<_>>()
            .join(" › ");
        if !path.to_lowercase().contains(&needle) {
            continue;
        }
        lines.push(Line::styled(
            format!(
                "{:11} {:10} {:+11.2}  {}",
                change.before, change.after, change.delta, path
            ),
            Style::default().fg(if change.delta > 0. {
                Color::LightRed
            } else if change.delta < 0. {
                Color::LightGreen
            } else {
                Color::Gray
            }),
        ));
    }
    f.render_widget(Paragraph::new(lines).scroll((a.scroll, 0)), bands[1]);
}
