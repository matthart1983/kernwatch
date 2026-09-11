//! Bounded terminal primitives. Braille geometry follows NetWatch's MIT graph renderer.
use crate::domain::Series;
use ratatui::{prelude::*, widgets::*};
pub const BG: Color = Color::Rgb(11, 16, 21);
pub const FG: Color = Color::Rgb(193, 205, 216);
pub const DIM: Color = Color::Rgb(139, 158, 173);
pub const CYAN: Color = Color::Rgb(65, 202, 212);
pub const PURPLE: Color = Color::Rgb(170, 128, 228);
pub const GOLD: Color = Color::Rgb(226, 179, 49);
pub const RED: Color = Color::Rgb(245, 103, 88);
pub const GREEN: Color = Color::Rgb(61, 190, 91);
pub const BORDER: Color = Color::Rgb(42, 57, 70);
pub const SELECT: Color = Color::Rgb(25, 40, 53);
pub fn vertical(area: Rect, c: &[Constraint]) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(c)
        .split(area)
}
pub fn horizontal(area: Rect, c: &[Constraint]) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(c)
        .split(area)
}
pub fn panel(f: &mut Frame, area: Rect, id: usize, title: &str, meta: &str, focus: bool) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if focus { CYAN } else { BORDER }))
        .title(Line::from(vec![
            Span::styled(
                if id == 0 {
                    " ".into()
                } else {
                    format!(" {id} ")
                },
                Style::default().fg(BG).bg(CYAN).bold(),
            ),
            Span::styled(format!(" {title} "), Style::default().fg(CYAN).bold()),
        ]));
    let block = if area.width as usize > Line::raw(title).width() + Line::raw(meta).width() + 10 {
        block.title(
            Line::styled(format!(" {meta} "), Style::default().fg(DIM)).alignment(Alignment::Right),
        )
    } else {
        block
    };
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}
fn safe_text(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                '�'
            } else {
                c
            }
        })
        .collect()
}
pub fn ellipsize(s: &str, width: usize) -> String {
    let s = safe_text(s).replace('\n', " ↵ ").replace('\t', " ");
    if Line::raw(&s).width() <= width {
        return s;
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let n = Span::raw(c.to_string()).width();
        if used + n >= width {
            break;
        }
        out.push(c);
        used += n;
    }
    out.push('…');
    out
}
pub fn text(f: &mut Frame, area: Rect, mut lines: Vec<Line<'static>>) {
    for line in &mut lines {
        for span in &mut line.spans {
            span.content = safe_text(&span.content).into();
        }
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(FG)),
        area,
    );
}
pub fn line(f: &mut Frame, area: Rect, label: impl Into<String>, color: Color) {
    f.render_widget(
        Paragraph::new(safe_text(&label.into())).style(Style::default().fg(color)),
        area,
    );
}
pub fn fields(f: &mut Frame, area: Rect, items: &[(String, String)]) {
    let lines = items
        .iter()
        .map(|(k, v)| {
            Line::from(vec![
                Span::styled(format!("{k:<15} "), Style::default().fg(DIM)),
                Span::raw(v.clone()),
            ])
        })
        .collect();
    text(f, area, lines);
}
pub fn severity(s: &str) -> Color {
    if s.contains("blocked") || s.contains("stall") || s == "error" {
        RED
    } else if s.contains("storm")
        || s.contains("warn")
        || s.contains("throttl")
        || s.contains("growth")
    {
        GOLD
    } else if s.contains("starved") || s.contains("wait") {
        Color::LightBlue
    } else if s.contains("unsigned") || s.contains("new") {
        PURPLE
    } else if s == "ok" || s == "healthy" || s == "resolved" {
        GREEN
    } else {
        FG
    }
}
pub fn right_line(f: &mut Frame, area: Rect, text: impl Into<String>, color: Color) {
    f.render_widget(
        Paragraph::new(text.into())
            .alignment(Alignment::Right)
            .style(Style::default().fg(color)),
        area,
    );
}
pub fn latency(value: Option<f64>) -> String {
    match value {
        Some(v) if v >= 1. => format!("{v:.1}ms"),
        Some(v) if v >= 0.001 => format!("{:.1}µs", v * 1000.),
        Some(v) => format!("{:.0}ns", v * 1_000_000.),
        None => "—".into(),
    }
}
pub fn table(
    f: &mut Frame,
    area: Rect,
    headers: &[&str],
    rows: &[Vec<String>],
    widths: &[u16],
    selected: usize,
) {
    if area.height == 0 {
        return;
    }
    if rows.is_empty() && area.height > 2 {
        line(
            f,
            Rect::new(area.x, area.y + 2, area.width, 1),
            "No rows in this scope",
            DIM,
        );
    }
    let available = area
        .width
        .saturating_sub(widths.len().saturating_sub(1) as u16);
    let total = widths.iter().sum::<u16>().max(1) as u32;
    let mut sizes = widths
        .iter()
        .map(|w| (*w as u32 * available as u32 / total) as u16)
        .collect::<Vec<_>>();
    let remainder = available.saturating_sub(sizes.iter().sum());
    if let Some(first) = sizes.first_mut() {
        *first += remainder;
    }
    let constraints = sizes
        .iter()
        .copied()
        .map(Constraint::Length)
        .collect::<Vec<_>>();
    let count = area.height.saturating_sub(2) as usize;
    let selected = selected.min(rows.len().saturating_sub(1));
    let offset = selected.saturating_sub(count.saturating_sub(1));
    let mut state = TableState::default();
    if !rows.is_empty() {
        state.select(Some(selected - offset));
    }
    let alignment = if headers
        .first()
        .is_some_and(|s| s.eq_ignore_ascii_case("irq"))
    {
        Alignment::Right
    } else {
        Alignment::Left
    };
    let data = rows.iter().skip(offset).map(|r| {
        Row::new(r.iter().enumerate().map(|(i, s)| {
            Cell::from(
                Line::raw(ellipsize(s, sizes.get(i).copied().unwrap_or(0) as usize))
                    .alignment(alignment),
            )
            .style(Style::default().fg(severity(s)))
        }))
    });
    f.render_stateful_widget(
        Table::new(data, constraints)
            .header(
                Row::new(headers.iter().enumerate().map(|(i, s)| {
                    Cell::from(
                        Line::raw(ellipsize(s, sizes.get(i).copied().unwrap_or(0) as usize))
                            .alignment(alignment),
                    )
                }))
                .style(Style::default().fg(DIM))
                .bottom_margin(1),
            )
            .highlight_style(Style::default().bg(SELECT))
            .column_spacing(1),
        area,
        &mut state,
    );
}
fn lerp(a: Color, b: Color, t: f64) -> Color {
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => Color::Rgb(
            (ar as f64 + (br as f64 - ar as f64) * t) as u8,
            (ag as f64 + (bg as f64 - ag as f64) * t) as u8,
            (ab as f64 + (bb as f64 - ab as f64) * t) as u8,
        ),
        _ => b,
    }
}
pub fn graph_scale(max: f64) -> f64 {
    if !max.is_finite() || max <= 0. {
        return 1.;
    }
    let magnitude = 10f64.powf(max.log10().floor());
    let normalized = max / magnitude;
    let step = [1., 2., 5., 10.]
        .into_iter()
        .find(|step| normalized <= *step * (1. + 1e-12))
        .unwrap_or(10.);
    (step * magnitude).max(max)
}

/// Missing samples paint nothing. Both columns in a braille cell have independent samples.
pub fn dots(
    f: &mut Frame,
    area: Rect,
    values: &[Option<f64>],
    max: f64,
    color: Color,
    flip: bool,
    alarm: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    // Own the full plot rectangle, including cells which become blank.
    // This also makes overlapping redraws independent of their previous height.
    f.render_widget(Clear, area);
    f.render_widget(Block::default().style(Style::default().bg(BG)), area);
    if !max.is_finite() || max <= 0. || values.is_empty() {
        return;
    }
    let n = area.width as usize * 2;
    let mut samples = vec![None; n];
    for (i, slot) in samples.iter_mut().enumerate() {
        let lo = i * values.len() / n;
        let hi = ((i + 1) * values.len() / n).max(lo + 1).min(values.len());
        *slot = values.get(lo..hi).and_then(|v| {
            v.iter()
                .filter_map(|v| v.filter(|n| n.is_finite()))
                .reduce(f64::max)
        });
    }
    let bit = [[0, 1, 2, 6], [3, 4, 5, 7]];
    let subh = area.height as usize * 4;
    for x in 0..area.width as usize {
        for y in 0..area.height as usize {
            let mut bits = 0u32;
            for c in 0..2 {
                if let Some(v) = samples[x * 2 + c] {
                    let h = ((v / max).clamp(0., 1.) * subh as f64).round().max(1.) as usize;
                    for (dy, bit_position) in bit[c].iter().enumerate() {
                        let depth = if flip {
                            y * 4 + dy + 1
                        } else {
                            subh - y * 4 - dy
                        };
                        if h >= depth {
                            bits |= 1 << bit_position;
                        }
                    }
                }
            }
            if bits != 0 {
                let height = if flip {
                    y as f64 / area.height as f64
                } else {
                    1. - y as f64 / area.height as f64
                };
                let fg = if alarm {
                    let level = samples[x * 2..x * 2 + 2]
                        .iter()
                        .flatten()
                        .copied()
                        .reduce(f64::max)
                        .unwrap_or(0.)
                        / max;
                    if level > 0.8 {
                        RED
                    } else if level > 0.45 {
                        GOLD
                    } else {
                        GREEN
                    }
                } else {
                    lerp(BG, color, 0.45 + height * 0.55)
                };
                f.buffer_mut()
                    .get_mut(area.x + x as u16, area.y + y as u16)
                    .set_char(char::from_u32(0x2800 + bits).unwrap())
                    .set_style(Style::default().fg(fg).bg(BG));
            }
        }
    }
}
/// One one-second bucket occupies exactly one terminal character (two braille
/// dot columns). Older samples clip on the left; shorter histories stay padded.
fn time_columns(series: &Series, cursor: u64, width: u16, max_seconds: u64) -> Vec<Option<f64>> {
    if width == 0 {
        return Vec::new();
    }
    let seconds = u64::from(width - 1).min(max_seconds);
    let values = series.window(cursor, seconds);
    let mut columns = vec![None; width as usize - values.len()];
    columns.extend(values);
    columns
}

pub fn history(
    f: &mut Frame,
    area: Rect,
    series: Option<&Series>,
    cursor: u64,
    color: Color,
    alarm: bool,
) {
    history_window(f, area, series, cursor, color, alarm, 600)
}
#[allow(clippy::too_many_arguments)]
pub fn history_window(
    f: &mut Frame,
    area: Rect,
    series: Option<&Series>,
    cursor: u64,
    color: Color,
    alarm: bool,
    seconds: u64,
) {
    series_plot(f, area, series, cursor, color, false, alarm, seconds);
}

/// Shared filled-area primitive for CPU graphs and timeline lanes.
#[allow(clippy::too_many_arguments)]
pub fn series_plot(
    f: &mut Frame,
    area: Rect,
    series: Option<&Series>,
    cursor: u64,
    color: Color,
    flip: bool,
    alarm: bool,
    seconds: u64,
) {
    if let Some(s) = series {
        let values = time_columns(s, cursor, area.width, seconds);
        dots(f, area, &values, graph_scale(s.max), color, flip, alarm);
    } else {
        line(f, area, "— unavailable", DIM);
    }
}

pub fn meter(f: &mut Frame, area: Rect, value: f64, max: f64, color: Color) {
    if area.height == 0 {
        return;
    }
    let filled = ((value / max.max(1.)).clamp(0., 1.) * area.width as f64).round() as u16;
    for x in 0..area.width {
        f.buffer_mut()
            .get_mut(area.x + x, area.y)
            .set_char('▄')
            .set_fg(if x < filled {
                lerp(BG, color, 0.45 + 0.55 * x as f64 / area.width.max(1) as f64)
            } else {
                BORDER
            });
    }
}
pub fn graph(
    f: &mut Frame,
    area: Rect,
    upper: Option<&Series>,
    lower: Option<&Series>,
    cursor: u64,
) {
    if area.width < 12 || area.height < 3 {
        return;
    }
    // A changing magnitude/unit label must not move the time-axis origin.
    let axis = 16.min(area.width.saturating_sub(2));
    let plot = Rect::new(
        area.x + axis,
        area.y,
        area.width - axis - 1,
        area.height - 2,
    );
    let halves = if lower.is_some() {
        vertical(
            plot,
            &[Constraint::Percentage(55), Constraint::Percentage(45)],
        )
    } else {
        vertical(plot, &[Constraint::Percentage(100)])
    };
    if let Some(s) = upper {
        line(
            f,
            Rect::new(area.x, area.y, axis, 1),
            format!("{}{}", graph_scale(s.max), s.unit),
            DIM,
        );
        series_plot(f, halves[0], Some(s), cursor, CYAN, false, false, 600);
    } else {
        line(f, plot, "Trace data unavailable · : probe", DIM);
    }
    if let Some(s) = lower {
        series_plot(f, halves[1], Some(s), cursor, PURPLE, true, false, 600);
        line(f, Rect::new(area.x, halves[1].y, 6, 1), "0", DIM);
    }
    let bottom = Rect::new(plot.x, area.bottom() - 1, plot.width, 1);
    line(
        f,
        bottom,
        format!("−{}s", plot.width.saturating_sub(1)),
        DIM,
    );
    if bottom.width > 20 {
        line(
            f,
            Rect::new(bottom.x + bottom.width / 2, bottom.y, 6, 1),
            format!("−{}s", plot.width.saturating_sub(1) - plot.width / 2),
            DIM,
        );
        line(f, Rect::new(bottom.right() - 3, bottom.y, 3, 1), "now", DIM);
    }
}
#[allow(clippy::too_many_arguments)]
pub fn card(
    f: &mut Frame,
    area: Rect,
    title: &str,
    value: &str,
    sub: &str,
    series: Option<&Series>,
    cursor: u64,
    warn: bool,
) {
    let inner = panel(f, area, 0, title, "", false);
    if inner.height == 0 {
        return;
    }
    line(
        f,
        Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 1),
        value,
        if warn { GOLD } else { GREEN },
    );
    if inner.height > 1 {
        line(
            f,
            Rect::new(inner.x + 1, inner.y + 1, inner.width.saturating_sub(2), 1),
            sub,
            DIM,
        );
    }
    if inner.height > 2 {
        history(
            f,
            Rect::new(
                inner.x + 1,
                inner.bottom() - 1,
                inner.width.saturating_sub(2),
                1,
            ),
            series,
            cursor,
            CYAN,
            false,
        );
    }
}
pub fn number(n: Option<f64>, unit: &str) -> String {
    n.map(|n| format!("{n:.1}{unit}"))
        .unwrap_or_else(|| "—".into())
}

#[cfg(test)]
mod latency_tests {
    #[test]
    fn preserves_submillisecond_latency() {
        assert_eq!(super::latency(Some(0.012)), "12.0µs");
        assert_eq!(super::latency(Some(0.00025)), "250ns");
        assert_eq!(super::latency(Some(18.)), "18.0ms");
        assert_eq!(super::latency(None), "—");
    }
}

#[cfg(test)]
mod irq_alignment_tests {
    use super::*;
    #[test]
    fn irq_headers_and_cells_share_right_edges() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(30, 4)).unwrap();
        terminal
            .draw(|f| {
                table(
                    f,
                    f.size(),
                    &["IRQ", "Name", "Rate"],
                    &[vec!["7".into(), "short".into(), "8".into()]],
                    &[1, 1, 1],
                    0,
                )
            })
            .unwrap();
        let b = terminal.backend().buffer();
        assert_eq!(b.get(9, 0).symbol(), "Q");
        assert_eq!(b.get(9, 2).symbol(), "7");
        assert_eq!(b.get(29, 2).symbol(), "8");
    }
}

#[cfg(test)]
mod timeline_spacing_tests {
    use super::{history_window, CYAN};
    use crate::domain::{Sample, Series};
    use ratatui::{backend::TestBackend, Terminal};

    fn series(samples: &[(u64, Option<f64>)]) -> Series {
        Series {
            max: 10.,
            samples: samples
                .iter()
                .map(|(at_ms, value)| Sample {
                    at_ms: *at_ms,
                    value: *value,
                })
                .collect(),
            ..Default::default()
        }
    }
    fn plotted_columns(s: &Series, cursor: u64) -> Vec<u16> {
        let mut t = Terminal::new(TestBackend::new(61, 1)).unwrap();
        t.draw(|f| history_window(f, f.size(), Some(s), cursor, CYAN, false, 60))
            .unwrap();
        (0..61)
            .filter(|x| {
                t.backend()
                    .buffer()
                    .get(*x, 0)
                    .symbol()
                    .chars()
                    .any(|c| ('\u{2801}'..='\u{28ff}').contains(&c))
            })
            .collect()
    }
    #[test]
    fn value_scale_does_not_change_time_axis_origin() {
        let mut s = series(&(0..=120).map(|i| (i * 1000, Some(5.))).collect::<Vec<_>>());
        let origin = |s: &Series| {
            let mut t = Terminal::new(TestBackend::new(80, 8)).unwrap();
            t.draw(|f| super::graph(f, f.size(), Some(s), None, 120_000))
                .unwrap();
            (0..80)
                .find(|x| {
                    (0..6).any(|y| {
                        t.backend()
                            .buffer()
                            .get(*x, y)
                            .symbol()
                            .chars()
                            .any(|c| ('\u{2801}'..='\u{28ff}').contains(&c))
                    })
                })
                .unwrap()
        };
        let before = origin(&s);
        s.max = 100_000.;
        s.unit = "requests/s".into();
        assert_eq!(origin(&s), before);
    }
    #[test]
    fn refresh_jitter_does_not_move_completed_buckets() {
        let s = series(&[(10_100, Some(1.)), (11_250, Some(2.)), (12_300, Some(3.))]);
        assert_eq!(s.window(12_400, 5), s.window(12_990, 5));
        assert_eq!(
            s.window(12_400, 5),
            vec![None, None, None, Some(1.), Some(2.), Some(3.)]
        );
    }
    #[test]
    fn each_elapsed_second_moves_history_by_one_slot() {
        let s = series(&[(60_100, Some(8.))]);
        assert_eq!(plotted_columns(&s, 60_500), vec![60]);
        assert_eq!(plotted_columns(&s, 60_999), vec![60]);
        assert_eq!(plotted_columns(&s, 61_001), vec![59]);
        assert_eq!(plotted_columns(&s, 62_900), vec![58]);
    }
    #[test]
    fn startup_keeps_fixed_width_and_padding_before_boot() {
        let s = series(&[(250, Some(4.))]);
        let window = s.window(500, 60);
        assert_eq!(window.len(), 61);
        assert!(window[..60].iter().all(Option::is_none));
        assert_eq!(plotted_columns(&s, 500), vec![60]);
    }
    #[test]
    fn missing_seconds_are_not_filled_with_previous_values_or_future_samples() {
        let s = series(&[
            (100, Some(1.)),
            (1_900, None),
            (3_100, Some(3.)),
            (3_900, Some(9.)),
        ]);
        assert_eq!(s.window(3_500, 3), vec![Some(1.), None, None, Some(3.)]);
        assert_eq!(s.window(3_950, 3), vec![Some(1.), None, None, Some(9.)]);
    }
}

#[cfg(test)]
mod redraw_tests {
    use super::*;
    use crate::domain::{Sample, Series};
    use ratatui::{backend::TestBackend, Terminal};
    #[test]
    fn scale_changes_only_at_defined_boundaries() {
        assert_eq!(graph_scale(10.1), 20.);
        assert_eq!(graph_scale(19.9), 20.);
        assert_eq!(graph_scale(20.1), 50.);
        assert_eq!(graph_scale(100.), 100.);
        assert_eq!(graph_scale(0.012), 0.02);
        assert_eq!(graph_scale(f64::NAN), 1.);
    }
    #[test]
    fn small_new_peak_does_not_repaint_completed_history() {
        let mut series = Series {
            max: 10.1,
            samples: (0..60)
                .map(|i| Sample {
                    at_ms: i * 1000,
                    value: Some(5.),
                })
                .collect(),
            ..Default::default()
        };
        let render = |s: &Series| {
            let mut terminal = Terminal::new(TestBackend::new(61, 8)).unwrap();
            terminal
                .draw(|f| history(f, f.size(), Some(s), 60000, CYAN, false))
                .unwrap();
            terminal.backend().buffer().clone()
        };
        let before = render(&series);
        series.max = 10.9;
        series.push(60000, Some(10.9));
        let after = render(&series);
        for x in 0..60 {
            for y in 0..8 {
                assert_eq!(
                    before.get(x, y),
                    after.get(x, y),
                    "completed column {x}, row {y} changed"
                );
            }
        }
    }
    #[test]
    fn uncollected_seconds_stay_blank_even_when_a_previous_sample_is_recent() {
        let s = Series {
            samples: vec![
                Sample {
                    at_ms: 990,
                    value: Some(2.),
                },
                Sample {
                    at_ms: 2010,
                    value: Some(3.),
                },
            ],
            ..Default::default()
        };
        assert_eq!(s.window(2100, 2), vec![Some(2.), None, Some(3.)]);
        assert_eq!(s.window(6000, 2), vec![None, None, None]);
    }
    #[test]
    fn clearing_plot_removes_old_bars_even_with_no_samples() {
        for values in [vec![], vec![None], vec![Some(f64::NAN)]] {
            let mut terminal = Terminal::new(TestBackend::new(10, 4)).unwrap();
            terminal
                .draw(|f| {
                    dots(f, f.size(), &[Some(10.)], 10., CYAN, false, false);
                    dots(f, f.size(), &values, 10., CYAN, false, false);
                })
                .unwrap();
            assert!(terminal
                .backend()
                .buffer()
                .content
                .iter()
                .all(|c| c.symbol() == " "));
        }
    }
    #[test]
    fn alarm_color_uses_each_historical_value_not_canvas_height() {
        let mut terminal = Terminal::new(TestBackend::new(2, 1)).unwrap();
        terminal
            .draw(|f| {
                dots(
                    f,
                    f.size(),
                    &[Some(10.), Some(10.), Some(90.), Some(90.)],
                    100.,
                    CYAN,
                    false,
                    true,
                )
            })
            .unwrap();
        assert_eq!(terminal.backend().buffer().get(0, 0).fg, GREEN);
        assert_eq!(terminal.backend().buffer().get(1, 0).fg, RED);
    }
    #[test]
    fn changing_current_card_alarm_does_not_recolor_history() {
        let series = Series {
            max: 100.,
            samples: vec![Sample {
                at_ms: 1000,
                value: Some(40.),
            }],
            ..Default::default()
        };
        let render = |warn| {
            let mut terminal = Terminal::new(TestBackend::new(30, 6)).unwrap();
            terminal
                .draw(|f| card(f, f.size(), "test", "40%", "", Some(&series), 1000, warn))
                .unwrap();
            (0..30)
                .map(|x| terminal.backend().buffer().get(x, 4).clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(render(false), render(true));
    }
}

#[cfg(test)]
mod fixed_sample_width_tests {
    use super::*;
    use crate::domain::{Sample, Series};
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn samples_have_identical_width_and_spacing_in_every_panel_size() {
        let series = Series {
            max: 10.,
            samples: vec![
                Sample {
                    at_ms: 100000,
                    value: Some(10.),
                },
                Sample {
                    at_ms: 103000,
                    value: Some(10.),
                },
            ],
            ..Default::default()
        };
        for width in [5, 12, 61, 120, 240] {
            let mut terminal = Terminal::new(TestBackend::new(width, 4)).unwrap();
            terminal
                .draw(|f| history(f, f.size(), Some(&series), 103900, CYAN, false))
                .unwrap();
            let columns: Vec<_> = (0..width)
                .filter(|x| terminal.backend().buffer().get(*x, 0).symbol() != " ")
                .collect();
            assert_eq!(columns, vec![width - 4, width - 1], "width {width}");
            for x in columns {
                for y in 0..4 {
                    assert_eq!(terminal.backend().buffer().get(x, y).symbol(), "⣿");
                }
            }
        }
    }

    #[test]
    fn resize_crops_history_without_resampling_values() {
        let series = Series {
            samples: (0..10)
                .map(|i| Sample {
                    at_ms: i * 1000,
                    value: Some(i as f64),
                })
                .collect(),
            ..Default::default()
        };
        assert_eq!(
            time_columns(&series, 9000, 3, 600),
            vec![Some(7.), Some(8.), Some(9.)]
        );
        let wide = time_columns(&series, 9000, 15, 600);
        assert!(wide[..5].iter().all(Option::is_none));
        assert_eq!(
            wide[5..],
            (0..10).map(|i| Some(i as f64)).collect::<Vec<_>>()
        );
        assert_eq!(
            time_columns(&series, 9000, 15, 2)[12..],
            vec![Some(7.), Some(8.), Some(9.)]
        );
        assert!(time_columns(&series, 9000, 0, 600).is_empty());
    }
}

#[cfg(test)]
mod insufficient_history_tests {
    use super::*;
    use crate::domain::{Sample, Series};
    use ratatui::{backend::TestBackend, Terminal};
    #[test]
    fn sparse_history_draws_only_observed_columns_and_never_fills_panel() {
        let s = Series {
            max: 10.,
            samples: vec![
                Sample {
                    at_ms: 990,
                    value: Some(5.),
                },
                Sample {
                    at_ms: 2010,
                    value: Some(8.),
                },
            ],
            ..Default::default()
        };
        for width in [12, 80, 240] {
            let mut t = Terminal::new(TestBackend::new(width, 4)).unwrap();
            t.draw(|f| series_plot(f, f.size(), Some(&s), 2100, CYAN, false, false, 600))
                .unwrap();
            let columns: Vec<_> = (0..width)
                .filter(|x| (0..4).any(|y| t.backend().buffer().get(*x, y).symbol() != " "))
                .collect();
            assert_eq!(columns, vec![width - 3, width - 1]);
        }
    }
    #[test]
    fn cpu_graph_and_timeline_use_identical_sample_geometry() {
        let s = Series {
            max: 10.,
            samples: vec![
                Sample {
                    at_ms: 990,
                    value: Some(5.),
                },
                Sample {
                    at_ms: 2010,
                    value: Some(8.),
                },
            ],
            ..Default::default()
        };
        let mut cpu = Terminal::new(TestBackend::new(97, 6)).unwrap();
        cpu.draw(|f| graph(f, f.size(), Some(&s), None, 2100))
            .unwrap();
        let mut history_plot = Terminal::new(TestBackend::new(80, 4)).unwrap();
        history_plot
            .draw(|f| history(f, f.size(), Some(&s), 2100, CYAN, false))
            .unwrap();
        for x in 0..80 {
            for y in 0..4 {
                assert_eq!(
                    cpu.backend().buffer().get(16 + x, y),
                    history_plot.backend().buffer().get(x, y)
                );
            }
        }
    }
}
