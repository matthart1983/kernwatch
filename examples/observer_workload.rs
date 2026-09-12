//! Bounded arithmetic workload with a shared counter for external throughput tests.
use std::{
    fs::OpenOptions,
    os::fd::AsRawFd,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
#[inline(never)]
fn busy(stop: &AtomicBool, counter: &AtomicU64) {
    let mut value = 1u64;
    while !stop.load(Ordering::Relaxed) {
        for _ in 0..10000 {
            value = std::hint::black_box(value.wrapping_mul(6364136223846793005).wrapping_add(1));
        }
        counter.fetch_add(10000, Ordering::Relaxed);
    }
}
fn main() -> std::io::Result<()> {
    let path = std::env::args().nth(1).expect("counter file path");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)?;
    file.set_len(16)?;
    // SAFETY: the new file has two zero-initialized aligned atomic counters;
    // the mapping stays live until both workers have stopped and joined.
    let memory = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            16,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            file.as_raw_fd(),
            0,
        )
    };
    if memory == libc::MAP_FAILED {
        return Err(std::io::Error::last_os_error());
    }
    let stop = Arc::new(AtomicBool::new(false));
    let mut workers = Vec::new();
    for i in 0..2 {
        let address = memory as usize + i * 8;
        let stop = stop.clone();
        workers.push(std::thread::spawn(move || {
            busy(&stop, unsafe { &*(address as *const AtomicU64) })
        }));
    }
    std::thread::sleep(Duration::from_secs(240));
    stop.store(true, Ordering::Relaxed);
    for worker in workers {
        worker.join().unwrap();
    }
    unsafe {
        libc::munmap(memory, 16);
    }
    Ok(())
}
