# Flame graphs

Syscall-entry stack profiles, folded and drawn as a zoomable icicle on the
Flame view (`F`).

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

## Not implemented

Both need new BPF programs, so they need a BPF-capable Clang to rebuild
`probes/kernwatch.bpf.o`.

- **On-CPU sampling.** A `perf_event` program on `PERF_COUNT_SW_CPU_CLOCK` at
  a fixed frequency, aggregating in-kernel into a hash map keyed by
  `{user_stack_id, kernel_stack_id, tgid, comm}`. In-kernel aggregation matters
  here: 99 Hz across 24 CPUs for 30 s is ~71k events at ~200 B each, about
  14 MB pushed through the ring buffer to be summed. `aya` 0.13 already has the
  program type, so no new dependency.
- **Off-CPU profiles.** `bpf_get_stackid` at `sched_switch` switch-out,
  weighted by the off-CPU duration the `offcpu` mode already computes
  (`probes.rs`). The correlation is half-built; this is the differentiated one,
  since `perf` does not give it up easily.

Also open: **kernel stacks** (the `stack_id` flags are hardcoded to
`BPF_F_USER_STACK`), and **demangling** — Rust and C++ symbols render mangled
(`_RNvCs…`), because a demangler that guesses wrong would break the rule above.

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

- `↑` `↓` select among the zoomed frame's children. Selection walks the tree,
  not the drawn cells, so it does not change with terminal width.
- `Enter` zooms into the selected frame; `Esc` widens one level, then leaves.
- `/` searches; every frame whose name matches is marked wherever it appears,
  and the subtitle counts the matches and the stacks passing through them.
- `P` chooses another subject; `x` stops a running capture and keeps what it
  collected.
- While a capture runs the subtitle counts it down (`18s of 30s`), so a quiet
  capture is distinguishable from a broken one.
- The subtitle reports stacks, the truncated share, and how many carry an
  unresolved address.
