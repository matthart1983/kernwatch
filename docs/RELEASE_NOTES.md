## kernwatch v0.3.0

Reworks how history is drawn and how detail panels are read.

**History plots keep a fixed time grid.** Every history shares absolute one-second buckets with one terminal character per bucket, so samples keep a constant width instead of stretching to fill the plot. Old samples clip at the left, new ones enter at the right, and a second with no observation stays blank rather than carrying the previous value forward. Vertical scales round up to 1/2/5 x powers of ten, so a small new peak no longer resizes every historical bar. Alarm color comes from each cell's own sample rather than its position in the plot.

**An idle disk holds its baseline.** Mean await and block p99 are undefined without completions, which left the I/O lanes as a row of gaps on a quiet disk. A device measured at zero completions is now an observed idle second and reports zero; a device with no rate yet still reports nothing and stays blank.

**The eBPF tab no longer reports a denial as data.** A failed inventory was pushed into the programs table as a short row, putting the errno under the program heading and shifting every later cell. The table now falls back to its empty state and the status line names the permission required. The redundant verdict column and the static probe-command panel are gone, and the programs table takes the reclaimed rows.

**Detail panels align into two columns.** Block device, cgroup, module, eBPF program and diagnose panels rendered each field as `name: value`, so values began at a different offset on every line and a wrapped value returned to the left edge. Names now share one dim column and values start at a common offset, with continuations hanging under the value.

Terminal frames are bracketed with synchronized-update commands on supporting terminals, and the Linux collector schedules against a one-second deadline instead of oversleeping in 50ms steps.

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
