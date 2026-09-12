# Flame graphs

Syscall-entry stack profiles, folded and drawn as a zoomable icicle on the
Flame view (`F`).

For the planned CPU sampler, kernel stacks, symbol improvements, and profile
comparison, see the [profiling roadmap](PROFILING_PLAN.md).

Stacks are taken when a thread enters a syscall, so this answers "what calls
into the kernel, and from where". It is **not** a CPU profile: a thread burning
CPU without making syscalls produces nothing, and one blocked in a single
`epoll_wait` produces one tall tower. The view says so on screen.

## What is implemented

The spine: aggregation, symbolization, rendering and export, fed by the user
stacks the syscall capture already collects.

- **Aggregation** (`src/flame.rs`). `Profile::add` folds one observed stack into
  a tree keyed by frame name. Counters are sample-weighted, so every share the
  panel reports is a share of observations rather than of insertions.
- **Symbolization** (`src/symbols.rs`). Kernel text from `/proc/kallsyms`; user
  text from the mapped file's own ELF symbol table, located through
  `/proc/PID/maps`. Mapped file offsets are translated back to virtual
  addresses through the image's `PT_LOAD` segments, which is what makes a
  symbol in a shared library resolve to the right name.
- **Layout** (`flame::layout`). An icicle: the root spans the panel, each child
  takes a share of its parent proportional to its samples.
- **Export**. Every report bundle carries `stacks.folded`, the `a;b;c 123`
  interchange format that flamegraph.pl, inferno and speedscope read.

## What it refuses to do

A profile is evidence, so the view would rather show less than imply more.

- **No symbol is borrowed across a gap.** An address past the end of one symbol
  and before the start of the next resolves to neither; the frame keeps its
  hexadecimal form. A wrong name in a profile is worse than an address.
- **No frame is widened to become visible.** Each child's width is measured
  against its parent on its own, never against a running total, so rounding
  cannot hand a cell to whichever equal sibling happened to be last. Frames
  narrower than one column fold into a `…` stand-in where there is room for
  one, and are counted in `hidden` where there is not.
- **Truncated stacks are counted, not repaired.** See below.
- **Restricted kernel symbols are detected, not rendered.** Where
  `kptr_restrict` hides addresses, `/proc/kallsyms` reports every symbol at
  zero. The table is discarded rather than used to produce a profile of
  nothing.

## Frame pointers

User stacks are walked through frame pointers by `bpf_get_stackid`. Fedora 38+
and Ubuntu 24.04+ compile with `-fno-omit-frame-pointer`, but plenty of
software does not, and older distributions do not.

Where they are missing the walk ends after one or two frames. The dangerous
part is that this looks like a *shallow profile* rather than a *broken* one, so
the share of samples whose stack ended at or below `flame::SHALLOW` frames is
counted and reported in the panel subtitle as `N% truncated`.

Reconstructing those stacks needs DWARF `.eh_frame` unwinding — a project in
itself, and what `perf --call-graph=dwarf` and parca do — or SFrame, which is
new and needs a recent kernel. Neither is implemented.

## Closing the gap with perf and flamegraph.pl

Where this is ahead: nothing is silently dropped, truncated stacks are counted
and reported, no symbol is guessed across a gap, and capture and analysis are
the same place. Where it is behind: the data itself. `perf` samples the CPU;
this samples syscall entry. That is the gap, and it is mostly one program away.

### Phase 0 — build the probe object in CI

Nothing builds `probes/kernwatch.bpf.o`. It is checked in, built by hand, and
neither `ci.yml` nor `release.yml` mentions clang or `probes/`, so an edit to
`kernwatch.bpf.c` can ship against a stale object with no warning.

A CI job that installs clang, runs `probes/build.sh`, and fails when the
rebuilt object differs from the checked-in one closes that hazard and unblocks
every item below for anyone without a local BPF-capable clang. Do this first
whatever else is chosen.

### Phase 1 — kernel stacks

`kw_sys_enter` calls `stack_id(ctx, &stacks, 256)`; flag `256` is
`BPF_F_USER_STACK`. A second call with flags `0` yields the kernel stack.
Carry both ids on the event and append the kernel frames beneath the user
frames.

The userspace half already exists: `Symbols::kernel` parses `/proc/kallsyms`,
detects `kptr_restrict`, and is covered by unit tests. This is a struct field
and a second lookup, and it shows what the kernel does with the syscall —
currently the missing half of every stack.

### Phase 2 — CPU sampling

A `SEC("perf_event")` program on `PERF_COUNT_SW_CPU_CLOCK` at 49 Hz, capturing
both stack ids.

Aggregate **in the kernel**: a hash map keyed by `{user_id, kernel_id, tgid}`
counting samples, drained at the end. 99 Hz across 24 CPUs for 30 s is ~71k
events at ~200 B through the ring buffer — about 14 MB — to produce numbers
that are only going to be summed. `bcc`'s `profile` aggregates in-kernel for
the same reason.

`aya`'s `PerfEvent::attach` supplies the scope directly, so no BPF-side filter
is needed:

- `PerfEventScope::OneProcessAnyCpu { pid }` with `inherit: true` — a whole
  process and the children it spawns
- `PerfEventScope::AllProcessesOneCpu { cpu }`, attached per CPU — system wide

One program therefore delivers a real CPU profile, process-wide capture, and
system-wide profiling together. It is the highest-value item by a distance: it
changes what the tool measures rather than how it presents it. The C can stay
in the existing minimal-header style, since `bpf_perf_event_data` is only
passed through to the helper and can remain opaque.

### Phase 3 — readable symbols

Rust and C++ frames render mangled. `rustc-demangle` is small and has no
dependencies; Itanium C++ demangling is a larger commitment and `cpp_demangle`
is a real dependency to weigh against a six-crate manifest.

Whichever is used, keep the existing rule: demangle only when the mangled form
parses, and otherwise leave the symbol exactly as captured.

### Phase 4 — differential profiles

Entirely userspace. Keep a baseline profile, compute per-frame deltas, and
render them diverging — grew against shrank. Regression work is where profiles
earn their keep, and `stacks.folded` is already the interchange format to diff
a saved run against.

Worth taking before DWARF: more value, far less risk.

### Phase 5 — DWARF unwinding, deferred

Walking stacks without frame pointers means capturing register state and a
stack copy in BPF, then evaluating `.eh_frame` CFI in userspace — what
`perf --call-graph=dwarf` and parca do, and a project in its own right.

The present behaviour is the honest fallback: detect the truncation and report
the share. Revisit only if profiling frame-pointer-less binaries becomes the
main use.

### Order

0 → 1 → 2 closes most of the gap, then 4, then 3. After phase 2 the honest
summary changes from a better lens on a narrower picture to a better lens on
the same one.

## Collecting a profile

`P` profiles the selected subject, from wherever it is selected: the Flame
view's own list, Tasks, Dense, or a syscall row on Syscalls (`u`, which takes
the observed caller as its subject). There is one action, one duration, and no
staged command to confirm. `: probe syscalls pid=TID seconds=N stack` remains
for the unusual case.

With nothing captured, the Flame view lists what it could profile: processes by
default, ranked by CPU, `g` to list every thread instead. Kernel threads are not
offered — their stacks are not in user space, so a capture on one can only come
back empty.

### What one capture actually covers

The probe filters on a **thread** id (`bpf_get_stackid` is attached to
`raw_tp/sys_enter`, and the filter compares `bpf_get_current_pid_tgid()`'s low
word). Selecting a process therefore captures its busiest thread, and the panel
says which thread and how many others were left out. To capture a different one,
switch the picker to threads with `g`.

Capturing a whole process would need the filter to compare the TGID instead,
which means rebuilding `probes/kernwatch.bpf.o` with a BPF-capable Clang.

## Reading the view

The cursor reads the picture, and can sit on any frame without zooming to it:

- `←` `→` step between siblings, which are drawn side by side.
- `↓` goes into the heaviest callee, `↑` back to the caller.
- `h` follows the heaviest callee all the way down — where an investigation
  usually starts.
- `Enter` zooms to the frame under the cursor, making it the root; `Esc`
  unwinds the cursor, then the zoom, then leaves the view.

Arrow keys move the time cursor everywhere else in kernwatch. They do not here:
rewinding the clock would swap the profile out from under the reader.

The panel under the graph leads with the frame the cursor is on — its share,
what it keeps for itself, its heaviest callee, and the whole path to it — and
the subject and its caveats follow. Frames wide enough carry their share in the
graph itself, so two can be compared without selecting each in turn.

- `/` searches; every frame whose name matches is marked wherever it appears,
  and the subtitle counts the matches and the stacks passing through them.
- `P` chooses another subject; `x` stops a running capture and keeps what it
  collected.
- While a capture runs the subtitle counts it down (`18s of 30s`), so a quiet
  capture is distinguishable from a broken one.
- The subtitle reports stacks, the truncated share, and how many carry an
  unresolved address.
