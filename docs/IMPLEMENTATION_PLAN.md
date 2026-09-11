> This is the original implementation plan. The delivered implementation and current acceptance evidence are tracked in [COMPLETION.md](COMPLETION.md) and [VALIDATION.md](VALIDATION.md).

# kernwatch implementation plan

11 September 2026. Target: [SPEC.md](SPEC.md). Reference set: [thirteen original diagrams](reference/renders-kernwatch).

## 1. Assessment of the current prototype

The previous implementation established a Rust executable, basic procfs/sysfs collection, a synthetic fixture, terminal lifecycle handling, and simple exports. It did not implement the mockup design. Most screens share `ui::view()` and `ui::detail()`: one table and a notes paragraph. The central model is formatted `Vec<Vec<String>>`, and application state has one global selected row. This architecture cannot support the linked, subject-specific panels in the diagrams without replacement.

The nine tests establish limited parser, input and rendering properties. The render test largely checks that a title exists and that drawing does not panic. It does not establish panel fidelity, graph correctness, data coverage or a working investigation workflow. The prior claim of thirteen implemented views overstated completion.

### Gap inventory

| Screen | Present | Missing to meet the diagram/spec |
| --- | --- | --- |
| Dense | A host sparkline and small tables | Mirrored component graph, per-core histories, memory meters, softirq matrix, scheduler/taint panels, concern verdicts and row histories, correlated timeline |
| Overview | Three generic cards, CPU sparkline, task list | Five diagnostic cards, baselines, mirrored CPU plot, health strips, per-core histories, annotated incident timeline |
| Tasks | Six-column table and generic detail | Full columns, concern modes, grouping/threads, persistent selection detail, paired causal-evidence plots, contextual actions |
| Scheduler | CPU busy table | Run queue, wakeup distributions, migrations/IRQ/softirq columns, latency plot/histogram, runnable waiting bars and affinity context |
| Memory | Memory counters and PSI prose | Cards, slab ranking/growth, NUMA and zone details, alloc/reclaim plot, PSS process table and drills |
| Block | Disk rates/in-flight table | Device inspector, queue/mount/telemetry, completed distributions, outstanding age, four histories and queue-to-IRQ links |
| Syscalls | Demo table; unavailable live | Real tracing, selectors, decoded stream, histogram, trace controls, loss/overhead, cross-view breadcrumb |
| IRQ | IRQ count/affinity table | Duration tracing, softirq time matrix, paired graph, change events, queue/RPS context and actions |
| Cgroups | Flat enumeration | Foldable hierarchy, counters/pressure/limits, quota and throttle charts, selected-group context/actions |
| Modules | Loaded names/size/state | Trust baseline, signing/origin, event timing, selected module/hook details, taint explanation |
| eBPF | Demo owner table; unavailable live | Program inventory, attachment/map details, measured runtime, owner graphs, bounded one-shot probes |
| Dmesg | Repeated command output | Source/severity controls, follow state, independent log stream, rate graph, trace health, linked event correlation |
| Diagnose | Four-row generic table and prose | Issue browser, cause graph, evidence table, ranked remediation, verification pipeline, report preview |
| Shared shell | Wrapped tab labels and generic footer | Compact status chrome, panel focus, command palette, context routes, recorder/replay state, named sorting, contextual actions |

### Keep, adapt, replace

Keep Cargo setup and terminal lifecycle as a starting point. Keep existing parser tests and CPU guest-accounting correction. Adapt basic collectors behind typed interfaces with fixtures, per-source timestamps, identity handling and timeouts. Adapt exports to structured domain objects.

Replace string-table domain models, per-view prose-as-functionality, global row-only state, synthetic display-only history, and the generic view renderer. Replace whole-dmesg subprocess polling with a bounded streaming collector. Keep a prototype tag/copy before restructuring if version control is introduced; do not overwrite unrelated NetWatch or SysWatch projects.

## 2. Reuse from the existing Rust applications

Inspect and extract small components with their tests and licence attribution; do not introduce a dependency on the entire network application.

| Source inspected | Useful component | Adaptation boundary |
| --- | --- | --- |
| `netwatch/src/graph.rs` | `area_graph_with`, `render_mirrored_with_max`, `resample_to_window`, `Ramp`, time-axis and palette helpers | Preserve dot geometry and themes; add typed, timestamped series and explicit missing-sample handling |
| `netwatch/src/ui/dense/paint.rs` | Panel border metadata, meters, miniature histories, gradient roles | Make reusable kernwatch widgets with owned rectangles; do not transplant NetWatch's networking grid |
| `netwatch/src/theme.rs` | Semantic status/data/UI tokens and terminal theme | Use kernwatch reference palette without confusing severity and data series |
| `netwatch/src/diagnose/issue.rs` and `engine.rs` | Typed evidence, issues, clock injection, lifecycle | Introduce kernel subjects/rules; no network-specific causes or actions |
| `netwatch/src/diagnose/baseline.rs` | Readiness and persisted baselines | Key by boot/topology/configuration scope; reset/invalidate correctly |
| `netwatch/src/diagnose/remediation.rs` | Host abstraction and action journal | Define kernel-specific preconditions, rollback and verification; do not reuse resolver mutations |
| `syswatch/src/recording.rs` | Versioned framed records and partial-tail recovery | New kernwatch schema/magic; do not imply compatibility with `.swr` |

## 3. Intended code structure

```text
src/
  main.rs                         CLI and process/terminal lifecycle
  app/                            per-view state, focus, routes, commands
  domain/                         typed subjects, metrics, topology, snapshots
  history/                        timestamped series, windows, histograms, events
  collectors/                     independent procfs/sysfs/log collectors
  tracing/                        probe control, ingestion, capability detection
  diagnose/                       issues, rules, baselines, causes, verification
  actions/                        plans, host adapter, journal, rollback
  recording/                      capture, replay, manifest, report generation
  ui/
    chrome.rs                     view strip, status, verdict, footer
    layout/                       one layout computation per view
    widgets/                      braille graphs, meters, tables, trees, chain
    views/                        thirteen dedicated renderers
    overlays/                     palette, sort/filter, drill, action, help
fixtures/
  incident/                       versioned entity/event/metric fixture
  procfs/                         parser fixtures and malformed/permission cases
  traces/                         event correlation and loss fixtures
  scenarios/                      quota, recovery, churn and degraded capture
probes/                           reproducible compiled BPF sources/build
examples/                         deterministic screen capture tools
 tests/                           layout, workflow, recording and integration
```

Data flow: acquisition → typed observations/events → bounded history and issue engine → immutable view model → dedicated renderers. UI commands return typed intents to the application layer. Renderers never read files, run commands, apply host changes or fabricate observations. Recording/export consume the same typed objects used to build the screen.

## 4. Ordered delivery milestones

Effort labels indicate relative complexity, not promised calendar dates. Each milestone ends with concrete evidence. Complete visual and backend gates separately; visual parity is an intermediate deliverable, not an excuse to drop live tracing.

### M0 — Establish measurable references (small)

Dependencies: none. Requirements: VIS-01…06, QA-01.

1. Preserve all thirteen original PNGs with archive checksum and provenance.
2. Record per-screen panel order, column names, title metadata, action hints and data series from SPEC section 4 in a tracking matrix.
3. Calibrate font/cell dimensions and palette using an actual terminal capture. Translate the proposed cell grids against the different PNG aspect ratios. Record any revised reference sizes explicitly.
4. Add deterministic `--demo --view --size --at` capture tooling that captures the rendered terminal buffer and a real terminal image under the same font/palette. Avoid relying only on plain-text output.
5. Save the prototype's captures alongside references as the starting gap evidence.

Exit: reproducible side-by-side captures and an explicit fidelity checklist for every screen. No claim of pixel-perfect matching based on a canvas with different font/scale.

### M1 — Typed domain, coherent fixture and history (large)

Dependencies: M0. Requirements: DATA-01…06, REC-01 foundations.

1. Replace `View.rows` as the source of truth with typed task/CPU/memory/device/IRQ/cgroup/module/program/log structures.
2. Introduce `Metric<T>`/availability, unit, normalization, source, interval and sample-count metadata; entity keys; injectable clock.
3. Add per-entity timestamped histories, histogram buckets, gap representation and topology links. Rendering reads a selected immutable time window.
4. Build a complete incident fixture containing all table rows and actual underlying samples for every graph, rather than inventing each screen independently.
5. Supply pre-onset, incident, recovery, quota-throttled, missing-trace and event-loss scenarios. Correct host/per-CPU arithmetic and distinguish overlapping accounting.
6. Introduce per-view state and stable selections, keeping parsing and fixture changes independent of rendering details.

Exit: invariant tests establish CPU/overhead arithmetic, counts/folding, shared event times, consistent cross-view identities, histogram windows and unknown states. The same selected subject can be resolved by every relevant view.

### M2 — NetWatch-quality visual primitives and shell (large)

Dependencies: M1. Requirements: VIS-01…06, NAV-01…05.

1. Extract/adapt NetWatch's braille, mirrored graph, ramp, meter and panel-border primitives with tests.
2. Implement a metric card, single-row history, health strip, fixed/log axes, histogram, stable table, foldable tree, paired timeline, and semantic verdict badge.
3. Add a cause-chain widget supporting horizontal, multi-row and vertical layouts with edge labels.
4. Implement compact chrome with reserved status space, active-label visibility, verdict strip and contextual footer.
5. Implement global command registry, panel focus, palette, named sorting, filters, routes and breadcrumb/back state. Resolve global/contextual shortcut collisions centrally.
6. Test widgets in zero/one/small-cell rectangles, Unicode/CJK content, empty data, missing history and theme fallback.

Exit: widget gallery demonstrates the same visual vocabulary as the diagrams. Every widget respects its rectangle; graphs include real axes/units; no large plain Sparkline remains as a stand-in.

### M3 — Dense and Overview fidelity (large)

Dependencies: M2. Requirements: SCREEN-00/01, QA-01.

1. Implement separate layout computations for Dense and Overview with panel-minimum/preferred sizes and breakpoint policies.
2. Recreate Dense's asymmetric eight-panel structure, CPU components/core histories, memory meters, IRQ matrix, scheduler/taint summaries, concern table and bottom timeline.
3. Recreate Overview's five diagnostic hero cards, CPU/core panel, health histories/findings, task table and timeline.
4. Bind values, inline histories, verdict sentences and timeline events to the shared fixture/time cursor.
5. Implement panel focus and cross-view drill intents; full backend actions are later milestones.
6. Capture both views at reference, 120×40 and 80×24 sizes. Review against the original PNGs before extending the visual system to other views.

Exit: all required panel groups are visible and recognisable at reference sizes, with comparable relative hierarchy/density. No oversized blank table regions; no missing diagnostic cards; no timeline substitute made of prose.

### M4 — Tasks, Scheduler and IRQ investigation (large)

Dependencies: M3. Requirements: SCREEN-02/03/07, NAV-04.

1. Implement the three dedicated layouts and full table schemas.
2. Implement task/process/thread grouping and filters; selection-linked metadata, paired evidence plots and contextual actions.
3. Implement scheduler selectors, latency curve/histogram, runnable wait bars and affinity explanation.
4. Implement IRQ table, vector-by-CPU execution-time matrix, paired rate/time chart and change markers.
5. Wire CPU → task → IRQ and reverse drill paths with shared cursor, stable selection and breadcrumb restoration.

Exit: a deterministic tour follows CPU3 contention through all three screens and back; each screen's selected-object panels update together. Each gets a reviewed reference-size capture.

### M5 — Memory, Block and Cgroups (large)

Dependencies: M4. Requirements: SCREEN-04/05/08.

1. Build Memory cards, slab bars, NUMA/zone inspector, alloc/reclaim graph and per-process PSS table.
2. Build Block table, device inspector, four linked histories and outstanding-request treatment.
3. Build Cgroups tree, selected group inspector, runtime/quota and throttling charts, pressure and effective-placement context.
4. Add memory → slab → process and device → queue → IRQ routes. Add cgroup → tasks and constraint inspection.
5. Validate parent/child aggregation, overlapping memory fields and pending-versus-completed I/O labels in the fixture.

Exit: reference captures for all three views; quota-throttling and pure contention scenarios yield different graphs/verdicts; no ownership/cause is inferred from the wrong metric.

### M6 — Syscalls, Modules, eBPF and Dmesg (large)

Dependencies: M5. Requirements: SCREEN-06/09/10/11.

1. Build Syscalls trace controls, full table, bounded decoded event stream, histogram and breadcrumb.
2. Build Modules filter/baseline table, selected details, trust metadata and individual event records.
3. Build eBPF program/map/owner panels, runtime distributions, overhead bars and one-shot probe UI.
4. Build Dmesg severity/source/follow controls, wrapped record list, log-rate graph, trace-health panel and correlation links.
5. Model probe states and capability reasons in the UI. Keep unimplemented live controls explicitly tracked, not quietly removed from scope.

Exit: four reviewed visual captures and interaction tests for selections, stream pause, filters, log follow and probe lifecycle using a deterministic backend.

### M7 — Diagnose, action previews and report parity (large)

Dependencies: M6. Requirements: SCREEN-12, DIAG-01/02, ACT-01, REC-02 foundations.

1. Introduce Issue/Evidence/Cause/Remediation/Verification objects and baseline readiness/lifecycle; fixture reports use these objects.
2. Implement issue list, cause graph, evidence rows, remediation list, verification strip and rendered Markdown preview in their proper panels.
3. Distinguish observed nodes from hypothesised links. Reflow cause nodes rather than reducing them to a table or clipping the final node.
4. Implement evidence-source/time drill, action dry-run dialog and explicit fixture verification transitions.
5. Generate report preview and exported report from the same objects, with proper headings, issue grouping, uncertainty and source links.

Exit: **visual parity milestone**: all thirteen screens have reviewed captures and their visible inspection workflows work in demo/replay fixtures. Diagnose shows the correct composition. This milestone is explicitly not specification-complete live software.

### M8 — Complete unprivileged collection and topology (large)

Dependencies: M1; integrate into the screens after M3–M7. Requirements: COL-01, DATA-01…05.

1. Split `Collector::sample` into independently scheduled sources, with timestamps, bounded work, deadlines and errors.
2. Add process/thread identity using start time; CPU hotplug IDs; device/cgroup lifecycle; namespace/host visibility metadata.
3. Expand CPU/vmstat/PSI, slab, NUMA, zone, process status/schedstat/context-switch and smaps collection as appropriate.
4. Expand block metadata, queue/IRQ mapping, cgroup tree/counters/limits and module signing/taint enrichment. Show source-specific uncertainty where attribution is absent.
5. Replace per-second `dmesg` spawning with a bounded source-aware log reader and optional capability-dependent sources.
6. Use slow/on-demand schedules for expensive details and render data age. Add measured source timing and monitor overhead.

Exit: screen values compare against source snapshots under controlled load; permission-denied, disappearing entities, hotplug and counter reset cases are covered. Basic live mode is useful without collapsing the diagram panels.

### M9 — Real tracing and BPF inventory (very large; compatibility risk)

Dependencies: M8; M4–M6 are the consumers. Requirements: COL-02…05.

1. Perform a backend spike: choose the Rust loader/BPF build approach after demonstrating scheduler attach, map reads, detach and kernel-feature discovery on the initial test environment. Record toolchain and architecture constraints before committing probe interfaces.
2. Implement BPF program inventory and runtime-stat capability handling separately from measurement probes.
3. Implement scheduler wakeup/switch/migration/exit correlation and histogram aggregation; validate against a controlled CPU-affinity workload and independent trace evidence.
4. Implement IRQ/softirq duration aggregation and CPU/vector histories; verify count versus time semantics and loss behaviour.
5. Implement block request lifecycle tracking with supported-kernel fixtures for requeues, merges and partial completions; separate pending ages from completed histograms.
6. Implement syscall enter/exit correlation, bounded decoding, return errors, PID/cgroup filters, selected stacks and one-shot duration limits.
7. Add optional allocator/path evidence needed for slab attribution; unique-path miss count alone is not proof of allocation ownership. Leave the cause unconfirmed when evidence cannot establish it.
8. Implement overload handling, sample-count/loss reporting, owned-probe cleanup, resource caps, cancellation and probe timing/overhead.

Exit: real privileged integration tests exercise each required probe in a disposable VM. Permission failures have exact reasons. Tests cover loss, identity reuse, exit, map pressure and shutdown. Returning an unavailable placeholder on all hosts does not pass.

### M10 — Recording, replay and live diagnostic engine (large)

Dependencies: M8/M9 and M7. Requirements: REC-01/02, DIAG-01/02.

1. Implement bounded ten-minute histories and versioned capture records with source quality and loss metadata.
2. Implement recording independent of freeze, atomic incident snapshots, replay seek/speed and cursor-driven multi-view rendering.
3. Add baseline persistence/readiness and the required kernel rule families, with hysteresis and lifecycle transitions.
4. Generate timeline events, cross-view verdicts and issue evidence from one evaluator. Do not independently author diagnosis prose in renderers.
5. Export complete manifests and evidence bundles with real counts/bytes; test partial writes, truncated records and unavailable artifacts.

Exit: record a controlled incident, replay it offline, navigate its evidence and export the same issue values and timestamps. A partial recording recovers its valid prefix. Trace loss prevents unjustified certainty.

### M11 — Live actions and measured verification (very large; host impact)

Dependencies: M9/M10. Requirements: ACT-01/02.

1. Implement typed action plans and host interfaces for supported affinity, RPS and cgroup changes.
2. Revalidate target identity, capabilities, current configuration and preconditions after the user reviews a dry-run and before application.
3. Journal original/effective values and the action's exact outcome; implement rollback when valid and report partial/stale outcomes.
4. Account for daemon/service-manager overrides. Separate temporary placement from persistent unit configuration.
5. Implement verification using explicit pre/post windows and sufficient samples, with passed/failed/inconclusive outcomes.
6. Implement destructive actions only with their specified concrete review and semantics; never call all actions reversible or run them on the development host as a test.

Exit: disposable-host tests establish no-write dry-run, successful application, effective-value checks, failed apply, stale targets, rollback and verification failure. The UI, timeline and report agree on what was actually done.

### M12 — Fidelity, performance and release audit (medium/large)

Dependencies: all prior gates. Requirements: QA-01…05.

1. Repeat the thirteen-view capture tour under the declared terminal configuration and all specified sizes; compare against references and review approved departures.
2. Run required investigation tours in demo, recorded and live modes. Verify cursor/selection/focus restoration.
3. Stress entity counts and trace overload; measure input p95, renderer timing, CPU/RSS, source costs and probe overhead. Fix unbounded allocation or queue behaviour.
4. Test terminal restoration on normal exit, errors and supported signal paths; test inaccessible sources and headless export.
5. Publish installation/probe-build instructions, actual kernel/architecture support, capability guidance, action behaviour and measured limits.
6. Mark each requirement's visual/interaction/live-data/evidence columns independently. Release only after mandatory failures are closed.

Exit: specification-complete artifact with reproducible captures, test reports, controlled-host trace/action evidence and explicit environmental limitations.

## 5. Validation strategy and delivery discipline

Each implementation change should be a reviewable unit: domain/fixture, one widget family, one screen, one collector, or one probe. Do not combine a sweeping UI rewrite with privileged action code. Update the requirement tracker and capture artifact for every screen change.

Golden-buffer tests must assert panel anchors, borders, graph axes and selected-row values, not merely the view title. Widget tests verify sample-to-cell mapping, scale clipping, constant/zero/missing series, histogram buckets and Unicode widths. Workflow tests verify subject/time preservation. Live parsers use injectable roots/readers and deterministic counters. Probe tests use captured event fixtures plus controlled-host integration. Action tests use a fake host plus disposable-VM verification.

Performance budgets from SPEC are targets to measure, not assumed achievements. Expensive collectors need cancellation/deadlines and independent scheduling; scrolling and typing must remain responsive while a source fails. Do not run performance experiments that change the user's host placement, quotas, probes or caches as part of visual development.

## 6. Required completion ledger

Create one row for SCREEN-00 through SCREEN-12 and for each VIS/NAV/DATA/COL/REC/DIAG/ACT/QA requirement. Fields: owner module, implementation change, visual capture, interaction test, live source/probe, evidence test, status, remaining blocker. Use `not started`, `partial`, `validated`, or `environment unavailable` with a concrete reason. `Environment unavailable` only applies once the backend is implemented and validated elsewhere.

The immediate next implementation unit is **M0–M2, then a faithful Dense screen**. Its review artifact must demonstrate the CPU split graph, coloured per-core histories, memory bars, IRQ matrix, scheduler/taint panels, rich task rows and shared timeline together. A generic dashboard with the right tab names is not an acceptable substitute.
