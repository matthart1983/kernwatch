use kernwatch::{app::App, model, ui};
use ratatui::{backend::TestBackend, style::Color, Terminal};
fn color(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::LightBlue => "#72b7ff".into(),
        _ => "#c1cdd8".into(),
    }
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let tab: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(12);
    let w = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(160);
    let h = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(52);
    let mut app = App::new(model::demo());
    app.switch(tab);
    if let Some(scene) = args.get(4).and_then(|s| s.parse::<usize>().ok()) {
        kernwatch::demo::show(&mut app, scene);
    }
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| ui::draw(f, &app)).unwrap();
    let b = terminal.backend().buffer();
    let cells = b
        .content
        .iter()
        .map(|c| serde_json::json!([c.symbol(), color(c.fg), color(c.bg)]))
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::json!({"width":w,"height":h,"cells":cells})
    );
}
