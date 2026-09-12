//! One incident clock and entity graph for all thirteen views.
use crate::domain::*;
const NOW: u64 = 600_000;
fn fields(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}
/// A synthetic profile for the demo: the Envoy worker's stacks during the
/// scheduling incident. Weights are made up, like the rest of the fixture, and
/// the tab marks itself as demo data.
pub fn profile() -> crate::flame::Profile {
    let mut p = crate::flame::Profile::new("demo fixture · synthetic stacks");
    for (stack, samples) in [
        (
            "envoy_main;worker_loop;epoll_wait;__x64_sys_epoll_wait;schedule",
            480,
        ),
        ("envoy_main;worker_loop;on_readable;http_parse;memchr", 260),
        (
            "envoy_main;worker_loop;on_readable;http_parse;header_map_insert;hash_bytes",
            140,
        ),
        (
            "envoy_main;worker_loop;on_readable;ssl_read;aes_gcm_decrypt;aesni_ctr32",
            220,
        ),
        (
            "envoy_main;worker_loop;on_writable;ssl_write;aes_gcm_encrypt;aesni_ctr32",
            180,
        ),
        (
            "envoy_main;worker_loop;upstream_connect;__x64_sys_connect;tcp_v4_connect",
            90,
        ),
        ("envoy_main;worker_loop;stats_flush;histogram_merge", 40),
        (
            "envoy_main;config_update;xds_decode;protobuf_parse;arena_alloc",
            30,
        ),
        ("envoy_main;worker_loop;on_readable;0x7f2c4a118b30", 24),
        ("envoy_main;worker_loop;timer_expire", 12),
        ("envoy_main;worker_loop;conn_close", 6),
        ("envoy_main;worker_loop;dns_resolve", 3),
        ("envoy_main;worker_loop;log_flush", 2),
        ("envoy_main;worker_loop;admin_handler", 1),
    ] {
        p.add(
            &stack.split(';').map(str::to_owned).collect::<Vec<_>>(),
            samples,
        );
    }
    // Stacks whose frame pointer chain ended immediately: the panel reports
    // the share rather than pretending the program is one call deep.
    p.add(&["envoy_main".to_string()], 96);
    p.sort();
    p
}
pub fn incident() -> Telemetry {
    let mut t = Telemetry {
        boot_id: "demo-boot".into(),
        hostname: "kw-demo".into(),
        kernel: "6.12.9".into(),
        at_ms: NOW,
        profile: profile(),
        ..Default::default()
    };
    for (id, busy) in [41., 37., 44., 98., 29., 33., 40., 31.]
        .into_iter()
        .enumerate()
    {
        let (softirq, kernel, user) = if id == 3 {
            (91., 0., 7.)
        } else {
            (1., 6., busy - 7.)
        };
        t.cpus.push(Cpu {
            wake_p50_ms: Some(if id == 3 { 4.2 } else { 0.2 }),
            id: id as u32,
            busy,
            user,
            kernel,
            softirq,
            irq: 0.,
            steal: 0.,
            runnable: Some(if id == 3 { 7 } else { 1 }),
            wake_p99_ms: Some(if id == 3 { 18. } else { 0.5 }),
            switches_s: Some(if id == 3 { 12400. } else { 3200. }),
            migrations_s: Some(if id == 3 { 0. } else { 28. }),
        });
    }
    for (pid, name, state, cpu, pct, rss, p50, p99, ctx, verdict, cgroup, wchan) in [
        (
            34,
            "ksoftirqd/3",
            "R",
            3,
            91.,
            0,
            0.,
            0.,
            12400.,
            "softirq storm",
            "/",
            "—",
        ),
        (
            3312,
            "kworker/3:1",
            "D",
            3,
            0.,
            0,
            0.,
            0.,
            2.,
            "blocked 4.1s",
            "/",
            "xfs_log_force",
        ),
        (
            1188,
            "envoy",
            "S",
            3,
            4.,
            412,
            2.1,
            18.,
            8100.,
            "starved on cpu3",
            "/system.slice/envoy.service",
            "ep_poll",
        ),
        (
            1191,
            "envoy wrk:1",
            "S",
            3,
            3.,
            0,
            1.9,
            16.,
            7700.,
            "starved on cpu3",
            "/system.slice/envoy.service",
            "ep_poll",
        ),
        (
            902,
            "postgres",
            "S",
            1,
            20.,
            1946,
            0.1,
            0.6,
            3200.,
            "ok",
            "/system.slice/postgresql.service",
            "ep_poll",
        ),
        (
            2210,
            "node",
            "R",
            5,
            32.,
            1126,
            0.1,
            0.3,
            410.,
            "stat storm",
            "/kubepods.slice/web-7f9c",
            "—",
        ),
        (
            911,
            "postgres wal",
            "D",
            1,
            0.,
            0,
            0.,
            0.,
            8.,
            "fsync wait",
            "/system.slice/postgresql.service",
            "io_schedule",
        ),
        (
            640,
            "containerd",
            "S",
            7,
            4.,
            310,
            0.1,
            0.4,
            800.,
            "ok",
            "/system.slice/containerd.service",
            "futex_wait",
        ),
        (
            5012,
            "endpointd",
            "S",
            6,
            3.,
            180,
            0.1,
            0.5,
            100.,
            "new module",
            "/system.slice/endpoint.service",
            "ep_poll",
        ),
        (
            16,
            "rcu_preempt",
            "I",
            0,
            0.4,
            0,
            0.1,
            0.9,
            1400.,
            "ok",
            "/",
            "rcu_gp_fqs",
        ),
    ] {
        t.tasks.push(Task {
            nice: Some(0),
            age_ms: Some(NOW.saturating_sub((100 + pid as u64 * 2) * 10)),
            anon_bytes: Some(rss * 1048576 * 80 / 100),
            file_bytes: Some(rss * 1048576 * 15 / 100),
            shmem_bytes: Some(rss * 1048576 - rss * 1048576 * 80 / 100 - rss * 1048576 * 15 / 100),
            swap_bytes: Some(0),
            minor_faults_s: Some(if pid == 2210 { 1200. } else { 20. }),
            rss_growth_bytes_s: Some(0.),
            pss_at_ms: Some(NOW - 2000),
            uid: Some(if rss == 0 { 0 } else { 1000 }),
            parent_pid: if pid == 1191 { 1188 } else { 1 },
            tgid: if pid == 1191 { 1188 } else { pid },
            kernel_thread: rss == 0,
            pid,
            start_ticks: 100 + pid as u64 * 2,
            name: name.into(),
            state: state.into(),
            cpu,
            cpu_pct: Some(pct),
            rss_bytes: rss * 1048576,
            pss_bytes: if rss > 0 { Some(rss * 1000000) } else { None },
            cgroup: cgroup.into(),
            affinity: if cpu == 3 { "3" } else { "0-7" }.into(),
            policy: "SCHED_OTHER".into(),
            wchan: wchan.into(),
            wake_p50_ms: if p50 > 0. { Some(p50) } else { None },
            wake_p99_ms: if p99 > 0. { Some(p99) } else { None },
            blocked_ms: if state == "D" { Some(4100.) } else { None },
            voluntary_s: Some(ctx),
            involuntary_s: Some(if pid == 1188 { 940. } else { 0. }),
            verdict: verdict.into(),
        });
    }
    for (key, value, unit) in [
        ("sched.p99", 18., "ms"),
        ("runqueue", 1.4, "/cpu"),
        ("softirq", 91., "%"),
        ("psi.cpu", 12., "%"),
        ("psi.memory", 0.8, "%"),
        ("psi.io", 3.1, "%"),
        ("fault.major", 0., "/s"),
        ("fault.minor", 2100., "/s"),
        ("memory.used", 19.6, "GiB"),
        ("memory.available", 11.2, "GiB"),
        ("memory.cache", 7.4, "GiB"),
        ("memory.slab", 2.9, "GiB"),
        ("memory.total", 32., "GiB"),
        ("slab.growth", 310., "MiB/h"),
        ("disk.read", 48., "MiB/s"),
        ("disk.write", 31., "MiB/s"),
        ("disk.p99", 1.9, "ms"),
        ("disk.age", 4100., "ms"),
        ("disk.iops", 2400., "/s"),
        ("disk.queue", 3., ""),
        ("irq.rate", 148000., "/s"),
        ("bpf.own", 2.1, "%"),
        ("bpf.total", 10.12, "%"),
        ("alloc", 2100., "/s"),
        ("reclaim", 0., "/s"),
        ("dstate", 2., "tasks"),
        ("quota", 50., "%"),
        ("cgroup.cpu", 7., "%"),
        ("throttled", 0., "ms"),
    ] {
        t.metrics.insert(
            key.into(),
            Measurement::known(value, unit, "demo fixture / 10s", NOW),
        );
    }
    let cpu_user = t.cpus.iter().map(|c| c.user).sum::<f64>() / 8.;
    let cpu_kernel = t
        .cpus
        .iter()
        .map(|c| c.kernel + c.softirq + c.irq)
        .sum::<f64>()
        / 8.;
    for (key, base, value, max, onset) in [
        ("cpu.user", cpu_user - 2., cpu_user, 100., 360),
        ("cpu.kernel", 6., cpu_kernel, 100., 378),
        ("sched.p99", 0.4, 18., 20., 522),
        ("softirq", 1., 91., 100., 378),
        ("memory.used", 19.5, 19.6, 32., 0),
        ("disk.read", 42., 48., 120., 0),
        ("disk.write", 20., 31., 120., 0),
        ("disk.p99", 0.5, 1.9, 5., 575),
        ("disk.iops", 1800., 2400., 3000., 0),
        ("disk.queue", 1., 3., 8., 580),
        ("dstate", 0., 2., 4., 596),
        ("irq.rate", 8000., 148000., 160000., 378),
        ("alloc", 1800., 2100., 3000., 0),
        ("reclaim", 0., 0., 1000., 0),
        ("quota", 50., 50., 100., 0),
        ("cgroup.cpu", 4., 7., 100., 522),
        ("throttled", 0., 0., 100., 0),
        ("bpf.own", 2.1, 2.1, 5., 0),
    ] {
        let samples = (0..=600)
            .map(|i| {
                let v = if i < onset { base } else { value };
                let jitter =
                    if v == 0. || key == "quota" || key == "dstate" || key.starts_with("bpf.") {
                        0.
                    } else {
                        v * (((i * 17 + 13) % 19) as f64 - 9.) / 180.
                    };
                Sample {
                    at_ms: i * 1000,
                    value: Some(if i == 600 {
                        value
                    } else {
                        (v + jitter).max(0.)
                    }),
                }
            })
            .collect();
        t.series.insert(
            key.into(),
            Series {
                name: key.into(),
                unit: t
                    .metrics
                    .get(key)
                    .map(|m| m.unit.clone())
                    .unwrap_or("%".into()),
                max,
                samples,
            },
        );
    }
    for c in &t.cpus {
        let samples = (0..=600)
            .map(|i| Sample {
                at_ms: i * 1000,
                value: Some(if i == 600 {
                    c.busy
                } else if c.id == 3 && i < 378 {
                    35.
                } else {
                    (c.busy + (i % 7) as f64 - 3.).clamp(0., 100.)
                }),
            })
            .collect();
        t.series.insert(
            format!("cpu.{}", c.id),
            Series {
                name: format!("cpu{}", c.id),
                unit: "%".into(),
                max: 100.,
                samples,
            },
        );
    }
    for task in &t.tasks {
        let current = task.wake_p99_ms.unwrap_or(0.);
        let samples = (0..=600)
            .map(|i| Sample {
                at_ms: i * 1000,
                value: task
                    .wake_p99_ms
                    .map(|_| if i < 522 { 0.4 } else { current }),
            })
            .collect();
        t.series.insert(
            format!("task.{}", task.pid),
            Series {
                name: task.name.clone(),
                unit: "ms".into(),
                max: 20.,
                samples,
            },
        );
    }
    for (at, src, msg, subject) in [
        (
            378000,
            "affinity monitor",
            "IRQ 47 affinity changed 0-7 → 3",
            "irq:47",
        ),
        (
            522000,
            "sched trace",
            "Envoy wakeup p99 0.4 → 18ms on CPU3",
            "task:1188",
        ),
        (
            596000,
            "block trace",
            "FLUSH remains in flight; queue IRQ shares CPU3",
            "device:nvme0n1",
        ),
        (
            600000,
            "kernwatch",
            "Incident window frozen; acquisition continues",
            "host",
        ),
    ] {
        t.events.push(Event {
            id: format!("event-{at}"),
            at_ms: at,
            source: src.into(),
            severity: if at == 600000 { "info" } else { "warn" }.into(),
            message: msg.into(),
            subject: subject.into(),
        });
    }
    t.histograms.insert(
        "sched".into(),
        Histogram {
            bounds: vec![0.1, 0.5, 1., 2., 4., 8., 16., 32.],
            counts: vec![3, 8, 12, 18, 55, 128, 220, 84],
            unit: "ms".into(),
        },
    );
    t.histograms.insert(
        "syscall".into(),
        Histogram {
            bounds: vec![0.001, 0.004, 0.016, 0.064, 0.256, 1., 4., 16., 64.],
            counts: vec![4, 25, 130, 44, 9, 15, 45, 30, 3],
            unit: "ms".into(),
        },
    );
    t.details.insert(
        "slab".into(),
        fields(&[
            ("dentry", "1.4 GiB"),
            ("xfs_inode", "612 MiB"),
            ("kmalloc-4k", "221 MiB"),
            ("buffer_head", "158 MiB"),
            ("radix_tree_node", "130 MiB"),
            ("other 212", "369 MiB"),
        ]),
    );
    t.details.insert(
        "numa".into(),
        fields(&[
            ("node 0", "CPU 0–3 · 16 GiB · used 10.8 GiB (68%)"),
            ("node 1", "CPU 4–7 · 16 GiB · used 8.8 GiB (55%)"),
            ("local / foreign", "94% / 6% · sampling window 10s"),
            ("migrations", "212 pages/s · 0 task migrations"),
            ("hugepages", "THP 1.2 GiB · 8 splits/s · madvise"),
            ("ksm", "off"),
            ("zone normal", "free 4.1 GiB · low 128 MiB · ok"),
            ("zone dma32", "free 1.9 GiB · low 12 MiB · ok"),
        ]),
    );
    t.details.insert(
        "device".into(),
        fields(&[
            ("model", "Demo NVMe controller · fw 1.0"),
            ("mount", "xfs / · 1.8 TiB"),
            ("queues", "8 hardware · depth 1023"),
            ("in flight", "3 requests · one FLUSH aged 4.1s"),
            ("write cache", "write back · FUA on"),
            ("telemetry", "41°C · spare 100% · 0 media errors"),
            ("queue 3", "IRQ 52 · effective CPU3"),
            (
                "evidence",
                "Shared placement observed; completion-path cause unverified",
            ),
        ]),
    );
    t.details.insert(
        "module".into(),
        fields(&[
            ("path", "/opt/example/example_probe.ko"),
            ("vermagic", "6.12.9 SMP preempt mod_unload"),
            ("srcversion", "3A1F…9C2"),
            ("loaded", "08:41:07 · loader observed: endpointd 5012"),
            ("params", "none"),
            ("symbols", "214 imports · 0 exports"),
            ("hooks", "file_open · bprm_check · socket_connect (fixture)"),
            (
                "runtime",
                "Attribution unavailable; module trust does not establish performance impact",
            ),
        ]),
    );
    t.details.insert(
        "bpf".into(),
        fields(&[
            ("loaded", "08:41:07 · endpointd 5012"),
            (
                "code",
                "18.4 KiB JIT · 41 KiB translated · 4,120 instructions",
            ),
            ("verifier", "passed · log captured at load time (fixture)"),
            ("maps", "6 · hash ×2 · lru_hash · ringbuf · array ×2"),
            ("ring buffer", "8 MiB · occupancy 61% · drops 0"),
            ("mean runtime", "2.4µs · 18k runs/s = 4.32% of one CPU"),
            ("owner", "endpointd · synthetic incident"),
        ]),
    );
    for name in ["scheduler", "irq", "block", "syscalls", "bpf", "logs"] {
        t.capabilities.insert(name.into(), Quality::Available);
    }
    let evidence = vec![
        Evidence {
            label: "IRQ 47 effective affinity narrowed to CPU3".into(),
            source: "affinity monitor".into(),
            at_ms: 378000,
            subject: "irq:47".into(),
            weight: 0.4,
            observed: true,
        },
        Evidence {
            label: "NET_RX execution 91%; other CPUs < 3%".into(),
            source: "softirq entry/exit".into(),
            at_ms: 600000,
            subject: "cpu:3".into(),
            weight: 0.3,
            observed: true,
        },
        Evidence {
            label: "Envoy effective task affinity CPU3; wakeup p99 18ms".into(),
            source: "sched trace + task affinity".into(),
            at_ms: 522000,
            subject: "task:1188".into(),
            weight: 0.2,
            observed: true,
        },
        Evidence {
            label: "NVMe queue IRQ also on CPU3; causal trace still needed".into(),
            source: "queue topology".into(),
            at_ms: 600000,
            subject: "device:nvme0n1".into(),
            weight: 0.1,
            observed: false,
        },
    ];
    t.issues.push(Issue{id:"sched-cpu3".into(),title:"Scheduler latency".into(),subject:"CPU3 · Envoy workers".into(),severity:"warn".into(),since_ms:522000,state:"open".into(),evidence,causes:vec![Cause{label:"IRQ placement".into(),detail:"47 → CPU3 · 09:09:40".into(),observed:true},Cause{label:"NET_RX 91%".into(),detail:"single RX queue · CPU3".into(),observed:true},Cause{label:"Envoy pinned".into(),detail:"wakeup p99 18ms".into(),observed:true},Cause{label:"Storage coupling?".into(),detail:"shared IRQ · unverified".into(),observed:false}],steps:vec!["Spread RX queues · inspect driver support and queue mapping".into(),"Move IRQ 47 · preview affinity and irqbalance interaction".into(),"Widen Envoy affinity · compare wakeup latency under the same load".into()],verification:"Compare 60s windows under equivalent load; scheduler p99 < 1ms. Storage requires independent completion evidence.".into()});
    for (id, title, subject, since) in [
        ("block-age", "Outstanding flush", "nvme0n1 · 4.1s", 600000),
        (
            "slab-growth",
            "Slab growth",
            "dentry +310 MiB/h · source unconfirmed",
            0,
        ),
    ] {
        t.issues.push(Issue {
            id: id.into(),
            title: title.into(),
            subject: subject.into(),
            severity: "warn".into(),
            since_ms: since,
            state: "open".into(),
            evidence: vec![Evidence {
                label: subject.into(),
                source: "incident observation".into(),
                at_ms: NOW,
                subject: subject.into(),
                weight: 1.,
                observed: true,
            }],
            causes: vec![Cause {
                label: "Cause unconfirmed".into(),
                detail: "Collect discriminating evidence".into(),
                observed: false,
            }],
            steps: vec!["Capture focused trace; compare baseline and current window".into()],
            verification: "Require matching trace evidence before attributing cause.".into(),
        });
    }
    for (key, value, max) in [
        ("runqueue", 1.4, 4.),
        ("psi.cpu", 12., 100.),
        ("fault.major", 0., 10.),
        ("psi.memory", 0.8, 100.),
        ("memory.cache", 7.4, 32.),
        ("memory.slab", 2.9, 32.),
        ("memory.available", 11.2, 32.),
        ("disk.age", 4100., 5000.),
        ("slab.growth", 310., 500.),
        ("fault.minor", 2100., 4000.),
    ] {
        t.series.entry(key.into()).or_insert_with(|| Series {
            name: key.into(),
            unit: String::new(),
            max,
            samples: (0..=600)
                .map(|i| Sample {
                    at_ms: i * 1000,
                    value: Some(value),
                })
                .collect(),
        });
    }
    if let Some(s) = t.series.get_mut("cpu.kernel") {
        s.max = 40.;
    }

    for (i, name) in ["nvme0n1", "nvme1n1"].iter().enumerate() {
        let mut d = Device {
            name: (*name).into(),
            major_minor: format!("259:{i}"),
            read_mib_s: Some(if i == 0 { 48. } else { 12. }),
            write_mib_s: Some(if i == 0 { 31. } else { 4. }),
            inflight: if i == 0 { 3 } else { 0 },
            iops: Some(if i == 0 { 2400. } else { 600. }),
            await_ms: Some(0.4),
            p99_ms: Some(if i == 0 { 1.9 } else { 0.3 }),
            busy_pct: Some(41.),
            ..Default::default()
        };
        d.fields = if i == 0 {
            t.details.get("device").cloned().unwrap_or_default()
        } else {
            vec![
                ("model".into(), "Demo secondary NVMe controller".into()),
                ("in flight".into(), "0 requests".into()),
                ("mount".into(), "/data · ext4 · fixture".into()),
            ]
        };
        d.fields
            .insert(0, ("device".into(), format!("/dev/{name}")));
        if i == 0 {
            d.fields
                .push(("controller IRQ candidates".into(), "52".into()));
            d.fields.push(("queue 3 CPUs".into(), "3".into()));
        }
        t.details.insert(format!("device:{name}"), d.fields.clone());
        d.fields.push(("scheduler".into(), "none".into()));
        if let Some(mut series) = t.series.get("disk.p99").cloned() {
            series.name = format!("block.dev{}.p99", d.major_minor);
            if i == 1 {
                for sample in &mut series.samples {
                    sample.value = Some(0.3);
                }
            }
            t.series.insert(series.name.clone(), series);
        }
        t.devices.push(d);
        for suffix in ["read", "write", "iops", "queue"] {
            if let Some(mut series) = t.series.get(&format!("disk.{suffix}")).cloned() {
                series.name = format!("device:{name}:{suffix}");
                let device = t.devices.last().unwrap();
                let target = match suffix {
                    "read" => device.read_mib_s,
                    "write" => device.write_mib_s,
                    "iops" => device.iops,
                    _ => Some(device.inflight as f64),
                }
                .unwrap_or(0.);
                let original = series.at(NOW).unwrap_or(1.).max(0.001);
                for sample in &mut series.samples {
                    sample.value = sample.value.map(|v| v * target / original);
                }
                series.max = (series.max * target / original).max(1.);
                t.series.insert(series.name.clone(), series);
            }
        }
    }
    for (path, quota, cpus, _runtime, memory) in [
        ("/", "max 100000", "0-7", 120., (19.6 * 1073741824.) as u64),
        ("/system.slice", "max 100000", "0-7", 98., 8_000_000_000),
        (
            "/system.slice/envoy.service",
            "50000 100000",
            "3",
            7.,
            432_013_312,
        ),
    ] {
        let runtime = t
            .tasks
            .iter()
            .filter(|x| {
                path == "/" || x.cgroup == path || x.cgroup.starts_with(&format!("{path}/"))
            })
            .filter_map(|x| x.cpu_pct)
            .sum::<f64>();
        let group = Cgroup {
            inode: 100 + t.cgroups.len() as u64,
            path: path.into(),
            quota: quota.into(),
            cpus: cpus.into(),
            runtime_pct: Some(runtime),
            throttled_ms_s: Some(0.),
            memory_bytes: Some(memory),
            fields: vec![
                ("path".into(), path.into()),
                ("cpu.max".into(), quota.into()),
                ("cpuset effective".into(), cpus.into()),
                ("runtime / one CPU".into(), format!("{runtime}%")),
                (
                    "pids.current".into(),
                    if path.ends_with("envoy.service") {
                        "2"
                    } else {
                        "10"
                    }
                    .into(),
                ),
                (
                    "memory.max".into(),
                    if path.ends_with("envoy.service") {
                        "1073741824"
                    } else {
                        "max"
                    }
                    .into(),
                ),
                (
                    "cpu.pressure".into(),
                    "some avg10=2.4 avg60=1.0 avg300=0.2".into(),
                ),
                (
                    "io.pressure".into(),
                    "some avg10=0.1 avg60=0.1 avg300=0.0".into(),
                ),
                ("throttled time".into(), "0 ms/s".into()),
                (
                    "limits".into(),
                    "Inspect ancestors and task affinity independently".into(),
                ),
            ],
        };
        t.details
            .insert(format!("cgroup:{path}"), group.fields.clone());
        if let Some(mut series) = t.series.get("sched.p99").cloned() {
            series.name = format!("sched.cgroup{}", group.inode);
            if !path.ends_with("envoy.service") {
                for sample in &mut series.samples {
                    sample.value = Some(0.5);
                }
            }
            t.series.insert(series.name.clone(), series);
        }
        for (kind, value) in [("cpu", 2.4), ("io", 0.1)] {
            let key = format!("cgroup:{path}:psi.{kind}");
            t.series.insert(
                key.clone(),
                Series {
                    name: key,
                    unit: "% some avg10".into(),
                    max: 100.,
                    samples: (1..=600)
                        .map(|i| Sample {
                            at_ms: i * 1000,
                            value: Some(value),
                        })
                        .collect(),
                },
            );
        }
        t.cgroups.push(group);
        for (suffix, key) in [("cpu", "cgroup.cpu"), ("throttled", "throttled")] {
            if let Some(mut series) = t.series.get(key).cloned() {
                series.name = format!("cgroup:{path}:{suffix}");
                if suffix == "cpu" {
                    for sample in &mut series.samples {
                        sample.value = sample.value.map(|v| v * runtime / 7.);
                    }
                    series.max = series.max.max(runtime * 1.2);
                }
                t.series.insert(series.name.clone(), series);
            }
        }
    }
    for (name, taint, kib, refs, used, loaded, hooks) in [
        (
            "example_probe",
            "OE",
            1228,
            1,
            "—",
            "09:13:02 observed",
            "lsm · kprobe ×12",
        ),
        ("gpu_vendor", "O", 63488, 41, "gpu_uvm gpu_drm", "boot", "—"),
        ("xfs", "", 2150, 2, "—", "boot", "—"),
        ("nvme", "", 61, 4, "—", "boot", "—"),
        ("mlx5_core", "", 2458, 1, "mlx5_ib", "boot", "—"),
        ("wireguard", "", 118, 0, "—", "boot", "—"),
        ("bpf_preload", "", 16, 0, "—", "boot", "—"),
    ] {
        let module = Module {
            name: name.into(),
            bytes: kib * 1024,
            refs,
            state: "Live".into(),
            taint: taint.into(),
            fields: vec![
                ("name".into(), name.into()),
                (
                    "path".into(),
                    format!("/lib/modules/6.12.9/extra/{name}.ko"),
                ),
                ("vermagic".into(), "6.12.9 SMP preempt mod_unload".into()),
                ("source version".into(), "3A1F…9C2 · synthetic".into()),
                ("loaded".into(), loaded.into()),
                (
                    "loader".into(),
                    if name == "example_probe" {
                        "endpointd PID 5012 · fixture"
                    } else {
                        "boot / module manager · fixture"
                    }
                    .into(),
                ),
                ("dependent modules".into(), used.into()),
                ("hooks".into(), hooks.into()),
                ("parameters".into(), "none in fixture".into()),
                ("taint".into(), taint.into()),
                (
                    "baseline".into(),
                    if loaded == "boot" { "accepted" } else { "new" }.into(),
                ),
                (
                    "interpretation".into(),
                    "Trust evidence only; performance attribution needs a measured execution path"
                        .into(),
                ),
                (
                    "provenance".into(),
                    "Synthetic fixture metadata; not a report about installed software".into(),
                ),
            ],
        };
        t.details
            .insert(format!("module:{name}"), module.fields.clone());
        t.modules.push(module);
    }
    for (name, runs, ns, cpu, attach) in [
        (
            "kernwatch",
            "100000",
            "210",
            "2.10",
            "scheduler / IRQ / block tracepoints",
        ),
        (
            "network agent",
            "148000",
            "250",
            "3.70",
            "tc ingress / egress",
        ),
        (
            "endpoint agent",
            "18000",
            "2400",
            "4.32",
            "LSM file / process hooks",
        ),
    ] {
        t.details.insert(
            format!("bpf:{name}"),
            vec![
                ("owner".into(), format!("{name} · fixture")),
                ("attach".into(), attach.into()),
                ("runs / second".into(), runs.into()),
                ("mean runtime ns".into(), ns.into()),
                ("CPU / one core".into(), format!("{cpu}%")),
                (
                    "maps".into(),
                    "hash ×2 · lru_hash · ringbuf · array ×2".into(),
                ),
                (
                    "code".into(),
                    "18.4 KiB JIT · 41 KiB translated · fixture".into(),
                ),
                (
                    "capture quality".into(),
                    "no reported drops in fixture; occupancy not inferred from capacity".into(),
                ),
            ],
        );
    }
    t.details.insert(
        "probe.scope".into(),
        vec![
            (
                "backend".into(),
                "fixture / simulated raw tracepoint capture".into(),
            ),
            (
                "scope".into(),
                "all · retained events are a selected sample".into(),
            ),
            (
                "buffer".into(),
                "16 MiB capacity; occupancy not measured".into(),
            ),
        ],
    );
    crate::model::demo_bpf_rows(&mut t);
    for (name, pid, duration, ret) in [
        ("newfstatat", 2210, 0.002, -2),
        ("epoll_wait", 1188, 22., 1),
        ("fsync", 902, 1.9, 0),
        ("read", 902, 0.004, 4812),
        ("write", 902, 0.006, 256),
        ("futex", 2210, 0.012, -11),
    ] {
        for i in 0..8 {
            t.events.push(Event { id:format!("syscall-{name}-{i}"),at_ms:t.at_ms-5000+i*100,source:"syscall trace · fixture".into(),severity:if ret < 0 { "error" } else { "info" }.into(),subject:format!("task:{pid}"),message:format!("{name} PID={pid} args=[fixture sample {i}] ret={ret} duration_ms={duration}; {}",if ret == -2 { "ENOENT" } else if ret == -11 { "EAGAIN" } else { "completed" }) });
        }
    }
    for (name, p99) in [
        ("newfstatat", 0.009),
        ("epoll_wait", 22.),
        ("fsync", 1.9),
        ("read", 0.031),
        ("write", 0.048),
        ("futex", 0.4),
    ] {
        let bounds = vec![p99 / 16., p99 / 4., p99 / 2., p99, p99 * 2.];
        t.histograms.insert(
            format!("syscall:{name}"),
            Histogram {
                bounds,
                counts: if name == "fsync" {
                    vec![80, 320, 1200, 836, 24]
                } else {
                    vec![128, 512, 2048, 1368, 40]
                },
                unit: "ms".into(),
            },
        );
        let mut series = Series {
            name: format!("syscall.{name}.p99"),
            unit: "ms".into(),
            max: p99 * 1.2,
            samples: Vec::new(),
        };
        for i in 1..=600 {
            series.push(i * 1000, Some(p99 * if i < 522 { 0.3 } else { 1. }));
        }
        t.series.insert(series.name.clone(), series);
    }
    for (id, pct) in [
        (101, 0.693),
        (102, 0.693),
        (103, 0.714),
        (201, 1.85),
        (202, 1.85),
        (301, 2.16),
        (302, 2.16),
    ] {
        let mut series = Series {
            name: format!("bpf.program{id}"),
            unit: "% / one CPU".into(),
            max: 5.,
            samples: Vec::new(),
        };
        for i in 1..=600 {
            series.push(i * 1000, Some(pct));
        }
        t.series.insert(series.name.clone(), series);
    }
    t.events.push(Event {
        id: "module-fixture-load".into(),
        at_ms: 580000,
        source: "module poll · fixture".into(),
        severity: "warn".into(),
        subject: "module:example_probe".into(),
        message:
            "example_probe appeared; out-of-tree and unsigned; performance causality unconfirmed"
                .into(),
    });
    let events = t.events.clone();
    for i in 1..=600 {
        let at = i * 1000;
        let count = events
            .iter()
            .filter(|e| e.at_ms > at - 1000 && e.at_ms <= at)
            .count();
        t.series
            .entry("events.rate".into())
            .or_insert_with(|| Series {
                name: "events.rate".into(),
                unit: "events/s".into(),
                max: 4.,
                samples: Vec::new(),
            })
            .push(at, Some(count as f64));
    }
    t.details.insert(
        "module baseline".into(),
        vec![
            ("accepted capture ms".into(), "0 · fixture baseline".into()),
            (
                "intent".into(),
                "Accepted initial inventory; taint remains historical".into(),
            ),
        ],
    );
    t.details.insert(
        "taint".into(),
        vec![
            ("taint".into(), "O E · synthetic fixture".into()),
            (
                "interpretation".into(),
                "Trust flags do not prove a performance cause".into(),
            ),
        ],
    );

    if let Some(h) = t.histograms.get("sched").cloned() {
        for (key, p99) in t
            .cpus
            .iter()
            .map(|x| (format!("sched.cpu{}", x.id), x.wake_p99_ms))
            .chain(
                t.tasks
                    .iter()
                    .map(|x| (format!("task.{}", x.pid), x.wake_p99_ms)),
            )
            .chain(t.cgroups.iter().map(|x| {
                (
                    format!("sched.cgroup{}", x.inode),
                    Some(if x.path.ends_with("envoy.service") {
                        18.
                    } else {
                        0.5
                    }),
                )
            }))
        {
            if let Some(p99) = p99 {
                let mut scaled = h.clone();
                for b in &mut scaled.bounds {
                    *b *= p99 / 18.;
                }
                t.histograms.insert(key, scaled);
            }
        }
    }
    for cpu in &t.cpus {
        if let Some(mut series) = t.series.get("sched.p99").cloned() {
            series.name = format!("sched.cpu{}", cpu.id);
            if cpu.id != 3 {
                for sample in &mut series.samples {
                    sample.value = Some(0.5);
                }
            }
            t.series.insert(series.name.clone(), series);
        }
    }

    for metric in t.metrics.values_mut() {
        metric.start_ms = metric.end_ms.saturating_sub(10000);
    }

    for (id, rate) in [(47, 148000.), (48, 31000.), (52, 2000.)] {
        if let Some(mut s) = t.series.get("irq.rate").cloned() {
            s.name = format!("irq.{id}.rate");
            for p in &mut s.samples {
                p.value = p.value.map(|v| v * rate / 148000.);
            }
            s.max = rate * 1.2;
            t.series.insert(s.name.clone(), s);
        }
    }
    for c in &t.cpus {
        if let Some(mut s) = t.series.get("softirq").cloned() {
            s.name = format!("softirq.cpu{}", c.id);
            if c.id != 3 {
                for p in &mut s.samples {
                    p.value = Some(c.softirq);
                }
            }
            t.series.insert(s.name.clone(), s);
        }
        let key = format!("sched.migrations.cpu{}", c.id);
        t.series.insert(
            key.clone(),
            Series {
                name: key,
                unit: "/s".into(),
                max: 50.,
                samples: (1..=600)
                    .map(|i| Sample {
                        at_ms: i * 1000,
                        value: c.migrations_s,
                    })
                    .collect(),
            },
        );
    }
    for task in &t.tasks {
        let key = format!("task.runtime{}", task.pid);
        t.series.insert(
            key.clone(),
            Series {
                name: key,
                unit: "% / one CPU".into(),
                max: 100.,
                samples: (1..=600)
                    .map(|i| Sample {
                        at_ms: i * 1000,
                        value: task.cpu_pct,
                    })
                    .collect(),
            },
        );
    }
    if let Some(series) = t.series.get_mut("disk.age") {
        for sample in &mut series.samples {
            sample.value = Some(sample.at_ms.saturating_sub(595900) as f64);
        }
    }
    if let Some(mut series) = t.series.get("softirq").cloned() {
        series.name = "task.runtime34".into();
        t.series.insert(series.name.clone(), series);
    }
    synchronize_current(&mut t);
    t
}

/// Alternative states retain the same identities and incident clock.
pub fn scenario(name: &str) -> Telemetry {
    let mut t = incident();
    match name {
        "pre" => {
            t.at_ms = 300000;
            t.events.retain(|e| e.at_ms <= t.at_ms);
            t.issues.clear();
            t.histograms.clear();
            t.modules.retain(|m| m.name != "example_probe");
            t.details.remove("module:example_probe");
            for series in t.series.values_mut() {
                series.samples.retain(|s| s.at_ms <= t.at_ms);
            }
            for (key, metric) in &mut t.metrics {
                if let Some(series) = t.series.get(key) {
                    metric.value = series.at(t.at_ms);
                }
                metric.end_ms = t.at_ms;
                metric.start_ms = t.at_ms - 10000;
            }
            if let Some(cpu) = t.cpus.iter_mut().find(|c| c.id == 3) {
                cpu.busy = 8.;
                cpu.user = 7.;
                cpu.softirq = 1.;
                cpu.runnable = Some(1);
                cpu.wake_p99_ms = Some(0.5);
                cpu.wake_p50_ms = Some(0.2);
            }
            for task in &mut t.tasks {
                task.wake_p99_ms = Some(0.5);
                task.wake_p50_ms = Some(0.2);
                task.blocked_ms = None;
                if task.state == "D" {
                    task.state = "S".into();
                }
                task.verdict = "ok".into();
                if task.pid == 34 {
                    task.cpu_pct = Some(1.);
                }
            }
        }
        "recovery" => {
            let start = t.at_ms;
            t.at_ms += 61000;
            for (key, series) in &mut t.series {
                let recovered = if key == "sched.p99"
                    || key.starts_with("sched.cpu")
                    || key.starts_with("sched.cgroup")
                    || key
                        .strip_prefix("task.")
                        .is_some_and(|v| v.parse::<u32>().is_ok())
                {
                    Some(0.5)
                } else if key == "task.runtime34" || key == "softirq.cpu3" {
                    Some(15.)
                } else if key == "disk.age" || key == "dstate" {
                    Some(0.)
                } else if key == "cpu.3" {
                    Some(22.)
                } else if key == "cpu.kernel" {
                    Some(8.)
                } else if key == "softirq" {
                    Some(15.)
                } else {
                    series.at(start)
                };
                for i in 1..=61 {
                    series.push(start + i * 1000, recovered);
                }
            }
            for (key, metric) in &mut t.metrics {
                if let Some(series) = t.series.get(key) {
                    metric.value = series.at(t.at_ms);
                }
                metric.end_ms = t.at_ms;
                metric.start_ms = t.at_ms - 60000;
            }
            for cpu in &mut t.cpus {
                cpu.wake_p99_ms = Some(0.5);
                cpu.wake_p50_ms = Some(0.2);
                if cpu.id == 3 {
                    cpu.softirq = 15.;
                    cpu.busy = 22.;
                    cpu.user = 7.;
                    cpu.runnable = Some(1);
                }
            }
            for task in &mut t.tasks {
                task.wake_p99_ms = Some(0.5);
                task.wake_p50_ms = Some(0.2);
                task.blocked_ms = None;
                if task.state == "D" {
                    task.state = "S".into();
                }
                if task.pid == 34 {
                    task.cpu_pct = Some(15.);
                }
                if task.name.starts_with("envoy") {
                    task.affinity = "0-7".into();
                }
                task.verdict = "verify".into();
            }
            t.events.push(Event{id:"demo-experiment".into(),at_ms:start,source:"fixture experiment".into(),severity:"info".into(),message:"Synthetic placement change; evaluate the following 60 seconds before declaring recovery".into(),subject:"cpu:3".into()});
            for issue in &mut t.issues {
                issue.state = "experiment · verification pending".into();
            }
        }
        "quota" => {
            t.issues.retain(|issue| issue.id != "sched-cpu3");
            for task in &mut t.tasks {
                if task.name.starts_with("envoy") {
                    task.cpu = if task.pid == 1188 { 0 } else { 1 };
                    task.cpu_pct = Some(if task.pid == 1188 { 28. } else { 22. });
                    task.affinity = "0-1".into();
                    task.verdict = "quota throttled".into();
                }
            }

            for task in t.tasks.iter().filter(|task| task.name.starts_with("envoy")) {
                if let Some(series) = t.series.get_mut(&format!("task.runtime{}", task.pid)) {
                    for sample in &mut series.samples {
                        if sample.at_ms > 400000 {
                            sample.value = task.cpu_pct;
                        }
                    }
                }
            }
            for group in &mut t.cgroups {
                if group.path.ends_with("envoy.service") {
                    group.runtime_pct = Some(50.);
                    group.cpus = "0-1".into();
                    group.throttled_ms_s = Some(310.);
                    group
                        .fields
                        .retain(|(k, _)| k != "runtime / one CPU" && k != "throttled time");
                    group
                        .fields
                        .push(("runtime / one CPU".into(), "50%".into()));
                    group
                        .fields
                        .push(("throttled time".into(), "310 ms/s".into()));
                    t.details
                        .insert(format!("cgroup:{}", group.path), group.fields.clone());
                }
            }
            for (key, series) in &mut t.series {
                if key == "throttled"
                    || key.ends_with("envoy.service:throttled")
                    || key.ends_with("envoy.service:cpu")
                {
                    for p in &mut series.samples {
                        if p.at_ms > 400000 {
                            p.value = Some(if key.ends_with(":cpu") { 50. } else { 310. });
                        }
                    }
                }
            }
            t.issues.insert(
                0,
                Issue {
                    id: "throttle:/system.slice/envoy.service".into(),
                    title: "Cgroup CPU throttling".into(),
                    subject: "envoy.service · 0.5 CPU quota".into(),
                    since_ms: 400000,
                    state: "open".into(),
                    severity: "warn".into(),
                    evidence: vec![Evidence {
                        label: "Runtime reaches 50%; throttled time 310 ms/s".into(),
                        source: "synthetic cpu.stat deltas".into(),
                        at_ms: t.at_ms,
                        observed: true,
                        ..Default::default()
                    }],
                    causes: vec![Cause {
                        label: "Runtime quota exhausted".into(),
                        detail: "Distinct from runnable wait time".into(),
                        observed: true,
                    }],
                    steps: vec![
                        "Inspect ancestor cpu.max and effective workload".into(),
                        "Preview quota change, then compare equivalent windows".into(),
                    ],
                    verification: "Throttled time below 50 ms/s for 60s under equivalent load"
                        .into(),
                },
            );
        }
        _ => {}
    }
    synchronize_current(&mut t);
    t
}

/// Materialize the same fixture at an earlier cursor without showing future observations.
pub fn at_time(scenario_name: &str, at: u64) -> Telemetry {
    let mut t = scenario(scenario_name);
    let end = t.at_ms;
    if at >= end {
        return t;
    }
    t.at_ms = at.min(end);
    for (key, measurement) in &mut t.metrics {
        if let Some(series) = t.series.get(key) {
            measurement.value = series.at(t.at_ms);
        } else if t.at_ms < measurement.end_ms {
            measurement.value = None;
            measurement.quality = Quality::Warming;
        }
        measurement.end_ms = t.at_ms;
        measurement.start_ms = t.at_ms.saturating_sub(10000);
    }
    for cpu in &mut t.cpus {
        if let Some(value) = t
            .series
            .get(&format!("cpu.{}", cpu.id))
            .and_then(|s| s.at(t.at_ms))
        {
            cpu.busy = value;
        }
        cpu.softirq = if cpu.id == 3 {
            t.series
                .get("softirq")
                .and_then(|s| s.at(t.at_ms))
                .unwrap_or(0.)
                .min(cpu.busy)
        } else {
            1.
        };
        cpu.user = (cpu.busy - cpu.kernel - cpu.softirq - cpu.irq - cpu.steal).max(0.);
        cpu.wake_p99_ms = t
            .series
            .get(&format!("sched.cpu{}", cpu.id))
            .and_then(|s| s.at(t.at_ms));
    }
    for task in &mut t.tasks {
        task.age_ms = Some(t.at_ms.saturating_sub(task.start_ticks * 10));
        task.wake_p99_ms = t
            .series
            .get(&format!("task.{}", task.pid))
            .and_then(|s| s.at(t.at_ms));
        task.cpu_pct = t
            .series
            .get(&format!("task.runtime{}", task.pid))
            .and_then(|s| s.at(t.at_ms));
        if t.at_ms < end {
            task.wake_p50_ms = None;
            task.pss_bytes = None;
            task.pss_at_ms = None;
        }
        if task.pid == 34 {
            task.cpu_pct = t.series.get("softirq").and_then(|s| s.at(t.at_ms));
        }
        if task.state == "D" {
            task.blocked_ms = Some(t.at_ms.saturating_sub(595900) as f64);
        }
        if t.at_ms < 596000 && task.state == "D" {
            task.state = "S".into();
            task.blocked_ms = None;
            task.verdict = "—".into();
        }
        if t.at_ms < 522000 && task.name.starts_with("envoy") {
            task.verdict = "ok".into();
        }
    }
    for cpu in &mut t.cpus {
        if t.at_ms < end {
            cpu.wake_p50_ms = None;
        }
    }
    for group in &mut t.cgroups {
        group.runtime_pct = t
            .series
            .get(&format!("cgroup:{}:cpu", group.path))
            .and_then(|s| s.at(t.at_ms));
        group.throttled_ms_s = t
            .series
            .get(&format!("cgroup:{}:throttled", group.path))
            .and_then(|s| s.at(t.at_ms));
        group
            .fields
            .retain(|(k, _)| k != "runtime / one CPU" && k != "throttled time");
        group.fields.push((
            "runtime / one CPU".into(),
            group
                .runtime_pct
                .map(|v| format!("{v:.1}%"))
                .unwrap_or("unavailable".into()),
        ));
        group.fields.push((
            "throttled time".into(),
            group
                .throttled_ms_s
                .map(|v| format!("{v:.1} ms/s"))
                .unwrap_or("unavailable".into()),
        ));
        t.details
            .insert(format!("cgroup:{}", group.path), group.fields.clone());
    }
    for device in &mut t.devices {
        device.p99_ms = t
            .series
            .get(&format!("block.dev{}.p99", device.major_minor))
            .and_then(|s| s.at(t.at_ms));
    }
    if t.at_ms < end {
        t.histograms.clear();
    }
    if t.at_ms < 580000 {
        t.modules.retain(|m| m.name != "example_probe");
        t.details.remove("module:example_probe");
    }
    t.events.retain(|e| e.at_ms <= t.at_ms);
    t.issues.retain(|i| i.since_ms <= t.at_ms);
    for issue in &mut t.issues {
        issue.evidence.retain(|e| e.at_ms <= t.at_ms);
    }
    t
}

fn synchronize_current(t: &mut Telemetry) {
    if let Some(metric) = t.metrics.get_mut("runqueue") {
        metric.value = Some(
            t.cpus.iter().filter_map(|c| c.runnable).sum::<u32>() as f64
                / t.cpus.len().max(1) as f64,
        );
    }
    if let Some(group) = t.cgroups.iter().find(|g| g.path.ends_with("envoy.service")) {
        for (key, value) in [
            ("cgroup.cpu", group.runtime_pct),
            ("throttled", group.throttled_ms_s),
        ] {
            if let Some(metric) = t.metrics.get_mut(key) {
                metric.value = value;
            }
        }
    }
    let mut points = Vec::new();
    for cpu in &t.cpus {
        points.push((format!("softirq.cpu{}", cpu.id), Some(cpu.softirq)));
        points.push((format!("sched.cpu{}", cpu.id), cpu.wake_p99_ms));
    }
    for task in &t.tasks {
        points.push((format!("task.runtime{}", task.pid), task.cpu_pct));
    }
    for group in &mut t.cgroups {
        group.runtime_pct = Some(
            t.tasks
                .iter()
                .filter(|task| {
                    group.path == "/"
                        || task.cgroup == group.path
                        || task.cgroup.starts_with(&format!("{}/", group.path))
                })
                .filter_map(|task| task.cpu_pct)
                .sum(),
        );
        group
            .fields
            .retain(|(k, _)| k != "runtime / one CPU" && k != "pids.current");
        group.fields.push((
            "pids.current".into(),
            t.tasks
                .iter()
                .filter(|task| {
                    group.path == "/"
                        || task.cgroup == group.path
                        || task.cgroup.starts_with(&format!("{}/", group.path))
                })
                .count()
                .to_string(),
        ));
        group.fields.push((
            "runtime / one CPU".into(),
            format!("{:.1}%", group.runtime_pct.unwrap_or(0.)),
        ));
        t.details
            .insert(format!("cgroup:{}", group.path), group.fields.clone());
    }
    for (key, value) in points {
        if let Some(last) = t
            .series
            .get_mut(&key)
            .and_then(|s| s.samples.last_mut())
            .filter(|s| s.at_ms == t.at_ms)
        {
            last.value = value;
        }
    }
    for (key, component) in [("cpu.user", 0), ("cpu.kernel", 1)] {
        let value = t
            .cpus
            .iter()
            .map(|c| {
                if component == 0 {
                    c.user
                } else {
                    c.kernel + c.softirq + c.irq
                }
            })
            .sum::<f64>()
            / t.cpus.len().max(1) as f64;
        if let Some(metric) = t.metrics.get_mut(key) {
            metric.value = Some(value);
        }
    }

    // Derive shared histories from the same disjoint CPU components and task runtimes.
    if let Some(series) = t.series.get_mut("softirq") {
        for sample in &mut series.samples {
            sample.value = sample.value.map(|v| v.min(93.));
        }
    }
    if let Some(soft) = t.series.get("softirq").cloned() {
        if let Some(cpu) = t.series.get_mut("cpu.3") {
            for sample in &mut cpu.samples {
                sample.value = soft.at(sample.at_ms).map(|v| v.min(93.) + 7.);
            }
        }
        for key in ["softirq.cpu3", "task.runtime34"] {
            if let Some(series) = t.series.get_mut(key) {
                for sample in &mut series.samples {
                    sample.value = soft.at(sample.at_ms).map(|v| v.min(93.));
                }
            }
        }
    }
    for group in &t.cgroups {
        let key = format!("cgroup:{}:cpu", group.path);
        let histories = t
            .tasks
            .iter()
            .filter(|task| {
                group.path == "/"
                    || task.cgroup == group.path
                    || task.cgroup.starts_with(&format!("{}/", group.path))
            })
            .filter_map(|task| t.series.get(&format!("task.runtime{}", task.pid)).cloned())
            .collect::<Vec<_>>();
        if let Some(series) = t.series.get_mut(&key) {
            for sample in &mut series.samples {
                sample.value = Some(histories.iter().filter_map(|s| s.at(sample.at_ms)).sum());
            }
        }
    }
    let cpus = t.cpus.clone();
    for key in ["cpu.user", "cpu.kernel"] {
        let values = t
            .series
            .get(key)
            .map(|s| {
                s.samples
                    .iter()
                    .map(|sample| {
                        let value = cpus
                            .iter()
                            .map(|cpu| {
                                let busy = t
                                    .series
                                    .get(&format!("cpu.{}", cpu.id))
                                    .and_then(|s| s.at(sample.at_ms))
                                    .unwrap_or(cpu.busy);
                                let soft = if cpu.id == 3 {
                                    t.series
                                        .get("softirq")
                                        .and_then(|s| s.at(sample.at_ms))
                                        .unwrap_or(cpu.softirq)
                                        .min(93.)
                                } else {
                                    cpu.softirq
                                };
                                if key == "cpu.user" {
                                    (busy - cpu.kernel - soft - cpu.irq - cpu.steal).max(0.)
                                } else {
                                    cpu.kernel + soft + cpu.irq
                                }
                            })
                            .sum::<f64>()
                            / cpus.len().max(1) as f64;
                        Some(value)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(series) = t.series.get_mut(key) {
            for (sample, value) in series.samples.iter_mut().zip(values) {
                sample.value = value;
            }
        }
    }
    for (key, metric) in &t.metrics {
        if let Some(series) = t.series.get_mut(key) {
            if let Some(last) = series.samples.last_mut().filter(|s| s.at_ms == t.at_ms) {
                last.value = metric.value;
            }
        }
    }
    for cpu in &t.cpus {
        if let Some(last) = t
            .series
            .get_mut(&format!("cpu.{}", cpu.id))
            .and_then(|s| s.samples.last_mut())
            .filter(|s| s.at_ms == t.at_ms)
        {
            last.value = Some(cpu.busy);
        }
    }
    for task in &t.tasks {
        if let Some(last) = t
            .series
            .get_mut(&format!("task.{}", task.pid))
            .and_then(|s| s.samples.last_mut())
            .filter(|s| s.at_ms == t.at_ms)
        {
            last.value = task.wake_p99_ms;
        }
    }
    for group in &t.cgroups {
        for (suffix, value) in [
            ("cpu", group.runtime_pct),
            ("throttled", group.throttled_ms_s),
        ] {
            if let Some(last) = t
                .series
                .get_mut(&format!("cgroup:{}:{suffix}", group.path))
                .and_then(|s| s.samples.last_mut())
                .filter(|s| s.at_ms == t.at_ms)
            {
                last.value = value;
            }
        }
    }
    for device in &t.devices {
        if let Some(last) = t
            .series
            .get_mut(&format!("block.dev{}.p99", device.major_minor))
            .and_then(|s| s.samples.last_mut())
            .filter(|s| s.at_ms == t.at_ms)
        {
            last.value = device.p99_ms;
        }
    }
}
