<p align="center">
  <h1 align="center">kernwatch</h1>
  <p align="center">
    <strong>Linux kernel observability, in your terminal.</strong>
  </p>
  <p align="center">
    <a href="https://github.com/matthart1983/kernwatch/releases"><img src="https://img.shields.io/github/v/release/matthart1983/kernwatch" alt="Release"></a>
    <a href="https://github.com/matthart1983/kernwatch/releases"><img src="https://img.shields.io/github/downloads/matthart1983/kernwatch/total.svg" alt="Downloads"></a>
    <img src="https://img.shields.io/badge/platform-Linux%20x86--64%20%7C%20ARM64-blue" alt="Platform">
    <img src="https://img.shields.io/badge/license-MIT-green" alt="License">
  </p>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/matthart1983/kernwatch/main/screenshots/demo/kernwatch-profiling.gif" alt="kernwatch CPU profiling: capture stacks, save a baseline and compare workloads" width="900">
</p>

<p align="center">
  <em>Real CPU samples from a bounded workload in a Linux VM; not a performance benchmark.</em>
</p>

Investigate CPU contention, scheduling, memory pressure, block IO and interrupts.
Fourteen views connect live counters with bounded tracing, stack profiles and
incident reports. Linux only, on x86-64 and ARM64.

## Install

Download a [release binary](https://github.com/matthart1983/kernwatch/releases/latest)
for Linux x86-64 or ARM64. Both glibc and static musl builds are available; choose
static musl for portability.
[Download and checksum instructions](docs/REFERENCE.md#download-a-binary).

Or build from source with **Rust 1.98+** and a C linker:

```sh
git clone https://github.com/matthart1983/kernwatch.git
cd kernwatch
cargo build --release --locked
```

The executable is `target/release/kernwatch`.
[Installation details](docs/REFERENCE.md#build-from-source).

## Run

```sh
kernwatch                       # monitor the current host
kernwatch --view dense          # combined dashboard
kernwatch --demo-tour           # guided simulation; no host probes
kernwatch --replay capture.kwr  # replay a recording offline
```

Use 160 columns for the full dashboard; compact layouts work at 80×24.
`[` / `]` switch views, `Tab` moves panel focus, `?` shows help, `q` quits.
[Every keybinding](docs/REFERENCE.md#controls) · [Demo guide](docs/DEMO.md)

## Views

| Key | View | Focus |
|:---:|---|---|
| `0` | **Dense** | CPU, memory, block I/O, IRQ, scheduler, modules/BPF, tasks, and timeline |
| `1` | Overview | Host health and leading findings |
| `2` | Tasks | Threads, placement, runtime, memory, and captured wake latency |
| `3` | Scheduler | CPU/task/cgroup scheduling and latency distributions |
| `4` | Memory | Process memory, slab, NUMA, hugepages, and reclaim |
| `5` | Block | Device throughput, queue activity, and captured request latency |
| `6` | Syscalls | Calls, negative returns, durations, arguments, and captured stacks |
| `7` | IRQ | Interrupt rates, affinity, per-CPU softirq data, and handler latency |
| `8` | Cgroups | Runtime, quota, placement, memory, and pressure |
| `9` | Modules | Loaded state, taint, file metadata, and observed changes |
| `b` | eBPF | Programs, maps, links, runtime counters, and capture overhead |
| `m` | Dmesg | Kernel/journal events and source health |
| `d` | Diagnose | Evidence, hypotheses, action previews, and verification |
| `F` | [Flame](docs/FLAME_GRAPHS.md) | CPU/syscall stack capture, zoomable icicles, and baseline comparison |

## Profile and trace

Press `F`, select a process and press `Enter` to capture CPU stacks. `x` stops
and keeps the capture, `B` saves a baseline, and `D` compares it with the next
capture. `e` exports profiles and folded stacks.
[Profiling guide](docs/FLAME_GRAPHS.md).

Ordinary counters use procfs/sysfs. Request-level latency and syscall tracing
require an explicit capture, supported kernel features and BPF permissions.
On Syscalls (`6`), `l` starts a 30-second capture and `x` stops it.
kernwatch does not elevate itself.
[Tracing commands and scope](docs/REFERENCE.md#tracing).

Supported task, IRQ/RPS and cgroup changes use action previews, target checks,
readback and rollback journals.

## Record and report

`f` freezes the display, `r` toggles recording, and `e` exports an incident report.
Reports retain observations, hypotheses, source quality and action records.
Replay does not sample the host.
[Recording and export details](docs/REFERENCE.md#recording-and-reports).

## Limits

Missing or denied measurements are not healthy zeroes. Captured percentiles use
bounded samples, and device mean latency is not request p99. Tracing support
varies with the kernel, permissions and security policy; privileged ARM64 probe
attachment has not yet been validated.

The project is under active development. Large profile comparisons still have
performance limitations. See the [performance review](docs/PERFORMANCE_REVIEW.md),
[known gaps](docs/GAPS.md) and [validation scope](docs/VALIDATION.md).

## Docs

| | |
|---|---|
| [Reference](docs/REFERENCE.md) | Setup, controls, tracing, recording and optional data sources |
| [CPU and syscall profiling](docs/FLAME_GRAPHS.md) | Capture, comparison, symbols and measurement quality |
| [Guided demo](docs/DEMO.md) | Simulated incident walkthrough |
| [Development](CONTRIBUTING.md) | Build, test and contribution workflow |
| [Design documents](docs/README.md) | Specification, implementation plans and audits |

## Related

[NetWatch](https://github.com/matthart1983/netwatch) covers network diagnostics,
[SysWatch](https://github.com/matthart1983/syswatch) system activity, and
[DiskWatch](https://github.com/matthart1983/diskwatch) disk diagnostics.

## License

[MIT](LICENSE).
