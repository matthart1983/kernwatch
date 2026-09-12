# Syscalls and eBPF review

Reviewed against SCREEN-06 and SCREEN-10 and the reference diagrams on 11 September 2026, after the v0.2.0 release. The following fixes are subsequent source changes, not part of the v0.2.0 binary assets.

## Confirmed defects addressed

| Finding | Change |
|---|---|
| Duration columns sorted lexicographically | Parse duration units and compare numeric milliseconds; the latency filter accepts ms, µs, us, ns and seconds. |
| Tiny live durations rounded away | Retain six decimal places in millisecond source rows/events; display table latency in ms/µs/ns. |
| Selecting `read` could also match `pread64` | Match the leading syscall name exactly; normal and expanded streams share the same selection and error/latency filters. Newest events appear first, with millisecond timestamps. |
| Expanded stream ignored error and latency filters | Reuse the same filtered event list in both layouts. |
| Missing selected histogram fell back to unrelated/global data | Both normal and expanded histograms require the selected syscall's key. Absence stays explicit. The caption identifies the selected syscall. |
| Three demo rows lacked histories/distributions | Populate all six named syscall fixtures; synthetic data remains confined to demo mode. |
| Trace state/scope/loss/cost not visible in tab controls | Show acquisition state, capture scope, drops, owned probe CPU and ingest CPU from existing telemetry. Show named filter modes. |
| Selected program metadata could fall back to the first program | Require the selected program's metadata; never display another program's details as its own. |
| Map/helper metadata hidden below compact panel | Prioritize attachment, owner/creator, runtime availability, helpers and maps; indicate remaining fields and how to expand/scroll. |
| Map capacity presented as entry count | Label `max_entries`; include key/value sizes and metadata failures; leave occupancy explicitly unavailable. |
| Inventory failure could still produce a seemingly complete total | Suppress total BPF runtime when enumeration is incomplete. |
| Owner bars were alphabetical and silently truncated | Rank by measured CPU, label owner/creator UID scope and show the number of displayed groups. |
| Missing kernel BPF configuration context | Read JIT, unprivileged-BPF and global-statistics settings without modifying them. |
| Common negative returns showed only numbers | Name common Linux errno values; retain numeric fallback for unknown/internal return codes. |

## Remaining implementation gaps

1. **Syscall decoding:** names cover common calls; most arguments remain bounded register values plus selected captured path strings. General flag/structure/FD-path decoding and complete syscall-name coverage are not implemented. Retained events are bounded samples, not an unlimited stream.
2. **User stacks:** addresses can be captured for a selected TID. ELF/debug symbolization and executable/address-space identity over capture time remain open.
3. **Map occupancy and consumer lag:** current metadata reports capacity and layout. Type-specific bounded occupancy sampling and ring-buffer consumer-position measurement are not implemented; capacity is not occupancy or lag.
4. **Program ownership:** owned handles identify kernwatch's own programs. Creator UID and BPF links are available for external programs, but process FD-holder enumeration and historical loader identity are absent.
5. **Runtime distributions and helper costs:** cumulative runtime/run-count deltas provide means and CPU cost. They do not supply external-program p99 or helper execution cost; these require new tracing evidence.
6. **Capture workflow:** bounded probe commands and cancellation work. The fully interactive scope/type control strip and general reviewed external-program detach workflow in the diagrams remain incomplete.

## Permission-dependent data

Linux BPF enumeration, translated instructions, kernel symbols, runtime statistics and capture attachment depend on kernel support and privileges. Those failures should appear as acquisition states and reasons; they are distinct from the implementation gaps above.

## Verification

Regression tests cover numeric duration sorting, exact event-name selection, negative-return filtering and rejection of unrelated histogram/program-detail fallbacks. Reference captures are regenerated from Ratatui and reviewed at the tab reference sizes; the layout suite also checks compact breakpoints. The existing recording/export and terminal smoke tests remain required.
