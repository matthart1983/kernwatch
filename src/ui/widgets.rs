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
fn ellipsize(s: &str, width: usize) -> String {
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
    if area.width == 0 || area.height == 0 || max <= 0. || values.is_empty() {
        return;
    }
    let n = area.width as usize * 2;
    let mut samples = vec![None; n];
    for (i, slot) in samples.iter_mut().enumerate() {
        let lo = i * values.len() / n;
        let hi = ((i + 1) * values.len() / n).max(lo + 1).min(values.len());
        *slot = values
            .get(lo..hi)
            .and_then(|v| v.iter().filter_map(|v| *v).reduce(f64::max));
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
                    if height > 0.8 {
                        RED
                    } else if height > 0.45 {
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
pub fn history(
    f: &mut Frame,
    area: Rect,
    series: Option<&Series>,
    cursor: u64,
    color: Color,
    alarm: bool,
) {
    history_window(f, area, series, cursor, color, alarm, 60)
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
    if let Some(s) = series {
        let values = s.window(cursor, seconds);
        dots(f, area, &values, s.max, color, false, alarm);
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
    let axis = upper
        .map(|s| format!("{}{}", s.max, s.unit).chars().count() as u16 + 1)
        .unwrap_or(7)
        .clamp(7, 16)
        .min(area.width.saturating_sub(2));
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
            format!("{}{}", s.max, s.unit),
            DIM,
        );
        dots(
            f,
            halves[0],
            &s.window(cursor, 120),
            s.max,
            CYAN,
            false,
            false,
        );
    } else {
        line(f, plot, "Trace data unavailable · : probe", DIM);
    }
    if let Some(s) = lower {
        dots(
            f,
            halves[1],
            &s.window(cursor, 120),
            s.max,
            PURPLE,
            true,
            false,
        );
        line(f, Rect::new(area.x, halves[1].y, 6, 1), "0", DIM);
    }
    let bottom = Rect::new(plot.x, area.bottom() - 1, plot.width, 1);
    line(f, bottom, "−120s", DIM);
    if bottom.width > 20 {
        line(
            f,
            Rect::new(bottom.x + bottom.width / 2, bottom.y, 6, 1),
            "−60s",
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
            if warn { GOLD } else { CYAN },
            warn,
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
