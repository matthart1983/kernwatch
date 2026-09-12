//! Native, privileged map-walk cost; creates private maps and attaches no probe.
use aya::maps::{PerCpuHashMap, PerCpuValues};
use kernwatch::cpu_profile::Key;
use std::time::Instant;
fn cpu_ns() -> u64 {
    let mut t: libc::timespec = unsafe { std::mem::zeroed() };
    unsafe {
        libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t);
    }
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_arch = "aarch64")]
    let object = aya::include_bytes_aligned!("../probes/kernwatch-aarch64.bpf.o");
    #[cfg(not(target_arch = "aarch64"))]
    let object = aya::include_bytes_aligned!("../probes/kernwatch.bpf.o");
    let mut bpf = aya::Ebpf::load(object)?;
    let mut counts =
        PerCpuHashMap::<_, Key, u64>::try_from(bpf.take_map("profile_counts").unwrap())?;
    let cpus = aya::util::nr_cpus().map_err(|(_, e)| e)?;
    for count in [100u32, 1000, 8192] {
        for tid in 0..count {
            counts.insert(
                Key {
                    tid,
                    ..Default::default()
                },
                PerCpuValues::try_from(vec![1u64; cpus])?,
                0,
            )?;
        }
        let mut times = Vec::new();
        let mut wall = Vec::new();
        for _ in 0..11 {
            let start = Instant::now();
            let before = cpu_ns();
            let mut total = 0;
            for entry in counts.iter() {
                let (_, slots) = entry?;
                total += slots.iter().sum::<u64>();
            }
            assert_eq!(total, count as u64 * cpus as u64);
            times.push((cpu_ns() - before) as f64 / 1e6);
            wall.push(start.elapsed().as_secs_f64() * 1000.);
        }
        times.sort_by(f64::total_cmp);
        wall.sort_by(f64::total_cmp);
        println!("MAP_SCAN entries={count} cpu_slots={cpus} median_cpu_ms={:.3} median_wall_ms={:.3} projected_50hz_cpu_pct={:.2}", times[5], wall[5], times[5] * 5.);
    }
    Ok(())
}
