//! Bounded workload for the real-VM previews. Build with frame pointers.
//! Three threads give the graph depth and width to explore: requests with recursive
//! rendering, a compactor that spends time in the kernel and a light metrics thread.
//! Exporting the first profile switches requests from cache-heavy to parse-heavy.
//! The tour also runs `webapp batch`, a pulsing neighbour, and gives this process an
//! expendable /dev/vda, so its other views have something real to show.
use std::hint::black_box;
use std::io::{Seek, Write};
use std::os::unix::fs::{FileExt, OpenOptionsExt};
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

#[inline(never)]
fn write_journal_block(disk: &std::fs::File, block: &[u8], offset: u64) {
    disk.write_all_at(block, offset).unwrap();
}

#[inline(never)]
fn read_checkpoint(disk: &std::fs::File, block: &mut [u8], offset: u64) {
    disk.read_exact_at(block, offset).unwrap();
}

#[inline(never)]
fn run_journal(disk: std::fs::File) {
    const BLOCK: usize = 1 << 20;
    const SPAN: u64 = 512 << 20;
    let deadline = Instant::now() + RUNTIME;
    // O_DIRECT needs an aligned buffer.
    let mut raw = vec![7; BLOCK + 4096];
    let start = raw.as_ptr().align_offset(4096);
    let block = &mut raw[start..start + BLOCK];
    let mut offset = 0;
    while Instant::now() < deadline {
        write_journal_block(&disk, block, offset);
        read_checkpoint(&disk, block, offset);
        disk.sync_data().unwrap();
        offset = (offset + BLOCK as u64) % SPAN;
        std::thread::sleep(Duration::from_millis(40));
    }
}

#[inline(never)]
fn stem_word(value: u64) -> u64 {
    burn(value, 8)
}

#[inline(never)]
fn tokenize(value: u64) -> u64 {
    let value = burn(value, 2);
    let value = stem_word(value);
    black_box(value)
}

/// Pulse rather than pin, so CPU history has a shape instead of a ceiling.
#[inline(never)]
fn index_documents() {
    let deadline = Instant::now() + RUNTIME;
    let mut value = 1;
    while Instant::now() < deadline {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(9) {
            value = tokenize(value);
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    black_box(value);
}

#[inline(never)]
fn build_segment() {
    let deadline = Instant::now() + RUNTIME;
    while Instant::now() < deadline {
        let segment = vec![1u8; 192 << 20];
        std::thread::sleep(Duration::from_secs(10));
        black_box(&segment);
        drop(segment);
        std::thread::sleep(Duration::from_secs(6));
    }
}

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name(name.into())
        .spawn(work)
        .unwrap()
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("batch") {
        unsafe { libc::prctl(libc::PR_SET_NAME, c"batch".as_ptr()) };
        spawn("indexer", index_documents);
        spawn("segments", build_segment);
        index_documents();
        return;
    }
    // A stable, short name makes the process easy to find in the picker.
    unsafe { libc::prctl(libc::PR_SET_NAME, c"webapp".as_ptr()) };
    spawn("compactor", compact_segments);
    spawn("metrics", report_metrics);
    let disk = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_DIRECT)
        .open("/dev/vda");
    if let Ok(disk) = disk {
        spawn("journal", move || run_journal(disk));
    }
    serve_requests();
}
