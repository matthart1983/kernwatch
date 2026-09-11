#[cfg(target_os = "linux")]
mod linux {
    use kernwatch::probes::Probes;
    use std::time::{Duration, Instant};
    pub fn main() {
        let mut failed = false;
        for mode in ["sched", "irq", "syscalls", "block", "all"] {
            match Probes::start(mode) {
                Ok(mut p) => {
                    if mode == "sched" {
                        let links = kernwatch::bpf_metadata::links();
                        println!("BPF_LINK_INVENTORY={links:?}");
                        if links.is_err() {
                            failed = true;
                        }
                        use std::os::fd::{AsFd, AsRawFd};
                        let mut inspected = 0;
                        for info in aya::programs::loaded_programs().flatten() {
                            if info
                                .name_as_str()
                                .is_some_and(|name| name.starts_with("kw_"))
                            {
                                let result = info.fd().map_err(|e| e.to_string()).and_then(|fd| {
                                    kernwatch::bpf_metadata::helpers(
                                        fd.as_fd().as_raw_fd(),
                                        info.size_translated().unwrap_or(0),
                                    )
                                    .map_err(|e| e.to_string())
                                });
                                println!("BPF_HELPERS {} {result:?}", info.id());
                                if result.is_err() {
                                    failed = true;
                                }
                                inspected += 1;
                            }
                        }
                        println!("BPF_HELPERS_INSPECTED={inspected}");
                        if inspected == 0 {
                            failed = true;
                        }
                    }
                    let start = Instant::now();
                    let work = std::thread::spawn(move || {
                        for _ in 0..200 {
                            if (mode == "block" || mode == "all")
                                && std::env::var("KERNWATCH_DISPOSABLE_GUEST").as_deref() == Ok("1")
                            {
                                use std::io::Write;
                                if let Ok(mut disk) =
                                    std::fs::OpenOptions::new().write(true).open("/dev/vda")
                                {
                                    disk.write_all(&[0x5a; 4096]).expect("guest disk write");
                                    disk.sync_all().expect("guest disk sync");
                                }
                            }
                            let _ = std::fs::metadata("/missing-kernwatch-smoke");
                            std::thread::sleep(Duration::from_millis(2));
                        }
                    });
                    while start.elapsed() < Duration::from_secs(2) {
                        if let Err(e) = p.poll() {
                            println!("FAIL {mode} poll: {e}");
                            failed = true;
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    let _ = work.join();
                    println!(
                        "ATTACH_OK {mode} events={} paired={} lost={}",
                        p.processed, p.correlator.paired, p.correlator.lost
                    );
                    let mut t = kernwatch::domain::Telemetry {
                        at_ms: kernwatch::enrich::monotonic_ms(),
                        ..Default::default()
                    };
                    p.apply(&mut t);
                    if mode == "sched" {
                        let identity_series = t
                            .metrics
                            .keys()
                            .filter(|key| key.starts_with("task.") && key.split('.').count() == 3)
                            .count();
                        println!("TASK_IDENTITY_LATENCY_SERIES={identity_series}");
                        if identity_series == 0 {
                            failed = true;
                        }
                    }
                    println!(
                        "MEASURED {mode} own_bpf_cpu={:?} ingest_thread_cpu={:?}",
                        t.value("bpf.own"),
                        t.value("trace.ingest_cpu")
                    );
                    if t.value("bpf.own").is_none() {
                        failed = true;
                    }
                    if mode == "block" || mode == "all" {
                        println!("BLOCK_COMPLETIONS={}", p.block_ms.len());
                        println!(
                            "BLOCK_DEVICE_KEYS={:?}",
                            p.device_ms.keys().collect::<Vec<_>>()
                        );
                        if p.block_ms.is_empty()
                            || !p.device_ms.iter().any(|(k, v)| *k != 0 && !v.is_empty())
                        {
                            failed = true;
                        }
                    }
                    if mode == "sched" {
                        println!("SCHED_CGROUPS={}", p.correlator.wake_cgroup.len());
                        println!("OFFCPU_TASKS={}", p.correlator.off_ms.len());
                        use std::os::unix::fs::MetadataExt;
                        let inode = std::fs::metadata("/sys/fs/cgroup").unwrap().ino();
                        println!(
                            "CGROUP_ROOT_ID={inode} OBSERVED={:?}",
                            p.correlator.wake_cgroup.keys().collect::<Vec<_>>()
                        );
                        if !p.correlator.wake_cgroup.contains_key(&inode) {
                            failed = true;
                        }
                        let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
                        let ticks = stat
                            .rsplit_once(')')
                            .unwrap()
                            .1
                            .split_whitespace()
                            .nth(19)
                            .unwrap()
                            .parse::<u64>()
                            .unwrap();
                        let observed =
                            p.correlator.task_starts.get(&std::process::id()).map(|ns| {
                                (*ns as u128 * unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as u128
                                    / 1_000_000_000) as u64
                            });
                        println!("TASK_START_TICKS={ticks} OBSERVED={observed:?}");
                        if observed != Some(ticks) {
                            failed = true;
                        }

                        if p.correlator.off_ms.is_empty() {
                            failed = true;
                        }
                        if p.correlator.wake_cgroup.is_empty() {
                            failed = true;
                        }
                    }
                    if (mode == "sched" || mode == "syscalls") && p.correlator.paired == 0 {
                        failed = true;
                        println!("FAIL {mode} no paired events");
                    }
                }
                Err(e) => {
                    failed = true;
                    println!("FAIL {mode}: {e}");
                }
            }
        }
        let pid = std::process::id();
        match Probes::start(&format!("syscalls pid={pid} stack seconds=2")) {
            Ok(mut p) => {
                for _ in 0..20 {
                    let _ = std::fs::metadata("/kernwatch-scoped-missing");
                    std::thread::sleep(Duration::from_millis(2));
                    let _ = p.poll();
                }
                let paired = p.correlator.paired;
                let stacks = p
                    .correlator
                    .events
                    .iter()
                    .filter(|e| e.message.contains("user_stack=0x"))
                    .count();
                let foreign = p
                    .correlator
                    .events
                    .iter()
                    .any(|e| e.subject != format!("task:{pid}"));
                println!("SCOPED_SYSCALL paired={paired} user_stacks={stacks} foreign={foreign}");
                if paired == 0 || stacks == 0 || foreign {
                    failed = true;
                }
            }
            Err(e) => {
                println!("FAIL scoped stack: {e}");
                failed = true;
            }
        }
        match Probes::start(&format!("syscalls pid={pid}")) {
            Ok(mut p) => {
                for _ in 0..110_000 {
                    unsafe { libc::syscall(libc::SYS_getpid) };
                }
                let _ = p.poll();
                let mut t = kernwatch::domain::Telemetry {
                    at_ms: kernwatch::enrich::monotonic_ms(),
                    ..Default::default()
                };
                p.apply(&mut t);
                println!(
                    "OVERLOAD lost={} quality={:?}",
                    p.correlator.lost,
                    t.metrics.values().next().map(|m| &m.quality)
                );
                if p.correlator.lost == 0
                    || t.metrics
                        .values()
                        .any(|m| m.quality == kernwatch::domain::Quality::Available)
                {
                    failed = true;
                }
            }
            Err(e) => {
                println!("FAIL overload: {e}");
                failed = true;
            }
        }
        if std::env::var("KERNWATCH_DISPOSABLE_GUEST").as_deref() == Ok("1") {
            let cgroup_result = (|| -> std::io::Result<()> {
                std::fs::write("/sys/fs/cgroup/cgroup.subtree_control", "+cpu +cpuset")?;
                std::fs::create_dir("/sys/fs/cgroup/kernwatch-test")?;
                for command in [
                    "quota /kernwatch-test 50000 100000",
                    "cpuset /kernwatch-test 0",
                ] {
                    let (path, value, description) = kernwatch::actions::parse_target(command)?;
                    let mut plan = kernwatch::actions::Plan::preview(
                        &kernwatch::actions::LinuxHost,
                        path,
                        value,
                        description,
                    )?;
                    plan.apply(&kernwatch::actions::LinuxHost)?;
                    plan.revert(&kernwatch::actions::LinuxHost)?;
                }
                std::fs::remove_dir("/sys/fs/cgroup/kernwatch-test")?;
                Ok(())
            })();
            println!("CGROUP_QUOTA_CPUSET_APPLY_ROLLBACK={cgroup_result:?}");
            if cgroup_result.is_err() {
                failed = true;
            }
            let pid = std::process::id();
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
            let end = stat.rfind(')').unwrap();
            let start = stat[end + 2..].split_whitespace().nth(19).unwrap();
            let (path, value, description) =
                kernwatch::actions::parse_target(&format!("task {pid} {start} 0")).unwrap();
            let mut plan = kernwatch::actions::Plan::preview(
                &kernwatch::actions::LinuxHost,
                path,
                value,
                description,
            )
            .unwrap();
            let result = plan
                .apply(&kernwatch::actions::LinuxHost)
                .and_then(|_| plan.revert(&kernwatch::actions::LinuxHost));
            println!("TASK_AFFINITY_APPLY_ROLLBACK={result:?}");
            if result.is_err() {
                failed = true;
            }
        }
        println!(
            "KERNWATCH_PROBES_RESULT={}",
            if failed { "FAIL" } else { "PASS" }
        );
        std::process::exit(if failed { 1 } else { 0 });
    }
}
#[cfg(target_os = "linux")]
fn main() {
    linux::main()
}
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("This probe example requires Linux");
}
