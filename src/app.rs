use crate::{
    model::{Snapshot, TABS},
    recording::{self, Recorder},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::{collections::VecDeque, path::PathBuf};
#[derive(Clone, Default)]
pub struct Route {
    pub evidence_index: usize,
    pub pressure_view: bool,
    pub memory_gib: bool,
    pub auto_scale: bool,
    pub metric: usize,
    pub grouping: usize,
    pub mode: usize,
    pub sort: usize,
    pub reverse: bool,
    pub scope_cpu: Option<u32>,
    pub tab: usize,
    pub selected: usize,
    pub filter: String,
    pub focus: usize,
    pub cursor: Option<u64>,
}
pub struct App {
    pub capabilities_view: bool,
    pub reviewing_action: bool,
    pub evidence_index: usize,
    pub pressure_view: bool,
    pub memory_gib: bool,
    pub auto_scale: bool,
    pub metric: usize,
    pub marked_modules: std::collections::BTreeSet<String>,
    pub grouping: usize,
    demo_host: crate::actions::DemoHost,
    timeline_frames: VecDeque<(usize, Snapshot)>,
    timeline_bytes: usize,
    latest_snapshot: Option<Snapshot>,
    pub clipboard: Option<String>,
    pub watched: std::collections::BTreeSet<(u32, u64)>,
    pub histogram: bool,
    pub demo_scenario: String,
    pub replay_playing: bool,
    pub replay_speed: f64,
    replay_elapsed: f64,
    pub folded: std::collections::BTreeSet<String>,
    pub scope_cpu: Option<u32>,
    pub pending_action: Option<crate::actions::Plan>,
    pub action_journal: Vec<crate::actions::Plan>,
    pub journal_path: PathBuf,
    pub terminal_theme: bool,
    pub snapshot: Snapshot,
    pub tab: usize,
    pub selected: usize,
    pub scroll: u16,
    pub frozen: bool,
    pub help: bool,
    pub detail: bool,
    pub filtering: bool,
    pub filter: String,
    pub reverse: bool,
    pub history: VecDeque<u64>,
    pub status: String,
    pub quit: bool,
    pub focus: usize,
    pub mode: usize,
    pub sort: usize,
    pub palette: bool,
    pub command: String,
    pub routes: Vec<Route>,
    pub recording: Option<Recorder>,
    pub replay: Option<Vec<Snapshot>>,
    pub replay_index: usize,
    pub time_cursor: Option<u64>,
    pub probe_request: Option<String>,
    pub expanded: bool,
}
impl App {
    pub fn new(snapshot: Snapshot) -> Self {
        let mut a = Self {
            capabilities_view: false,
            reviewing_action: false,
            evidence_index: 0,
            pressure_view: false,
            memory_gib: false,
            auto_scale: false,
            metric: 0,
            marked_modules: Default::default(),
            grouping: 0,
            demo_host: Default::default(),
            timeline_frames: VecDeque::new(),
            timeline_bytes: 0,
            latest_snapshot: None,
            clipboard: None,
            watched: Default::default(),
            histogram: false,
            demo_scenario: "incident".into(),
            replay_playing: false,
            replay_speed: 1.,
            replay_elapsed: 0.,
            folded: Default::default(),
            scope_cpu: None,
            pending_action: None,
            action_journal: Vec::new(),
            journal_path: PathBuf::from(format!("kernwatch-actions-{}.json", recording::stamp())),
            terminal_theme: false,
            snapshot,
            tab: 12,
            selected: 0,
            scroll: 0,
            frozen: false,
            help: false,
            detail: false,
            filtering: false,
            filter: String::new(),
            reverse: false,
            history: VecDeque::new(),
            status: "Ready · : command palette".into(),
            quit: false,
            focus: 0,
            mode: 0,
            sort: 0,
            palette: false,
            command: String::new(),
            routes: Vec::new(),
            recording: None,
            replay: None,
            replay_index: 0,
            time_cursor: None,
            probe_request: None,
            expanded: false,
        };
        a.push_history();
        let first = a.snapshot.clone();
        a.retain_frame(&first);
        a.latest_snapshot = Some(first);
        a
    }
    fn push_history(&mut self) {
        let c = &self.snapshot.cpu;
        self.history.push_back(if c.is_empty() {
            0
        } else {
            (c.iter().sum::<f64>() / c.len() as f64).round() as u64
        });
        if self.history.len() > 600 {
            self.history.pop_front();
        }
    }
    pub fn panel_count(&self) -> usize {
        [4, 4, 3, 4, 3, 3, 3, 4, 3, 4, 3, 4, 8][self.tab]
    }
    pub fn cursor(&self) -> u64 {
        self.time_cursor.unwrap_or(self.snapshot.telemetry.at_ms)
    }
    fn retain_frame(&mut self, snapshot: &Snapshot) {
        if self
            .timeline_frames
            .back()
            .is_some_and(|(_, s)| s.telemetry.at_ms == snapshot.telemetry.at_ms)
        {
            return;
        }
        let mut frame = snapshot.clone();
        frame.telemetry.series.clear();
        let size = serde_json::to_vec(&frame).map(|v| v.len() * 3).unwrap_or(0);
        self.timeline_bytes += size;
        self.timeline_frames.push_back((size, frame));
        while self.timeline_frames.len() > 600
            || (self.timeline_bytes > 128 * 1024 * 1024 && self.timeline_frames.len() > 1)
        {
            if let Some((size, _)) = self.timeline_frames.pop_front() {
                self.timeline_bytes = self.timeline_bytes.saturating_sub(size);
            }
        }
    }
    pub fn seek_time(&mut self, at: u64) {
        if let Some(frames) = &self.replay {
            self.replay_index = frames
                .iter()
                .rposition(|s| s.telemetry.at_ms <= at)
                .unwrap_or(0);
            self.snapshot = frames[self.replay_index].clone();
            self.time_cursor = None;
            return;
        }
        let latest = self.latest_snapshot.as_ref().unwrap_or(&self.snapshot);
        if at >= latest.telemetry.at_ms {
            self.snapshot = latest.clone();
            self.time_cursor = None;
            return;
        }
        if self.snapshot.demo {
            let mut frame = latest.clone();
            frame.telemetry = crate::fixture::at_time(&self.demo_scenario, at);
            frame.cpu = frame.telemetry.cpus.iter().map(|c| c.busy).collect();
            self.snapshot = frame;
            self.time_cursor = Some(at);
            return;
        }
        if let Some((_, frame)) = self
            .timeline_frames
            .iter()
            .rev()
            .find(|(_, s)| s.telemetry.at_ms <= at)
        {
            let mut frame = frame.clone();
            frame.telemetry.series = latest.telemetry.series.clone();
            self.time_cursor = Some(frame.telemetry.at_ms);
            self.snapshot = frame;
        } else {
            self.status =
                "Earliest retained entity snapshot; graph history may extend further".into();
        }
    }
    pub fn update(&mut self, mut s: Snapshot) {
        if s.demo && self.demo_scenario != "incident" {
            s.telemetry = crate::fixture::scenario(&self.demo_scenario);
            s.cpu = s.telemetry.cpus.iter().map(|c| c.busy).collect();
        }
        for module in &mut s.telemetry.modules {
            if self.marked_modules.contains(&module.name) {
                module.fields.push((
                    "analyst mark".into(),
                    "suspicious; operator annotation, not proven malicious".into(),
                ));
                s.telemetry
                    .details
                    .insert(format!("module:{}", module.name), module.fields.clone());
            }
        }
        if self.replay.is_none() {
            self.retain_frame(&s);
            self.latest_snapshot = Some(s.clone());
        }
        if let Some(rec) = self.recording.as_mut() {
            if let Err(e) = rec.push(&s) {
                self.status = format!("Recording failed: {e}");
                self.recording = None;
            }
        }
        if !self.frozen && self.replay.is_none() && self.time_cursor.is_none() {
            let identity = self.selected_task().map(|x| (x.pid, x.start_ticks));
            let old_boot = self.snapshot.telemetry.boot_id.clone();
            let row_key = if [4, 6, 7, 8, 9].contains(&self.tab) {
                self.rows()
                    .get(self.selected)
                    .and_then(|r| r.first())
                    .cloned()
            } else {
                None
            };
            let old_generation = row_key.as_ref().map(|key| self.object_generation(key));
            let scheduler_key = (self.tab == 2)
                .then(|| {
                    self.scheduler_subjects()
                        .get(self.selected)
                        .map(|r| r.0.clone())
                })
                .flatten();
            let events = self.visible_events();
            let follow = self.tab == 10 && self.selected + 1 >= events.len();
            let event_id = events.get(self.selected).map(|e| e.id.clone());
            self.snapshot = s;
            if self.tab == 10 {
                let events = self.visible_events();
                self.selected = if follow {
                    events.len().saturating_sub(1)
                } else {
                    event_id
                        .and_then(|id| events.iter().position(|e| e.id == id))
                        .unwrap_or(0)
                };
            }
            if self.tab == 1 {
                if let Some(key) = identity {
                    if let Some(i) = self
                        .visible_tasks()
                        .iter()
                        .position(|x| (x.pid, x.start_ticks) == key)
                    {
                        if old_boot == self.snapshot.telemetry.boot_id {
                            self.selected = i;
                        } else {
                            self.detail = false;
                            self.capabilities_view = false;
                            self.selected = 0;
                            self.status = "Host boot identity changed; selection reset".into();
                        }
                    } else {
                        self.detail = false;
                        self.capabilities_view = false;
                        self.selected = 0;
                        self.status =
                            "Selected task exited or its PID was reused; selection reset".into();
                    }
                }
            }
            if let Some(key) = row_key {
                if let Some(index) = self.rows().iter().position(|r| r.first() == Some(&key)) {
                    if old_boot != self.snapshot.telemetry.boot_id
                        || old_generation.as_ref() != Some(&self.object_generation(&key))
                    {
                        self.selected = 0;
                        self.detail = false;
                        self.capabilities_view = false;
                        self.status =
                            format!("Selected object {key} was replaced; selection reset");
                    } else {
                        self.selected = index;
                    }
                } else {
                    self.selected = 0;
                    self.detail = false;
                    self.capabilities_view = false;
                    self.status = format!("Selected object {key} disappeared; selection reset");
                }
            }
            if let Some(key) = scheduler_key {
                self.selected = self
                    .scheduler_subjects()
                    .iter()
                    .position(|r| r.0 == key)
                    .unwrap_or(0);
            }
            self.push_history();
        }
    }
    fn object_generation(&self, key: &str) -> String {
        let t = &self.snapshot.telemetry;
        match self.tab {
            4 => t
                .devices
                .iter()
                .find(|d| d.name == key)
                .map(|d| {
                    format!(
                        "{} {:?}",
                        d.major_minor,
                        d.fields.iter().find(|(k, _)| k == "sysfs identity")
                    )
                })
                .unwrap_or_default(),
            7 => t
                .cgroups
                .iter()
                .find(|g| g.path == key)
                .map(|g| g.inode.to_string())
                .unwrap_or_default(),
            8 => t
                .modules
                .iter()
                .find(|m| m.name == key)
                .map(|m| {
                    format!(
                        "{} {:?}",
                        m.bytes,
                        m.fields
                            .iter()
                            .filter(|(k, _)| k == "source version" || k == "sysfs identity")
                            .collect::<Vec<_>>()
                    )
                })
                .unwrap_or_default(),
            9 => t
                .details
                .get(&format!("bpf:{key}"))
                .map(|f| {
                    format!(
                        "{:?}",
                        f.iter()
                            .filter(|(k, _)| k == "tag" || k == "loaded at")
                            .collect::<Vec<_>>()
                    )
                })
                .unwrap_or_default(),
            6 => self
                .rows()
                .iter()
                .find(|r| r.first().is_some_and(|id| id == key))
                .and_then(|r| r.get(1))
                .cloned()
                .unwrap_or_default(),
            _ => key.into(),
        }
    }
    pub fn visible_tasks(&self) -> Vec<&crate::domain::Task> {
        let query = self.filter.to_lowercase();
        let mut tasks = self
            .snapshot
            .telemetry
            .tasks
            .iter()
            .filter(|x| {
                query.is_empty()
                    || format!("{} {} {} {}", x.name, x.pid, x.verdict, x.cgroup)
                        .to_lowercase()
                        .contains(&query)
            })
            .filter(|x| self.scope_cpu.map(|cpu| x.cpu == cpu).unwrap_or(true))
            .filter(|x| match if self.tab == 1 { self.mode } else { 0 } {
                1 => x.state == "R",
                2 => x.state == "D",
                3 => x.kernel_thread,
                4 => x.pid != x.tgid,
                _ => true,
            })
            .collect::<Vec<_>>();
        match self.sort {
            1 => tasks.sort_by(|x, y| x.name.cmp(&y.name).then(x.pid.cmp(&y.pid))),
            2 => tasks.sort_by(|x, y| x.state.cmp(&y.state)),
            3 => tasks.sort_by_key(|x| x.cpu),
            4 => tasks.sort_by(|x, y| y.cpu_pct.unwrap_or(0.).total_cmp(&x.cpu_pct.unwrap_or(0.))),
            5 => tasks.sort_by_key(|x| std::cmp::Reverse(x.rss_bytes)),
            6 => tasks.sort_by(|x, y| {
                y.wake_p99_ms
                    .unwrap_or(0.)
                    .total_cmp(&x.wake_p99_ms.unwrap_or(0.))
            }),
            _ => {}
        }
        if self.reverse {
            tasks.reverse();
        }
        if self.tab == 1 {
            match self.grouping {
                1 => tasks.sort_by(|a, b| a.cgroup.cmp(&b.cgroup)),
                2 => tasks.sort_by_key(|t| t.uid),
                3 => tasks.sort_by_key(|t| t.parent_pid),
                _ => {}
            }
        }
        tasks
    }
    pub fn memory_tasks(&self) -> Vec<&crate::domain::Task> {
        let query = self.filter.to_lowercase();
        let mut tasks = self
            .snapshot
            .telemetry
            .tasks
            .iter()
            .filter(|x| x.pid == x.tgid && x.rss_bytes > 0)
            .filter(|x| {
                format!("{} {} {}", x.pid, x.name, x.cgroup)
                    .to_lowercase()
                    .contains(&query)
            })
            .collect::<Vec<_>>();
        tasks.sort_by_key(|x| std::cmp::Reverse(x.pss_bytes.unwrap_or(x.rss_bytes)));
        tasks
    }
    pub fn selected_task(&self) -> Option<&crate::domain::Task> {
        let tasks = self.visible_tasks();
        tasks
            .get(self.selected.min(tasks.len().saturating_sub(1)))
            .copied()
    }
    pub fn visible_events(&self) -> Vec<&crate::domain::Event> {
        let query = self.filter.to_lowercase();
        let mut events = self
            .snapshot
            .telemetry
            .events
            .iter()
            .filter(|e| {
                query.is_empty()
                    || format!("{} {} {}", e.source, e.severity, e.message)
                        .to_lowercase()
                        .contains(&query)
            })
            .filter(|e| match self.mode {
                1 => ["warn", "warning", "error"].contains(&e.severity.as_str()),
                2 => e.severity == "error",
                _ => true,
            })
            .collect::<Vec<_>>();
        events.sort_by_key(|e| (e.at_ms, &e.id));
        events
    }
    pub fn scheduler_subjects(&self) -> Vec<(String, String, Option<u32>, Option<String>)> {
        let t = &self.snapshot.telemetry;
        let query = self.filter.to_lowercase();
        let mut subjects = match self.mode {
            1 => t
                .tasks
                .iter()
                .map(|x| {
                    (
                        format!("{} {}", x.pid, x.name),
                        format!("task.{}", x.pid),
                        Some(x.cpu),
                        Some(x.cgroup.clone()),
                    )
                })
                .collect::<Vec<_>>(),
            2 => t
                .cgroups
                .iter()
                .map(|g| {
                    (
                        g.path.clone(),
                        format!("sched.cgroup{}", g.inode),
                        None,
                        Some(g.path.clone()),
                    )
                })
                .collect(),
            _ => t
                .cpus
                .iter()
                .map(|c| {
                    (
                        format!("cpu{}", c.id),
                        format!("sched.cpu{}", c.id),
                        Some(c.id),
                        None,
                    )
                })
                .collect(),
        };
        subjects.retain(|x| x.0.to_lowercase().contains(&query));
        if self.reverse {
            subjects.reverse();
        }
        subjects
    }
    pub fn rows(&self) -> Vec<Vec<String>> {
        if self.tab == 10 {
            return self
                .visible_events()
                .iter()
                .map(|e| {
                    vec![
                        e.at_ms.to_string(),
                        e.severity.clone(),
                        e.source.clone(),
                        e.message.clone(),
                    ]
                })
                .collect();
        }
        if self.tab == 1 {
            return self
                .visible_tasks()
                .into_iter()
                .map(|x| {
                    vec![
                        format!("{} {}", x.name, x.pid),
                        x.state.clone(),
                        x.cpu.to_string(),
                        x.cpu_pct.map(|v| format!("{v:.1}")).unwrap_or("—".into()),
                        format!("{:.0} MiB", x.rss_bytes as f64 / 1048576.),
                        x.wake_p99_ms
                            .map(|v| format!("{v:.1}ms"))
                            .unwrap_or("—".into()),
                    ]
                })
                .collect();
        }
        let telemetry = &self.snapshot.telemetry;
        let formatted = match self.tab {
            4 if !telemetry.devices.is_empty() => Some(
                telemetry
                    .devices
                    .iter()
                    .filter(|d| match self.mode {
                        1 => d.partition,
                        2 => d.name.starts_with("dm-"),
                        3 => d
                            .fields
                            .iter()
                            .any(|(k, v)| k.starts_with("mounts") && v != "none observed"),
                        _ => true,
                    })
                    .map(crate::model::device_row)
                    .collect(),
            ),
            7 if !telemetry.cgroups.is_empty() => Some(
                telemetry
                    .cgroups
                    .iter()
                    .filter(|g| self.mode != 2 || g.throttled_ms_s.unwrap_or(0.) > 0.)
                    .filter(|g| {
                        self.mode != 3
                            || g.fields.iter().any(|(k, v)| {
                                k.ends_with(".pressure")
                                    && v.split_whitespace()
                                        .filter_map(|s| {
                                            s.strip_prefix("avg10=")
                                                .and_then(|s| s.parse::<f64>().ok())
                                        })
                                        .any(|n| n > 0.)
                            })
                    })
                    .filter(|g| {
                        self.mode == 1
                            || !self.folded.iter().any(|parent| {
                                g.path != *parent
                                    && (parent == "/" || g.path.starts_with(&format!("{parent}/")))
                            })
                    })
                    .map(crate::model::cgroup_row)
                    .collect(),
            ),
            8 if !telemetry.modules.is_empty() => Some(
                telemetry
                    .modules
                    .iter()
                    .filter(|m| match self.mode {
                        1 => m.taint.contains('O'),
                        2 => m.taint.contains('E'),
                        3 => m.fields.iter().any(|(k, v)| k == "baseline" && v == "new"),
                        4 => m.refs == 0,
                        _ => true,
                    })
                    .map(crate::model::module_row)
                    .collect(),
            ),
            _ => None,
        };
        let query = self.filter.to_lowercase();
        if self.snapshot.demo && self.tab == 5 && self.snapshot.telemetry.at_ms < 595000 {
            return Vec::new();
        }
        let mut rows = formatted.unwrap_or_else(|| {
            self.snapshot
                .views
                .get(self.tab)
                .map(|v| {
                    v.rows
                        .iter()
                        .filter(|r| r.join(" ").to_lowercase().contains(&query))
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        rows.retain(|r| r.join(" ").to_lowercase().contains(&query));
        if self.tab == 5 && self.mode == 1 {
            rows.retain(|r| {
                r.get(2)
                    .and_then(|s| s.replace(',', "").parse::<f64>().ok())
                    .unwrap_or(0.)
                    > 0.
            });
        }
        if self.tab == 5 && self.mode == 2 {
            rows.retain(|r| {
                r.get(5)
                    .and_then(|v| v.trim_end_matches("ms").parse::<f64>().ok())
                    .is_some_and(|v| v > 1.)
            });
        }
        if self.tab == 9 && self.mode > 0 {
            rows.retain(|r| {
                let owned = if self.snapshot.demo {
                    r.get(7).is_some_and(|s| s == "kernwatch")
                } else {
                    r.first().is_some_and(|id| {
                        self.snapshot
                            .telemetry
                            .details
                            .get("owned_program_ids")
                            .is_some_and(|ids| ids.iter().any(|(key, _)| key == id))
                    })
                };
                if self.mode == 1 {
                    owned
                } else {
                    !owned
                }
            });
        }

        if self.sort > 0 {
            let column = (self.sort - 1) % self.snapshot.views[self.tab].columns.len().max(1);
            rows.sort_by(|a, b| {
                let av = a.get(column).map(String::as_str).unwrap_or("");
                let bv = b.get(column).map(String::as_str).unwrap_or("");
                match (
                    av.replace(',', "").parse::<f64>(),
                    bv.replace(',', "").parse::<f64>(),
                ) {
                    (Ok(a), Ok(b)) => a.total_cmp(&b),
                    _ => av.cmp(bv),
                }
            });
        }
        if self.reverse {
            rows.reverse();
        }
        rows
    }
    pub fn switch(&mut self, t: usize) {
        self.tab = t.min(12);
        self.selected = 0;
        self.scroll = 0;
        self.detail = false;
        self.capabilities_view = false;
        self.reviewing_action = false;
        self.focus = 0;
        self.filter.clear();
        self.mode = 0;
        self.scope_cpu = None;
        if t == 10 {
            self.selected = self.visible_events().len().saturating_sub(1);
        }
        if t == 2 {
            self.selected = self
                .snapshot
                .telemetry
                .cpus
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.busy.total_cmp(&b.busy))
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }
    fn proposed_action(&self) -> Option<String> {
        let t = &self.snapshot.telemetry;
        let issue = t.issues.get(self.selected)?;
        if let Some(group) = issue.id.strip_prefix("throttle:") {
            let g = t.cgroups.iter().find(|g| g.path == group)?;
            let mut values = g.quota.split_whitespace();
            let quota = values.next()?.parse::<u64>().ok()?.checked_mul(2)?;
            let period = values.next()?.parse::<u64>().ok()?;
            return Some(format!("quota {group} {quota} {period}"));
        }
        if !(issue.id.contains("sched")
            || issue.id.starts_with("cpu:")
            || issue.id.contains("softirq"))
        {
            return None;
        }
        let cpu = t
            .cpus
            .iter()
            .max_by(|a, b| a.softirq.total_cmp(&b.softirq))?;
        let irq = self.snapshot.views[6]
            .rows
            .iter()
            .filter(|r| r.get(4) == Some(&cpu.id.to_string()))
            .max_by(|a, b| {
                let n = |r: &Vec<String>| {
                    r.get(2)
                        .and_then(|v| v.replace(',', "").parse::<f64>().ok())
                        .unwrap_or(0.)
                };
                n(a).total_cmp(&n(b))
            })?
            .first()?;
        let target = t
            .cpus
            .iter()
            .filter(|c| c.id != cpu.id)
            .map(|c| c.id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        if target.is_empty() {
            return None;
        }
        Some(format!("irq {irq} {target}"))
    }
    pub fn open_subject(&mut self, subject: &str, at: u64) {
        let (kind, id) = subject.split_once(':').unwrap_or(("source", subject));
        let kind = match kind {
            "throttle" => "cgroup",
            "blocked" => "task",
            "irq-placement" => "irq",
            other => other,
        };
        let id = if kind == "task" {
            id.split(':').next().unwrap_or(id)
        } else {
            id
        };
        if kind == "source" {
            let target = if subject.starts_with("sched.") {
                Some(2)
            } else if subject.starts_with("disk.") {
                Some(4)
            } else if subject.starts_with("psi.") || subject.starts_with("slab.") {
                Some(3)
            } else if subject.starts_with("bpf.") {
                Some(9)
            } else if subject.starts_with("module.") {
                Some(8)
            } else if subject.starts_with("softirq.") {
                Some(6)
            } else {
                None
            };
            if let Some(tab) = target {
                self.drill(tab);
                self.seek_time(at);
                return;
            }
        }
        let tab = match kind {
            "task" => 1,
            "cpu" => 2,
            "device" => 4,
            "irq" => 6,
            "cgroup" => 7,
            "module" => 8,
            "bpf" => 9,
            _ => 10,
        };
        self.drill(tab);
        self.seek_time(at);
        if kind == "cpu" {
            self.selected = self
                .snapshot
                .telemetry
                .cpus
                .iter()
                .position(|c| c.id.to_string() == id)
                .unwrap_or(0);
        } else {
            self.filter = id.into();
            self.selected = 0;
        }
    }
    pub fn drill(&mut self, t: usize) {
        self.routes.push(Route {
            evidence_index: self.evidence_index,
            pressure_view: self.pressure_view,
            memory_gib: self.memory_gib,
            auto_scale: self.auto_scale,
            metric: self.metric,
            grouping: self.grouping,
            mode: self.mode,
            sort: self.sort,
            reverse: self.reverse,
            scope_cpu: self.scope_cpu,
            tab: self.tab,
            selected: self.selected,
            filter: self.filter.clone(),
            focus: self.focus,
            cursor: self.time_cursor,
        });
        self.switch(t);
    }
    fn back(&mut self) {
        if self.detail {
            self.detail = false;
            self.capabilities_view = false;
            self.reviewing_action = false;
            self.expanded = false;
            return;
        }
        if !self.filter.is_empty() && self.routes.is_empty() {
            self.filter.clear();
            return;
        }
        if let Some(r) = self.routes.pop() {
            self.seek_time(r.cursor.unwrap_or(u64::MAX));
            self.mode = r.mode;
            self.sort = r.sort;
            self.evidence_index = r.evidence_index;
            self.pressure_view = r.pressure_view;
            self.memory_gib = r.memory_gib;
            self.auto_scale = r.auto_scale;
            self.metric = r.metric;
            self.grouping = r.grouping;
            self.reverse = r.reverse;
            self.scope_cpu = r.scope_cpu;
            self.tab = r.tab;
            self.selected = r.selected;
            self.filter = r.filter;
            self.focus = r.focus;
            self.time_cursor = r.cursor;
        }
    }
    pub fn key(&mut self, k: KeyEvent) {
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.palette {
            match k.code {
                KeyCode::Esc => self.palette = false,
                KeyCode::Enter => {
                    let c = std::mem::take(&mut self.command);
                    self.execute(&c);
                    self.palette = false;
                }
                KeyCode::Backspace => {
                    self.command.pop();
                }
                KeyCode::Char(c) => self.command.push(c),
                _ => {}
            }
            return;
        }
        if self.filtering {
            match k.code {
                KeyCode::Esc => {
                    self.filtering = false;
                    self.filter.clear()
                }
                KeyCode::Enter => self.filtering = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Char(c) => self.filter.push(c),
                _ => {}
            }
            self.selected = 0;
            return;
        }
        if self.help {
            if matches!(
                k.code,
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
            ) {
                self.help = false
            }
            return;
        }
        if k.modifiers.contains(KeyModifiers::ALT) {
            if let KeyCode::Char(c) = k.code {
                if let Some(n) = c.to_digit(10) {
                    self.focus = (n.saturating_sub(1) as usize).min(self.panel_count() - 1);
                    return;
                }
            }
        }
        match k.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char(':') => self.palette = true,
            KeyCode::Char('f') => {
                self.frozen = !self.frozen;
                self.status = if self.frozen {
                    "Display frozen · recording is independent"
                } else {
                    "Live display resumed"
                }
                .into();
            }
            KeyCode::Tab => self.focus = (self.focus + 1) % self.panel_count(),
            KeyCode::BackTab => {
                self.focus = (self.focus + self.panel_count() - 1) % self.panel_count()
            }
            KeyCode::Char(']') => self.switch((self.tab + 1) % 13),
            KeyCode::Char('[') => self.switch((self.tab + 12) % 13),
            KeyCode::Right | KeyCode::Left => {
                if let Some(frames) = &self.replay {
                    if k.code == KeyCode::Right {
                        self.replay_index =
                            (self.replay_index + 1).min(frames.len().saturating_sub(1));
                    } else {
                        self.replay_index = self.replay_index.saturating_sub(1);
                    }
                    if let Some(s) = frames.get(self.replay_index) {
                        self.snapshot = s.clone();
                    }
                } else {
                    let target = if k.code == KeyCode::Right {
                        self.cursor().saturating_add(1000)
                    } else {
                        self.cursor().saturating_sub(1000)
                    };
                    self.seek_time(target);
                }
            }
            KeyCode::Down | KeyCode::Char('j')
                if self.tab == 11 && self.focus == 1 && !self.detail =>
            {
                let n = self
                    .snapshot
                    .telemetry
                    .issues
                    .get(self.selected)
                    .map(|i| i.evidence.len())
                    .unwrap_or(0);
                self.evidence_index = (self.evidence_index + 1).min(n.saturating_sub(1));
            }
            KeyCode::Up | KeyCode::Char('k')
                if self.tab == 11 && self.focus == 1 && !self.detail =>
            {
                self.evidence_index = self.evidence_index.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.detail && !(self.expanded && self.table_focus()) {
                    self.scroll = self.scroll.saturating_add(1)
                } else {
                    self.selected = (self.selected + 1).min(self.row_count().saturating_sub(1));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.detail && !(self.expanded && self.table_focus()) {
                    self.scroll = self.scroll.saturating_sub(1)
                } else {
                    self.selected = self.selected.saturating_sub(1);
                }
            }
            KeyCode::PageDown => {
                if self.detail && !(self.expanded && self.table_focus()) {
                    self.scroll = self.scroll.saturating_add(10)
                } else {
                    self.selected = (self.selected + 10).min(self.row_count().saturating_sub(1));
                }
            }
            KeyCode::PageUp => {
                self.selected = self.selected.saturating_sub(10);
                self.scroll = self.scroll.saturating_sub(10);
            }
            KeyCode::Home => {
                self.selected = 0;
                self.scroll = 0
            }
            KeyCode::End => self.selected = self.row_count().saturating_sub(1),
            KeyCode::Enter => match self.tab {
                0 | 12 => self.drill(match self.focus {
                    0 => 2,
                    1 => 3,
                    2 => 4,
                    3 => 6,
                    4 => 2,
                    5 => 9,
                    6 => 1,
                    _ => 11,
                }),
                1 if self.focus == 0 => self.detail = true,
                1 => {
                    let cpu = self.selected_task().map(|t| t.cpu);
                    self.drill(2);
                    if let Some(cpu) = cpu {
                        self.selected = self
                            .snapshot
                            .telemetry
                            .cpus
                            .iter()
                            .position(|c| c.id == cpu)
                            .unwrap_or(0);
                    }
                }
                2 => {
                    let subject = self.scheduler_subjects().get(self.selected).cloned();
                    let mode = self.mode;
                    self.drill(1);
                    if let Some((label, _, cpu, group)) = subject {
                        match mode {
                            1 => self.filter = label.split_whitespace().next().unwrap_or("").into(),
                            2 => self.filter = group.unwrap_or_default(),
                            _ => self.scope_cpu = cpu,
                        }
                    }
                }
                3 if self.focus == 3 => {
                    let identity = self
                        .memory_tasks()
                        .get(self.selected)
                        .map(|x| (x.pid, x.start_ticks));
                    self.drill(1);
                    if let Some((pid, start)) = identity {
                        self.filter = pid.to_string();
                        self.selected = self
                            .visible_tasks()
                            .iter()
                            .position(|x| x.pid == pid && x.start_ticks == start)
                            .unwrap_or(0);
                        self.detail = true;
                    }
                }
                3 => self.detail = true,
                4 => {
                    let id = self
                        .rows()
                        .get(self.selected)
                        .and_then(|r| r.first())
                        .cloned();
                    let irqs = id
                        .as_ref()
                        .and_then(|id| {
                            self.snapshot
                                .telemetry
                                .devices
                                .iter()
                                .find(|d| &d.name == id)
                        })
                        .and_then(|d| {
                            d.fields
                                .iter()
                                .find(|(k, _)| k == "controller IRQ candidates")
                        })
                        .map(|(_, v)| v.clone());
                    self.drill(6);
                    if let Some(irqs) = irqs {
                        if let Some(index) = self.rows().iter().position(|r| {
                            r.first().is_some_and(|id| irqs.split(',').any(|v| v == id))
                        }) {
                            self.selected = index;
                        }
                        self.status = format!("{} controller IRQ candidates: {irqs}; confirm queue mapping with driver evidence",id.unwrap_or_default());
                    } else {
                        self.status =
                            "Controller IRQ mapping unavailable; select a measured IRQ manually"
                                .into();
                    }
                }
                6 => {
                    let cpu = self
                        .rows()
                        .get(self.selected)
                        .and_then(|r| r.get(4))
                        .and_then(|v| v.parse::<u32>().ok());
                    self.drill(2);
                    if let Some(cpu) = cpu {
                        self.selected = self
                            .snapshot
                            .telemetry
                            .cpus
                            .iter()
                            .position(|c| c.id == cpu)
                            .unwrap_or(0);
                    }
                }
                7 => {
                    let group = self
                        .rows()
                        .get(self.selected)
                        .and_then(|r| r.first())
                        .cloned();
                    self.drill(1);
                    if let Some(group) = group {
                        self.filter = group;
                    }
                }
                10 => self.detail = true,
                11 if self.focus == 1 => {
                    if let Some(e) = self
                        .snapshot
                        .telemetry
                        .issues
                        .get(self.selected)
                        .and_then(|i| i.evidence.get(self.evidence_index))
                        .cloned()
                    {
                        self.open_subject(&e.subject, e.at_ms);
                        self.status =
                            format!("Evidence: {} · {} at {}ms", e.label, e.source, e.at_ms);
                    }
                }
                11 if self.focus == 2 => {
                    if let Some(command) = self.proposed_action() {
                        self.execute(&format!("preview {command}"));
                    } else {
                        self.status="No supported placement/quota experiment follows from this evidence; inspect the source first".into();
                    }
                }
                _ => self.detail = !self.detail,
            },
            KeyCode::Esc => self.back(),
            KeyCode::Char('/') => self.filtering = true,
            KeyCode::Char('s') => {
                self.palette = true;
                self.command = "sort ".into();
                self.status = format!(
                    "Sort fields: concern, {}",
                    self.snapshot.views[self.tab].columns.join(", ")
                );
            }
            KeyCode::Char('S') => self.reverse = !self.reverse,
            KeyCode::Char('h') => self.histogram = !self.histogram,
            KeyCode::Char('c') if self.tab == 3 => {
                self.drill(1);
                self.status =
                    "Select a process, Enter inspect, then : probe syscalls pid=TID seconds=10"
                        .into();
            }
            KeyCode::Char('z') if self.tab == 2 => self.auto_scale = !self.auto_scale,
            KeyCode::Char('u') if self.tab == 5 => {
                if let Some(pid) = self
                    .rows()
                    .get(self.selected)
                    .and_then(|r| r.get(7))
                    .and_then(|v| v.parse::<u32>().ok())
                {
                    self.palette = true;
                    self.command = format!("probe syscalls pid={pid} seconds=10 stack");
                } else {
                    self.status = "Select a syscall with an observed caller TID".into();
                }
            }
            KeyCode::Char('U') if self.tab == 3 => self.memory_gib = !self.memory_gib,
            KeyCode::Char('M') if self.tab == 2 => {
                self.metric = (self.metric + 1) % 3;
                self.status = format!(
                    "Scheduler metric: {}",
                    ["wakeup latency", "runtime", "migrations to selected CPU"][self.metric]
                );
            }
            KeyCode::Char('G') if self.tab == 1 => {
                self.grouping = (self.grouping + 1) % 4;
                self.selected = 0;
                self.status = format!(
                    "Group by {} · rows retain thread identity; shared RSS is not summed",
                    ["none", "cgroup", "UID", "parent"][self.grouping]
                );
            }
            KeyCode::Char('g') => {
                self.mode = (self.mode + 1)
                    % match self.tab {
                        1 | 8 => 5,
                        3 | 4 | 7 => 4,
                        _ => 3,
                    };
                self.selected = 0;
            }
            KeyCode::Char('p') if self.replay.is_some() => {
                self.replay_playing = !self.replay_playing
            }
            KeyCode::Char(' ') if self.tab == 7 => {
                if let Some(path) = self
                    .rows()
                    .get(self.selected)
                    .and_then(|r| r.first())
                    .cloned()
                {
                    if !self.folded.remove(&path) {
                        self.folded.insert(path);
                    }
                }
            }
            KeyCode::Char(' ') => {
                self.expanded = !self.expanded;
                self.detail = self.expanded;
            }
            KeyCode::Char('y') => {
                self.clipboard = if self.tab == 1 {
                    self.selected_task().map(|t| {
                        format!(
                            "{} pid={} start={} cgroup={}",
                            t.name, t.pid, t.start_ticks, t.cgroup
                        )
                    })
                } else {
                    self.rows()
                        .get(self.selected)
                        .and_then(|r| r.first())
                        .cloned()
                };
            }
            KeyCode::Char('t') if self.tab == 1 => {
                if let Some(pid) = self.selected_task().map(|x| x.pid) {
                    self.drill(5);
                    self.palette = true;
                    self.command = format!("probe syscalls pid={pid} seconds=10");
                    self.status =
                        "Review selected thread capture; Enter starts a bounded trace".into();
                }
            }
            KeyCode::Char('x') if self.tab == 8 => self.execute("mark"),
            KeyCode::Char('w') if self.tab == 1 => self.execute("watch"),
            KeyCode::Char('u') if self.tab == 7 => self.detail = true,
            KeyCode::Char('p') if self.tab == 7 => {
                self.focus = 2;
                self.pressure_view = !self.pressure_view;
                self.status = if self.pressure_view {
                    "Selected cgroup CPU / IO pressure history"
                } else {
                    "Selected cgroup runtime / quota / throttling"
                }
                .into();
            }
            KeyCode::Char('r') => self.execute("record"),
            KeyCode::Char('e') => self.execute("export"),
            KeyCode::Char('v') => self.execute("verify"),
            KeyCode::Char('c') if self.tab == 1 => {
                let group = self.selected_task().map(|t| t.cgroup.clone());
                self.drill(7);
                if let Some(group) = group {
                    self.selected = self
                        .rows()
                        .iter()
                        .position(|r| r.first() == Some(&group))
                        .unwrap_or(0);
                }
            }
            KeyCode::Char('E') if self.tab == 5 => self.mode = if self.mode == 1 { 0 } else { 1 },
            KeyCode::Char('B') if self.tab == 8 => self.execute("baseline"),
            KeyCode::Char('c' | 'Q') if self.tab == 7 => {
                if let Some(group) = self
                    .rows()
                    .get(self.selected)
                    .and_then(|r| r.first())
                    .cloned()
                {
                    self.palette = true;
                    self.command = if k.code == KeyCode::Char('Q') {
                        format!("preview quota {group} ")
                    } else {
                        format!("preview cpuset {group} ")
                    };
                }
            }
            KeyCode::Char('c') if self.tab == 10 => {
                let event = self.visible_events().get(self.selected).cloned().cloned();
                self.drill(11);
                if let Some(event) = event {
                    let matched = self
                        .snapshot
                        .telemetry
                        .issues
                        .iter()
                        .position(|i| i.evidence.iter().any(|e| e.subject == event.subject));
                    if let Some(i) = matched {
                        self.selected = i;
                        self.status = format!(
                            "Related event {} · {}ms; relationship is evidence, not proof of cause",
                            event.id, event.at_ms
                        );
                    } else {
                        self.status = format!(
                            "No issue directly matches event {}; inspect the available findings",
                            event.id
                        );
                    }
                }
            }
            KeyCode::Char('t') if self.tab == 10 => {
                if let Some(event) = self.visible_events().get(self.selected).cloned().cloned() {
                    self.open_subject(&event.subject, event.at_ms);
                }
            }
            KeyCode::Char('i') if [1, 2, 4].contains(&self.tab) => self.drill(6),
            KeyCode::Char('a') => {
                self.palette = true;
                self.command = if self.tab == 1 {
                    self.selected_task()
                        .map(|t| format!("preview task {} {} ", t.pid, t.start_ticks))
                        .unwrap_or("preview task ".into())
                } else {
                    "preview irq ".into()
                };
            }
            KeyCode::Char('l') if self.tab == 1 || self.tab == 12 => {
                if let Some(task) = self.selected_task() {
                    let pid = task.pid;
                    if self.snapshot.demo || self.replay.is_some() {
                        self.status = "Latency capture requires live mode".into();
                    } else {
                        self.probe_request = Some(format!("sched pid={pid} seconds=30"));
                        self.status = format!("Starting 30s scheduler capture for TID {pid}");
                    }
                }
            }
            KeyCode::Char('n') if self.tab == 9 => {
                self.palette = true;
                self.command = "probe ".into();
            }
            KeyCode::Char(c) => {
                if let Some(t) = TABS.iter().position(|(_, key)| *key == c) {
                    self.switch(t)
                }
            }
            _ => {}
        }
    }
    fn table_focus(&self) -> bool {
        self.focus == 0 || (self.tab == 12 && self.focus == 6) || (self.tab == 0 && self.focus == 2)
    }
    fn row_count(&self) -> usize {
        if self.tab == 3 && self.focus == 3 {
            self.memory_tasks().len()
        } else if self.tab == 3 && self.focus == 0 {
            self.snapshot
                .telemetry
                .details
                .get("slab")
                .map(Vec::len)
                .unwrap_or(0)
        } else if self.tab == 2 {
            self.scheduler_subjects().len()
        } else if self.tab == 11 {
            self.snapshot.telemetry.issues.len()
        } else if self.tab == 1
            || (self.tab == 12 && self.focus == 6)
            || (self.tab == 0 && self.focus == 2)
        {
            self.visible_tasks().len()
        } else {
            self.rows().len()
        }
    }
    pub fn execute(&mut self, command: &str) {
        let mut parts = command.trim().splitn(2, ' ');
        let cmd = parts.next().unwrap_or("");
        let arg = parts.next().unwrap_or("").trim();
        match cmd {
            "mark" if self.tab == 8 => {
                if let Some(name) = self
                    .rows()
                    .get(self.selected)
                    .and_then(|r| r.first())
                    .cloned()
                {
                    let marked = if self.marked_modules.remove(&name) {
                        false
                    } else {
                        self.marked_modules.insert(name.clone());
                        true
                    };
                    if let Some(module) = self
                        .snapshot
                        .telemetry
                        .modules
                        .iter_mut()
                        .find(|m| m.name == name)
                    {
                        module.fields.retain(|(k, _)| k != "analyst mark");
                        if marked {
                            module.fields.push((
                                "analyst mark".into(),
                                "suspicious; operator annotation, not proven malicious".into(),
                            ));
                        }
                        self.snapshot
                            .telemetry
                            .details
                            .insert(format!("module:{name}"), module.fields.clone());
                    }
                    self.status = format!(
                        "{name}: analyst mark {}; included in exported snapshot",
                        if marked { "set" } else { "cleared" }
                    );
                }
            }
            "actions" => {
                self.reviewing_action = true;
                self.detail = true;
                self.expanded = false;
                self.scroll = 0;
            }
            "sort" => {
                if arg == "concern" {
                    self.sort = 0;
                } else if let Some(index) = self.snapshot.views[self.tab]
                    .columns
                    .iter()
                    .position(|s| s.eq_ignore_ascii_case(arg))
                {
                    self.sort = index + 1;
                } else {
                    self.status = format!(
                        "Unknown sort field. Choose concern or {}",
                        self.snapshot.views[self.tab].columns.join(", ")
                    );
                    return;
                }
                self.selected = 0;
                self.status = format!("Sort: {arg} · S reverse");
            }
            "view" => {
                if let Some(i) = TABS.iter().position(|(n, _)| n.eq_ignore_ascii_case(arg)) {
                    self.switch(i)
                } else {
                    self.status = "Unknown view".into()
                }
            }
            "filter" => {
                self.filter = arg.into();
                self.selected = 0
            }
            "capabilities" => {
                self.capabilities_view = true;
                self.detail = true;
                self.scroll = 0;
            }
            "inspect" => {
                self.capabilities_view = false;
                self.detail = true;
            }
            "watch" => {
                if let Some(task) = self.selected_task() {
                    let key = (task.pid, task.start_ticks);
                    if self.watched.remove(&key) {
                        self.status = "Removed task from watchlist".into();
                    } else {
                        self.watched.insert(key);
                        self.status = "Task identity added to watchlist".into();
                    }
                }
            }
            "freeze" => self.frozen = !self.frozen,
            "export" => {
                self.status = match self.export() {
                    Ok(p) => format!("✓ exported {} · 8 files", p.display()),
                    Err(e) => format!("Export failed: {e}"),
                }
            }
            "record" => {
                if let Some(rec) = self.recording.take() {
                    self.status = format!(
                        "Recording saved: {} · {} frames",
                        rec.path.display(),
                        rec.count
                    );
                } else {
                    let path = PathBuf::from(format!("kernwatch-{}.kwr", recording::stamp()));
                    match Recorder::create(path) {
                        Ok(mut rec) => match rec.push(&self.snapshot) {
                            Ok(()) => {
                                self.status = format!("Recording {}", rec.path.display());
                                self.recording = Some(rec)
                            }
                            Err(e) => self.status = e.to_string(),
                        },
                        Err(e) => self.status = format!("Recording failed: {e}"),
                    }
                }
            }
            "play" => self.replay_playing = self.replay.is_some(),
            "pause" => self.replay_playing = false,
            "speed" => {
                if let Ok(speed) = arg.parse::<f64>() {
                    if (0.1..=16.).contains(&speed) {
                        self.replay_speed = speed;
                        self.status = format!("Replay {speed}×");
                    } else {
                        self.status = "Speed must be 0.1..16".into();
                    }
                }
            }
            "replay" => match recording::read(std::path::Path::new(arg)) {
                Ok(frames) if !frames.is_empty() => {
                    self.snapshot = frames[0].clone();
                    self.replay = Some(frames);
                    self.replay_index = 0;
                    self.replay_elapsed = 0.;
                    self.replay_playing = false;
                    self.time_cursor = None;
                    self.frozen = true;
                    self.status = "Replay loaded · ← → seek".into()
                }
                Ok(_) => self.status = "Recording is empty".into(),
                Err(e) => self.status = format!("Replay failed: {e}"),
            },
            "live" => {
                self.replay = None;
                self.frozen = false;
                self.time_cursor = None
            }
            "probe" => {
                self.probe_request = Some(arg.into());
                self.status = format!("Requesting {arg} trace acquisition…")
            }
            "stop-probe" => self.probe_request = Some("stop".into()),
            "preview" => {
                if self.replay.is_some() {
                    self.status =
                        "Replay is read-only; action previews require demo or live mode.".into();
                } else {
                    self.status = match crate::actions::parse_target(arg).and_then(
                        |(path, value, description)| {
                            if self.snapshot.demo {
                                let before =
                                    match arg.split_whitespace().collect::<Vec<_>>().as_slice() {
                                        ["irq", id, _] => self.snapshot.views[6]
                                            .rows
                                            .iter()
                                            .find(|r| r.first().is_some_and(|v| v == id))
                                            .and_then(|r| r.get(3))
                                            .cloned(),
                                        ["task", pid, start, _] => self
                                            .snapshot
                                            .telemetry
                                            .tasks
                                            .iter()
                                            .find(|t| {
                                                t.pid.to_string() == *pid
                                                    && t.start_ticks.to_string() == *start
                                            })
                                            .map(|t| t.affinity.clone()),
                                        ["quota", group, _, _] => self
                                            .snapshot
                                            .telemetry
                                            .cgroups
                                            .iter()
                                            .find(|g| g.path == *group)
                                            .map(|g| g.quota.clone()),
                                        ["cpuset", group, _] => self
                                            .snapshot
                                            .telemetry
                                            .cgroups
                                            .iter()
                                            .find(|g| g.path == *group)
                                            .map(|g| g.cpus.clone()),
                                        _ => None,
                                    }
                                    .ok_or_else(|| {
                                        std::io::Error::other(
                                            "target is not represented in the fixture",
                                        )
                                    })?;
                                self.demo_host
                                    .0
                                    .borrow_mut()
                                    .entry(path.clone())
                                    .or_insert(before);
                                crate::actions::Plan::preview(
                                    &self.demo_host,
                                    path,
                                    value,
                                    format!("DEMO SIMULATION: {description}"),
                                )
                            } else {
                                crate::actions::Plan::preview(
                                    &crate::actions::LinuxHost,
                                    path,
                                    value,
                                    description,
                                )
                            }
                        },
                    ) {
                        Ok(plan) => {
                            let msg = format!(
                                "DRY RUN {}: {} → {}. To confirm: apply {}",
                                plan.description,
                                plan.before.trim(),
                                plan.after,
                                plan.id
                            );
                            self.pending_action = Some(plan);
                            self.reviewing_action = true;
                            self.detail = true;
                            self.expanded = false;
                            self.scroll = 0;
                            msg
                        }
                        Err(e) => format!("Preview failed: {e}"),
                    };
                }
            }
            "apply" => {
                if self.replay.is_some() {
                    self.status = "Replay is read-only".into();
                    return;
                }
                if let Some(mut plan) = self.pending_action.take() {
                    if plan.description.starts_with("DEMO SIMULATION:") != self.snapshot.demo {
                        self.status =
                            "Action belongs to a different acquisition mode; preview again".into();
                        self.pending_action = Some(plan);
                    } else if arg != plan.id.to_string() {
                        self.status = "Action id does not match reviewed preview".into();
                        self.pending_action = Some(plan);
                    } else {
                        self.action_journal.push(plan.clone());
                        if let Err(e) =
                            crate::actions::save_journal(&self.journal_path, &self.action_journal)
                        {
                            self.action_journal.pop();
                            self.pending_action = Some(plan);
                            self.status =
                                format!("Apply refused: cannot persist reviewed intent: {e}");
                            return;
                        }
                        self.status = match if self.snapshot.demo { plan.apply(&self.demo_host) } else { plan.apply(&crate::actions::LinuxHost) } {
                            Ok(()) => {
                                if self.snapshot.demo { "DEMO applied in memory; use scenario recovery then verify to test the fixture outcome" } else { "Action applied and read back; performance verification pending" }.into()
                            }
                            Err(e) => format!("Apply failed: {e}"),
                        };
                        if plan.applied {
                            plan.applied_at_ms = Some(self.snapshot.telemetry.at_ms);
                        }
                        *self.action_journal.last_mut().unwrap() = plan;
                        self.reviewing_action = true;
                        self.detail = true;
                        self.save_journal();
                    }
                } else {
                    self.status = "No reviewed action; use preview first".into();
                }
            }
            "revert" => {
                if self.replay.is_some() {
                    self.status = "Replay is read-only".into();
                    return;
                }
                if self.action_journal.last().is_some_and(|p| {
                    p.description.starts_with("DEMO SIMULATION:") != self.snapshot.demo
                }) {
                    self.status =
                        "Action belongs to a different acquisition mode; rollback refused".into();
                    return;
                }
                if let Some(plan) = self.action_journal.last_mut() {
                    plan.outcome = "rollback requested; readback pending".into();
                }
                if let Err(e) =
                    crate::actions::save_journal(&self.journal_path, &self.action_journal)
                {
                    self.status = format!("Rollback refused: cannot persist intent: {e}");
                    return;
                }
                if let Some(plan) = self.action_journal.last_mut() {
                    self.status = match if self.snapshot.demo {
                        plan.revert(&self.demo_host)
                    } else {
                        plan.revert(&crate::actions::LinuxHost)
                    } {
                        Ok(()) => "Action reverted and read back".into(),
                        Err(e) => format!("Revert failed: {e}"),
                    };
                    self.save_journal();
                } else {
                    self.status = "No applied action".into();
                }
            }
            "theme" => {
                self.terminal_theme = !self.terminal_theme;
                self.status = if self.terminal_theme {
                    "Terminal palette"
                } else {
                    "Reference palette"
                }
                .into();
            }
            "baseline" => {
                let path = PathBuf::from("kernwatch-baseline.json");
                let mut baseline =
                    serde_json::to_value(&self.snapshot.views[8]).unwrap_or_default();
                baseline["version"] = 1.into();
                baseline["accepted_at_unix_ms"] = serde_json::json!(std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis());
                baseline["boot_id"] = self.snapshot.telemetry.boot_id.clone().into();
                baseline["capture_at_ms"] = self.snapshot.telemetry.at_ms.into();
                baseline["meaning"] = "Operator accepted this inventory; acceptance does not establish trust or causality".into();
                self.status = match serde_json::to_vec_pretty(&baseline)
                    .map_err(std::io::Error::from)
                    .and_then(|v| std::fs::write(&path, v))
                {
                    Ok(()) => format!("Module baseline saved: {}", path.display()),
                    Err(e) => format!("Baseline failed: {e}"),
                };
            }
            "verify" => {
                let issue = self.snapshot.telemetry.issues.get(self.selected);
                let key = issue.map(|i| i.id.as_str()).unwrap_or("");
                let target = if key.starts_with("sched") {
                    Some(("sched.p99".to_string(), 1.))
                } else if key.starts_with("cpu:") {
                    Some((key.replace("cpu:", "cpu."), 90.))
                } else if key.contains("block") || key.contains("flush") || key == "disk.age" {
                    Some(("disk.age".into(), 1000.))
                } else if key.contains("slab") {
                    Some(("slab.growth".into(), 10.))
                } else if key == "psi.memory" {
                    Some((key.into(), 10.))
                } else if let Some(group) = key.strip_prefix("throttle:") {
                    Some((format!("cgroup:{group}:throttled"), 50.))
                } else if key == "bpf.total" {
                    Some((key.into(), 20.))
                } else {
                    None
                };
                if let Some((metric, threshold)) = target {
                    let end = self.cursor();
                    let result = self
                        .snapshot
                        .telemetry
                        .series
                        .get(&metric)
                        .map(|s| crate::diagnose::verify(s, end, threshold))
                        .unwrap_or(crate::diagnose::Verdict::Inconclusive);
                    let result = if self
                        .action_journal
                        .last()
                        .and_then(|p| p.applied_at_ms)
                        .is_some_and(|at| end < at.saturating_add(60000))
                    {
                        crate::diagnose::Verdict::Inconclusive
                    } else {
                        result
                    };
                    let before = self
                        .action_journal
                        .last()
                        .and_then(|p| p.applied_at_ms)
                        .and_then(|at| {
                            self.snapshot
                                .telemetry
                                .series
                                .get(&metric)
                                .map(|s| (at, crate::diagnose::verify(s, at, threshold)))
                        });
                    let explanation=format!("{result:?}: {metric} ≤ {threshold} over {}..{end}ms; ≥60 samples, no gaps >1.5s. Causal attribution is separate.",end.saturating_sub(60000));
                    let explanation = format!("{explanation} Pre-action window: {before:?}.");
                    if let Some(plan) = self.action_journal.last_mut() {
                        plan.verification = Some(explanation.clone());
                    }
                    if !self.action_journal.is_empty() {
                        self.save_journal();
                    }
                    self.status = format!("Verification {explanation}");
                    self.frozen = true;
                    if let Some(issue) = self.snapshot.telemetry.issues.get_mut(self.selected) {
                        issue.state = format!("verification {result:?}");
                        issue.verification = explanation;
                    }
                } else {
                    self.status =
                        "Verification inconclusive: select an issue with a measurable target"
                            .into();
                }
            }
            "scenario" => {
                if self.snapshot.demo {
                    if !["incident", "pre", "recovery", "quota"].contains(&arg) {
                        self.status = "Scenario: pre | incident | quota | recovery".into();
                        return;
                    }
                    self.demo_scenario = arg.into();
                    self.snapshot.telemetry = crate::fixture::scenario(arg);
                    self.snapshot.cpu = self
                        .snapshot
                        .telemetry
                        .cpus
                        .iter()
                        .map(|c| c.busy)
                        .collect();
                    self.selected = 0;
                    self.time_cursor = None;
                    self.latest_snapshot = Some(self.snapshot.clone());
                    self.status = format!("Demo {arg} scenario · synthetic evidence");
                }
            }
            _ => self.status = format!("Unknown command: {command}"),
        }
    }
    pub fn tick(&mut self, elapsed_ms: u64) {
        if !self.replay_playing {
            return;
        }
        let Some(frames) = &self.replay else {
            return;
        };
        self.replay_elapsed += elapsed_ms as f64 * self.replay_speed;
        while self.replay_index + 1 < frames.len() {
            let delay = frames[self.replay_index + 1]
                .telemetry
                .at_ms
                .saturating_sub(frames[self.replay_index].telemetry.at_ms)
                .max(1) as f64;
            if self.replay_elapsed < delay {
                break;
            }
            self.replay_elapsed -= delay;
            self.replay_index += 1;
            self.snapshot = frames[self.replay_index].clone();
            self.time_cursor = None;
        }
        if self.replay_index + 1 == frames.len() {
            self.replay_playing = false;
            self.status = "Replay complete".into();
        }
    }
    fn save_journal(&mut self) {
        match crate::actions::save_journal(&self.journal_path, &self.action_journal) {
            Ok(()) => {}
            Err(e) => self.status = format!("{}; journal write failed: {e}", self.status),
        }
    }
    pub fn export(&self) -> std::io::Result<PathBuf> {
        recording::export_with_actions(&self.snapshot, &self.action_journal)
    }
}

/// OSC52 transports bytes as base64; control characters cannot escape the payload.
pub fn clipboard_base64(bytes: &[u8]) -> String {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = a << 16 | b << 8 | c;
        out.push(alphabet[((n >> 18) & 63) as usize] as char);
        out.push(alphabet[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            alphabet[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            alphabet[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
