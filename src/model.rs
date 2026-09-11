use serde::{Deserialize, Serialize};

pub const SYSCALL_COLUMNS: [&str; 10] = [
    "syscall",
    "calls/s",
    "negative/s",
    "top return",
    "mean",
    "p99",
    "max",
    "caller TID",
    "retained",
    "verdict",
];

pub const TABS: [(&str, char); 13] = [
    ("Overview", '1'),
    ("Tasks", '2'),
    ("Scheduler", '3'),
    ("Memory", '4'),
    ("Block", '5'),
    ("Syscalls", '6'),
    ("IRQ", '7'),
    ("Cgroups", '8'),
    ("Modules", '9'),
    ("eBPF", 'b'),
    ("Dmesg", 'm'),
    ("Diagnose", 'd'),
    ("Dense", '0'),
];
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct View {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub notes: Vec<String>,
}
impl View {
    pub fn new(columns: &[&str], rows: Vec<Vec<String>>, notes: &[&str]) -> Self {
        Self {
            columns: columns.iter().map(|s| s.to_string()).collect(),
            rows,
            notes: notes.iter().map(|s| s.to_string()).collect(),
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub telemetry: crate::domain::Telemetry,
    pub demo: bool,
    pub captured: u64,
    pub cpu: Vec<f64>,
    pub mem_used: u64,
    pub mem_total: u64,
    pub load: String,
    pub views: Vec<View>,
    pub findings: Vec<String>,
}
pub fn row(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}
pub fn demo() -> Snapshot {
    let mut s = Snapshot {
        telemetry: crate::fixture::incident(),
        demo: true,
        captured: 1789118002,
        cpu: vec![41., 37., 44., 98., 29., 33., 40., 31.],
        mem_used: 20070,
        mem_total: 32768,
        load: "9.8 / 8.1 / 6.4".into(),
        views: vec![View::default(); 13],
        findings: vec![
            "Observed: CPU3 NET_RX 91%; Envoy wakeup p99 18 ms.".into(),
            "Observed: one outstanding FLUSH aged 4.1 s; completed I/O p99 1.9 ms.".into(),
            "Hypothesis: shared CPU affinity contributes to latency; storage causality unverified."
                .into(),
            "Separate finding: dentry slab +310 MB/h; unique-path allocation evidence needed."
                .into(),
        ],
    };
    s.views[1] = View::new(&["Task / PID","State","CPU","CPU %","RSS","Wake p99"], vec![row(&["ksoftirqd/3 34","R","3","91.0","-","unavailable"]),row(&["envoy 1188","S","3","4.0","412 MB","18 ms"]),row(&["envoy 1191","S","3","3.0","shared","16 ms"]),row(&["kworker/3:1 3312","D","3","0.0","-","unavailable"]),row(&["node 2210","R","5","32.0","1.1 GB","0.3 ms"]),row(&["postgres 902","S","1","20.0","1.9 GB","0.6 ms"])], &["CPU percentages are per logical CPU, sampled over the same 10-second window.","CPU3: softirq worker 91% + Envoy workers 7% = 98%; 2% idle.","The blocked worker has an outstanding wait aged 4.1 s, not a wakeup percentile.","Enter opens the selected task's details. Scheduler latency in this fixture represents trace data."]);
    s.views[2] = View::new(
        &["CPU", "Busy %", "NET_RX %", "Runnable", "Wake p99"],
        s.cpu
            .iter()
            .enumerate()
            .map(|(i, c)| {
                vec![
                    i.to_string(),
                    format!("{c:.0}"),
                    if i == 3 { "91" } else { "1" }.into(),
                    if i == 3 { "7" } else { "1" }.into(),
                    if i == 3 { "18 ms" } else { "0.5 ms" }.into(),
                ]
            })
            .collect(),
        &[
            "Observed: Envoy workers are affined to CPU3; other CPUs have spare capacity.",
            "Wakeup latency requires sched_wakeup -> sched_switch correlation.",
            "Hypothesis: widen worker affinity and compare the same latency window before/after.",
        ],
    );
    s.views[3] = View::new(&["Metric","Value","State"],vec![row(&["Used","19.6 GiB","61.3%"]),row(&["Available","11.2 GiB","includes reclaimable"]),row(&["Page cache","7.4 GB","overlapping accounting"]),row(&["Slab","2.9 GB","dentry +310 MB/h"]),row(&["Memory PSI some","0.8%","10-second average"]),row(&["Major faults","0/s","nominal"])], &["Repeated misses of the same cached names do not establish unbounded negative-dentry growth.","To attribute growth: measure unique parent/name pairs, dentry allocations and frees.","Cache, slab and available are overlapping Linux accounting categories; do not sum them."]);
    s.views[4] = View::new(&["Device","Read MB/s","Write MB/s","Done p99","Oldest in flight"],vec![row(&["nvme0n1","48","31","1.9 ms","FLUSH 4.1 s"]),row(&["nvme1n1","12","4","1.2 ms","0.3 ms"])], &["An outstanding request age is not a completed-request latency percentile.","nvme0 queue 3 and NIC receive IRQ share CPU3. This is correlation, not proof of a completion-path stall.","Verify with IRQ entry/exit and block request issue/completion traces before declaring a cause."]);
    s.views[5] = View::new(&SYSCALL_COLUMNS, vec![
        row(&["newfstatat","14200","13900","ENOENT","0.002ms","0.009ms","0.012ms","2210","4096","lookup errors"]),
        row(&["epoll_wait","9100","0","—","12ms","22ms","24ms","1188","4096","blocking wait"]),
        row(&["fsync","41","0","—","1.2ms","1.9ms","2.4ms","902","2460","completed only"]),
        row(&["read","6400","12","EAGAIN","0.004ms","0.031ms","1.2ms","902","4096","completed"]),
        row(&["write","5900","0","—","0.006ms","0.048ms","2.1ms","902","4096","completed"]),
        row(&["futex","3100","210","EAGAIN","0.012ms","0.4ms","3ms","2210","4096","negative returns"])
    ], &["Duration includes blocked time; it does not isolate wakeup delay. Rates cover the fixture interval; stream contains selected events.", "Outstanding FLUSH age is excluded from completed-call statistics. ENOENT alone does not attribute slab growth."]);
    s.views[6] = View::new(&["IRQ","Name","Affinity","Rate/s","Observation"],vec![row(&["47","eth0-rx-0","3","148,000","shares CPU3"]),row(&["48","eth0-tx-0","0-7","31,000","spread"]),row(&["52","nvme0q3","3","2,000","shares CPU3"])], &["Observed affinity change at 09:09:40; wakeup latency increase at 09:12:04.","Timing supports investigation. It does not prove that softirq activity caused the 4.1-second flush.","Candidate experiments: distribute RX queues; move IRQ affinity; widen task affinity. Evaluate separately."]);
    s.views[7] = View::new(&["Cgroup","CPU %","Quota","Affinity","Throttled delta"],vec![row(&["envoy.service","7.0","0.5 CPU","3","0 / 60 s"]),row(&["postgresql.service","20.0","unlimited","all","0 / 60 s"]),row(&["web","32.0","2 CPUs","5","0 / 60 s"])], &["CPU quota is consumed by runtime, not time waiting for CPU.","This fixture shows contention without quota throttling: Envoy uses 7% of one CPU.","Task CPU affinity and cgroup cpuset are separate controls. Inspect both before changing placement."]);
    s.views[8] = View::new(&["Module","Size","Refs","Status"],vec![row(&["example_probe","1.2 MB","1","new / unsigned"]),row(&["xfs","2.1 MB","1","baseline"]),row(&["nvme","61 KB","4","baseline"])], &["Demo module names are fictional. An unsigned module is a trust finding, not proof of a latency cause.","Taint O/E records out-of-tree and unsigned modules; historical taint can persist after unload."]);
    s.views[9] = View::new(&["Owner","Runs/s","Mean ns","CPU % (one core)"],vec![row(&["kernwatch","100,000","210","2.10"]),row(&["network agent","148,000","250","3.70"]),row(&["endpoint agent","18,000","2,400","4.32"])], &["All CPU overhead uses one-core normalization: calls/second * nanoseconds/call / 10,000,000.","Total instrumented program runtime = 10.12% of one CPU (1.265% of this 8-CPU host).","kernwatch's own overhead = 2.10% of one CPU; this is not the total.","These values are deterministic demo data, not measurements of the running monitor."]);
    s.views[10] = View::new(&["Time","Source","Event"],vec![row(&["09:09:40","affinity trace","IRQ 47 moved to CPU3"]),row(&["09:12:04","sched trace","Envoy wakeup p99 increased to 18 ms"]),row(&["09:13:22","block trace","Outstanding FLUSH aged 4.1 s"]),row(&["09:13:22","kernwatch","Incident snapshot captured"])], &["Each event has its own row and source. Synthetic trace events are not presented as kernel log messages.","Chronological ordering supports a hypothesis; causal verification is pending."]);
    s.views[11] = View::new(&["Finding","Evidence","Confidence"],vec![row(&["CPU3 contention","91% NET_RX; Envoy pinned","observed"]),row(&["Scheduler impact","wakeup p99 18 ms","observed"]),row(&["Storage coupling","queue IRQ shares CPU3","hypothesis"]),row(&["Dentry growth","+310 MB/h; origin unknown","observed"] )], &["Hypothesis: IRQ placement contributes to scheduler delay. No verified root cause yet.","1. Record baseline wakeup latency, CPU distribution and request completion latency.","2. Test one placement change at a time; inspect driver/affinity constraints first.","3. Compare equivalent windows under equivalent load. A single IRQ move need not reduce per-CPU NET_RX below 15%.","4. Confirm or reject the storage hypothesis with completion-path tracing.","Host changes require an explicit reviewed plan ID. Demo actions are simulated in memory."]);
    s.views[9].columns = [
        "id",
        "program",
        "type",
        "runs/s",
        "mean ns",
        "CPU %",
        "maps",
        "owner",
        "attachment",
    ]
    .iter()
    .map(|v| v.to_string())
    .collect();
    s.views[9].rows = demo_bpf_rows(&mut s.telemetry);
    s.views[7].columns = CGROUP_COLUMNS.iter().map(|s| s.to_string()).collect();
    s.views[7].rows = s.telemetry.cgroups.iter().map(cgroup_row).collect();
    s.views[8].columns = MODULE_COLUMNS.iter().map(|s| s.to_string()).collect();
    s.views[8].rows = s.telemetry.modules.iter().map(module_row).collect();
    s.views[6].columns = [
        "IRQ",
        "Name",
        "Rate/s",
        "Configured",
        "Effective",
        "Observed CPU",
        "Handler p99",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    s.views[6].rows = vec![
        row(&["47", "eth0-rx-0", "148000", "3", "3", "cpu3", "0.012 ms"]),
        row(&["48", "eth0-tx-0", "31000", "0-7", "0-7", "cpu0", "0.008 ms"]),
        row(&["52", "nvme0q3", "2000", "3", "3", "cpu3", "0.006 ms"]),
    ];
    s
}

pub const MODULE_COLUMNS: [&str; 9] = [
    "Module", "Size", "Refs", "Used by", "Unsigned", "Tree", "Loaded", "Hooks", "Verdict",
];
pub fn module_row(m: &crate::domain::Module) -> Vec<String> {
    let field = |key: &str| {
        m.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or("—".into())
    };
    vec![
        m.name.clone(),
        format!("{:.1} KiB", m.bytes as f64 / 1024.),
        m.refs.to_string(),
        field("dependent modules"),
        if m.taint.starts_with("unavailable") || m.taint == "—" {
            "unknown"
        } else if m.taint.contains('E') {
            "yes"
        } else {
            "not flagged"
        }
        .into(),
        if m.taint.starts_with("unavailable") || m.taint == "—" {
            "unknown"
        } else if m.taint.contains('O') {
            "out-of-tree"
        } else {
            "in-tree"
        }
        .into(),
        field("loaded"),
        field("hooks"),
        format!(
            "{} {}{}",
            field("baseline"),
            m.taint,
            if m.fields.iter().any(|(k, _)| k == "analyst mark") {
                " · marked"
            } else {
                ""
            }
        ),
    ]
}

pub const DEVICE_COLUMNS: [&str; 10] = [
    "Device",
    "Scheduler",
    "IOPS",
    "Read MiB/s",
    "Write MiB/s",
    "Mean ms",
    "p99 ms",
    "In flight",
    "Busy %",
    "Verdict",
];
pub fn device_row(d: &crate::domain::Device) -> Vec<String> {
    let n = |v: Option<f64>| v.map(|v| format!("{v:.2}")).unwrap_or("—".into());
    vec![
        d.name.clone(),
        d.fields
            .iter()
            .find(|(k, _)| k == "scheduler")
            .map(|(_, v)| v.clone())
            .unwrap_or("—".into()),
        n(d.iops),
        n(d.read_mib_s),
        n(d.write_mib_s),
        n(d.await_ms),
        n(d.p99_ms),
        d.inflight.to_string(),
        n(d.busy_pct),
        if d.p99_ms.unwrap_or(0.) > 10. {
            "latency"
        } else if d.inflight > 0 {
            "in flight"
        } else {
            "—"
        }
        .into(),
    ]
}

pub const CGROUP_COLUMNS: [&str; 11] = [
    "Cgroup",
    "Tasks",
    "Runtime %",
    "Quota / period µs",
    "Throttle ms/s",
    "Memory MiB",
    "Limit MiB",
    "CPU PSI",
    "IO PSI",
    "CPUs",
    "Verdict",
];
pub fn cgroup_row(g: &crate::domain::Cgroup) -> Vec<String> {
    let field = |key: &str| {
        g.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or("—".into())
    };
    let psi = |key: &str| {
        field(key)
            .split_whitespace()
            .find_map(|v| v.strip_prefix("avg10=").map(str::to_owned))
            .unwrap_or("—".into())
    };
    let num = |v: Option<f64>| v.map(|v| format!("{v:.1}")).unwrap_or("—".into());
    vec![
        g.path.clone(),
        field("pids.current"),
        num(g.runtime_pct),
        g.quota.clone(),
        num(g.throttled_ms_s),
        g.memory_bytes
            .map(|v| format!("{:.0}", v as f64 / 1048576.))
            .unwrap_or("—".into()),
        field("memory.max")
            .parse::<u64>()
            .map(|n| format!("{:.0}", n as f64 / 1048576.))
            .unwrap_or_else(|_| field("memory.max")),
        psi("cpu.pressure"),
        psi("io.pressure"),
        g.cpus.clone(),
        if g.throttled_ms_s.is_some_and(|v| v > 50.) {
            "throttled"
        } else if g.runtime_pct.is_none() {
            "warming"
        } else {
            "observed"
        }
        .into(),
    ]
}

pub fn demo_bpf_rows(t: &mut crate::domain::Telemetry) -> Vec<Vec<String>> {
    [
        (
            "101",
            "kw_sched",
            "RawTracePoint",
            "33000",
            "210",
            "0.693",
            "kernwatch",
            "sched_switch",
        ),
        (
            "102",
            "kw_irq",
            "RawTracePoint",
            "33000",
            "210",
            "0.693",
            "kernwatch",
            "softirq_entry",
        ),
        (
            "103",
            "kw_block",
            "RawTracePoint",
            "34000",
            "210",
            "0.714",
            "kernwatch",
            "block_rq_issue",
        ),
        (
            "201",
            "net_ingress",
            "SchedClassifier",
            "74000",
            "250",
            "1.850",
            "network agent",
            "tc ingress",
        ),
        (
            "202",
            "net_egress",
            "SchedClassifier",
            "74000",
            "250",
            "1.850",
            "network agent",
            "tc egress",
        ),
        (
            "301",
            "endpoint_open",
            "Lsm",
            "9000",
            "2400",
            "2.160",
            "endpoint agent",
            "file_open",
        ),
        (
            "302",
            "endpoint_exec",
            "Lsm",
            "9000",
            "2400",
            "2.160",
            "endpoint agent",
            "bprm_check",
        ),
    ]
    .iter()
    .map(|(id, name, kind, runs, ns, cpu, owner, attach)| {
        let mut detail = t
            .details
            .get(&format!("bpf:{owner}"))
            .cloned()
            .unwrap_or_default();
        detail.retain(|(k, _)| {
            ![
                "attach",
                "runs / second",
                "mean runtime ns",
                "CPU / one core",
            ]
            .contains(&k.as_str())
        });
        detail.insert(0, ("program".into(), format!("{id} {name}")));
        detail.extend([
            ("attach".into(), attach.to_string()),
            ("runs / second".into(), runs.to_string()),
            ("mean runtime ns".into(), ns.to_string()),
            ("CPU / one core".into(), format!("{cpu}%")),
        ]);
        t.details.insert(format!("bpf:{id}"), detail);
        row(&[id, name, kind, runs, ns, cpu, "6", owner, attach])
    })
    .collect()
}
