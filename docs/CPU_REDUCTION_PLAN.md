# Reduce kernwatch's own CPU consumption

Status: implementation and verification in progress, 2026-09-12. Shared history,
identity indexes, demand-driven enrichment, task fast-path changes, dirty rendering
and cached comparisons are implemented; combined acceptance is still running. Baseline application commit:
`50c116338f1063f07de63b2d6ded4f10e70153d7` (same runtime code as v0.4.0).

## Conclusion

kernwatch has substantial observer overhead even with no BPF probes enabled.
The priority is to stop repeatedly copying histories and metadata, reduce
background helper work, and make the per-task fast path cheaper. Merely lowering
the terminal refresh rate will recover only a small part of the normal-monitoring
CPU. Large Flame comparisons have an additional, separate rendering problem.

The previous short runs measured roughly 20–24% of one logical CPU, not 20–24%
of the whole 24-CPU machine. The real profiling demo deliberately runs two busy
workload threads in a two-CPU VM; its 100% utilization is not an overhead test.
The measurements below examine kernwatch itself on the native host.

## Evidence and attribution

### Native call-stack profile

`perf record -e cpu-clock:u -F 99 --call-graph dwarf,16384` sampled the release
binary and its descendants for approximately 50 seconds after a ten-second
delay. There were 460 samples and no reported lost samples. This is a coarse
userspace profile: it excludes CPU spent inside the kernel and cannot precisely
rank low-frequency functions.

The sampler's `Enricher::enrich`, `Source::publish`, allocation, deallocation,
string cloning and memory copying are prominent. Child `modinfo` processes also
appear: `lzma_decode` alone accounts for 6.52% of these userspace samples.
That percentage is neither whole-host CPU nor total kernwatch overhead.
Earlier `/proc/PID/stat` measurements exclude helper processes; future headline
overhead must include them.

### Instrumented long run

A separate checkout adds scoped `CLOCK_THREAD_CPUTIME_ID` and monotonic timers,
call counts and aggregate task/history/RSS gauges. The main checkout and installed
binary are unchanged. The Dense screen runs at 160×48 without probes, privileged
collection, recording, or user navigation. About 2,800 threads and 4,100 series
are visible. Scoped CPU includes user and kernel execution, unlike the perf
profile above; scope wall time also includes waiting.

CPU scopes are **inclusive**. For example, `enricher.tasks` is part of
`enricher.total`, and `app.retain_frame` is part of `app.update`. Do not add parent
and child scopes together.

The run warmed for 15 seconds and then measured 660 seconds. Whole-process CPU
averaged **29.50% of one logical core**; the final minute averaged **34.57%**.
This excludes helper-process CPU. On this 24-CPU host, 34.57% of one core is
about 1.44% of aggregate CPU capacity, though scheduling can concentrate that
work on individual cores.

| Time from startup | Process CPU in the preceding window | RSS at window end |
| --- | ---: | ---: |
| 75 s | 21.67% (15–75 s) | 176.1 MiB |
| 195 s | 26.09% (75–195 s) | 270.1 MiB |
| 375 s | 30.11% (195–375 s) | 405.7 MiB |
| 555 s | 30.92% (375–555 s) | 543.7 MiB |
| 615 s | 33.06% (555–615 s) | 575.1 MiB |
| 675 s | 34.57% (615–675 s) | 575.2 MiB |

RSS peaked at 577.5 MiB. Series sample count reached about **2.37 million** and
flattened near its capacity. Memory also flattened over the last minute. This
supports history/copy growth as a major contributor; one minute of flattening
does not establish a long-term leak-free bound.

The final approximately four minutes of scoped timing identify these costs:

| Scope | CPU as % of one core | Interpretation |
| --- | ---: | --- |
| Per-thread procfs scan | 5.71% | Enumeration, stat/status/wchan reads, parsing and task construction |
| Topology request/merge on sampler | 3.38% | Includes identity joins, request and result copies |
| Enricher history clone | 1.41% | Copies existing history into the next collection state |
| Per-task history maintenance | 1.39% | Records/prunes runtime series |
| App update | 6.15% | Includes retention (2.67%) and latest-snapshot copy (2.34%) |
| Systemd metadata worker | 5.80% | Parent worker CPU, including launch cost; excludes helper CPU |
| Cgroup collection worker | 2.67% | Discovery, counter/configuration/ancestor reads; excludes its history copy |
| Module metadata worker | 1.75% | Parent worker CPU, including launch cost |
| Module/PSS worker | 0.75% | Existing bounded slow pass |
| Terminal draw | 0.93% | About 1.14 ms wall time per draw; repeated around nine times/s |

These rows select distinct scopes except for the explicitly nested app-update
breakdown. They do not cover every allocation, drop, worker or loop operation.
The sampler's full collection cycle averages about 12.44% of a core in this
window. Normal display drawing is a small fraction of the observed total.

The timers add some overhead, and the host is not a controlled idle lab.
Measurements identify work and scaling; final before/after acceptance must use
uninstrumented binaries and repeat equivalent windows.

## Root causes

1. **Full histories are copied multiple times per sample.**
   `Enricher::enrich` clones `t.series` into `self.history`; source workers clone
   and publish their own histories. `App::retain_frame` clones a complete
   snapshot and then clears its series. `App::update` makes another complete
   copy for `latest_snapshot`. At thousands of series and hundreds of samples
   per series, cost grows with history age despite a stable task count.
   `Series::push` also drains the front of a `Vec` after 600 samples, moving the
   remaining samples on each append. A ring alone will not eliminate the copies.
   Files: [`enrich.rs`](../src/enrich.rs), [`sources.rs`](../src/sources.rs),
   [`app.rs`](../src/app.rs), [`domain.rs`](../src/domain.rs).

2. **Metadata is assembled, cloned and merged every second, even on cache hits.**
   Every idle source is requested again at the next collector tick. Per-object
   TTLs reduce some reads, but do not eliminate request construction, cached
   result cloning, full-map publication or merging. `Source::publish` repeatedly
   scans old/current task vectors to confirm identities; PSS does two searches
   per task. `Extra::cached` scans its whole cache for expiration on every lookup.
   These are avoidable scaling costs. Identity checks themselves must remain.
   Files: [`sources.rs`](../src/sources.rs), [`enrichment.rs`](../src/enrichment.rs).

3. **Slow discovery and helper commands run continuously in the background.**
   Cgroup collection walks up to 512 directories, reads some fields twice and
   rereads ancestor limits for each descendant. Systemd metadata visits up to
   four units per request, potentially executing both `systemctl show` and
   `systemctl cat`; a large round-robin can take longer than the TTL, so each
   revisit can be another cache miss. Module metadata also launches `modinfo`.
   PSS is already bounded to 32 leaders on a five-second pass: it should not be
   incorrectly described as an unrestricted every-second full-host scan.
   Files: [`inventory.rs`](../src/inventory.rs), [`enrichment.rs`](../src/enrichment.rs).

4. **Growing memory can amplify helper-launch cost.**
   `command::run` installs a `pre_exec` hook for parent-death handling. Rust 1.98
   declines its `posix_spawn` path when such hooks exist, leaving fork/exec.
   This creates a plausible connection between growing resident histories and
   increasing metadata-worker CPU. The hook provides useful cleanup guarantees;
   removing it without an equivalent lifecycle design is not an acceptable fix.
   Sources: [`command.rs`](../src/command.rs) and the
   [Rust 1.98 process implementation](https://github.com/rust-lang/rust/blob/1.98.0/library/std/src/sys/process/unix/unix.rs).

   A separate C mechanism benchmark touched 16/256/512 MiB and launched
   `/usr/bin/true` 40 times using either `fork` + `prctl` + `exec` or
   `posix_spawn`. Three repetitions produced these median parent CPU costs:

   | Touched memory | fork + prctl + exec | posix_spawn |
   | --- | ---: | ---: |
   | 16 MiB | 0.135 ms/launch | 0.018 ms/launch |
   | 256 MiB | 2.632 ms/launch | 0.020 ms/launch |
   | 512 MiB | 6.130 ms/launch | 0.020 ms/launch |

   This verifies the memory-sensitive fork mechanism, not the exact fraction
   of kernwatch's metadata CPU caused by it. The benchmark ran after the soak,
   in a restricted PID namespace on the same native kernel. It measures parent
   CPU only and its posix_spawn case does not provide kernwatch's parent-death
   guarantee. It is therefore not a drop-in replacement benchmark. The result
   is consistent with systemd-worker CPU rising from roughly 1.6% early to 5.8%
   late while resident history grew.

5. **The task fast path reads and parses more than necessary.**
   Every visible thread gets `stat`, `status` and `wchan` reads each tick. `stat`
   is tokenized once in `parse_task`, then again in `Enricher::enrich`. Paths,
   string keys and temporary vectors are rebuilt. This stage costs about 6%
   of a core on the measured ~2,800-thread host, before enrichment joins.
   Files: [`collect.rs`](../src/collect.rs), [`enrich.rs`](../src/enrich.rs).

6. **Drawing and capture publication ignore whether their output is needed.**
   Main redraws after every 100 ms event wait. In ordinary Dense mode this is
   about 1% of a core; it is worthwhile but not the main bottleneck. In comparison
   mode it repeatedly rebuilds a much more expensive union and formatted list.
   The trace worker clones/sorts profiles after each poll, with only a 20 ms
   sleep, although the UI merges telemetry on roughly one-second collector
   updates. The earlier benchmark measured 154–417 ms comparison draws and
   28.5 ms clone/sort operations at 8,192 paths.
   Files: [`main_linux.rs`](../src/main_linux.rs), [`probes.rs`](../src/probes.rs),
   [`flame.rs`](../src/flame.rs), [`ui/views.rs`](../src/ui/views.rs).

## Implementation sequence

### 0. Make observer cost measurable — one small PR

Add opt-in per-stage CPU/wall timing and allocation/copy counters, giving each
topology worker a distinct name. Extend the benchmark to capture user/system CPU,
helper count and child CPU, history samples/bytes, collection delay, missed
deadlines, render count and input-to-draw latency. Keep raw host records local.
Expose kernwatch's own total cost separately from the capture worker's cost;
do not hide its PID to make system-wide profiles look quieter.

Gate: reconcile non-overlapping stage totals with process CPU; report helper
CPU separately and as part of the total observer footprint. Verify instrumentation
overhead against the same binary with instrumentation disabled.

### 1. Remove wasted copies and repeated identity searches — first optimization PR

- Construct the retained entity snapshot without cloning series that will be
  discarded. Count symbol, task and quality metadata in its memory budget.
- Redesign `snapshot`/`latest_snapshot` ownership so ordinary live updates do not
  maintain two independently owned copies of the same snapshot. Preserve a
  separate latest value only when freeze, history navigation or replay needs it.
- Build identity indexes once per merge: tasks `(pid, start_ticks)`, cgroups
  `(path, inode)`, modules `(name, sysfs identity)`, devices `(name, major:minor)`.
  Reuse them across metadata and PSS merging; combine the two PSS lookups.
- Move cache expiry sweeps out of individual object lookups into one bounded
  maintenance pass. Do not return expired data as a fresh observation.

Gate: frozen snapshots and baseline profiles remain unchanged while new samples
arrive; seeking back and returning live works; PID reuse and object replacement
still reject stale enrichment. Benchmark 100, 1,000, 3,000 and 10,000 tasks.
Near-linear joins and no clone-then-discard history operation are required.

### 2. Own history once and bound its memory — second optimization PR

Separate live history storage from entity snapshots. Use bounded ring/chunked
series storage and immutable sharing for consumers that need the same history
version. Avoid a design where `Arc::make_mut` copies an entire 600-sample vector
on every append because a snapshot still references it. Task/image histories
must be evicted by identity and bounded by actual owned bytes.

Make recording/export materialize the required wire representation at its own
cadence. Preserve current recording compatibility, absolute one-second buckets,
gaps, sample-time cursor behavior and the rule against carrying stale values
into unobserved buckets. Keep profiles and demangled symbol tables shared across
unchanged captures and retained snapshots.

Gate: owned history bytes plateau after the retention window on stable inputs;
no per-tick copy proportional to every retained sample. Test long symbol names,
task churn, a frozen display, active recording and repeated profile captures.
The budget must cover the symbol tables omitted by the current estimator.

### 3. Put slow enrichment on a demand and CPU budget — third optimization PR

Give each source an explicit next-due time, freshness policy and request identity.
Separate fast rates from configuration/discovery. Keep CPU, task state, PSI,
device rates and active cgroup runtime on the fast path. Cache cgroup topology,
configuration and ancestor limits once per generation; do not reread every
ancestor for every descendant. Refresh changed objects and newly selected
inspectors promptly.

Keep only needed identity fields in source requests. Publish changed results
instead of reconstructing whole cached tables every second. Bound command
launches globally as well as per source. Fetch unit file bodies, expensive module
metadata and storage health primarily when requested or when identity changes.
Use source-specific age limits instead of the current universal ten-second
staleness threshold if a source is deliberately scheduled more slowly.

Reduce helper frequency and duplicate requests before redesigning process
launch. If fork overhead remains significant after history memory is reduced,
use a small dedicated helper launcher/broker or direct APIs. Preserve literal
arguments, output limits, timeout, process-group cleanup, parent-death behavior
and reaping. A new dependency or broker needs a demonstrated benefit.

Gate: Dense monitoring does not continuously refresh unrelated inspector file
bodies; fast telemetry retains its one-second cadence. Missing, pending and stale
metadata stay explicit. Opening an inspector forces refresh without blocking the
UI. Include helper CPU in before/after numbers.

### 4. Tighten the per-task fast path — fourth optimization PR

Parse `stat` once into typed fields, retain PID/start identity as a numeric key,
and reuse read buffers/path capacity. Read `wchan` only where its state/detail
is useful. Separate dynamic per-thread counters from slower status/configuration
fields; give slower observations their own timestamps and real rate intervals.
Refresh selected/watched tasks promptly, while keeping core per-thread CPU and
state observations at one second for every visible task.

Gate: no cached context-switch value is treated as a new zero delta. Counter
resets, CPU hotplug, task exit/reuse, missing permissions and cgroup moves remain
correct. Do not weaken fresh identity validation before host controls.

### 5. Stop redundant rendering and profile publication — fifth optimization PR

Track dirty state and draw for new visible data, input, resize, status changes,
or an actual animation deadline. Keep event handling responsive independently of
draw cadence. A static comparison should not repeatedly consume CPU rebuilding
its data. Cache comparison counts, union and ordering by profile/baseline revision;
index deltas and format only visible rows. Preserve narrow and removed frames.

Separate BPF ingestion cadence from publication cadence. Publish on new data at
a bounded rate, reuse unchanged profiles, and sort only when required. Measure
populated count-map polling before choosing batch reads or a slower polling
schedule. Preserve final detach/drain, hotplug coverage, identity checks and loss
reporting. Do not use a slower poll merely to conceal dropped samples.

Gate: unchanged screens do not redraw at 10 Hz; keys and resize still redraw
promptly. Both comparison modes, filtering, scrolling and baseline replacement
invalidate exactly the required caches. Capture results and final counts remain
identical for equivalent controlled workloads.

### 6. Verify the combined result and set the default budget

Run uninstrumented old/new/new/old comparisons after warm-up, then a 30-minute
stable-host run and a task-churn run. Test 100/1,000/3,000/10,000 visible threads,
small and large cgroup trees, privileged/unprivileged acquisition, Dense/Tasks,
frozen/replay modes, inspector refresh, recording/export, 49/99 Hz process and
system profiles, and 8,192-path comparisons. Use native x86-64 and ARM64 for
sampler overhead; use VMs for intrusive lifecycle tests, not overhead claims.

Proposed acceptance budgets, to be validated rather than advertised now:

| Case | Target |
| --- | --- |
| Dense, ~3,000 threads, ~500 cgroups, no probes | At least 50% less whole-observer CPU than matched baseline; aim for ≤10% of one core |
| Stable input after history capacity is reached | Accounted bytes bounded; no sustained RSS growth over the following 20 minutes |
| New one-second telemetry | p95 publication lag below 200 ms; no silent sampling gaps |
| Keyboard/resize | p95 input-to-draw below 100 ms on the reference terminal |
| 8,192-path comparison | Initial build below 50 ms; cached redraw below 5 ms on the reference host |
| Active CPU profile | Workload throughput change ≤2% median across repeated matched runs, with uncertainty reported |

Treat thresholds as engineering goals, not universal guarantees. If the CPU goal
conflicts with acquisition fidelity, expose the sampling/freshness tradeoff
explicitly. Do not disable correctness checks or silently reduce scope to pass.

## Scope and completion

Land each PR only with its focused correctness tests and before/after measurements.
The first delivery is copy elimination and linear merges; the second removes
history-dependent growth. Slow enrichment and task polling then address the
steady background cost. Rendering/profile work closes the heavy Flame-mode gap.
Do not declare completion from the small busy-loop sampler benchmark alone.

Local evidence: `/tmp/kernwatch-self.perf.data`, the corresponding flat/caller
reports, `/tmp/kernwatch-cpu-stages.json`, `/tmp/kernwatch-cpu-soak.json`, and
the instrumented checkout `/tmp/kernwatch-cpu-analysis`. The timings are collected
with `/tmp/kernwatch-instrument.py` and `/tmp/kernwatch-stage-soak.py`; these are
analysis-only tools and are not linked into the release binary.

The shorter baseline and profile-scaling results remain in
[`PERFORMANCE_REVIEW.md`](PERFORMANCE_REVIEW.md).
