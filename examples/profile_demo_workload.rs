//! Bounded workload for the real-VM profiling preview. Build with frame pointers.
//! Exporting the first profile switches from cache-heavy to parse-heavy work.
use std::time::{Duration, Instant};

#[inline(never)]
fn cache_lookup(mut value: u64, rounds: u64) -> u64 {
    for _ in 0..rounds {
        value = std::hint::black_box(value.wrapping_mul(6364136223846793005).wrapping_add(1));
    }
    value
}

#[inline(never)]
fn parse_headers(mut value: u64, rounds: u64) -> u64 {
    for _ in 0..rounds {
        value = std::hint::black_box(value.rotate_left(7).wrapping_mul(1442695040888963407));
    }
    value
}

#[inline(never)]
fn serve_requests() {
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut value = 1;
    while Instant::now() < deadline {
        let changed = std::fs::read_dir("/tmp")
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("kernwatch-report-")
            });
        let (cache, parse) = if changed { (1, 9) } else { (9, 1) };
        value = cache_lookup(value, cache * 2_000_000);
        value = parse_headers(value, parse * 2_000_000);
    }
    std::hint::black_box(value);
}

fn main() {
    // A stable, short name makes the process easy to find in the picker.
    unsafe { libc::prctl(libc::PR_SET_NAME, c"profile-demo".as_ptr()) };
    let worker = std::thread::spawn(serve_requests);
    serve_requests();
    worker.join().unwrap();
}
