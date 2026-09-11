# Visual acceptance review

11 September 2026. Review compares the supplied diagrams with the actual Ratatui buffers in `screenshots/current`. Captures use Adwaita Mono, 12×24 cells, the reference palette and the deterministic incident clock. Full symbol/foreground buffers are regression-tested. Every view also renders at seven sizes, including 80×24 with expanded panel inspection.

| Screen | Reviewed content |
|---|---|
| Dense | Eight numbered panels; mirrored CPU history, actual core IDs, memory bars, block/IRQ/scheduler/taint, concern tasks, shared timeline |
| Overview | Five hero cards, CPU/core plots, rule-derived health, concern table and timeline |
| Tasks | Full thread table, groups/counts, policy/nice/type/uptime/placement, runtime/wakeup plots, actions |
| Scheduler | CPU/task/cgroup scope, metric/scale/histogram controls, full CPU table and histories, placement panel |
| Memory | Six cards, slab ranking, NUMA/zones/hugepages, allocation/reclaim, full process memory table and consistent unit toggle |
| Block | Ten device columns plus history, queue/device inspection, mirrored throughput and lifecycle plots |
| Syscalls | Shared demo/live summary columns, verdict/history, distinct completed events, bounded arguments/stacks, selected distribution |
| IRQ | Configured/effective/observed placement, duration, history, softirq matrix and selected rate/execution plot |
| Cgroups | Eleven columns plus CPU history, inclusive hierarchy, configuration, quota/pressure plots and action controls |
| Modules | Nine columns, full selected metadata, baseline/mark controls, separate lifecycle events and historical taint |
| eBPF | Program/owner/attachment, runtime/verdict/history, selected maps, owner costs and bounded probe controls |
| Dmesg | Distinct records, source/severity, follow/filter, event-rate history and acquisition details |
| Diagnose | Simultaneous issues, evidence/cause cards, remediation/verification and report observations |

## Intentional corrections

The diagrams contain illustrative claims that are inappropriate as live observations. The implementation preserves their screen structure while correcting those claims:

- CPU components share one clock and reconcile. One-core BPF costs remain separate from whole-host utilization.
- Completed block/syscall latency excludes outstanding FLUSH age. Mean runtime does not imply p99.
- Quota constrains runtime, not runnable waiting. Inclusive cgroup counters are not summed with descendants.
- IRQ affinity discovered by polling has an interval and an unknown actor. Driver queue candidates do not prove a completion-path cause.
- Slab growth and repeated ENOENT are separate observations. The cause remains unconfirmed without allocation/free evidence.
- Module trust and historical taint do not prove a latency cause. Fixture modules are vendor-neutral examples; unknown live metadata remains unknown.
- Program ownership is asserted for handles owned by this capture. External ownership, map occupancy and helper attribution are not fabricated.
- Empty/sparse live sources retain space; the monitor does not fabricate rows to fill a diagram. Full values and long records are available in the inspector.
- Reports contain the eight files actually written, with measured manifest sizes. The report pane presents observations from the same report generator.
- Browser window controls and raster antialiasing are excluded. Table headers/values use display-width ellipsis, and contextual keyboard controls replace diagram-only shortcuts.

The dense panel allocation and shared visual vocabulary are deliberate. This is a terminal implementation of the design, not a raster reproduction or a generic dashboard with renamed tabs.

Follow-up: Dense uses `0`, eBPF uses `b`, as requested. The shared timeline now shows current values, a growing collected-history window, and explicitly labelled procfs fallbacks while percentile capture is absent. All thirteen navigation captures were refreshed; the Dense timeline was visually inspected.

Dense IRQ/Scheduler follow-up: compact Dense now retains both panels; full Dense shows aggregate IRQ rate, labelled per-CPU softirq units, and scheduler counter values/histories. Refreshed captures were reviewed against the actual render; live compact/full text captures verify real values outside demo mode.

Task latency and IRQ alignment: Dense and IRQ values now share right-aligned columns, with right-aligned CPU matrix entries. Task latency retains sub-millisecond precision, explicit capture guidance and per-identity histories. Dense capture visually reviewed after refreshing all reference buffers.
