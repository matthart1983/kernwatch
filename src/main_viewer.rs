use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use kernwatch::{app::App, model, ui};
use ratatui::{
    backend::{CrosstermBackend, TestBackend},
    Terminal,
};
use std::{
    io,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);
#[cfg(unix)]
extern "C" fn stop_signal(_: libc::c_int) {
    EXIT_REQUESTED.store(true, Ordering::Relaxed);
}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            event::DisableMouseCapture
        );
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("kernwatch — Linux kernel observability\n\nkernwatch [--demo | --replay FILE] [--view NAME] [--at MS] [--snapshot | --render WIDTHxHEIGHT] [--trace]\n\nThis platform supports demo and Linux recording replay. Live monitoring requires Linux. --demo uses a synthetic incident. --demo-tour runs a guided synthetic tour; any key takes control.\n--trace starts a bounded 30-second BPF capture (requires kernel permissions).\n--replay reads recorded frames without sampling the host; --at selects a recorded time.\n--snapshot prints JSON. --render prints a terminal frame.\nViews: dense overview tasks scheduler memory block syscalls irq cgroups modules ebpf dmesg diagnose\nKeys: Tab focus, [ ] views, Enter drill, Esc back, / filter, f freeze, r record, e export, : commands, ? help, q quit");
        return Ok(());
    }
    let mut demo = false;
    let mut demo_tour = false;
    let settings = kernwatch::settings::Settings::load();
    let mut tab = settings.as_ref().map(|s| s.view).unwrap_or(12);
    let mut snapshot = false;
    let mut render = None;
    let mut replay_path = None;
    let mut trace = false;
    let mut at = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--demo" => demo = true,
            "--demo-tour" => {
                demo = true;
                demo_tour = true;
            }
            "--trace" => trace = true,
            "--replay" => {
                i += 1;
                replay_path = Some(args.get(i).ok_or("--replay needs a file")?.clone());
            }
            "--at" => {
                i += 1;
                at = Some(
                    args.get(i)
                        .ok_or("--at needs milliseconds")?
                        .parse::<u64>()?,
                );
            }
            "--snapshot" => snapshot = true,
            "--tab" | "--view" => {
                i += 1;
                let name = args.get(i).ok_or("--tab needs a name")?;
                tab = model::TABS
                    .iter()
                    .position(|(n, _)| n.eq_ignore_ascii_case(name))
                    .ok_or("Unknown tab")?;
            }
            "--render" => {
                i += 1;
                let (w, h) = args
                    .get(i)
                    .ok_or("--render needs WIDTHxHEIGHT")?
                    .split_once('x')
                    .ok_or("Expected WIDTHxHEIGHT")?;
                let (w, h): (u16, u16) = (w.parse()?, h.parse()?);
                if w == 0 || h == 0 || w > 400 || h > 200 {
                    return Err("Render size must be 1..400 by 1..200".into());
                }
                render = Some((w, h));
            }
            a => return Err(format!("Unknown argument: {a}").into()),
        }
        i += 1;
    }
    if demo && replay_path.is_some() {
        return Err("--demo and --replay are mutually exclusive".into());
    }
    if trace && (demo || replay_path.is_some() || snapshot || render.is_some()) {
        return Err("--trace requires interactive live mode".into());
    }
    let frames = replay_path
        .as_ref()
        .map(|p| kernwatch::recording::read(std::path::Path::new(p)))
        .transpose()?;
    if frames.as_ref().is_some_and(Vec::is_empty) {
        return Err("Recording is empty".into());
    }
    let initial = if let Some(frames) = &frames {
        frames[0].clone()
    } else if demo {
        model::demo()
    } else {
        return Err("Live monitoring requires Linux. Use --demo-tour, --demo or --replay FILE on this platform.".into());
    };
    let mut app = App::new(initial);
    app.switch(tab);
    app.terminal_theme = settings.as_ref().map(|s| s.terminal_theme).unwrap_or(false);
    if let Some(frames) = frames {
        app.replay = Some(frames);
        app.frozen = true;
        app.status = "Replay loaded · ← → seek".into();
    }
    if let Some(at) = at {
        if let Some(frames) = &app.replay {
            let index = frames
                .iter()
                .rposition(|s| s.telemetry.at_ms <= at)
                .unwrap_or(0);
            app.snapshot = frames[index].clone();
            app.replay_index = index;
        } else {
            app.seek_time(at);
        }
    }
    if demo_tour {
        kernwatch::demo::show(&mut app, 0);
    }
    if snapshot {
        println!("{}", serde_json::to_string_pretty(&app.snapshot)?);
        return Ok(());
    }
    if let Some((w, h)) = render {
        let mut term = Terminal::new(TestBackend::new(w, h))?;
        term.draw(|f| ui::draw(f, &app))?;
        let buffer = term.backend().buffer();
        for y in 0..h {
            let line = (0..w)
                .map(|x| buffer.get(x, y).symbol())
                .collect::<String>();
            println!("{}", line.trim_end());
        }
        return Ok(());
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        previous(info);
    }));
    enable_raw_mode()?;
    let _guard = Guard;
    #[cfg(unix)]
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = stop_signal as *const () as usize;
        libc::sigemptyset(&mut action.sa_mask);
        for signal in [libc::SIGTERM, libc::SIGHUP, libc::SIGINT] {
            libc::sigaction(signal, &action, std::ptr::null_mut());
        }
    }
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut last_tick = Instant::now();
    let mut last_sample = Instant::now();
    let tour_started = Instant::now();
    let mut tour_scene = 0;
    while !app.quit && !EXIT_REQUESTED.load(Ordering::Relaxed) {
        let now = Instant::now();
        app.tick(now.duration_since(last_tick).as_millis() as u64);
        last_tick = now;
        if app.replay.is_none() && last_sample.elapsed() >= Duration::from_secs(1) {
            app.update(model::demo());
            last_sample = now;
        }
        if app.probe_request.take().is_some() {
            app.status = "Live tracing requires Linux; this build supports demo and replay".into();
        }
        if let Some(text) = app.clipboard.take() {
            use std::io::Write;
            write!(
                io::stdout(),
                "\x1b]52;c;{}\x07",
                kernwatch::app::clipboard_base64(text.as_bytes())
            )?;
        }
        if demo_tour {
            let scene = (tour_started.elapsed().as_secs() / kernwatch::demo::SECONDS_PER_SCENE)
                as usize
                % kernwatch::demo::SCENES.len();
            if scene != tour_scene {
                kernwatch::demo::show(&mut app, scene);
                tour_scene = scene;
            }
        }
        terminal.draw(|f| ui::draw(f, &app))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Release {
                    if demo_tour {
                        demo_tour = false;
                        app.status = "Demo tour paused · manual control".into();
                    }
                    app.key(key);
                }
            }
        }
    }
    (kernwatch::settings::Settings {
        view: app.tab,
        terminal_theme: app.terminal_theme,
    })
    .save()?;
    Ok(())
}
