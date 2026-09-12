//! Opt-in observer timing. Scopes are inclusive, counters name the particular
//! allocation/copy sites they cover; these are not a global allocator census.
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
    time::Instant,
};
#[derive(Default, Serialize)]
struct Row {
    calls: u64,
    cpu_ns: u64,
    wall_ns: u64,
    max_cpu_ns: u64,
    max_wall_ns: u64,
}
struct State {
    start: Instant,
    last: Instant,
    rows: BTreeMap<String, Row>,
    gauges: BTreeMap<String, u64>,
    counters: BTreeMap<String, u64>,
    observations: BTreeMap<String, std::collections::VecDeque<u64>>,
}
fn destination() -> Option<&'static std::path::Path> {
    static PATH: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| std::env::var_os("KERNWATCH_CPU_COST").map(Into::into))
        .as_deref()
}
pub fn enabled() -> bool {
    destination().is_some()
}
fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(State {
            start: Instant::now(),
            last: Instant::now(),
            rows: BTreeMap::new(),
            gauges: BTreeMap::new(),
            counters: BTreeMap::new(),
            observations: BTreeMap::new(),
        })
    })
}
fn cpu() -> u64 {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a valid timespec is provided to a read-only thread-clock query.
    unsafe {
        libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t);
    }
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}
pub struct Guard(Option<(&'static str, Instant, u64)>);
pub fn scope(name: &'static str) -> Guard {
    Guard(enabled().then(|| (name, Instant::now(), cpu())))
}
fn write(s: &State) {
    if let Some(path) = destination() {
        let value = serde_json::json!({"elapsed_seconds":s.start.elapsed().as_secs_f64(), "stages":s.rows, "gauges":s.gauges, "counters":s.counters,"observations":s.observations});
        if let Ok(bytes) = serde_json::to_vec_pretty(&value) {
            let temp = path.with_extension(format!("{}.tmp", std::process::id()));
            if std::fs::write(&temp, bytes).is_ok() {
                let _ = std::fs::rename(temp, path);
            }
        }
    }
}
pub fn flush() {
    if enabled() {
        write(&state().lock().unwrap());
    }
}
pub fn gauge(name: &str, value: u64) {
    if enabled() {
        state().lock().unwrap().gauges.insert(name.into(), value);
    }
}
/// Last 4096 observations per named distribution, bounded even in long runs.
pub fn observe(name: &str, value: u64) {
    if enabled() {
        let mut state = state().lock().unwrap();
        let values = state.observations.entry(name.into()).or_default();
        if values.len() == 4096 {
            values.pop_front();
        }
        values.push_back(value);
    }
}
pub fn counter(name: &str, value: u64) {
    if enabled() {
        *state()
            .lock()
            .unwrap()
            .counters
            .entry(name.into())
            .or_default() += value;
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        let Some((name, start, before)) = self.0 else {
            return;
        };
        let elapsed_cpu = cpu().saturating_sub(before);
        let wall = start.elapsed().as_nanos() as u64;
        let mut s = state().lock().unwrap();
        let row = s.rows.entry(name.into()).or_default();
        row.calls += 1;
        row.cpu_ns += elapsed_cpu;
        row.wall_ns += wall;
        row.max_cpu_ns = row.max_cpu_ns.max(elapsed_cpu);
        row.max_wall_ns = row.max_wall_ns.max(wall);
        if s.last.elapsed().as_secs() >= 10 {
            write(&s);
            s.last = Instant::now();
        }
    }
}
