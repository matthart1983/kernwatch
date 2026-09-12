use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen},
};
use kernwatch::{app::App, collect::Collector, model, ui};
use ratatui::{
    backend::{CrosstermBackend, TestBackend},
    Terminal,
};
use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);
extern "C" fn stop_signal(_: libc::c_int) {
    EXIT_REQUESTED.store(true, Ordering::Relaxed);
}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            EndSynchronizedUpdate,
            LeaveAlternateScreen,
            event::DisableMouseCapture
        );
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("kernwatch — Linux kernel observability\n\nkernwatch [--demo | --replay FILE] [--view NAME] [--at MS] [--snapshot | --render WIDTHxHEIGHT] [--trace]\n\nLive mode reads procfs/sysfs. --demo uses a synthetic incident. --demo-tour runs a guided synthetic tour; any key takes control.\n--trace starts a bounded 30-second BPF capture (requires kernel permissions).\n--replay reads recorded frames without sampling the host; --at selects a recorded time.\n--snapshot prints JSON. --render prints a terminal frame.\nViews: dense overview tasks scheduler memory block syscalls irq cgroups modules ebpf dmesg diagnose flame\nKeys: Tab focus, [ ] views, Enter drill, Esc back, / filter, f freeze, r record, e export, : commands, ? help, q quit");
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
    let mut collector = None;
    let initial = if let Some(frames) = &frames {
        frames[0].clone()
    } else if demo {
        model::demo()
    } else {
        let c = collector.insert(Collector::new());
        c.sample();
        std::thread::sleep(Duration::from_millis(200));
        c.sample()
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
    let latest = Arc::new(Mutex::new(None));
    let stop = Arc::new(AtomicBool::new(false));
    let shared = latest.clone();
    let stopped = stop.clone();
    let mut last_trace_status = String::from("Trace stopped");
    let (probe_tx, probe_rx) = std::sync::mpsc::sync_channel::<String>(8);
    let trace_data = Arc::new(Mutex::new((
        None::<kernwatch::domain::Telemetry>,
        String::from("Trace stopped"),
    )));
    let td = trace_data.clone();
    let ts = stop.clone();
    let tracer = std::thread::spawn(move || {
        let mut session: Option<kernwatch::probes::Probes> = None;
        let mut started = Instant::now();
        let mut duration = Duration::from_secs(30);
        while !ts.load(Ordering::Relaxed) {
            while let Ok(mode) = probe_rx.try_recv() {
                session = None;
                mark_trace_stopped(&mut td.lock().unwrap().0);
                if mode == "stop" {
                    td.lock().unwrap().1 = "Trace stopped".into();
                } else {
                    td.lock().unwrap().0 = None;
                    match kernwatch::probes::Probes::start(&mode) {
                        Ok(s) => {
                            started = Instant::now();
                            duration = Duration::from_secs(s.duration_seconds);
                            session = Some(s);
                            td.lock().unwrap().1 =
                                format!("{mode} active · {}s bounded capture", duration.as_secs());
                        }
                        Err(e) => {
                            let mut failed = kernwatch::domain::Telemetry::default();
                            let kind = mode.split_whitespace().next().unwrap_or("syscalls");
                            let names: Vec<&str> = if kind == "all" { vec!["scheduler", "irq", "block", "syscalls"] }
                                else { vec![if kind == "sched" || kind == "offcpu" { "scheduler" } else { kind }] };
                            for name in names { failed.capabilities.insert(name.into(), kernwatch::domain::Quality::Error(e.to_string())); }
                            failed.details.insert("probe.capture_id".into(), vec![("failed".into(), kernwatch::recording::stamp().to_string())]);
                            let mut state = td.lock().unwrap();
                            state.0 = Some(failed);
                            state.1 = format!("{mode}: {e}");
                        },
                    }
                }
            }
            if let Some(s) = session.as_mut() {
                if let Err(e) = s.poll() {
                    td.lock().unwrap().1 = format!("Trace read failed: {e}");
                    session = None;
                    mark_trace_stopped(&mut td.lock().unwrap().0);
                } else {
                    let mut telemetry = kernwatch::domain::Telemetry {
                        at_ms: kernwatch::enrich::monotonic_ms(),
                        ..Default::default()
                    };
                    s.apply(&mut telemetry);
                    td.lock().unwrap().0 = Some(telemetry);
                }
            }
            if session.is_some() && started.elapsed() > duration {
                session = None;
                let mut state = td.lock().unwrap();
                mark_trace_stopped(&mut state.0);
                state.1 = format!(
                    "Capture completed ({}s) · retained evidence, current measurements stopped",
                    duration.as_secs()
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    });
    let live_enabled = Arc::new(AtomicBool::new(app.replay.is_none()));
    let collect_enabled = live_enabled.clone();
    let worker = std::thread::spawn(move || {
        let mut next_sample = Instant::now();
        while !stopped.load(Ordering::Relaxed) {
            if !collect_enabled.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            let next = if demo {
                model::demo()
            } else {
                collector.get_or_insert_with(Collector::new).sample()
            };
            if let Ok(mut slot) = shared.lock() {
                *slot = Some(next);
            }
            next_sample += Duration::from_secs(1);
            if next_sample <= Instant::now() {
                next_sample = Instant::now() + Duration::from_secs(1);
            }
            while !stopped.load(Ordering::Relaxed) {
                let remaining = next_sample.saturating_duration_since(Instant::now());
                if remaining.is_zero() { break; }
                std::thread::sleep(remaining.min(Duration::from_millis(50)));
            }
        }
    });
    if trace {
        let _ = probe_tx.try_send("all".into());
    }
    let mut capture_id = None;
    let mut trace_history = std::collections::BTreeMap::<String, kernwatch::domain::Series>::new();
    let state_path = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|p| std::path::PathBuf::from(p).join(".local/state"))
        })
        .map(|p| p.join("kernwatch/baselines.json"));
    let mut engine = state_path
        .as_ref()
        .and_then(|p| kernwatch::diagnose::Engine::load(p).ok())
        .unwrap_or_default();
    let mut baseline_saved = Instant::now();
    let tour_started = Instant::now();
    let mut tour_scene = 0;
    let mut last_tick = Instant::now();
    let result = (|| -> io::Result<()> {
        while !app.quit && !EXIT_REQUESTED.load(Ordering::Relaxed) {
            app.tick(last_tick.elapsed().as_millis() as u64);
            last_tick = Instant::now();
            live_enabled.store(app.replay.is_none(), Ordering::Relaxed);
            if let Some(mut s) = latest.lock().unwrap().take() {
                if !s.demo {
                    let data = trace_data.lock().unwrap();
                    if let Some(c) = &data.0 {
                        let id = c.details.get("probe.capture_id").cloned();
                        if capture_id != id {
                            trace_history.clear();
                            capture_id = id;
                        }
                        s.telemetry.metrics.extend(c.metrics.clone());
                        s.telemetry.histograms.extend(c.histograms.clone());
                        s.telemetry.details.extend(c.details.clone());
                        s.telemetry.capabilities.extend(c.capabilities.clone());
                        s.telemetry.events.extend(c.events.clone());
                        s.telemetry.trace_drops = c.trace_drops;
                        for (key, measurement) in &c.metrics {
                            let series = trace_history.entry(key.clone()).or_insert_with(|| {
                                kernwatch::domain::Series {
                                    name: key.clone(),
                                    unit: measurement.unit.clone(),
                                    max: 1.,
                                    samples: Vec::new(),
                                }
                            });
                            let value =
                                if measurement.quality == kernwatch::domain::Quality::Available {
                                    measurement.value
                                } else {
                                    None
                                };
                            series.max = series.max.max(value.unwrap_or(0.));
                            series.push(s.telemetry.at_ms, value);
                            s.telemetry.series.insert(key.clone(), series.clone());
                        }
                        kernwatch::probes::hydrate(&mut s);
                    }
                    for (key, series) in &mut trace_history {
                        if !s.telemetry.series.contains_key(key) {
                            series.push(s.telemetry.at_ms, None);
                            s.telemetry.series.insert(key.clone(), series.clone());
                        }
                    }
                }
                if !s.demo {
                    engine.update(&mut s.telemetry);
                    if baseline_saved.elapsed() > Duration::from_secs(60) && app.replay.is_none() {
                        if let Some(path) = &state_path {
                            if let Err(e) = engine.save(path) {
                                app.status = format!("Baseline save failed: {e}");
                            }
                        }
                        baseline_saved = Instant::now();
                    }
                }
                app.update(s);
                if !app.snapshot.demo {
                    let status = trace_data.lock().unwrap().1.clone();
                    if status != last_trace_status {
                        last_trace_status = status.clone();
                        app.status = status;
                    }
                }
            }
            if let Some(mode) = app.probe_request.take() {
                if app.replay.is_some() {
                    app.status =
                        "Replay is read-only; switch to live before starting a probe".into();
                } else if app.snapshot.demo {
                    app.status="Demo trace evidence is already loaded; use live mode to capture the kernel".into();
                } else {
                    if let Err(e) = probe_tx.try_send(mode) {
                        app.status = format!("Probe request queue unavailable: {e}");
                    }
                }
            }
            if let Some(text) = app.clipboard.take() {
                use std::io::Write;
                write!(
                    io::stdout(),
                    "\x1b]52;c;{}\x07",
                    kernwatch::app::clipboard_base64(text.as_bytes())
                )?;
                app.status = "Copy sent to terminal via OSC52".into();
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
            execute!(terminal.backend_mut(), BeginSynchronizedUpdate)?;
            let drawn = terminal.draw(|f| ui::draw(f, &app)).map(|_| ());
            let finished = execute!(terminal.backend_mut(), EndSynchronizedUpdate);
            drawn?;
            finished?;
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(k) = event::read()? {
                    if k.kind != KeyEventKind::Release {
                        if demo_tour {
                            demo_tour = false;
                            app.status = "Demo tour paused · manual control".into();
                        }
                        app.key(k);
                    }
                }
            }
        }
        Ok(())
    })();
    if let Err(e) = (kernwatch::settings::Settings {
        view: app.tab,
        terminal_theme: app.terminal_theme,
    })
    .save()
    {
        app.status = format!("Settings save failed: {e}");
    }
    if !app.snapshot.demo && app.replay.is_none() {
        if let Some(path) = &state_path {
            let _ = engine.save(path);
        }
    }
    stop.store(true, Ordering::Relaxed);
    let _ = worker.join();
    let _ = tracer.join();
    result?;
    Ok(())
}

fn mark_trace_stopped(data: &mut Option<kernwatch::domain::Telemetry>) {
    if let Some(data) = data {
        for metric in data.metrics.values_mut() {
            metric.quality = kernwatch::domain::Quality::Stopped;
            metric.value = None;
        }
        for quality in data.capabilities.values_mut() {
            *quality = kernwatch::domain::Quality::Stopped;
        }
        data.details
            .retain(|key, _| !key.starts_with("wake:") && !key.starts_with("wakecpu:"));
    }
}
