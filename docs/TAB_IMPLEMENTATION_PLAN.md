# Tab completion implementation plan

Date: 12 September 2026. Based on the [source and live-data audit](TAB_GAP_AUDIT.md) at `288a045`. This is the current forward plan; the original implementation plan remains design history. Work is not implemented by this document.

Implementation progress is tracked in [Tab fixes](TAB_FIXES.md); several packages are partially addressed, not complete.

## Delivery order

Complete P0 correctness work first, then shared table behavior and existing-data wiring, then missing acquisition and attribution. Ship reviewable increments with a demonstration of the affected behavior. Do not combine all collectors and UI changes into one release-sized patch.

| Order | Work package | Priority | Depends on |
|---|---|---|---|
| 1 | W01 Measurement and availability contracts | P0 | — |
| 2 | W02 Subject identity throughout histories and selection | P0 | W01 |
| 3 | W03 Bind diagnostic verification to actions | P0 | W01–02 |
| 4 | W04 Shared typed tables and compact behavior | P1 | W01–02 |
| 5 | W05 Dense, Overview and Tasks integration | P1 | W04 |
| 6 | W06 Memory modes, NUMA and PSS coverage | P1 | W04 |
| 7 | W07 Cgroup controllers, hierarchy and I/O | P1 | W04 |
| 8 | W08 IRQ placement and selected history | P1 | W04 |
| 9 | W09 Scheduler current waiting lifecycle | P1 | W02, W04 |
| 10 | W10 Block operation lifecycle and metadata | P1 | W02, W04 |
| 11 | W11 Dmesg source/time consistency | P1 | W01, W04 |
| 12 | W12 Syscall decoding and capture controls | P1 | W02, W04 |
| 13 | W13 eBPF inventory and stats completeness | P1 | W01–02, W04 |
| 14 | W14 Module lifecycle and trust metadata | P1 | W02, W04 |
| 15 | W15 Evidence-driven Diagnose workflow | P1 | W03–04, relevant collector packages |
| 16 | W16 Attribution extensions and release acceptance | P2 | W05–15 |

Packages 6–14 are largely independent after the foundation. Ordering favors finishing already-collected data before adding probes. Detailed scheduling should follow the W01/W04 schema decisions and privileged-kernel access; this plan does not imply a reliable calendar estimate for unproven attribution features.

## Work packages and acceptance

### W01 — Measurement and availability contracts

Files: `domain.rs`, `enrich.rs`, `inventory.rs`, `bpf.rs`, `logs.rs`, `sources.rs`, `ui/views.rs`.

Define source, subject scope, unit, aggregation window, sample age and availability for displayed measurements. Keep metric-current and series-history writes consistent. Correct Overview softirq/D-state wiring without conflating NET_RX and aggregate CPU time. Replace missing-as-zero and failed-map-list-as-empty paths. Expose bounded acquisition coverage. Make graph captions derive from actual queries.

Acceptance: unprivileged Overview shows independently validated softirq CPU accounting and D-state count; unknown memory and failed map enumeration stay unknown; ENOENT is distinguishable from EACCES; reset/stale/stopped/partial states survive recording/replay. Unit tests exercise each affected semantic branch, including real zero.

### W02 — Stable identity and selection

Files: `domain.rs`, `inventory.rs`, `enrich.rs`, `probes.rs`, `app.rs`, `recording.rs`.

Use typed subject keys: task PID/start ticks, device identity/major:minor, cgroup identity, and BPF identity with creation metadata where available. Preserve the existing device identity instead of overwriting it. Remove live PID-only history/drill dependence. Reconcile selections by identity after filtering, sorting, refresh and removal. Define migration behavior for older recordings; unknown old identity must not imply continuity.

Acceptance: synthetic PID/device/BPF reuse does not inherit an old subject's graph, baseline or metadata. Selecting B after A never shows A's details when B lacks data. Removed rows produce a deliberate new selection or unavailable panel. Old supported recordings still open with explicit legacy scope.

### W03 — Verification correctness

Files: `actions.rs`, `app.rs`, `diagnose.rs`, `recording.rs`.

Persist issue ID, subject key, action ID, criterion, units, capture quality and apply time. Verify the chosen action using its own pre/post windows. Allow observation-only verification without modifying an unrelated action. Retain an inconclusive outcome when coverage or elapsed time is insufficient.

Acceptance: apply action A, select issue B, and verify B; A's journal must remain unchanged. Restart/replay preserves associations and criteria. Insufficient/lost samples never yield a successful result. Engine and UI use the same threshold definition.

### W04 — Tables and responsive views

Files: `app.rs`, `model.rs`, `ui/views.rs`, `ui/widgets.rs`.

Define each table once with typed column keys, numeric sorting, filter semantics, selected identity, viewport offset and compact priorities. Remove reliance on legacy six-column task headers for the detailed table. Give each advertised mode a named state and observable effect. Apply the same selected data to normal, expanded and compact panels.

Acceptance: every table can reach its last row; every advertised sortable column sorts its underlying value; filters preserve coherent selection; independent panel focus works. At 80×24 and 110×32, essential identity/status columns and an expansion path remain available. At 160×52 and 240×80, additional columns appear without losing controls. Retain the existing minimum-size guard.

### W05 — Dense, Overview and Tasks

Files: `ui/views.rs`, `app.rs`, task enrichment and metadata workers.

Finish task process/thread grouping and folding, concern counts and peer context. Surface collected scheduler weight. Make card readiness and acquisition actions meaningful. Specify which Dense panels survive each breakpoint and how omitted details are reached; preserve key `0`, timeline sharing and IRQ alignment.

Acceptance: live idle and loaded samples show useful unprivileged data; stopped probes have explicit start/context paths. All eight Dense groups render at the full supported size. Task filters/folding have correct shown/total counts without double-counting shared process memory. Selected task histories stay scoped across refresh.

### W06 — Memory

Files: `enrich.rs`, `inventory.rs`, `sources.rs`, `domain.rs`, memory renderer.

Implement distinct overview/slab/hugepage/cgroup modes. Store slab values numerically, add size/growth sorting and dynamic scales. Collect node meminfo and numastat deltas with named counters and reset handling. Rotate bounded PSS work fairly and prioritize the selected process; show age, sample coverage and unknown PSS without mixed RSS ranking.

Acceptance: modes visibly change the intended tables; slab lists beyond six entries remain reachable; small and large hosts scale correctly. Fixture NUMA imbalance produces corresponding node rows. Processes outside the initial 32 eventually receive samples under stable load; denied/exited processes remain explicit. Allocation page flow is not labeled allocation attribution.

### W07 — Cgroups

Files: `inventory.rs`, `domain.rs`, `app.rs`, cgroup renderer.

Wire controller filters, model controller availability, add `io.stat` per-device deltas, and calculate effective ancestor constraints separately from own configuration. Complete tree indentation/folding and traversal coverage. Decouple CPU usage from throttle-field presence.

Acceptance: nested parent quota affects the displayed effective constraint; absent/disabled/unlimited/zero values differ; CPU usage survives missing throttle fields; I/O changes under a controlled cgroup workload. More than 512 groups results in a visible cap/coverage state, not a supposedly complete inventory.

### W08 — IRQ

Files: `irq.rs`, `domain.rs`, `app.rs`, IRQ/Dense renderers.

Parse CPU sets once, distinguish configured/effective/observed placement, support multi-CPU selection and rate distribution. Add imbalance context, selected-history event markers and available queue/RPS/RFS metadata. Keep exact actor attribution explicitly unavailable without an event source.

Acceptance: sparse, offline, single, ranged and missing CPU masks render correctly. Multi-CPU IRQs show the chosen scope and corresponding history. Rate/handler-duration units stay separate; denied tracing does not erase counter data. Numeric entries stay right-aligned at supported widths.

### W09 — Scheduler

Files: `probes/kernwatch.bpf.c`, `probes.rs`, `domain.rs`, scheduler renderer.

Capture/maintain outstanding runnable waits, current running task and enqueue/run/exit transitions. Render current wait ages separately from completed latency distributions. Handle migration, repeated wake events, task exit and trace loss. Add source-qualified verdicts and scheduler configuration context where readable.

Acceptance: a controlled contended workload shows runnable tasks waiting and the actual running task; completed waits leave the outstanding set. Loss invalidates completeness and stale ages. CPU/task/cgroup mode selections agree between rows, plot, histogram and waiting panel. Validate privileged x86-64 and ARM64 behavior before claiming parity.

### W10 — Block

Files: block probe and aggregator, `inventory.rs`, `domain.rs`, block renderer.

Add read/write/discard/flush operation identity and lifecycle handling. Scope diskstats rates explicitly, retain pending/completed distinction, add available geometry/cache/firmware metadata and correct virtual-device support states. Keep topology candidates separate from exact driver mappings.

Acceptance: controlled read/write/flush workloads produce correctly labeled events and counters. Request completion/reuse/loss does not leave immortal outstanding requests. Partition and whole-device selection cannot borrow each other's distributions. Device replacement resets histories and metadata.

### W11 — Dmesg and event history

Files: `logs.rs`, event merge in `enrich.rs`/sources, `app.rs`, log renderer.

Unify event-list and event-rate population selection, with per-source retention/drop counts. Honor the displayed history window. Add usable source filters and selected-message wrapping/expansion while preserving follow behavior.

Acceptance: journal-only, kmsg-only and mixed-source fixtures have correctly scoped rates; a ten-minute label actually queries ten minutes or is corrected to retained duration. Long messages remain readable. Stopped/denied/missing sources are distinguishable and historical events remain accessible after live-source failure.

### W12 — Syscalls

Files: syscall probe/decoder, `probes.rs`, typed syscall model, `app.rs`, renderer.

Introduce architecture-tagged syscall metadata; incrementally decode common file, network, memory and synchronization calls. Preserve raw numbers and arguments. Make capture-average versus rolling rates explicit. Specify count, duration and filter populations consistently. Add bounded stack-symbol resolution only with matching process/build identity and failure states.

Acceptance: representative x86-64 and ARM64 calls decode correctly, unknown numbers remain usable, failed pointer reads and truncation are visible. Selected table/event/histogram populations agree. Existing latency-unit and numeric-sort regressions remain covered. Offline replay needs no live process to display previously captured raw evidence.

### W13 — eBPF

Files: `bpf.rs`, stats lease/capture control, `domain.rs`, renderer.

Fix partial map acquisition and program identity, surface enumeration limits, provide explicit stats-session ownership/lifetime, and make creator-UID labels consistent. Design optional FD-holder and occupancy sampling with bounded cost and map-type-specific support.

Acceptance: stats disabled, available, denied and partial enumeration each have distinct behavior. Removing/recreating a program cannot produce inherited rate deltas. Failed map-ID retrieval never shows a confirmed zero maps. Do not show runtime p99 or helper cost until independently measured distributions/cost evidence exists.

### W14 — Modules

Files: `inventory.rs`, module metadata worker, optional lifecycle probe, renderer.

Represent unknown trust metadata explicitly, track observed load generations and improve baseline changes. Add loader/lifecycle evidence where supported; retain interval uncertainty for polling-only operation.

Acceptance: missing taint data cannot classify a module as in-tree; reload fixtures do not retain stale metadata or baseline status. Exact loader/time fields are populated only from supporting events. Unsupported hook attribution has a named support boundary.

### W15 — Diagnose workflow

Files: `diagnose.rs`, `app.rs`, renderer, report export.

Replace static pipeline marks with actual acquisition/correlation/ranking/verification state. Rank issues by documented severity, impact and confidence, with stable ties; implement viewport scrolling. Add rule-specific evidence/alternative hypotheses for scheduler contention, cgroup throttle, memory pressure and block waits. Make report preview scope explicit and allow selected-issue evidence export.

Acceptance: lower-ranked rows remain reachable; no-data state cannot show completed analysis. Known workload fixtures identify the relevant symptom and cite measured evidence while leaving unproved causes unconfirmed. Selected issue, proposed action, verification and exported evidence refer to the same subject and observation window.

### W16 — Attribution extensions and release acceptance

First conduct bounded feasibility work for allocation call-site attribution, module hooks/runtime, BPF helper costs, exact IRQ-change actors and exact queue/vector mapping. Record kernel/API support, acquisition overhead and failure semantics. Features that cannot meet those conditions remain explicitly unsupported; do not fill them with fixture-derived values.

Then validate the complete product using the matrix below. Update the requirement ledger from actual evidence, refresh the demo and README to show real supported behaviors, and prepare release binaries only after acceptance. This planning task does not publish a release.

## Validation matrix and completion gate

- **Each table/panel:** live-shaped populated, empty, stopped, denied, unsupported, partial, stale and reset cases where applicable. Assert meaningful values, selection and state—not merely that drawing does not panic.
- **Interaction:** sort/filter/group/mode/focus/scroll/expand/drill; normal and compact views must resolve the same subject. Exercise datasets longer than the viewport.
- **Measurement:** independently computed counter deltas, units, windows, percentile populations, loss, counter reset, task/device reuse and clock boundaries.
- **Recording:** live fixture → record → replay → export preserves identity, availability, coverage and action verification. Keep compatibility tests for supported existing recordings.
- **Linux live:** unprivileged host, permission-restricted environment, and controlled privileged workloads on supported x86-64 and ARM64 kernels. Record kernel/BTF/probe support and overhead; do not infer attach success from compilation.
- **Portability:** build/test all four released Linux target variants. GNU/musl variants need acquisition packaging checks as well as executable startup.
- **UI evidence:** all 13 tabs at supported breakpoints, active and stopped capture, long names, large inventories, dense timeline, right-aligned IRQ values and key `0`. Demonstrate real capture separately from the synthetic demo.
- **Engineering gate:** formatting, Clippy, relevant unit/workflow/render/PTY tests and release build; measure collection/UI overhead under representative load. Rerun affected tests after changes and the full suite at the release gate.

A package is complete only when its acceptance checks pass, availability boundaries are documented, and screenshots/demo claims match live behavior. No remaining static control, unrelated fallback value, or unbound verification result may be hidden behind a screen-level “validated” label.
