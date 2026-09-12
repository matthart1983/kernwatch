# Performance review — v0.4.0

Reviewed 2026-09-12 against release commit
`5d44c0b864551a59c6e20c538499c7b8092ab0b9`. The subsequent demo commit changes
documentation, recording tools and a workload example, not application code.
This review records findings; the optimizations below have not been implemented.

Ordinary monitoring improved over v0.3.0 on the measured host. Small profiles
are responsive and the native sampler's controlled acceptance workload has low
polling CPU. Large comparisons and repeated profile copies are the main
performance risks. Symbol metadata also escapes the timeline's memory estimate.

## Findings

### P1: comparison redraws rebuild and format the whole profile

`Profile::comparison_union` in [`src/flame.rs`](../src/flame.rs) looks up every
child with a linear sibling scan in both trees. A broad node therefore takes
quadratic work. `flame_difference` in [`src/ui/views.rs`](../src/ui/views.rs)
recomputes counts, clones the union, searches changes for each drawn cell, sorts
all changes and formats every call path before the terminal clips the list.
[`src/main_linux.rs`](../src/main_linux.rs) redraws even unchanged screens after
each 100 ms event wait.

At 8,192 terminal paths, a wide comparison took **154 ms median / 160 ms p95**
per draw; a deeper comparison took **417 ms median / 441 ms p95**. The ordinary
Flame view remained below 1 ms in the same cases. A finished capture can thus
keep a core busy rebuilding unchanged data and delay keyboard handling.

Recommended correction: cache the comparison and union by capture/baseline
revision, index children and deltas by identity/path, and format only visible
rows. Invalidate on new samples, baseline or comparison mode changes; filtering
and scrolling should reuse the comparison. Verify against added/removed paths
and both share/count modes, then rerun the scaling harness.

### P1: the timeline budget does not count symbol metadata

`App::frame_bytes` in [`src/app.rs`](../src/app.rs) counts profile tree nodes but
omits `Profile.frames`, `tasks`, and quality/metadata allocations. Retained
snapshots clone those fields. The nominal 48 MiB timeline budget therefore
cannot bound profiles with large symbol tables.

In the 8,192-path deep case, the symbol map alone serialized to **19.87 MiB**.
The frame estimate was **10.59 MiB**, exactly the same after removing the entire
symbol map. This is a direct accounting omission, independent of allocator
overhead. Sixty sequential updates added about **223 MiB RSS** and retained only
the final four seconds of history. The RSS delta includes latest/current copies
and allocator retention as well as timeline snapshots; it is not a measurement
of timeline allocations alone.

Recommended correction: account for every owned profile field immediately,
then share immutable profile/symbol data across retained snapshots. Verify
accounting with long demangled names, many images and task identities, and
verify that old snapshots remain unchanged when a live capture grows.

### P2: profile publication repeats expensive work faster than the UI consumes it

The trace loop calls `Probes::apply` after each poll, then sleeps 20 ms.
[`src/probes.rs`](../src/probes.rs) clones and sorts the full profile on every
publication. The UI merges the latest telemetry with its roughly one-second
collector updates, so most published copies are replaced before consumption.

Cloning and sorting the 8,192-path deep profile took **28.55 ms median** per
call, before map reads, symbol resolution, telemetry construction or UI work.
That operation alone exceeds the loop's 20 ms sleep and materially reduces its
polling cadence. This is measured copy/sort cost, not a whole-profiler CPU
percentage.

The CPU sampler also iterates the entire cumulative count map on every poll,
including unchanged entries (`CpuSampler::poll` in
[`src/cpu_profile.rs`](../src/cpu_profile.rs)). At the default 8,192-key limit
and a nominal 50 polls/s, that would mean 409,600 entry visits/s before per-CPU
summing. This is an upper-rate calculation; actual polling slows as work grows.
The acceptance workload has few distinct stacks and does not exercise this
limit.

Recommended correction: separate ingestion from publication, publish on a
bounded display cadence or request, and avoid sorting unchanged profiles.
Consider batch map reads after measuring populated-map polling on native Linux.
Keep the detached final drain and explicit loss accounting intact.

## Host monitoring comparison

Measured the published v0.3.0 x86-64 glibc binary against the local v0.4.0 release
build, on the same 24-logical-CPU host running Linux 7.1.13-200.fc44.x86_64.
The harness saw 602 processes at its start. Both binaries ran the Dense view in
a 160×48 PTY, without privileges or probes, with isolated state directories.
The order was old/new/new/old. Each run warmed for 10 seconds and measured the
next 30 seconds; other host work was not frozen.

CPU below is percent of **one logical CPU**, not total machine capacity.
RSS is measured at approximately 40 seconds from startup.

| Version | CPU, two runs | End RSS, two runs | Mean CPU | Mean end RSS |
| --- | --- | --- | ---: | ---: |
| v0.3.0 | 24.17%, 24.40% | 196.9, 198.1 MiB | 24.28% | 197.5 MiB |
| v0.4.0 | 21.00%, 20.17% | 142.9, 144.0 MiB | 20.58% | 143.4 MiB |

That is about **15% less CPU and 27% less RSS** at this point in these runs.
Both versions used 13 threads and emitted roughly 2.5–2.7 kB/s of terminal
output. This comparison includes earlier collector/timeline improvements that
landed with v0.4.0; it does not attribute the improvement to the CPU sampler.
Two runs per version are insufficient for a general performance guarantee.

A separate v0.4.0 run warmed for 15 seconds, then measured 120 seconds. CPU
averaged **23.97% of one core**: sampler 10.73%, topology workers combined 7.69%,
main/other kernwatch-named threads 5.48%, tracer 0.05%, and BPF inventory near
zero. Small differences from the total reflect tick rounding and read timing.
No probes were enabled.

RSS rose from **109.8 MiB at 15 seconds to 227.1 MiB at 135 seconds**. At roughly
45, 75 and 105 seconds it was 144.6, 178.7 and 205.7 MiB. Memory had **not settled**
by the end of this measurement. Live series retain up to 600 samples
(`Series::push` in [`src/domain.rs`](../src/domain.rs)), so this short run does
not establish a leak or a steady-state bound. The next ordinary-monitoring
measurement should run beyond that ten-minute history window. The 40-second
improvement above must not be presented as steady-state memory usage.

## Profile scaling

The release-mode `performance_review` example uses actual aggregation,
comparison, snapshot retention and Ratatui rendering into a 160×48 TestBackend.
It includes rendering and buffer comparison, but excludes terminal transport,
BPF and live collection. Each operation warms once, then runs at least seven
iterations, up to 100 or roughly 300 ms of measurement.

Wide profiles have one parent and N leaf handlers. Deep profiles have groups
of 64 paths, each with ten distinct nested frames. Both include raw/demangled
symbol metadata. These are deterministic stress inputs, not recorded production
profiles. Each row runs in a fresh process.

| Shape | Terminal paths | Normal draw median | Comparison median / p95 | Clone + sort median |
| --- | ---: | ---: | ---: | ---: |
| Wide | 100 | 0.19 ms | 0.41 / 0.45 ms | 0.013 ms |
| Wide | 1,000 | 0.17 ms | 4.16 / 4.40 ms | 0.154 ms |
| Wide | 8,192 | 0.20 ms | 154.36 / 159.90 ms | 1.287 ms |
| Deep | 100 | 0.50 ms | 4.65 / 6.09 ms | 0.201 ms |
| Deep | 1,000 | 0.20 ms | 43.44 / 46.23 ms | 2.043 ms |
| Deep | 8,192 | 0.51 ms | 416.96 / 440.64 ms | 28.547 ms |

Normal drawing prunes frames that cannot occupy a terminal cell, so draw time
need not increase monotonically with path count. Comparison still visits and
formats the full profile. Seven-iteration tail estimates are diagnostic rather
than statistically stable p95 estimates.

## Native CPU sampler evidence

The [v0.4.0 release run](https://github.com/matthart1983/kernwatch/actions/runs/34678305261)
passed native x86-64 and ARM64 capture checks and recorded:

| Runner | Hz | Polling thread CPU | BPF CPU | Sampled / unsampled throughput |
| --- | ---: | ---: | ---: | ---: |
| ARM64 | 49 | 0.473% | 0.007% | 1.000 |
| ARM64 | 99 | 0.529% | 0.028% | 0.993 |
| x86-64 | 49 | 0.723% | 0.025% | 1.003 |
| x86-64 | 99 | 0.856% | 0.069% | 0.999 |

These short, single-run checks use the same worker count for the sampled and
unsampled portions. They establish that busy loops are captured without a
large throughput change in that workload. Ratios above one reflect measurement
noise, not a speedup caused by profiling.

The harness calls `Probes::apply` once after capture; it does **not** include
the application's repeated publication, rendering and history retention in
these CPU figures. It also excludes sampler startup from the ingest CPU timer.
Do not describe these numbers as total application or total capture overhead.
Native populated-map scaling and a production-duration profiling soak remain
unmeasured. The demo runs under QEMU TCG and supplies no native overhead evidence.

## Reproduce

```sh
cargo build --locked --release --example performance_review
./target/release/examples/performance_review wide 8192
./target/release/examples/performance_review deep 8192

python3 scripts/monitor_bench.py /path/to/v0.3.0 ./target/release/kernwatch \
  ./target/release/kernwatch /path/to/v0.3.0 \
  --warmup 10 --seconds 30 --output /tmp/kernwatch-monitor-performance.json

python3 scripts/monitor_bench.py ./target/release/kernwatch \
  --warmup 15 --seconds 120 --output /tmp/kernwatch-monitor-long.json
```

Run the host harness outside a restricted PID namespace and avoid running
other benchmarks concurrently. It does not enable probes or change host
configuration. Its JSON contains aggregate timings, memory and kernwatch thread
CPU, not host task names or recordings. Keep raw host evidence local.
