use kernwatch::{app::App, model, ui};
use ratatui::{backend::TestBackend, style::Color, Terminal};
fn color(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::LightBlue => "#72b7ff".into(),
        Color::LightRed => "#ff7070".into(),
        Color::LightGreen => "#70dd90".into(),
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
    if args.get(4).map(String::as_str) == Some("diff") {
        let make = |paths: &[(&str, u64)]| {
            let mut p = kernwatch::flame::Profile::new("synthetic CPU comparison");
            p.metadata.kind = kernwatch::flame::Kind::Cpu;
            p.metadata.unit = "cpu_samples".into();
            for (path, n) in paths {
                p.add(&path.split(';').map(str::to_owned).collect::<Vec<_>>(), *n);
            }
            p.quality.attempted = p.root.samples;
            p.quality.user_stacks = p.root.samples;
            p.sort();
            p
        };
        app.snapshot.telemetry.profile = make(&[
            ("main;parse;headers", 60),
            ("main;cache;lookup", 30),
            ("main;old_path", 10),
        ]);
        app.execute("profile-baseline");
        app.snapshot.telemetry.profile = make(&[
            ("main;parse;headers", 90),
            ("main;cache;lookup", 5),
            ("main;new_path", 5),
        ]);
        app.execute("profile-diff share");
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
