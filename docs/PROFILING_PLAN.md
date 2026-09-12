# Profiling roadmap

The goal is an integrated terminal profiler that makes capture, investigation,
and comparison easier while exposing the limits of its evidence. CPU sampling
is the largest missing capability. Better presentation alone does not establish
parity with perf, and syscall-entry profiles must remain labelled separately.

This document records the implementation contract. Build verification, profile
metadata, CPU sampling with kernel stacks, readable symbols and differential
profiles are now implemented. See [verification](PROFILING_VALIDATION.md) for
executed checks and outstanding platform validation. The historical assumptions
below explain the design changes; [Flame graphs](FLAME_GRAPHS.md) describes use.

## Corrections to the original starting assumptions

- `probes/build.sh` already builds both x86-64 and aarch64 objects, but neither
  CI nor release workflows invoke it. Rust embeds the checked-in objects.
- Aya 0.13.1 supports perf events, but `OneProcessAnyCpu { pid }` forwards a
  task ID to `perf_event_open`. Inheritance applies to new tasks, not existing
  workers. A single attachment to the leader is not whole-process coverage.
  See [perf_event_open(2)](https://man7.org/linux/man-pages/man2/perf_event_open.2.html).
- A kernel stack taken at `raw_tp/sys_enter` describes that instant's entry
  path. It cannot show later execution inside the syscall. CPU sampling is
  needed to observe where kernel CPU time is spent.
- The current stack map holds 1,024 entries of 256 bytes: at most 32 addresses
  per entry. The shallow counter does not detect stacks reaching that limit.
- Shallow stacks suggest unwinding trouble; they do not prove truncation.
  Likewise, errno 14 does not establish missing frame pointers as the cause.
  Preserve the error and offer possible causes, not a definitive diagnosis.
- The renderer folds tiny frames and sometimes widens a cursor path to one
  column. Unsized symbols use estimated extents. Claims of perfect visibility
  and symbol certainty need qualification before becoming product promises.

## 0. Make probe builds verifiable

Update `probes/build.sh`, `.github/workflows/ci.yml`, and
`.github/workflows/release.yml` together. Pin the Clang/LLVM toolchain and
normalize debug paths so object comparisons are reproducible. Build both
objects in CI, compare against the checked-in copies, and publish the generated
objects with source revision, compiler version, and hashes. Release builds must
embed objects rebuilt from the release source before compiling Rust.

Add a privileged Linux validation lane to load and attach probes; successful
compilation does not prove verifier acceptance. Exercise both supported
architectures on matching hosts and keep ordinary unit tests unprivileged.

Acceptance: changing the C source without regenerating objects fails CI;
release provenance identifies the source and toolchain; both architectures
pass load/attach and teardown checks. Build failure must not fall back silently
to an older object.

## 1. Define profile semantics and quality

Extend `src/flame.rs`, `src/domain.rs`, `src/probes.rs`, `src/recording.rs`,
and the Flame view with structured capture metadata: kind, scope, duration,
requested frequency, weight unit, stack domains, and quality counters.
Retain compatible decoding of older recordings, marking unavailable metadata
unknown. Keep syscall observations, CPU samples, and eventual off-CPU duration
as separate units that cannot be accidentally combined.

Report attempted samples, usable user/kernel stacks, walk errors by errno,
map insertion failures, unresolved frames, shallow stacks, and stacks reaching
the configured depth limit. Count partial stacks separately from total failure.
Compute user-stack depth quality before appending kernel or synthetic frames.
Keep percentages explicit about their denominators and visible when zoomed.

Acceptance: failed walks and full-depth stacks cannot disappear into an
apparently complete profile; old recordings still open; exports retain capture
metadata alongside `stacks.folded`. Missing frame pointers are a suggested
cause, not an asserted diagnosis.

## 2. Add CPU sampling with user and kernel stacks

Add a `SEC("perf_event")` program in `probes/kernwatch.bpf.c`, initially using
`PERF_COUNT_SW_CPU_CLOCK` at a requested 49 Hz. Capture separate signed user
and kernel stack IDs, preserving errors independently. Aggregate counts in a
bounded per-CPU hash keyed by process identity, user stack ID, and kernel stack
ID. Handle concurrent insertion failures and map capacity explicitly. Size the
stack map and counter map against a documented memory budget.

For the initial process scope, attach `AllProcessesOneCpu` on every online CPU
and filter by TGID in BPF. This covers existing and new threads of the selected
process without silently including descendant processes. System-wide mode
uses the same attachments without the TGID filter. Expose permission failures;
do not silently substitute leader-only capture. Handle CPU hotplug and target
exit/reuse, and report partial attachment coverage. A lower-overhead attachment
per task can follow once existing-thread enumeration and creation races have
their own tested design.

Userspace should read cumulative map snapshots during capture, applying only
deltas to the live profile. Do not delete active counters while writers run.
Detach all sampling links before the final read so stopping preserves the final
counts. Resolve new stack identities while processes are alive; retain mapping
and image identity to handle exit, exec, and address reuse. Mark unavailable
symbols explicitly. Avoid a symbol lookup on every repeated sample.

In `src/app.rs`, command parsing, and `src/ui/views.rs`, expose CPU and syscall
capture as distinct choices. Make CPU the default profiling action only after
the acceptance checks pass. Join user and kernel paths with an explicit domain
boundary; distinguish missing user stacks from a kernel-only workload. Preserve
domain information and metadata in reports. Sample counts remain observations,
not exact CPU nanoseconds; show requested rate and achieved sampling evidence.

Acceptance: a syscall-free busy loop produces samples; existing and newly
created workers appear in process scope; unrelated processes do not. A workload
doing kernel work yields kernel frames where symbol access permits. Test
restricted symbols, failed walks, map exhaustion, target exit, stop, CPU coverage,
and repeated live reads without double counting. Compare dominant functions
against perf on controlled workloads with matching scope and stack settings,
and measure workload slowdown plus profiler CPU/memory overhead at 49 and 99 Hz.
Record results before claiming parity; there is no fixed overhead promise yet.

Optional follow-up: collect kernel stacks at syscall entry as entry-context
evidence. This requires coordinated C/Rust event-layout changes, independent
stack errors, and regenerated objects. It is not a substitute for this phase.

## 3. Make symbols readable without changing identity

Add parsing-based Rust and C++ demangling in `src/symbols.rs`. Keep raw names
and image/address identity separate from display names, so display formatting
cannot merge distinct functions. Leave failed parses unchanged. Provide readable
folded export while retaining raw identities in structured reports. Tighten or
label estimated symbol ranges rather than claiming they are certain.

Acceptance: Rust and C++ fixtures display readable names; invalid mangling
remains unchanged; same-name functions in different images stay distinguishable.
This phase can proceed independently once the metadata contract is settled.

## 4. Add differential profiles

Add baseline selection from a retained capture or saved report. Match stable
frame identities across address randomization, form the union of paths, and
show additions, removals, and signed changes with colour plus numeric labels.
Keep baseline/current counts and quality visible. Offer explicit raw-count and
sample-share comparisons; do not silently normalize. Require matching capture
kinds and units and surface differences in scope, duration, and frequency.
External folded files have unknown provenance and need explicit interpretation.

Acceptance: identical profiles yield zero deltas; added and removed paths remain
visible; unequal sample totals behave correctly in both comparison modes;
incompatible units are rejected. Preserve two ordinary folded exports and a
structured comparison report rather than inventing an ambiguous folded format.

## 5. Reassess unwinding and off-CPU capture

Defer DWARF/SFrame support until measurements show how often unwinding limits
real use. Investigate capture of registers and stack bytes, unwind metadata,
architecture support, memory bounds, and overhead as a separate design. Do not
promise it is merely a different helper flag or kernel upgrade.

Off-CPU profiles also need a separate design: capture at switch-out, correlate
with resumption, distinguish sleeping from runnable time, and handle migration,
exit, lost events, and intervals crossing capture boundaries. Use duration
weights and keep them separate from CPU sample counts.

## Delivery order

Build verification → metadata/quality → CPU sampling with kernel stacks →
readable symbols → differential profiles. Symbol work can overlap CPU work;
unwinding and off-CPU capture follow evidence of demand. Each phase ships only
after its acceptance checks, with measurements replacing speculative dates.

The first major milestone is an integrated CPU profiler with tested scope,
live analysis, explicit capture quality, and portable export. It closes the
largest collection gap; it does not erase perf's unwinding and platform coverage
advantages or establish superiority over other tools without comparison.
