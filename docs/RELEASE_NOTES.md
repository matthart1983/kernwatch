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
