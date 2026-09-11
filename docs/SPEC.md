# kernwatch product and implementation specification

Status: authored from the supplied diagrams; implementation target. 11 September 2026.

This specification describes the application the diagrams depict. The current Rust implementation and its remaining gaps are tracked in [COMPLETION.md](COMPLETION.md); this document remains the acceptance target. Implementing a tab name, a generic table, or an unavailable-state message does not complete that feature.

## 1. Source and intent

The visual source of truth is the thirteen PNGs in [reference/renders-kernwatch](reference/renders-kernwatch), copied unchanged from `NetWatch btop redesign(2).zip`. Each is 2604 pixels wide; their heights vary from 1314 to 2344 pixels. They are design canvases, not established terminal cell dimensions. Window title bars and desktop window-control buttons are presentation framing and are not drawn inside the TUI.

This document preserves their panel composition, hierarchy, graph language, information density, and investigation workflows. Requirements concerning collector architecture, storage formats, supported terminal sizes, and precise keyboard conflict resolution are design decisions introduced here. Corrected numbers and causal language deliberately supersede mistakes in the diagrams. The older NetWatch network-diagnosis specification is background material, not a source of kernel-specific requirements.

kernwatch helps a Linux operator answer: what changed, which task/CPU/device is affected, what evidence explains the symptoms, what can be tried, and whether the change helped. The defining workflow is **overview → focused subsystem → linked evidence → diagnosis → dry-run/action → verification → incident report**.

### Completion levels

- **Visual parity:** every diagram has its own faithful, interactive screen using a complete deterministic incident. This is a measurable milestone, not a live release.
- **Live inspection:** real collectors populate supported panels; acquisition states and gaps remain visible. Some tracing work may still be incomplete.
- **Specification complete:** the required tracing, navigation, recording, diagnosis, action and verification behaviours below work on the declared supported Linux environment. Unavailable on a permission-restricted host is valid; an unimplemented required backend is unfinished work.

## 2. Visual system and terminal behaviour

**VIS-01 — Layout.** Use thin rounded borders, numbered cyan panel-title badges, titles embedded in borders, right-aligned metadata, and a compact footer. Distinct screens retain their distinct panels. Selected-object detail and explanatory graphs remain on the same screen as the originating table at the reference size. Opening a full-screen detail overlay is optional; it cannot substitute for these panels.

**VIS-02 — Colour roles.** Default colours are dark blue-black background `#0b1015`, light grey primary text `#c1cdd8`, readable secondary text `#8b9ead`, cyan data/accent `#41cad4`, purple secondary series, amber warning, red error, and green healthy. Final purple, red, green, border and selected-row tokens are calibrated against the supplied PNGs in the first visual implementation milestone. Data magnitude uses a series ramp; severity uses its own status ramp. Every verdict has text or an icon as well as colour. Supply a terminal-palette fallback; gradients collapse to semantic colours in that mode.

**VIS-03 — Graph language.** Use NetWatch-style braille area graphs with two horizontal samples and four vertical dots per cell, mirrored paired series where shown, graduated colour, fixed zero lines, labelled units and time axes. Small inline histories use the same renderer. A standard full-block Sparkline is not the main graph implementation. Mirrored percentages remain positive measurements drawn on opposite sides; labels identify each series. Histograms have bucket bounds and counts, not a fabricated continuous time axis.

**VIS-04 — Density.** Allocate space according to actual content and the intended visual hierarchy. Tables have bounded preferred heights with scrolling; short lists must not leave most of the screen empty while graphs/details disappear. Charts retain usable minimum heights. Core readings, verdicts, and object names have column priority over ancillary metadata.

**VIS-05 — Size contract.** Proposed reference sizes are Dense 160×52, Overview 160×68, Tasks/Memory/Cgroups/eBPF 160×60, and the other screens 160×50. These are cell-grid translations to validate against the diagrams, not claims about the original renderer. Preserve panel ordering, relative emphasis and data richness at those sizes. At 120×40, stack secondary columns and allow panel-local scrolling/focus expansion. At 80×24, show a deliberately compact summary of the current view and allow each of its panels to be focused full screen. Below 80×24, show a resize notice. Small-terminal support does not justify simplifying the reference-size application.

**VIS-06 — Containment.** Every panel receives an explicit outer, border and content rectangle. Text, graphs, axes and meters must remain in their content rectangles. Use display width, not byte length. Selected tab names, key hints, graph units and critical values cannot silently truncate. Long table values may ellipsize with a full-value detail affordance. Cause chains reflow; they do not clip their last node. Log entries and module events wrap or scroll as individual records.

## 3. Shared application shell and interaction

**NAV-01 — Stable navigation.** View order follows the latest diagram set: Dense, Overview, Tasks, Scheduler, Memory, Block, Syscalls, IRQ, Cgroups, Modules, eBPF, Dmesg, Diagnose. Resolve the conflicting diagram shortcuts to `0` Dense, `1` Overview, `2` Tasks, `3` Scheduler, `4` Memory, `5` Block, `6` Syscalls, `7` IRQ, `8` Cgroups, `9` Modules, `b` eBPF, `m` Dmesg, `d` Diagnose. The same mapping is generated into navigation, help and palette. First launch opens Dense; `--view` overrides it and later launches restore the selected view.

**NAV-02 — Chrome.** Top navigation reserves a right-hand area for recording, display-live/frozen/replay state, CPU count, and trace availability. Recording and freezing are independent states. Active view remains fully named. At narrow widths use a visible subset with overflow arrows or a view picker, rather than truncating every label or spending multiple rows on all tabs. A second strip carries a concise issue verdict and contextual drill/freeze/export actions when relevant.

**NAV-03 — Focus and keys.** Tab/Shift-Tab move panel focus; `[`/`]` cycle views. Arrows move within the focused panel, including timeline cursors. `Alt+1`…`Alt+9` focus numbered panels, avoiding the diagrams' collision between numeric tabs and numeric panel focus. Enter drills into the selected entity/evidence; Esc restores the previous route, selection, filter and time cursor. `:` opens the command palette, `/` filters, `s` opens a named sort picker, `z` folds/expands, `f` freezes the display, `r` starts/stops recording, `e` exports, `?` opens help, and `q` quits. View-specific keys must not silently shadow reserved global keys: for example syscall errors-only uses `E`, and module baseline uses `B`. Input fields and explicit action dialogs capture keys within their mode.

**NAV-04 — Context.** A route carries view, subject, panel, selection, filter, and time window/cursor. Breadcrumbs represent actual drill history. Selection is stable by entity identity, not its current row index. Changing the selected row updates associated detail, charts and contextual actions together. Back navigation preserves context.

**NAV-05 — Discoverability.** Footers show actions for the focused panel. The palette lists full commands, shortcuts, availability and reason when unavailable. Visible controls correspond to working actions; an unimplemented control is explicitly marked during development and blocks final feature completion.

## 4. Screen requirements

### SCREEN-00 — Dense

Reference: [00-dense.png](reference/renders-kernwatch/00-dense.png).

Composition: full-width CPU panel; three-column middle; full-width correlated timeline. CPU panel contains a large mirrored user/kernel+IRQ graph on the left and a vertical list of per-core mini histories on the right. Left middle column contains memory meters and block-device throughput/stall summaries. Centre contains IRQ rates plus a softirq CPU matrix, scheduler histories, and a compact taint/eBPF panel. Right contains tasks ranked by concern, with verdict badges and per-row latency histories. The timeline contains aligned CPU, CPU3 softirq, scheduler p99, completed I/O p99 and D-state tracks with event annotations.

Required details include load, run queue, PSI, CPU3 emphasis, memory categories and slab-growth sentence, read/write rates, outstanding flush age, IRQ CPU affinity, runnable count and migration context, module status, probe overhead, folded healthy tasks and task state/wchan where space permits. Dense is not a collection of equal-width tables. Panel selection opens the corresponding subsystem with its subject intact. At reference size all eight panel groups are visible simultaneously.

### SCREEN-01 — Overview

Reference: [01-overview.png](reference/renders-kernwatch/01-overview.png).

Five hero cards: scheduler wakeup p99, run queue per CPU, CPU3 softirq, CPU PSI some, and major faults. Each includes value/unit, small history, and baseline or scope metadata. Next: mirrored CPU graph with eight per-core strips beneath it. Next: five health rows (scheduler, IRQ, memory, block I/O and D-state), each with current reading, severity, history and an explanatory findings list. Then the concern-ranked task table and a shared-axis incident timeline. Export feedback includes actual path, file count and bytes.

The hero cards must not be replaced by generic host CPU/memory/load cards. Health severity is derived from the same issues shown in Diagnose. Missing trace data occupies its intended card/plot with a source-specific status and acquisition path.

### SCREEN-02 — Tasks

Reference: [02-tasks.png](reference/renders-kernwatch/02-tasks.png).

Filter strip: concern, all, running, D-state, kernel, threads; grouping by none/cgroup/user/parent. Table: task and PID/TID, state, CPU, policy, CPU%, RSS, wakeup p50/p99, voluntary/involuntary context-switch rates, wchan, verdict and 60-second history. Show counts for total, matched, displayed and folded tasks with correct semantics.

Below: selected task's type, uptime, CPU placement, effective affinity and origin when known, policy/nice/weight, runtime trend, softirq attribution if applicable, preemptions, run queue and cgroup. A paired evidence plot links worker CPU consumption to Envoy wakeup latency using the same time axis. A short explanation distinguishes observation from inference. Actions: selected CPU scheduler, associated IRQ, affinity preview, stack inspection, copy identity and watchlist. Thread folding must not double-count process and thread CPU/RSS totals.

### SCREEN-03 — Scheduler

Reference: [03-sched.png](reference/renders-kernwatch/03-sched.png).

Selectors: per CPU/task/cgroup; wakeup latency/runtime/migrations. Table: runnable count, wakeup p50/p99, switches/s, migrations/s, IRQ%, softirq%, steal%, top task, verdict and latency history. Selected CPU gets a large latency plot with onset marker, scale toggle and histogram mode. A lower runnable-task panel shows waiting durations with bars, distinguishes running from waiting, and explains affinity constraints and observed migrations. Actions drill to waiting tasks or associated IRQs and preview affinity changes. Scheduler configuration fields are shown only when actually detected; do not infer EEVDF or a tunable from kernel version alone.

### SCREEN-04 — Memory

Reference: [04-memory.png](reference/renders-kernwatch/04-memory.png).

Selectors: overview/slab/hugepages/per-cgroup. Six cards: used, available, page cache, slab, memory PSI, faults. A slab ranking with proportional bars and growth evidence comes next. NUMA detail covers node CPU membership, used memory, local/foreign activity when available, migrations, huge pages, KSM and zone watermarks. An allocation/reclaim paired graph precedes top processes by PSS/RSS, anon/file/shmem, swap, minor faults, growth and verdict.

Actions drill into slab cache, process memory and related syscall evidence; unit toggle applies consistently. Any cache-drop control begins with an impact preview and is not labelled harmless merely because it can be repeated. Memory accounting labels explicitly identify overlapping categories. Process PSS is collected on demand/at a slower cadence, with age shown.

### SCREEN-05 — Block

Reference: [05-block.png](reference/renders-kernwatch/05-block.png).

Selectors: devices/partitions/device-mapper/mounts and trace latency/iostat. Device table contains scheduler, IOPS, read/write throughput, mean completed latency, completed p99, in-flight count, utilization, verdict and history. Lower left: selected device model/firmware, mount/filesystem, queue configuration, cache/FUA, telemetry availability, and outstanding requests. Lower right: mirrored throughput plus IOPS, completed latency and queue-depth histories.

Show outstanding FLUSH age separately from completed p99; a pending request cannot be part of completed latency statistics. Evidence can link device → queue → IRQ → CPU → waiting task. Distinguish handler duration, request completion latency, and request age. SMART data is optional capability-dependent enrichment, not fabricated from diskstats.

### SCREEN-06 — Syscalls

Reference: [06-syscalls.png](reference/renders-kernwatch/06-syscalls.png).

Trace controls select all/PID/cgroup, error or latency filters, event source, rate and measured overhead. Breadcrumbs retain memory/slab/process context. Table: syscall, calls/s, errors/s, top errno, mean/p99/max completed duration, top caller, verdict and history. A bounded live event pane shows timestamp, decoded arguments, return value, errno and duration for the selected process. Below: latency histogram for the selected syscall plus a short interpretation and scheduler drill.

Support start/stop capture, pause/resume display, errors-only, user stack and trace export. Lost events and partial decoding are explicit. syscall duration is never renamed scheduler wakeup latency. ENOENT rate alone does not prove slab growth.

### SCREEN-07 — IRQ / softirq

Reference: [07-irq.png](reference/renders-kernwatch/07-irq.png).

Top verdict summarizes imbalance and affinity change evidence. Hardware IRQ table: vector, name/type, configured/effective affinity, observed CPU distribution, rate, handler duration p99, verdict and history. Middle softirq matrix shows each vector's execution time by CPU, consistent totals, and selected CPU emphasis. Bottom paired history shows selected IRQ rate and associated softirq execution percentage on a shared time axis, with affinity-change markers.

Actions inspect/edit affinity, RX queue configuration, RPS/RFS where supported, and open selected-CPU scheduling. Interrupt counts are not execution time. An affinity change discovered by polling has a detection interval; exact time and actor require event evidence. Do not attribute a change to irqbalance from proximity alone.

### SCREEN-08 — Cgroups

Reference: [08-cgroups.png](reference/renders-kernwatch/08-cgroups.png).

Tree/flat/throttled/pressure selectors and controller filters. Hierarchical table: task count, CPU%, cpu.max, throttling, memory/current limit, CPU/I/O PSI, effective cpuset, verdict and CPU history. Expansion preserves hierarchy and avoids summing parent-inclusive values with children.

Selected group detail includes quota/period, usage and throttling deltas with windows, effective cpuset, weight, memory limits, PIDs and I/O rates. Paired charts compare runtime against quota and throttled duration over the same window. Explanation distinguishes contention, own quota exhaustion and ancestor limits. Actions open tasks, preview cpuset/quota changes, inspect unit configuration, pressure history and copy path. Task affinity and cgroup cpuset remain separate fields.

### SCREEN-09 — Modules

Reference: [09-modules.png](reference/renders-kernwatch/09-modules.png).

Filters: all/out-of-tree/unsigned/new/unused; baseline timestamp and trust verdict. Table: module, size, refs, dependencies, signing status, origin, observed load time, known hooks and verdict. Selected module: file, vermagic, source version, loader attribution when captured, parameters, symbols/hooks when discoverable, and measured runtime attribution only when supported. Bottom panel explains taint flags and presents one module event per record.

Support inspect, baseline acceptance, suspicious marker and export. Baseline acceptance records operator intent and does not clear historical kernel taint. Unknown signing/loader/hook data is unknown; `/proc/modules` alone cannot establish it. The fixture uses explicitly fictional vendor-neutral module identities.

### SCREEN-10 — eBPF

Reference: [10-ebpf.png](reference/renders-kernwatch/10-ebpf.png).

Filters by owner and program type; JIT/statistics/privilege status. Program table: ID/name/type/attachment, owner attribution, runs/s, mean runtime, CPU%, maps, verdict and history. Selected detail: load observation, translated/JIT size, instruction count, verifier information if captured, map types and occupancy/drop metrics where measured, helpers and runtime histogram when available. Owner overhead bars and kernwatch's own probe history follow. A final panel launches bounded one-shot probes for off-CPU time, run-queue latency, softirq execution, block latency and failed opens.

List programs with a real BPF backend. Runtime metrics require statistics availability and a measured interval. Mean runtime counters do not imply p99 or helper-level attribution. Map occupancy is not consumer lag unless a measurement defines that relationship. Probe launch shows target, duration, acquisition cost/limits and status; cancel detaches only kernwatch-owned probes. General program detach requires explicit target/action review.

### SCREEN-11 — Dmesg / tracing

Reference: [11-dmesg.png](reference/renders-kernwatch/11-dmesg.png).

Severity, warning/error counters and source controls (kernel log/ftrace/audit where supported). Main log uses distinct timestamp, source, severity and message fields; long records retain their boundaries. Log-rate chart annotates incident events. Tracing panel lists actual tracer, enabled events, buffers, event rate, dropped events and utilization. A correlation summary links event IDs and intervals to Diagnose.

Support filter, follow, freeze, jump to source, correlate and export. Default order is newest last with follow enabled, matching the diagram. kernwatch-generated findings remain source-labelled and are not presented as native dmesg records. Unavailable optional sources retain an explanatory state.

### SCREEN-12 — Diagnose

Reference: [12-diagnose.png](reference/renders-kernwatch/12-diagnose.png).

Pipeline strip: collect → correlate → rank → verify → report, with real progress/status. Upper left: issues ranked by impact, onset, affected subject, severity and related/informational findings. Upper right: selected issue's cause graph with distinct observed nodes and hypothesised edges, followed by evidence/source/time/weight rows. Lower panels: ranked remediation, explicit verification criteria/status, and report preview.

At reference size issue list, cause/evidence, remediation and report are visible together. Node cards reflow into additional rows at intermediate widths, then a vertical chain in compact mode. No inference becomes confirmed just because a weighted score is high. Selecting an issue changes all dependent panels. Evidence drills to its source and captured time; remediation Enter opens a dry-run; verification records measured before/after windows. Reports use the same Issue/Evidence/Action/Verification objects as the UI.

## 5. Measurement and fixture contracts

**DATA-01 — Typed values.** Domain models retain raw units, source, subject, interval start/end, count, freshness and quality. Formatting occurs at render/export boundaries. Acquisition states include warming-up, available, stale, permission-denied, unsupported, stopped and error; unknown is not zero. Numeric sorting uses raw values.

**DATA-02 — Identity.** Task keys include boot identity, PID/TID and start time; CPUs retain actual IDs through hotplug; devices, cgroups, IRQs, modules and BPF programs have stable keys and disappearance handling. A row must not silently become another process after PID reuse.

**DATA-03 — Time.** Use monotonic timestamps for event ordering, rates and durations, with an explicit mapping to wall time. All charts for one incident use a shared cursor/window. Preserve gaps and collector delays. Do not resample unrelated series to imply a simultaneous onset. Missing history is blank, not synthetic zero.

**DATA-04 — Percentages.** CPU metrics declare one-core or whole-host normalization. CPU component percentages and per-core totals reconcile within rounding for the same interval. Softirq worker runtime and vector execution time can overlap; never add overlapping measurements as disjoint components. PSI values retain some/full and avg10/60/300 semantics. System-level CPU full is not treated as a useful zero-health reading. [Kernel PSI reference](https://docs.kernel.org/accounting/psi.html)

**DATA-05 — Quota and distributions.** Quota represents allowed runtime over a period; waiting is separate. Cgroup usage/throttling labels include scope and delta window, and ancestor constraints are inspected. Histograms include event counts and approximation/bucket metadata; don't average p99s or mix completed with outstanding events. [Kernel cgroup v2 reference](https://docs.kernel.org/admin-guide/cgroup-v2.html)

**DATA-06 — Deterministic incident.** One versioned fixture owns entities, counters, distributions, timeline, issues and action outcomes for every screen. Capture epoch, startup baseline, IRQ-placement observation, later wakeup-latency increase, pending flush and independent slab growth all share one clock. Provide pre-incident, incident and post-experiment states; report expected outcomes as hypotheses until the fixture's verification evidence is evaluated.

Use CPU3 busy 98%, worker runtime 91%, Envoy workers 4% and 3%, idle 2% over the common fixture window. Choose disjoint host CPU components that reconcile with all per-core totals, replacing the diagrams' inconsistent 38%+27% aggregate. Retain p99 wakeup 18 ms and outstanding flush age 4.1 s, while completed I/O p99 remains a distinct 1.9 ms metric. Include a separate quota-throttling fixture so that screen's graph and explanation are genuinely exercised. Retain slab growth but leave allocation origin unconfirmed unless the fixture contains unique-name allocation/free evidence.

## 6. Collection and capability requirements

**COL-01 — Basic Linux collectors.** procfs/sysfs provide CPU components, process/thread runtime and state, memory/vmstat/PSI, diskstats, IRQ counts/affinity, cgroup v2, modules and taint. Device/CPU/NUMA relationships are collected as topology. Process smaps, module metadata and storage telemetry use slower/on-demand collectors. Sources have individual last-success times and errors; one blocked source cannot stall others or the UI.

**COL-02 — Scheduler tracing.** Correlate wakeup/switch/migration/exit events into per-task, per-CPU and per-cgroup distributions and time series. Handle repeated wakeups, runnable preemption, task exit, lost events, map eviction, migration and PID reuse. Trace availability is runtime-discovered; event format and kernel configuration vary. [Kernel event tracing reference](https://docs.kernel.org/trace/events.html)

**COL-03 — IRQ/block/syscall tracing.** Measure IRQ/softirq entry-to-exit duration with per-CPU context and loss handling. Track block request identity and issue/completion lifecycle, including requeue/partial completion/merge semantics on supported kernels. Track syscall enter/exit per thread, return/error and duration; capture selected arguments/stacks with explicit bounds. Degrade affected metrics if pairing integrity is lost.

**COL-04 — BPF integration.** Ship a compiled probe backend with reproducible build instructions and a tested compatibility matrix. Discover BTF, attach points, helper support, access and runtime-stat availability instead of claiming support based only on a version number. Separate BPF program inventory from kernwatch's measurement probes. High-rate aggregation happens near acquisition; userspace receives bounded aggregates and selected events. Ring-buffer loss is observable; it is not hidden by successful reads. [Kernel BPF ring-buffer reference](https://docs.kernel.org/bpf/ringbuf.html)

**COL-05 — Deployment modes.** Unprivileged inspection, privileged tracing, and offline replay share the same renderer/domain types. Production collectors do not inject demo values. Capability failures have exact reasons and acquisition guidance. Supported architectures/kernel configurations are declared after probe compatibility work, with 6.12.x from the diagram as an initial test target rather than a universal compatibility promise.

## 7. Recording, diagnosis and actions

**REC-01.** Retain bounded timestamped histories for 60-second panel plots and a ten-minute incident timeline. Aggregate long-lived baseline statistics separately. Recording writes a versioned stream of snapshots/events with framing, capture metadata and loss counters; replay supports pause, seek, speed and cross-view cursor consistency. Freeze captures an immutable display/incident view while acquisition may continue. Record, freeze, live-follow and replay are distinct state fields.

**REC-02.** Export a manifest, report.md, report.json, environment, typed observations/series, issue graph, selected trace/log evidence and action/verification journal. Report the actual contents and sizes. Never imply eleven files or fourteen megabytes merely because the diagram used those numbers. Recover partial recordings and surface export failures without corrupting the terminal.

**DIAG-01.** Issue objects own subject, rule, severity, onset, latest evidence, lifecycle, candidate causes, remediation and verification. Baselines have readiness, identity scope, sample count and persistence; sustained deviations and hysteresis prevent single-sample noise from opening incidents. Retain distinct observed symptoms even when grouping them under a candidate cause.

**DIAG-02.** Required rule families: per-CPU contention, scheduler wakeup delay, blocked-task/outstanding-I/O age, IRQ-placement imbalance, cgroup quota throttling, memory/slab growth, module trust drift and high instrumentation overhead. Each names required evidence, missing discriminating tests and confounders. Confidence is evidence coverage/support with a documented rule, not an invented probability. Correlation alone never proves the IRQ-to-storage chain.

**ACT-01.** Actions are typed: inspect, probe, preview, apply, revert, verify, export. Every apply action specifies target identity, preconditions, capability, exact before/after values, scope, risks and verification. UI selection/Enter opens the concrete dry-run; an explicit apply command confirms it. Dry-run performs no host mutation. Numeric remediation choices are scoped to the action dialog so they cannot hijack global tab keys.

**ACT-02.** Implement supported IRQ/RPS, task affinity and cgroup quota/cpuset changes with revalidation, audit journal and rollback where possible. Verify effective values; account for irqbalance and service managers changing settings afterward. Mark queue reconfiguration, cache dropping, task termination and program detach according to their actual impact; do not advertise every operation as reversible. Failed/stale-target/partial actions remain inspectable and cannot be reported successful. General privileged changes are never executed by the renderer.

## 8. Acceptance contract

**QA-01 — Visual evidence.** Every screen gets a reference-size capture from the actual Ratatui application with a fixed terminal font, cell size, palette, fixture and clock. Review side by side with its PNG. All named panels, columns, graphs, verdicts, metadata and visible actions must be present; documented corrections are the only intentional departures. Automated cell snapshots and layout assertions supplement this review, not replace it. Browser chrome and raster antialiasing are excluded from pixel comparisons.

**QA-02 — Interaction evidence.** Required tours: Dense → CPU3 scheduler → Envoy → cgroup → back; memory → slab → selected process syscalls → back; block → queue IRQ → scheduler; log event → diagnosis evidence → dry-run → fixture verification → report. Verify focus, selection, filter, entity and time cursor preservation throughout.

**QA-03 — Technical evidence.** Parser fixtures, arithmetic invariants, timeline alignment, trace pairing/loss cases, entity churn, numeric sorting, baseline readiness, action dry-run/rollback and replay recovery are tested. Privileged integration tests run in a disposable Linux environment with controlled workloads. Read-only host samples cannot validate tracing or remediation.

**QA-04 — Responsiveness.** Proposed targets: input-to-frame p95 below 50 ms at 160×60; collector stalls cannot block input; render and collector costs measured separately. Histories, process caches, trace maps, queues and recording buffers have explicit limits. Benchmark 10,000 tasks, 256 CPUs, 1,000 cgroups and high-rate trace overload; use virtualization/aggregation and show loss. Publish measured overhead for probes on/off rather than promising the fixture's overhead figures.

**QA-05 — Honest completion.** Track each screen on visual, interaction, live-data and evidence correctness independently. A green cargo test or an unsupported-state panel cannot turn an incomplete column into a completed requirement. Specification-complete release requires all mandatory rows to pass and limitations to be environmental or explicitly scoped optional enrichment.
