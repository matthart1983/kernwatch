# Tab and table implementation audit

Date: 12 September 2026. Source baseline: `288a045` (main), following the syscall/eBPF fixes. This audit and its [implementation plan](TAB_IMPLEMENTATION_PLAN.md) supersede broad screen-completion claims in the earlier ledger. No application changes are included in this review.

## Assessment

All 13 screens have renderers. The remaining work is primarily incomplete subpanels, inconsistent table behavior, missing collectors, and incorrect measurement/availability semantics. A screen appearing in the demo does not establish that its live data, selection, controls, compact rendering, and replay work together.

The most urgent problems are correctness defects: available live data is disconnected from cards; missing values sometimes become zero; some histories lose subject identity; Diagnose verification can update the wrong action. These should be fixed before expanding acquisition.

## Method and limits

Reviewed the diagram-derived [specification](SPEC.md), prior plans and completion ledger, screen renderers, App selection/actions, typed models, procfs/sysfs enrichment, inventory workers, tracing aggregation, and diagnostic rules. Compared demo and live snapshots using a temporary program calling the existing Collector seven times over approximately six seconds. No probes were attached or kernel settings changed.

The unprivileged sample contained 24 CPUs, 12 visible tasks, 5 devices, 154 cgroups, 181 modules, 300 events, 256 metrics and 803 series. Raw host data remains outside the repository. Counts demonstrate acquisition coverage on this environment, not correctness on every host. The task population is visibility-limited and must not be treated as a complete host process inventory.

Scheduler, IRQ-duration, block-duration and syscall capture were stopped. BPF enumeration was denied; the systemd bus and device health access failed. These are environment/acquisition conditions, not proof those collectors are missing. Privileged tracing behavior was inspected in source, not freshly exercised in this audit. Previously recorded tests and screenshots remain historical evidence; they do not validate the missing requirements identified here.

## Cross-screen correctness findings

| ID / priority | Finding and evidence | Required outcome |
|---|---|---|
| X01 / P0 | Overview reads `softirq`, whereas ordinary live CPU accounting produces `softirq.peak`. It also reads `dstate` as a metric although enrichment only records its series. `Telemetry::value` does not fall back to series. The sample had a valid softirq peak metric and D-state series but neither requested card metric. [Cards](../src/ui/views.rs#L800), [producer](../src/enrich.rs#L624), [lookup](../src/domain.rs#L245). | Explicit metric contracts shared by producer and consumer. Distinguish aggregate softirq CPU time from traced NET_RX duration; do not silently substitute one for the other. |
| X02 / P0 | Device sysfs identity is pushed into fields and immediately overwritten by a new vector. All five sampled devices lacked it. Device histories use names; task runtime uses PID without start ticks. Identity-aware wake histories already exist but are not used consistently. [Device](../src/inventory.rs#L89), [runtime](../src/enrich.rs#L663), [scheduler subjects](../src/app.rs#L522). | Retain identity through rates, histories, selection, drills and replay. Start a new series epoch after subject replacement. |
| X03 / P0 | Missing `memory.current` becomes zero. Kernel-log open failures all become Denied, including ENOENT. BPF map-ID acquisition errors become an empty list. [Cgroups](../src/inventory.rs#L239), [logs](../src/logs.rs#L27), [maps](../src/bpf.rs#L89). | Preserve zero versus missing, denied, unsupported, stopped, stale and partial results. Never imply an empty map inventory when enumeration failed. |
| X04 / P0 | Verify selects a metric from the selected issue but writes its result to the last action journal entry, without checking issue/target identity. Criteria are hardcoded independently of issue criteria. [Verification](../src/app.rs#L1796). | Bind verification to a specific issue, action and subject generation, using persisted criteria and valid pre/post windows. |
| X05 / P1 | Compact rendering falls back to legacy `View` columns; Tasks loses detailed columns, Scheduler has only CPU/busy, Memory becomes a metric list, Diagnose uses legacy observations. Sorting is also split between legacy headers and dedicated row builders. [Compact](../src/ui/views.rs#L2365), [task sorting](../src/app.rs#L387), [memory sorting](../src/app.rs#L435). | One typed table definition and subject selection per panel, with deliberate compact column priorities and numeric/unit-aware sorting. |
| X06 / P1 | Captions and data windows disagree: Dmesg says “last 10m” but uses the shared 60-second history helper. Its event-rate source counts only kmsg while the list merges other events. [Dmesg](../src/ui/views.rs#L2176), [history](../src/ui/widgets.rs#L306), [rate](../src/enrich.rs#L672). | Derive caption, time range, population and aggregation from the same query. |
| X07 / P1 | Bounded acquisition is not consistently accompanied by visible coverage: 512 cgroups, 32 PSS leaders, 4096 BPF programs. [Cgroups](../src/inventory.rs#L210), [PSS](../src/inventory.rs#L434), [BPF](../src/bpf.rs#L59). | Show sampled/retained/limited counts and age. Do not call bounded or partially visible totals host-wide totals. |

## Screen-by-screen coverage

### Dense — partial integration

Implemented: eight panel groups, CPU/memory/block summaries, IRQ and scheduler sections, tasks and shared timeline. This is not an absent screen.

Gaps: compact Dense intentionally drops several groups; the scope of that fallback needs an explicit product rule and screenshot acceptance. Device summaries show a bounded subset without a complete inventory workflow inside the panel. Healthy task folding and coverage indications remain incomplete. Apply X01–X03 to shared values and histories; test live stopped/active capture states rather than only populated demo panels. Keep key `0` mapped to Dense.

### Overview — live card defects

Implemented: five hero cards, health strip, task concerns and timeline.

Gaps: X01 disconnects real softirq and D-state data. Capture-dependent cards need an actionable source state, not an unexplained dash. Fixed card captions do not consistently communicate actual baseline/readiness. Runnable pressure calculated from visible tasks must declare that scope. Acceptance must include a normal unprivileged sample with useful cards and a separate traced sample with latency evidence.

### Tasks — detailed table present, interaction incomplete

Implemented: detailed task rows, policy/nice, runtime, wake percentiles, context-switch rates, placement, identity-aware wake history and metadata acquisition.

Gaps: sort choices derive from a smaller legacy schema; grouping is primarily ordering/repeated-label suppression rather than complete process/thread folding. The normal detail view omits some collected fields, including scheduler weight. Runtime history and some drills still need generation-safe task identity (X02). The waiting explanation lacks a real selected-CPU peer comparison. Affinity-change actor attribution needs a new event source and cannot be reconstructed from polling.

### Scheduler — waiting view is not a waiting queue

Implemented: CPU/task/cgroup modes, wake/runtime/migration selection, histories, distributions and scale controls.

Gaps: the lower panel shows up to six resident tasks and historical wake p99, not current queued tasks and wait ages. It calls every sampled `R` task “running”, although `R` includes runnable tasks. See [placement renderer](../src/ui/views.rs#L1240). Subject sorting and identity are inconsistent; an “ok” verdict cannot substitute for missing latency evidence. Scheduler configuration/context is incomplete.

Required new acquisition: enqueue-to-run lifecycle, running identity, outstanding wait age, exits and loss handling. Completed wake latency and outstanding wait age must remain separate measurements.

### Memory — mode and NUMA gaps

Implemented: six cards, slab summary, zone/NUMA metadata, hugepage/KSM fields, allocation/reclaim histories and process memory table.

Gaps: overview and slab modes follow the same rendering path; hugepage/cgroup modes replace only one subpanel. Slab displays six entries with a fixed 1434 MiB bar maximum. [Renderer](../src/ui/views.rs#L1284). NUMA collection is CPU lists and bounded zone fields, not per-node memory/locality/migration telemetry. [Collector](../src/enrich.rs#L435). PSS samples at most 32 leaders with no fair rotating coverage; missing PSS falls back to RSS for ranking, mixing measurements. Allocation/reclaim page counters do not attribute allocations to tasks or call sites.

Required: real mode-specific tables, typed slab sizes/growth, dynamic scaling, node memory/numastat deltas, explicit PSS coverage and optional allocation attribution as a separate scoped capture.

### Block — operation and identity gaps

Implemented: ten-column device table, selected throughput/IOPS/queue histories, completed latency and outstanding-request evidence, topology candidates and optional health metadata.

Gaps: diskstats IOPS/await currently count reads+writes, without including or clearly scoping out discard/flush operations. Device identity is lost (X02). The trace payload does not provide the operation detail needed to claim an outstanding request is specifically a FLUSH. Firmware/cache/FUA and physical geometry coverage is incomplete. Exact queue-to-IRQ mapping remains a driver-dependent boundary, not something topology candidates establish.

Required: operation-aware request lifecycle, reset/hotplug-safe histories, clear partition/whole-device scope, and honest health support for physical versus virtual devices.

### Syscalls — acquisition exists, decoding remains bounded

Implemented: shared latency columns, negative-return/errno display, numeric latency sorting, selected histories/distributions, event filters and capture scope. Recent selection and unit fixes should be preserved. See [focused review](SYSCALL_EBPF_REVIEW.md).

Gaps: names/argument decoding cover a small syscall subset; full flags, FD/path context and structured arguments are not implemented generally. Raw stack addresses are not symbolized user stacks. Capture-average rates need to be distinguished from rolling rates. Event-retention scope and aggregate population must be explicit when filters select different populations.

Required: architecture-tagged syscall metadata and decoding, bounded argument capture with failure/truncation states, optional symbols with matching process identity/build context. Unknown syscalls must retain their number and raw arguments.

### IRQ — placement edge cases and missing context

Implemented: seven-column hardware IRQ table, independent counters, configured/effective/observed placement, interval change events and traced handler latency.

Gaps: deriving placement from whether an affinity string parses as one integer mishandles missing masks; multi-CPU effective masks cannot drive the current single-CPU pairing reliably. Aggregate distribution and selected-CPU selection need explicit semantics. The imbalance/affinity verdict strip, timed markers on the selected plot, and RX queue/RPS/RFS context are incomplete. Polling proves an interval change, not its exact actor/time.

Required: parsed CPU sets, offline/sparse CPU handling, observed-rate distributions and selected-CPU histories. Preserve right-aligned IRQ entries across widths.

### Cgroups — advertised controls exceed implementation

Implemented: eleven columns, CPU/throttle histories, PSI/limits, quota overlay, unit metadata and reviewed actions.

Gaps: “controllers cpu mem io” is advertised but not wired to controller filters. [Controls](../src/ui/views.rs#L1797). No `io.stat` acquisition supplies the requested selected-group I/O rates. Own versus effective ancestor limits are not modeled. Tree presentation/folding and traversal coverage need completion. Missing memory becomes zero; CPU-rate availability is unnecessarily coupled to throttle-field availability.

Required: typed controller availability, independent counters, hierarchy/effective-limit evaluation and per-device cgroup I/O. A disabled controller, an unlimited limit and a zero measurement must look different.

### Modules — inventory implemented, attribution absent

Implemented: nine-column table, taint/trust-related fields, baseline/marks, interval lifecycle events and slow metadata enrichment.

Gaps: hooks are explicitly unavailable; loader/runtime attribution has no implementing event source. Baselines based on names cannot establish all same-name reload lifecycles. Missing taint metadata must not imply an in-tree/unsigned-flag-negative conclusion. Lifecycle timestamps are observations, not exact prelaunch load times. [Inventory](../src/inventory.rs#L345).

Required: explicit unknown trust metadata, generation-aware observed lifecycle, and separately scoped load/unload/loader evidence. Hook/runtime attribution needs a feasibility decision and declared kernel support before it is promised.

### eBPF — permission and feature gaps are different

Implemented: program inventory, run counts/mean runtime where stats exist, attach metadata, map dimensions and creator-UID cost grouping. This audit host denied enumeration; that does not make the tab unimplemented.

Gaps: map-ID failures still look like an empty map list (X03); enumeration caps need coverage. Program ID alone is insufficient for cache/rate identity across reuse. Creator UID does not identify current FD holders. Map occupancy, runtime distributions and helper-level costs are not generally acquired. Statistics acquisition controls should be independently understandable instead of requiring users to infer which capture owns a stats lease.

Required: generation-aware inventory, explicit stats availability/ownership, partial map enumeration, and measured support boundaries for expensive occupancy/ownership attribution. Do not invent p99 from a mean.

### Dmesg / tracing — list present, rate semantics wrong

Implemented: merged event list, severity/text filtering, follow navigation and acquisition health.

Gaps: X06 gives mismatched source/window claims. Normal message lines truncate instead of wrapping; expanded display behaves differently. Missing kmsg is classified as denied. Dedicated source controls, detailed ring utilization and a complete tracer configuration flow remain incomplete. Retention, source loss and probe loss should be separated.

Required: consistent source-aware list/rate queries, truthful time windows, readable selected messages and source-specific retention/drop metadata.

### Diagnose — workflow presentation exceeds execution state

Implemented: persistent issues, baselines/hysteresis, evidence/cause panels, action proposals and reports.

Gaps: pipeline checkmarks are static. Issue ordering comes from an ID-keyed map although the panel says “by impact”; the issue renderer does not scroll its offset to keep lower selections visible. [Renderer](../src/ui/views.rs#L2206), [engine](../src/diagnose.rs#L198). Most live rules produce generic unconfirmed cause steps; rich demo chains do not imply equivalent live correlation. Verification has X04. Report preview is global rather than clearly tied to the selected issue.

Required: actual stage/readiness state, explicit severity/impact/confidence ranking, scrolling, evidence-backed rule-specific hypotheses, correctly bound verification and selected-issue reports. Keep correlation separate from demonstrated causation.

## What is legitimately unavailable

Permission-denied BPF, stopped probes, missing kmsg, inaccessible systemd, unsupported SMART and absent symbols must have explanatory states and recovery paths. They are not all implementation defects. Conversely, static controls, absent `io.stat`/NUMA collectors, missing current-wait lifecycle and placeholder attribution require code; obtaining root access alone will not complete them.

There is no defensible completion percentage from the existing screenshot/test inventory. Acceptance must be reassessed per panel using the plan's observable gates.
