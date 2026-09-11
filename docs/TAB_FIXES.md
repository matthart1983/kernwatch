# Tab gap fixes — 12 September 2026

This change addresses the concrete correctness and wiring defects from the [audit](TAB_GAP_AUDIT.md). It does **not** mark the entire [16-package plan](TAB_IMPLEMENTATION_PLAN.md) complete.

## Implemented

- Overview uses the live aggregate softirq CPU-time metric, with the appropriate source label. D-state now has a current measurement as well as a history. Runnable counts declare visible-task scope.
- Device metadata retains sysfs identity; a new device generation clears name-keyed histories. BPF program tag/load identity invalidates cached runtime deltas and helper metadata. Task runtime and Scheduler task histories include start ticks in live mode.
- Missing cgroup memory remains unknown, including through serialization. CPU usage no longer depends on throttle-counter availability. Missing kmsg is distinct from permission denial. Failed BPF map-ID acquisition no longer appears as zero maps; capped program enumeration is partial and cannot generate a complete total.
- Action previews from Diagnose retain issue, subject and boot association. Verification updates only a matching applied action, uses that action's time window and retains its parsed threshold. Unrelated actions remain untouched; older unbound actions are not implicitly associated.
- Diagnose replaces static pipeline checkmarks with evidence/readiness wording, scrolls selected findings into view, and orders live findings by active state and severity. Report preview explicitly says it covers the whole snapshot.
- Compact Tasks uses the dedicated task panel. Compact Memory, Scheduler and Diagnose use the actual models instead of legacy placeholder rows. Task sort options include the detailed fields; memory supports named sorts and reversal without substituting RSS for unknown PSS.
- Memory slab scaling follows collected sizes rather than a fixture constant; slab mode exposes cache details and supports scrolling. Modes have names. NUMA collection adds node total/free/used memory and locality counter deltas. Allocation/reclaim graphs identify pages/s. PSS samples rotate through bounded process-leader inventory, preserve per-process sample time and discard departed identities.
- Cgroup modes now include CPU, memory and I/O controller filters. Per-device `io.stat` histories and details are acquired. Selected metadata includes the tightest observed ancestor CPU quota and memory limit, its source, and shared-sibling scope; acquisition errors remain unknown.
- IRQ placement distinguishes single CPU, multiple CPUs and unknown affinity. Numeric alignment remains covered by its existing regression test.
- Scheduler displays observed outstanding runnable wait ages for identity-confirmed tasks, separately from completed latency. It clears those waits on completion, exit or loss. The display states that this is not a complete kernel run queue; sampled CPU placement is not an exact enqueue destination. Procfs `R` is labeled runnable rather than running.
- Block metadata adds available firmware/cache/FUA/physical block information; IOPS/await explicitly describe read/write completion scope.
- Missing module trust metadata is unknown. Selected device/module/cgroup/BPF details no longer fall back to another subject's generic details.
- The kmsg event-rate graph identifies its source and actual 60-second window.

## Validation

Eight targeted regression tests cover unrelated-action verification, unknown-memory roundtrip and legacy numeric decoding, compact typed rows, PSS/RSS sorting, task generation, controller filters, unknown module trust and outstanding-wait completion/loss.

The full suite contains 82 tests. Reference captures are regenerated for changed UI text/layout and checked by the layout suite. All 82 tests pass. Formatting, Clippy with warnings denied, the release build, keyboard/SIGTERM PTY workflows, and the guided-demo smoke test pass. The rebuilt binary is installed at `~/.local/bin/kernwatch`, with the prior binary backed up beside it.

Read-only live sampling confirmed available D-state/softirq measurements, identities on all five sampled devices, generation-scoped task-runtime histories, 35 NUMA detail fields and 456 cgroup I/O series. Counts reflect this host, not universal coverage. No privileged attach or kernel-setting changes were performed during this fix pass.

## Work still required by the broader plan

Full kernel run-queue/running-task coverage and privileged ARM64 validation remain open. Other open work includes fully independent memory-mode layouts, selected-process PSS prioritization, complete process/thread folding, exhaustive typed table/expanded-view parity, multi-CPU IRQ history selection, operation-aware FLUSH tracing, full syscall decoding/symbolization, map occupancy/FD-holder/helper-cost acquisition, exact module/IRQ actor attribution, and richer evidence-driven diagnostic rules. Those require additional implementation and, in several cases, controlled privileged workloads; the UI must not claim they already exist.

The new cgroup ancestor values are configuration ceilings, not predictions of currently available resources. Legacy recordings with absent identity cannot retroactively establish generation continuity. Existing unbound action journals are intentionally not guessed into issue associations.

## Follow-up: stable timeline spacing

History now uses absolute one-second buckets and fixed-length windows, including blank pre-boot/startup slots. A missing second remains a gap rather than inheriting the preceding sample. Refreshes within a second update only that bucket; they do not slide previously completed buckets. The shared Dense/Overview timeline keeps a fixed ten-minute range instead of stretching all collected data across the viewport. Event markers use the same bucket-to-column mapping. Large graphs reserve a fixed value-axis gutter so changing magnitudes or units cannot shift the time-axis origin.

Five regression tests cover refresh jitter, equal elapsed steps, startup padding, missing/future samples and stable graph origin. Rendering still aggregates time buckets into the terminal's finite braille columns; resizing changes the available resolution, not the displayed time range.

Timeline follow-up validation: all 87 tests pass, including the five spacing regressions; Clippy, formatting, refreshed reference captures and release build pass. The installed local binary has been updated.
