## kernwatch v0.1.1

Fixes musl linking for atomic incident-report export. The v0.1.0 build did not publish a release.

Initial binary release of the Rust and Ratatui Linux kernel monitor, with thirteen views, a guided demo, bounded eBPF tracing, recording/replay and incident reports.

### Downloads

| Archive | Platform |
|---|---|
| `kernwatch-linux-x86_64-static.tar.gz` | Linux x86-64, static musl build; recommended for portability |
| `kernwatch-linux-x86_64.tar.gz` | Linux x86-64, glibc build produced on Ubuntu 22.04 |

Each archive includes the named executable, MIT license and README. Verify downloads against `SHA256SUMS` or the individual `.sha256` file. The BPF object is embedded; no probe compilation is required to run the application.

```sh
tar -xzf kernwatch-linux-x86_64-static.tar.gz
mkdir -p ~/.local/bin
install -m 755 kernwatch-linux-x86_64-static ~/.local/bin/kernwatch
~/.local/bin/kernwatch --demo-tour
```

The demo uses synthetic data and requires no privileges. Run `kernwatch --view dense` for host counters; explicit eBPF capture requires kernel BTF, supported tracepoints and permissions.

### Validation and scope

Both release targets run formatting, Clippy, the test suite, JSON snapshot validation and real-terminal smoke tests before upload. The static build is checked for dynamic dependencies. These are user-space checks, not privileged probe-attachment certification.

The pre-rename baseline passed privileged tests on kernels 6.19.10 and 7.1.13. Those tests have not been repeated on these release binaries. ARM, macOS and Windows are not supported by this release. Remaining gaps include user-stack symbolization, allocation/module-loader attribution and an interactive tracefs fallback; see `docs/GAPS.md` in the source tree.
