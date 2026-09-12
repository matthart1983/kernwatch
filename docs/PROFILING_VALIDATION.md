# Profiling verification

Implementation scope: CPU sampling with user/kernel stacks, process/thread/system
scopes, quality metadata, readable symbols, baseline comparison and reproducible
probe builds. DWARF/SFrame unwinding and off-CPU stack profiles remain the explicit
follow-up designs in the roadmap.

## Local checks

- Rust unit and integration suite: 165 tests passed, including old recordings,
  profile quality denominators, comparison arithmetic, added/removed paths,
  capture controls, baseline reload and report-manifest coverage.
- Clippy with all targets and warnings denied: passed.
- Probe verification with Fedora Clang 22.1.8-4.fc44: both x86-64 and aarch64
  objects rebuilt identically from another directory. Source and object hashes
  are emitted in `dist/probes/provenance.json`.
- The normal Flame screen and differential screen were rendered and visually
  reviewed; the existing Syscalls capture was updated for its corrected `u` hint.

## Live x86-64 guest

Executed in an isolated QEMU TCG guest using Linux 7.1.13-200.fc44.x86_64,
2 virtual CPUs and 1 GiB RAM. The host's sysctls and kernel configuration were
not changed. The workload and sampler were built with frame pointers enabled.

`examples/cpu_smoke.rs` verified nonzero samples from syscall-free loops at
49 and 99 Hz; existing and newly spawned workers; process and thread isolation;
system-wide inclusion of an unrelated process; visible bounded-map pressure;
target-exit handling; and repeated final reads without double counting.
A separate `perf record -e cpu-clock -F 49 --call-graph fp` capture identified
the same busy-loop function as the dominant work (100% in its reference report).
Guest output and assertions are recorded in `tests/vm/cpu.log` locally.

The test records same-worker-count sampled/unsampled throughput and profiler
thread/BPF CPU measurements. TCG scheduling produces substantial noise, including
sampled throughput exceeding unsampled throughput in some runs. These numbers
are diagnostic, not a production overhead guarantee or a claim of superiority.

## Reproduce

```sh
CLANG=/path/to/clang-22 python3 scripts/verify_probes.py
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
RUSTFLAGS='-C force-frame-pointers=yes' cargo build --locked --release --example cpu_smoke
sudo target/release/examples/cpu_smoke
```

For disposable-guest testing, set `KERNWATCH_GUEST_BINARY` to
`target/release/examples/cpu_smoke` when running `tests/vm/build_initramfs.py`.
`KERNWATCH_GUEST_PERF=1` includes the host's perf and sleep executables in that
isolated image for the reference comparison. Guest-only hotplug and restricted
symbol tests are gated by the init program's disposable-guest environment marker.

## Platform gates

The reusable probe workflow rebuilds both objects and performs privileged live
checks on Ubuntu x86-64 and ARM64 runners. These remote checks must pass for the
actual implementation commit before claiming verification on ARM64. Local BPF
cross-compilation alone does not prove verifier acceptance on that architecture.

The current local validation does not establish reliable cross-build function
matching, DWARF/SFrame coverage, complete mapping-event history, or production
sampling overhead. Those limits are documented in `FLAME_GRAPHS.md`.
