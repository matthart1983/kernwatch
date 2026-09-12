//! Reproducible synthetic tour. Uses the same application and renderer as interactive mode.
use crate::{app::App, model};
pub const SECONDS_PER_SCENE: u64 = 5;
pub const SCENES: [(&str, usize, &str, &str); 10] = [
    (
        "Dense: identify the CPU3 scheduling incident",
        12,
        "incident",
        "",
    ),
    (
        "Tasks: inspect Envoy worker wake latency",
        1,
        "incident",
        "envoy",
    ),
    (
        "Scheduler: compare per-CPU scheduling latency",
        2,
        "incident",
        "",
    ),
    (
        "IRQs: inspect interrupt rate and CPU placement",
        6,
        "incident",
        "",
    ),
    (
        "Memory: examine slab growth and pressure",
        3,
        "incident",
        "",
    ),
    (
        "Block: separate pending age from completed latency",
        4,
        "incident",
        "",
    ),
    (
        "Cgroups: inspect service quota and placement",
        7,
        "incident",
        "",
    ),
    (
        "eBPF: attribute kernel CPU to the programs loaded on the host",
        9,
        "incident",
        "",
    ),
    (
        "Flame: read where the worker's stacks actually spend their samples",
        13,
        "incident",
        "",
    ),
    (
        "Diagnose: review evidence before proposing changes",
        11,
        "incident",
        "",
    ),
];
pub fn show(app: &mut App, index: usize) {
    let (caption, tab, scenario, filter) = SCENES[index % SCENES.len()];
    *app = App::new(model::demo());
    app.execute(&format!("scenario {scenario}"));
    app.switch(tab);
    app.filter = filter.into();
    app.status = format!(
        "DEMO TOUR {}/{} · {caption} · any key takes control",
        index % SCENES.len() + 1,
        SCENES.len()
    );
}
#[cfg(test)]
mod tests {
    #[test]
    fn tour_is_synthetic_and_never_queues_host_changes() {
        let mut app = crate::app::App::new(crate::model::demo());
        for i in 0..super::SCENES.len() {
            super::show(&mut app, i);
            assert!(app.snapshot.demo);
            assert!(app.probe_request.is_none());
            assert!(app.pending_action.is_none());
            assert_eq!(app.tab, super::SCENES[i].1);
            assert!(app.status.contains("DEMO TOUR"));
        }
    }
}
