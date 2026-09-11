# Validation evidence

Local validation of the renamed kernwatch checkout, 11 September 2026. These results were collected locally before publication; GitHub Actions results are reported separately.

## Current checkout

- `cargo test --locked`: **74 passing tests** on Linux (32 library, 10 behaviour, 7 layout, 1 portable recording/export, 24 workflow). Coverage includes parsing, identity, navigation, thirteen layouts, recording recovery, replay, exports, action previews, rollback and demo scene safety.
- `cargo clippy --locked --all-targets -- -D warnings` and `cargo fmt --check`: passed.
- `cargo build --release --locked`: passed.
- `python3 scripts/pty_smoke.py`: passed keyboard navigation, resize, recording, freeze, export, normal shutdown, SIGTERM and terminal restoration.
- `python3 scripts/demo_smoke.py`: passed tour startup and automatic advance, manual takeover, quit, terminal restoration and rejection of live tracing in demo mode.
- `python3 scripts/build_demo.py`: generated eight actual Ratatui frames, a 40-second GIF and a 40-second MP4 at 1920×1248. See [the demo guide](DEMO.md).
- All thirteen [reference captures](../screenshots/current) were regenerated with the kernwatch branding. Layout tests compare Ratatui symbols and foreground colours; PNGs exclude desktop window chrome.

Generated test logs are local, ignored artifacts. Reproduce the commands above to obtain evidence for your own checkout and environment. These checks do not establish compatibility with every kernel or terminal.

## Historical kernel validation

Before the rename and guided demo changes, the kwatch baseline passed privileged smoke tests on Fedora kernels `6.19.10-300.fc44.x86_64` and `7.1.13-200.fc44.x86_64` in disposable QEMU guests. Those tests exercised scheduler/IRQ/syscall/block probes, task-scoped capture, BPF inventory, event loss, and application/rollback of affinity and cgroup controls. The renamed checkout has not repeated that VM validation, and historical raw host/VM logs are not included here.

Architecture-specific probe objects target Linux x86-64 and ARM64 with kernel BTF and raw tracepoints. ARM64 builds pass native user-space tests, but privileged ARM64 attachment has not yet been validated. Other kernels need their own attach and lifecycle tests.

To reproduce privileged validation, build the `probe_smoke` release example, run `tests/vm/build_initramfs.py`, and boot its generated initramfs with a compatible kernel and an expendable virtio disk. `KERNWATCH_DISPOSABLE_GUEST=1` is set only by the guest init. That mode writes `/dev/vda`; never set it for a host test.

## Explicit bounds and interpretation

| Component | Bound / interpretation |
|---|---|
| Entity timeline | Up to 600 frames, capped at an estimated 128 MiB; graph history can extend beyond retained entity frames |
| Ordinary histories | Up to 600 samples per series; missing time is rendered as gaps |
| Tasks | Up to 10,000 thread identities; runtime is per thread, RSS is shared address-space accounting |
| Cgroups | Up to 512 visited groups; parent counters include descendants |
| IRQ counts / affinity | Independent worker; up to 4,096 vector rows; 64 retained polling-change events |
| PSS | Up to 32 process leaders, five-second cadence |
| BPF ring buffer | 16 MiB, loss counter observed and pending correlations invalidated |
| Pending correlations | 16,384 entries before invalidation |
| Distribution samples | Last 4,096 values per retained subject; percentiles are bounded-sample estimates |
| Stack traces | 1,024 unique stacks, up to 32 addresses; capture requires a selected TID |
| Trace events / kernel messages | 1,000 / 300 retained records |
| Probe duration | Default 30 seconds, configurable 1–60 seconds |
| Replay | 32 MiB maximum frame; 512 MiB cumulative encoded input limit |
| Source handoff | Bounded request/result channels; old topology marked stale after 10 seconds |

Source permission errors are expected in restricted containers. Kernel logs, slab information, BPF inventory, tracing, user-stack walking, module signer metadata and optional storage telemetry depend on the host. Loader capture and stack symbolization still require implementation, as tracked in [GAPS.md](GAPS.md). Missing metadata is not replaced with vendor names, zero latency or invented ownership. User stacks retain addresses, not a claim of complete symbolization.

The numeric fixture corrects contradictions in the diagrams: aggregate CPU components reconcile with per-core readings; quota measures runtime rather than waiting; outstanding flush age is not completed p99; module trust is not performance attribution. Kernel IRQ entry/exit measurements are wall durations and may include nested work.
