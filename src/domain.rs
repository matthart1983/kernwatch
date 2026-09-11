//! Raw measurements, identity and acquisition quality; no terminal formatting.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum Quality {
    #[default]
    Warming,
    Available,
    Stale,
    Denied(String),
    Unsupported(String),
    Stopped,
    Error(String),
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Measurement {
    pub value: Option<f64>,
    pub unit: String,
    pub source: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub count: u64,
    pub quality: Quality,
}
impl Measurement {
    pub fn known(value: f64, unit: &str, source: &str, end_ms: u64) -> Self {
        Self {
            value: Some(value),
            unit: unit.into(),
            source: source.into(),
            start_ms: end_ms,
            end_ms,
            count: 1,
            quality: Quality::Available,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Sample {
    pub at_ms: u64,
    pub value: Option<f64>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Series {
    pub name: String,
    pub unit: String,
    pub max: f64,
    pub samples: Vec<Sample>,
}
impl Series {
    pub fn at(&self, cursor: u64) -> Option<f64> {
        self.samples
            .iter()
            .rev()
            .find(|s| s.at_ms <= cursor)
            .and_then(|s| s.value)
    }
    pub fn window(&self, end: u64, seconds: u64) -> Vec<Option<f64>> {
        // Absolute, one-second buckets. A refresh within the same second cannot
        // shift historical points. Negative (pre-boot) slots stay blank.
        let last_second = end / 1000;
        let mut index = 0;
        let mut out = Vec::with_capacity(seconds as usize + 1);
        for offset in (0..=seconds).rev() {
            let Some(second) = last_second.checked_sub(offset) else {
                out.push(None);
                continue;
            };
            let start = second * 1000;
            let stop = start.saturating_add(999).min(end);
            while index < self.samples.len() && self.samples[index].at_ms <= stop {
                index += 1;
            }
            out.push(
                index
                    .checked_sub(1)
                    .and_then(|i| self.samples.get(i))
                    // A bucket requires its own observation. Never carry a
                    // previous value forward to fill uncollected time.
                    .filter(|s| s.at_ms >= start)
                    .and_then(|s| s.value.filter(|v| v.is_finite())),
            );
        }
        out
    }
    pub fn push(&mut self, at_ms: u64, value: Option<f64>) {
        if let Some(last) = self.samples.last() {
            if last.at_ms == at_ms {
                return;
            }
        }
        self.samples.push(Sample { at_ms, value });
        if self.samples.len() > 600 {
            self.samples.drain(..self.samples.len() - 600);
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Task {
    #[serde(default)]
    pub nice: Option<i32>,
    #[serde(default)]
    pub age_ms: Option<u64>,
    #[serde(default)]
    pub anon_bytes: Option<u64>,
    #[serde(default)]
    pub file_bytes: Option<u64>,
    #[serde(default)]
    pub shmem_bytes: Option<u64>,
    #[serde(default)]
    pub swap_bytes: Option<u64>,
    #[serde(default)]
    pub minor_faults_s: Option<f64>,
    #[serde(default)]
    pub rss_growth_bytes_s: Option<f64>,
    #[serde(default)]
    pub pss_at_ms: Option<u64>,
    #[serde(default)]
    pub uid: Option<u32>,
    #[serde(default)]
    pub parent_pid: u32,
    #[serde(default)]
    pub tgid: u32,
    #[serde(default)]
    pub kernel_thread: bool,
    pub pid: u32,
    pub start_ticks: u64,
    pub name: String,
    pub state: String,
    pub cpu: u32,
    pub cpu_pct: Option<f64>,
    pub rss_bytes: u64,
    pub pss_bytes: Option<u64>,
    pub cgroup: String,
    pub affinity: String,
    pub policy: String,
    pub wchan: String,
    pub wake_p50_ms: Option<f64>,
    pub wake_p99_ms: Option<f64>,
    pub blocked_ms: Option<f64>,
    pub voluntary_s: Option<f64>,
    pub involuntary_s: Option<f64>,
    pub verdict: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cpu {
    #[serde(default)]
    pub wake_p50_ms: Option<f64>,
    pub id: u32,
    pub busy: f64,
    pub user: f64,
    pub kernel: f64,
    pub softirq: f64,
    pub irq: f64,
    pub steal: f64,
    pub runnable: Option<u32>,
    pub wake_p99_ms: Option<f64>,
    pub switches_s: Option<f64>,
    pub migrations_s: Option<f64>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub at_ms: u64,
    pub source: String,
    pub severity: String,
    pub message: String,
    pub subject: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Evidence {
    pub label: String,
    pub source: String,
    pub at_ms: u64,
    pub subject: String,
    pub weight: f64,
    pub observed: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cause {
    pub label: String,
    pub detail: String,
    pub observed: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub subject: String,
    pub severity: String,
    pub since_ms: u64,
    pub state: String,
    pub evidence: Vec<Evidence>,
    pub causes: Vec<Cause>,
    pub steps: Vec<String>,
    pub verification: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Histogram {
    pub bounds: Vec<f64>,
    pub counts: Vec<u64>,
    pub unit: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunnableWait {
    pub pid: u32,
    pub start_ticks: u64,
    pub cpu: u32,
    pub age_ms: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Telemetry {
    #[serde(default)]
    pub runnable_waits: Vec<RunnableWait>,
    pub boot_id: String,
    pub hostname: String,
    pub kernel: String,
    pub at_ms: u64,
    pub cpus: Vec<Cpu>,
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(default)]
    pub cgroups: Vec<Cgroup>,
    #[serde(default)]
    pub modules: Vec<Module>,
    pub metrics: BTreeMap<String, Measurement>,
    pub series: BTreeMap<String, Series>,
    pub events: Vec<Event>,
    pub issues: Vec<Issue>,
    pub histograms: BTreeMap<String, Histogram>,
    pub details: BTreeMap<String, Vec<(String, String)>>,
    pub capabilities: BTreeMap<String, Quality>,
    pub trace_drops: u64,
}
impl Telemetry {
    pub fn concern(&self, family: &str) -> Option<&Issue> {
        self.issues.iter().find(|i| {
            if i.state == "resolved" || i.state.contains("Passed") || i.state == "data unavailable"
            {
                return false;
            }
            match family {
                "sched" => i.id.contains("sched"),
                "irq" => {
                    i.id.contains("irq")
                        || i.evidence
                            .iter()
                            .any(|e| e.observed && e.source.contains("softirq"))
                }
                "memory" => i.id.contains("memory") || i.id.contains("slab"),
                "block" => {
                    i.id.contains("block") || i.id.contains("flush") || i.id.starts_with("disk.")
                }
                "dstate" => {
                    i.id.starts_with("blocked:") || i.id.contains("flush") || i.id == "block-age"
                }
                "cpu" => i.id.starts_with("cpu:"),
                _ => false,
            }
        })
    }
    pub fn value(&self, key: &str) -> Option<f64> {
        self.metrics.get(key).and_then(|m| m.value)
    }
    pub fn record(&mut self, key: &str, value: Option<f64>, unit: &str, max: f64) {
        let s = self.series.entry(key.into()).or_insert_with(|| Series {
            name: key.into(),
            unit: unit.into(),
            max,
            samples: Vec::new(),
        });
        s.max = s.max.max(max);
        s.push(self.at_ms, value);
    }
    pub fn selected_task(&self, row: usize) -> Option<&Task> {
        self.tasks.get(row.min(self.tasks.len().saturating_sub(1)))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Device {
    pub name: String,
    pub major_minor: String,
    pub partition: bool,
    pub read_mib_s: Option<f64>,
    pub write_mib_s: Option<f64>,
    pub iops: Option<f64>,
    pub inflight: u64,
    pub await_ms: Option<f64>,
    #[serde(default)]
    pub p99_ms: Option<f64>,
    pub busy_pct: Option<f64>,
    pub fields: Vec<(String, String)>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cgroup {
    pub path: String,
    pub inode: u64,
    pub runtime_pct: Option<f64>,
    pub throttled_ms_s: Option<f64>,
    pub memory_bytes: Option<u64>,
    pub quota: String,
    pub cpus: String,
    pub fields: Vec<(String, String)>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub bytes: u64,
    pub refs: u64,
    pub state: String,
    pub taint: String,
    pub fields: Vec<(String, String)>,
}
