//! Privileged, read-only CPU sampler acceptance test. No host configuration edits.
use kernwatch::probes::Probes;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
#[inline(never)]
fn profile_busy_loop(stop: Arc<AtomicBool>, work: Arc<AtomicU64>) -> u64 {
    let mut n = 1u64;
    while !stop.load(Ordering::Relaxed) {
        for _ in 0..10_000 {
            n = std::hint::black_box(n.wrapping_mul(6364136223846793005).wrapping_add(1));
        }
        work.fetch_add(10_000, Ordering::Relaxed);
    }
    n
}
#[inline(never)]
fn deep_stack(depth: u32, stop: Arc<AtomicBool>, work: Arc<AtomicU64>) -> u64 {
    if depth == 0 {
        profile_busy_loop(stop, work)
    } else {
        let value = deep_stack(depth - 1, stop, work);
        std::hint::black_box(value ^ depth as u64)
    }
}
fn worker(stop: &Arc<AtomicBool>, work: &Arc<AtomicU64>) -> (std::thread::JoinHandle<u64>, u32) {
    let (tx, rx) = mpsc::channel();
    let stop = stop.clone();
    let work = work.clone();
    let handle = std::thread::spawn(move || {
        tx.send(unsafe { libc::syscall(libc::SYS_gettid) as u32 })
            .unwrap();
        profile_busy_loop(stop, work)
    });
    (handle, rx.recv().unwrap())
}
fn collect(probe: &mut Probes, seconds: u64) -> std::io::Result<()> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(seconds) {
        probe.poll()?;
        std::thread::sleep(Duration::from_millis(20));
    }
    probe.finish()?;
    let total = probe.profile.root.samples;
    probe.poll()?;
    assert_eq!(
        total, probe.profile.root.samples,
        "final read double counted"
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stop = Arc::new(AtomicBool::new(false));
    let work = Arc::new(AtomicU64::new(0));
    if std::env::args().any(|a| a == "--busy-child") {
        profile_busy_loop(stop, work);
        return Ok(());
    }
    let (before, before_tid) = worker(&stop, &work);
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .arg("--busy-child")
        .spawn()?;
    // The child is always reaped, including assertion failures during unwinding.
    struct ChildGuard<'a>(&'a mut std::process::Child);
    impl Drop for ChildGuard<'_> {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let guard = ChildGuard(&mut child);
    let foreign = guard.0.id();
    let pid = std::process::id();
    let mut workers = vec![before];
    let initial = work.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_secs(2));
    let baseline = (work.load(Ordering::Relaxed) - initial) / 2;
    println!("BASELINE iterations_per_second={baseline}");
    for hz in [49, 99] {
        let mut probe = Probes::start(&format!("cpu tgid={pid} seconds=3 hz={hz}"))?;
        let (after, after_tid) = worker(&stop, &work);
        workers.push(after);
        let initial = work.load(Ordering::Relaxed);
        collect(&mut probe, 3)?;
        let sampled_rate = (work.load(Ordering::Relaxed) - initial) / 3;
        let reference_start = work.load(Ordering::Relaxed);
        std::thread::sleep(Duration::from_secs(2));
        let reference_rate = (work.load(Ordering::Relaxed) - reference_start) / 2;
        println!("MATCHED_THROUGHPUT hz={hz} sampled={sampled_rate} unsampled={reference_rate} ratio={:.3}", sampled_rate as f64 / reference_rate.max(1) as f64);
        let total = probe.profile.root.samples;
        assert!(total > 0);
        assert!(probe.profile.quality.user_stacks > 0);
        assert!(
            probe
                .profile
                .folded()
                .iter()
                .any(|f| f.contains("profile_busy_loop")),
            "busy function not resolved"
        );
        for tid in [before_tid, after_tid] {
            assert!(
                probe
                    .profile
                    .tasks
                    .keys()
                    .any(|k| k.starts_with(&format!("{pid}:{tid}:"))),
                "worker {tid} absent"
            );
        }
        assert!(
            probe
                .profile
                .tasks
                .keys()
                .all(|k| k.starts_with(&format!("{pid}:"))),
            "foreign process in scoped capture"
        );
        let mut telemetry = kernwatch::domain::Telemetry::default();
        probe.apply(&mut telemetry);
        println!("CPU_OK hz={hz} samples={total} quality={:?} cpus={:?} throughput={} ingest_cpu={:?} bpf_cpu={:?}",probe.profile.quality,probe.profile.metadata.cpus,sampled_rate,telemetry.value("trace.ingest_cpu"),telemetry.value("bpf.own"));
    }
    let mut thread = Probes::start(&format!("cpu pid={before_tid} seconds=2"))?;
    collect(&mut thread, 2)?;
    assert!(thread.profile.root.samples > 0);
    assert!(thread
        .profile
        .tasks
        .keys()
        .all(|k| k.starts_with(&format!("{pid}:{before_tid}:"))));
    println!("THREAD_SCOPE_OK");
    // Force sustained kernel execution; incidental interrupts in a user-only
    // workload are not a reliable kernel-stack acceptance test.
    let kernel_stop = Arc::new(AtomicBool::new(false));
    let kernel_done = kernel_stop.clone();
    let kernel_worker = std::thread::spawn(move || {
        use std::io::Read;
        let mut zero = std::fs::File::open("/dev/zero").unwrap();
        let mut buffer = vec![0u8; 4 * 1024 * 1024];
        while !kernel_done.load(Ordering::Relaxed) {
            zero.read_exact(&mut buffer).unwrap();
            std::hint::black_box(&buffer);
        }
    });
    let mut system = Probes::start("cpu seconds=2")?;
    collect(&mut system, 2)?;
    kernel_stop.store(true, Ordering::Relaxed);
    kernel_worker.join().unwrap();
    assert!(system
        .profile
        .tasks
        .keys()
        .any(|k| k.starts_with(&format!("{foreign}:"))));
    assert!(
        system.profile.quality.kernel_stacks > 0,
        "system-wide capture has no kernel evidence"
    );
    println!("SYSTEM_SCOPE_OK samples={}", system.profile.root.samples);
    let mut tiny = Probes::start(&format!("cpu tgid={pid} entries=1 seconds=2"))?;
    collect(&mut tiny, 2)?;
    assert!(
        tiny.profile.quality.map_failures > 0,
        "map pressure was not reported"
    );
    println!("MAP_PRESSURE_OK quality={:?}", tiny.profile.quality);
    let mut exiting = Probes::start(&format!("cpu tgid={foreign} seconds=2"))?;
    guard.0.kill()?;
    guard.0.wait()?;
    collect(&mut exiting, 1)?;
    assert!(exiting
        .profile
        .metadata
        .warnings
        .iter()
        .any(|w| w.contains("Target exited")));
    println!("TARGET_EXIT_OK");
    let (tx, rx) = mpsc::channel();
    let deep_stop = stop.clone();
    let deep_work = work.clone();
    let deep = std::thread::spawn(move || {
        tx.send(unsafe { libc::syscall(libc::SYS_gettid) as u32 })
            .unwrap();
        deep_stack(160, deep_stop, deep_work)
    });
    workers.push(deep);
    let deep_tid = rx.recv()?;
    let mut depth = Probes::start(&format!("cpu pid={deep_tid} seconds=2"))?;
    collect(&mut depth, 2)?;
    assert!(
        depth.profile.quality.depth_limit > 0,
        "full-depth stacks not reported"
    );
    println!("DEPTH_LIMIT_OK count={}", depth.profile.quality.depth_limit);
    if std::env::var("KERNWATCH_DISPOSABLE_GUEST").as_deref() == Ok("1") {
        // These setting changes are confined to the disposable initramfs guest.
        let mut hotplug = Probes::start(&format!("cpu tgid={pid} seconds=2"))?;
        hotplug.poll()?;
        let online = "/sys/devices/system/cpu/cpu1/online";
        if std::path::Path::new(online).exists() {
            std::fs::write(online, "0")?;
            hotplug.poll()?;
            assert!(!hotplug.profile.metadata.cpus.contains(&1));
            std::fs::write(online, "1")?;
            hotplug.poll()?;
            assert!(hotplug.profile.metadata.cpus.contains(&1));
            assert!(hotplug
                .profile
                .metadata
                .warnings
                .iter()
                .any(|w| w.contains("CPU set changed")));
            println!("CPU_HOTPLUG_OK");
        }
        hotplug.finish()?;
        std::fs::write("/proc/sys/kernel/kptr_restrict", "2")?;
        let mut restricted = Probes::start(&format!("cpu tgid={pid} seconds=1"))?;
        std::fs::write("/proc/sys/kernel/kptr_restrict", "0")?;
        collect(&mut restricted, 1)?;
        assert!(restricted
            .profile
            .metadata
            .warnings
            .iter()
            .any(|w| w.contains("Kernel symbols restricted")));
        println!("RESTRICTED_SYMBOLS_OK");
    }
    // Optional reference profiler comparison in guests that include perf.
    if std::path::Path::new("/perf").exists() {
        let status = std::process::Command::new("/perf")
            .args([
                "record",
                "-e",
                "cpu-clock",
                "-F",
                "49",
                "--call-graph",
                "fp",
                "-p",
                &pid.to_string(),
                "-o",
                "/tmp/cpu-reference.data",
                "--",
                "/sleep",
                "2",
            ])
            .status()?;
        assert!(status.success(), "perf reference capture failed");
        let report = std::process::Command::new("/perf")
            .args([
                "report",
                "--stdio",
                "--no-children",
                "-i",
                "/tmp/cpu-reference.data",
                "--sort",
                "symbol",
            ])
            .output()?;
        assert!(report.status.success());
        let report = String::from_utf8_lossy(&report.stdout);
        assert!(
            report.contains("profile_busy_loop"),
            "perf did not resolve the same busy function"
        );
        println!("PERF_REFERENCE_OK\n{report}");
    }
    stop.store(true, Ordering::Relaxed);
    for worker in workers {
        worker.join().unwrap();
    }
    Ok(())
}
