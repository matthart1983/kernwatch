//! Bounded workload for the real-VM flame graph preview. Build with frame pointers.
//! Three threads give the graph depth and width to explore: requests with recursive
//! rendering, a compactor that spends time in the kernel and a light metrics thread.
//! Exporting the first profile switches requests from cache-heavy to parse-heavy.
use std::hint::black_box;
use std::io::{Seek, Write};
use std::time::{Duration, Instant};

const RUNTIME: Duration = Duration::from_secs(180);
const UNIT: u64 = 200_000;

#[inline(always)]
fn burn(mut value: u64, units: u64) -> u64 {
    for _ in 0..units * UNIT {
        value = black_box(
            value
                .rotate_left(7)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1),
        );
    }
    value
}

#[inline(never)]
fn percent_decode(value: u64, units: u64) -> u64 {
    burn(value, units)
}

#[inline(never)]
fn decode_header_value(value: u64, units: u64) -> u64 {
    let value = burn(value, units / 3);
    let value = percent_decode(value, units - units / 3);
    black_box(value)
}

#[inline(never)]
fn parse_headers(value: u64, units: u64) -> u64 {
    let value = burn(value, units / 3);
    let value = decode_header_value(value, units - units / 3);
    black_box(value)
}

#[inline(never)]
fn parse_query(value: u64) -> u64 {
    let value = burn(value, 1);
    let value = percent_decode(value, 2);
    black_box(value)
}

#[inline(never)]
fn parse_request(value: u64, parse: u64) -> u64 {
    let value = parse_headers(value, parse);
    let value = parse_query(value);
    burn(value, 2)
}

#[inline(never)]
fn sha256_block(value: u64) -> u64 {
    burn(value, 4)
}

#[inline(never)]
fn hmac_sha256(value: u64) -> u64 {
    let value = burn(value, 1);
    let value = sha256_block(value);
    black_box(value)
}

#[inline(never)]
fn verify_token(value: u64) -> u64 {
    let value = burn(value, 1);
    let value = hmac_sha256(value);
    black_box(value)
}

#[inline(never)]
fn authenticate(value: u64) -> u64 {
    let value = verify_token(value);
    burn(value, 2)
}

#[inline(never)]
fn siphash_round(value: u64, units: u64) -> u64 {
    burn(value, units)
}

#[inline(never)]
fn hash_key(value: u64, units: u64) -> u64 {
    let value = burn(value, units / 4);
    let value = siphash_round(value, units - units / 4);
    black_box(value)
}

#[inline(never)]
fn cache_lookup(value: u64, units: u64) -> u64 {
    let value = burn(value, units / 4);
    let value = hash_key(value, units - units / 4);
    black_box(value)
}

#[inline(never)]
fn decode_column(value: u64) -> u64 {
    burn(value, 3)
}

#[inline(never)]
fn decode_row(value: u64) -> u64 {
    let value = burn(value, 2);
    let value = decode_column(value);
    black_box(value)
}

#[inline(never)]
fn load_row(value: u64) -> u64 {
    let value = decode_row(value);
    burn(value, 3)
}

#[inline(never)]
fn fetch_page(value: u64, cache: u64) -> u64 {
    let value = cache_lookup(value, cache);
    let value = load_row(value);
    black_box(value)
}

#[inline(never)]
fn escape_html(value: u64) -> u64 {
    burn(value, 10)
}

#[inline(never)]
fn expand_block(value: u64, depth: u64) -> u64 {
    let value = burn(value, 2);
    let value = if depth == 0 {
        escape_html(value)
    } else {
        expand_block(value, depth - 1)
    };
    black_box(value)
}

#[inline(never)]
fn render_page(value: u64) -> u64 {
    let value = expand_block(value, 5);
    burn(value, 2)
}

trait Codec {
    fn compress(value: u64) -> u64;
}

struct Deflate;

impl Codec for Deflate {
    #[inline(never)]
    fn compress(value: u64) -> u64 {
        let value = burn(value, 2);
        let value = find_match(value);
        black_box(value)
    }
}

#[inline(never)]
fn find_match(value: u64) -> u64 {
    burn(value, 6)
}

#[inline(never)]
fn send_response(value: u64) -> u64 {
    let value = Deflate::compress(value);
    burn(value, 2)
}

#[inline(never)]
fn handle_request(value: u64, parse: u64, cache: u64) -> u64 {
    let value = parse_request(value, parse);
    let value = authenticate(value);
    let value = fetch_page(value, cache);
    let value = render_page(value);
    let value = send_response(value);
    black_box(value)
}

#[inline(never)]
fn serve_requests() {
    let deadline = Instant::now() + RUNTIME;
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
        let (parse, cache) = if changed { (40, 3) } else { (3, 40) };
        value = handle_request(value, parse, cache);
    }
    black_box(value);
}

#[inline(never)]
fn compare_keys(value: u64) -> u64 {
    burn(value, 14)
}

#[inline(never)]
fn merge_runs(value: u64) -> u64 {
    let value = burn(value, 6);
    let value = compare_keys(value);
    black_box(value)
}

#[inline(never)]
fn checksum_block(value: u64) -> u64 {
    burn(value, 10)
}

#[inline(never)]
fn write_segment(file: &mut std::fs::File, block: &[u8]) {
    file.rewind().unwrap();
    for _ in 0..8 {
        file.write_all(block).unwrap();
    }
}

#[inline(never)]
fn compact_segments() {
    let deadline = Instant::now() + RUNTIME;
    let mut file = std::fs::File::create("/tmp/.webapp-segment").unwrap();
    let block = vec![7; 1 << 20];
    let mut value = 1;
    while Instant::now() < deadline {
        let started = Instant::now();
        value = merge_runs(value);
        value = checksum_block(value);
        write_segment(&mut file, &block);
        // Idle for part of each cycle, so this thread isn't an even half of the graph.
        std::thread::sleep(started.elapsed() / 2);
    }
    black_box(value);
}

#[inline(never)]
fn aggregate_histograms(value: u64) -> u64 {
    burn(value, 6)
}

#[inline(never)]
fn report_metrics() {
    let deadline = Instant::now() + RUNTIME;
    let mut value = 1;
    while Instant::now() < deadline {
        value = aggregate_histograms(value);
        std::thread::sleep(Duration::from_millis(250));
    }
    black_box(value);
}

fn main() {
    // A stable, short name makes the process easy to find in the picker.
    unsafe { libc::prctl(libc::PR_SET_NAME, c"webapp".as_ptr()) };
    for (name, work) in [
        ("compactor", compact_segments as fn()),
        ("metrics", report_metrics as fn()),
    ] {
        std::thread::Builder::new()
            .name(name.into())
            .spawn(work)
            .unwrap();
    }
    serve_requests();
}
