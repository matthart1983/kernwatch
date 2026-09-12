# CPU profiles and flame graphs

The Flame view (`F`) captures, resolves and explores stacks in the terminal.
Live profiling defaults to CPU sampling with user and kernel stacks. Syscall-entry
capture remains available as a separate measurement. Both export folded stacks
for external flame graph viewers.

## Capture

On a live host, select a subject and press `P` in Tasks, Dense or the Flame
picker. Captures last 30 seconds. In the Flame picker, processes are the default;
`g` switches to individual threads. `C` switches the next capture between CPU
and syscall-entry stacks. `P` on a graph returns to the picker; `x` stops capture
and retains the final observations. Demo and replay never start a host capture.

Explicit command-palette forms are:

```text
probe cpu tgid=1234 seconds=30 hz=49
probe cpu pid=1235 seconds=30 hz=99
probe cpu seconds=30 hz=49
probe syscalls pid=1235 seconds=30 stack
```

CPU `tgid=` covers the selected process's existing and new threads, excluding
child processes. CPU `pid=` selects one thread; omitting both selects the system.
The implementation attaches a software CPU-clock perf event on every online CPU
and filters task identity in BPF. It does not rely on inheritance from the process
leader. Attachments require host BPF/perf privileges; failures are reported rather
than silently narrowing scope. CPU changes between polls produce a coverage warning.

Syscall capture selects one thread. Choosing a process for that capture selects
its busiest observed thread, not the whole process. `u` on a Syscalls row starts
this capture for its observed caller. A syscall-free busy loop appears in CPU
sampling but contributes nothing to syscall-entry capture. A call entered before
a syscall capture begins contributes no entry observation, even if still blocked.

CPU sampling uses a requested frequency, not an exact timer guarantee. Counts
are CPU observations, not measured nanoseconds. Syscall counts measure entries,
not CPU or blocked time. Never combine these units.

## Read the graph

- `←` / `→` move between siblings; `↓` enters the heaviest callee; `↑` returns.
- `h` starts at the zoomed root and follows the heaviest path to its end.
- `Enter` zooms to the cursor; `Esc` unwinds cursor and zoom before leaving.
- `/` searches readable frame names. Matching stack counts count each observed
  stack once even when several frames match.
- The detail panel shows the selected frame's share, own observations, path,
  and image/raw symbol when available.

Widths are proportional with integer-column rounding. Sub-column siblings fold
into a `…` marker when space permits; the details name omitted callees and their
shares. Cursor-path frames may be widened to one column so they remain visible.
The numeric share remains authoritative. Terminal height and width still limit
what can be drawn at once.

## Capture quality and symbols

The view and `profile.json` retain attempted observations, usable user and kernel
stacks, failed and partial observations, walk errors by domain/errno, stack-map
read failures, map insertion failures, unresolved frames, and depth-limit hits.
CPU samples may have only one usable domain. In particular, a user-mode sample
can have no kernel stack; that does not invalidate its user stack.

“Shallow” means at most two user frames, measured before adding kernel frames.
Its percentage is over usable user stacks for CPU captures. It is a heuristic:
a legitimately short stack can be shallow and an incomplete stack can be longer.
A stack reaching the configured 127-address limit is counted separately; that
also indicates a limit was reached rather than proving how much was omitted.
Failed walks do not become successful observations in the graph.

Errno 14 does not prove missing frame pointers. Where a binary omits them,
rebuilding with `-fno-omit-frame-pointer` may improve the walk. DWARF and SFrame
unwinding are not implemented. Old recordings remain readable and explicitly
lack the new diagnostics; absent metadata is not interpreted as zero failures.

User symbols come from executable mappings and their own ELF function tables,
with file offsets translated through `PT_LOAD`. Sized symbols do not extend
across gaps; unsized user symbols resolve only at their exact start. Deleted,
unreadable or unmapped images retain unresolved addresses. Rust and Itanium C++
names are demangled only when parsing succeeds. Structured profiles keep raw
names, display names, image fingerprints and image-relative function identities
separate. Different images or same-name functions at different addresses do not
merge merely because their display labels match.

Kernel symbols come from `/proc/kallsyms`. Restricted or unavailable tables leave
addresses unresolved. Kernel symbol extents are estimates bounded by the next
symbol and a 64 KiB ceiling; the capture records that caveat. Kernel stacks in a
CPU capture show sampled kernel execution. A stack at syscall entry alone would
only describe that entry instant.

## Compare captures

Press `B` to retain the current profile as a baseline. Capture another workload,
then press `D` to compare sample shares. Commands provide explicit control:

```text
profile-baseline
profile-baseline kernwatch-report-123/baseline.profile.json
profile-diff share
profile-diff counts
profile-diff off
```

A report's `profile.json` can also be loaded as the baseline. Both captures must
have known, matching kinds and units. Unknown older profiles and bare folded
files are not silently assigned a measurement type. Scope, duration and requested
frequency differences produce warnings.

The comparison icicle includes added and removed paths. Its width is the union
of paths, using the larger baseline/current count for each terminal path; colour
and signed numbers encode the change. The list below shows inclusive baseline
and current counts and changes. Red means growth; green means reduction. Share
mode reports percentage-point changes, while counts mode subtracts observations
without normalization. Use arrows or Page Up/Down to scroll the list, `/` to
filter, and `Esc` to return to the current capture.

Matching survives address randomization of the same executable images. Changed
image contents get new fingerprints and may appear as added/removed paths;
matching functions across rebuilt binaries is a remaining limitation.

## Export and implementation

`e` exports `stacks.folded` and `profile.json` with the normal report. A retained
baseline adds `baseline.stacks.folded` and `baseline.profile.json`; an active
comparison also adds `comparison.json`. Every payload appears in the manifest.
Folded exports use readable names, while the JSON preserves frame identity,
quality and capture metadata.

CPU aggregation is a bounded per-CPU hash. Live reads apply cumulative deltas
without deleting active counters. Stopping detaches sampling before the final
read. Task start and exec identities prevent reuse of cached stacks across those
identity changes; unavailable images remain unresolved. User mappings are read
when a new stack is resolved, so mapping changes before resolution remain a
limitation without a full mapping-event history.

Default limits are 8,192 stack entries of 127 addresses and 8,192 counter keys.
The counter values alone cost `8192 × possible CPUs × 8` bytes, in addition to
keys, kernel bookkeeping, stack storage (about 8 MiB), the existing 16 MiB ring,
and userspace symbol/profile storage. CPU `entries=N` bounds both maps from
1 to 8,192 entries; pressure is counted, never silently treated as completeness.

`probes/build.sh` builds x86-64 and aarch64 objects. Verification pins Fedora
Clang 22.1.8-4.fc44 and rebuilds in another directory to reject stale or
non-reproducible objects. CI supplies these objects and provenance to Rust builds
and runs privileged sampling checks on both runner architectures. See the
[implementation plan](PROFILING_PLAN.md) and [verification record](PROFILING_VALIDATION.md)
for remaining scope and actual validation results.
