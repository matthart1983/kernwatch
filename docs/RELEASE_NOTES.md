## kernwatch v0.4.1

Updates the terminal UI, terminal input, BPF loader, and C++ demangler dependencies while preserving the existing Rust edition and release optimisation settings.

- Upgrade Ratatui 0.27.0 to 0.30.2, Crossterm 0.27.0 to 0.29.0, Aya 0.13.1 to 0.14.0, and cpp_demangle 0.4.5 to 0.5.1; refresh compatible transitive dependencies.
- Migrate terminal rendering, BPF loader configuration, CPU sampling attachment, and demangling calls to the current APIs.
- Use the published cpp_demangle crate instead of the temporary vendored source. Release archives retain its MIT and Apache licences.
- Keep optional Ratatui features limited to the terminal backend, layout cache, and underline colours used by Kernwatch.

Addresses the dependency-update portion of [issue #1](https://github.com/matthart1983/kernwatch/issues/1). Binary stripping, LTO settings, and the edition 2024 migration are outside this patch.

Release validation covers formatting, Clippy, unit/integration tests, terminal and recording/replay smoke tests, and Linux x86-64/ARM64 glibc and musl builds. The probe workflow verifies embedded BPF objects and exercises live CPU sampling on x86-64 and ARM64.

## kernwatch v0.4.0

Adds real CPU profiling to the Flame view. Press **F**, select a process, and press **Enter** to capture a 30-second profile at 49 Hz. CPU sampling sees busy loops without syscalls and supports process-wide, thread-only, and system-wide capture. Syscall-entry profiling remains available as a separate mode.

- Capture user and kernel stacks, with explicit counts for failed, shallow, depth-limited, and dropped samples. Shallow stacks are a warning, not proof of missing frame pointers.
- Read Rust and C++ demangled symbols without assigning unresolved addresses to neighbouring user symbols. Frames too narrow to draw remain available by name and share.
- Save a baseline with **B** and compare with **D**, using sample shares or counts. Reports include `profile.json` and `stacks.folded`, plus baseline and comparison files when present.
- Rebuild and verify both embedded BPF objects in CI using a pinned compiler. Release archives include probe provenance and third-party licenses.

Validated with 166 unit/integration tests, Clippy, terminal and release smoke tests, disposable-VM probe tests, and privileged native x86-64/ARM64 CPU sampling. See [the profiling validation record](https://github.com/matthart1983/kernwatch/blob/v0.4.0/docs/PROFILING_VALIDATION.md) for scope and measurements. Every release target also runs its own build and smoke checks before publication.

Linux x86-64 and ARM64 binaries are available in glibc and static musl variants. Live profiling requires sufficient BPF/perf permissions. DWARF/SFrame unwinding and off-CPU profiling are not implemented; user stacks depend on frame pointers. Differential symbol identity matches the same binary across ASLR, not rebuilt binaries.

## kernwatch v0.3.0

Reworks how history is drawn and how detail panels are read.

**History plots keep a fixed time grid.** Every history shares absolute one-second buckets with one terminal character per bucket, so samples keep a constant width instead of stretching to fill the plot. Old samples clip at the left, new ones enter at the right, and a second with no observation stays blank rather than carrying the previous value forward. Vertical scales round up to 1/2/5 x powers of ten, so a small new peak no longer resizes every historical bar. Alarm color comes from each cell's own sample rather than its position in the plot.

**An idle disk holds its baseline.** Mean await and block p99 are undefined without completions, which left the I/O lanes as a row of gaps on a quiet disk. A device measured at zero completions is now an observed idle second and reports zero; a device with no rate yet still reports nothing and stays blank.

**The eBPF tab no longer reports a denial as data.** A failed inventory was pushed into the programs table as a short row, putting the errno under the program heading and shifting every later cell. The table now falls back to its empty state and the status line names the permission required. The redundant verdict column and the static probe-command panel are gone, and the programs table takes the reclaimed rows.

**Detail panels align into two columns.** Block device, cgroup, module, eBPF program and diagnose panels rendered each field as `name: value`, so values began at a different offset on every line and a wrapped value returned to the left edge. Names now share one dim column and values start at a common offset, with continuations hanging under the value.

Terminal frames are bracketed with synchronized-update commands on supporting terminals, and the Linux collector schedules against a one-second deadline instead of oversleeping in 50ms steps.

**macOS and Windows are dropped.** The two viewer targets built the demo and the replay of Linux recordings and nothing else: no kernel telemetry, no host controls. They are no longer built, tested or published, and the crate no longer compiles off Linux. 0.2.0 assets stay available for anyone still replaying recordings on those platforms. The release matrix is now the four Linux assets below.

## kernwatch v0.2.0

Adds every target from netwatch's release matrix, with explicit platform scope.

| Asset | Modes |
|---|---|
| `kernwatch-linux-x86_64.tar.gz` | Linux live monitoring, demo and replay; glibc (Ubuntu 22.04 build) |
| `kernwatch-linux-x86_64-static.tar.gz` | Same, static musl |
| `kernwatch-linux-aarch64.tar.gz` | Linux ARM64 live monitoring, demo and replay; glibc (Ubuntu 24.04 build) |
| `kernwatch-linux-aarch64-static.tar.gz` | Same, static musl |
| `kernwatch-macos-x86_64.tar.gz` | Intel macOS: demo, Linux recording replay and export |
| `kernwatch-macos-aarch64.tar.gz` | Apple Silicon macOS: demo, Linux recording replay and export |
| `kernwatch-windows-x86_64.exe.zip` | Windows x86-64: demo, Linux recording replay and export |

Each archive contains the named executable, README and MIT license. Verify downloads with `SHA256SUMS` or the individual `.sha256` files. Rename/install the executable as `kernwatch` (`kernwatch.exe` on Windows), then run `kernwatch --demo-tour`.

Linux builds embed architecture-specific CO-RE probes and use the target's syscall numbers. macOS and Windows builds reject live monitoring and host controls; they do not claim native kernel monitoring.

All targets run native compilation, Clippy, unit/UI tests, JSON/render checks and recording/replay checks. Unix targets also run real-terminal smoke tests. Windows terminal interaction is not automated by the Unix PTY scripts. Static builds are checked for shared-library dependencies.

Privileged probe tests have not been repeated on these release binaries. Historical kernel validation covered the x86-64 pre-rename baseline; ARM64 probe attachment requires separate privileged validation. User-stack symbolization, allocation/module-loader attribution and interactive tracefs fallback remain open work.
