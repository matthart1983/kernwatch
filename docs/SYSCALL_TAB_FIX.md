# Syscall tab: empty capture and data delivery fixes

The live tab had no obvious capture action, and failures were reported only in a transient status message. Syscall p99 histories were recorded in the tracing worker's local series but were not published as measurements consumed by the main UI merge. Row selection also did not track syscall identity on refresh, and applying aggregate filters such as ENOENT a second time to raw event text could empty the event pane.

Implemented:

- A full empty-state explanation at normal and compact sizes, including stopped, active-but-no-completions, failed, unsupported and replay states.
- `l` starts a bounded 30-second syscall capture and clears stale display filters/freeze. `x` stops it. Thread-specific capture remains available with `:probe syscalls pid=TID seconds=30`.
- Persistent capture failure quality in the published snapshot, visible on the tab after the status line changes.
- Syscall p99 published through the measurement path used to build live UI history.
- Capture IDs reset old histories on a new session; failed starts no longer present the previous capture as the new one.
- Selection follows the syscall name when rows reorder. Aggregate table filters no longer independently suppress selected raw events; an empty row selection shows no unrelated events.

Opening the tab alone does not attach system-wide probes. Live syscall collection requires Linux BPF privileges. Start `sudo "$(command -v kernwatch)" --view syscalls`, then press `l`; use a bounded thread-specific capture to reduce traffic. Recordings require syscall capture evidence to populate this tab. Demo mode remains synthetic and never starts a live probe.

Validation: four interaction/rendering regressions cover the empty state, persistent errors, live capture/stop controls, demo isolation, selection and filtered events. An isolated QEMU guest using Linux 7.1.13 x86-64 successfully attached the actual syscall probes, collected 110 paired calls with zero loss, hydrated nine table rows, produced history measurements/distributions, selected an ENOENT row and rendered the event stream. Guest exit was zero. This test did not change host kernel settings or attach host probes. ARM64 privileged acquisition was not rerun for this UI/data-flow fix.

Final validation: all 91 tests, Clippy and release build passed. A real unprivileged terminal session verified capture instructions, the `l` action, a persistent acquisition error, and clean exit. The local installed binary was rebuilt.
