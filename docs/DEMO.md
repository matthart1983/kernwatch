# Review the kernwatch demo

The guided demo uses a synthetic incident and runs without host probes.

## Run it locally

From the project directory:

```sh
cargo build --release --locked
./target/release/kernwatch --demo-tour
```

Or use `./scripts/demo.sh`. Prefer a terminal at least 160 columns wide and 52 rows high. The tour loops through eight views, five seconds each. Press any key to stop autoplay and take control; `q` exits.

For manual exploration, run `kernwatch --demo`. Use `0` for Dense, `2` for Tasks, `3` for Scheduler, `7` for IRQ, and `d` for Diagnose.

## What the tour demonstrates

1. **Dense:** a single synthetic incident with high CPU3 softirq activity and delayed tasks.
2. **Tasks:** Envoy worker runtime and captured wake-latency data.
3. **Scheduler:** compare CPUs and their scheduling distributions.
4. **IRQ:** inspect interrupt rates and placement in right-aligned columns.
5. **Memory:** review slab growth and pressure without asserting a proven leak.
6. **Block:** distinguish outstanding request age from completed latency.
7. **Cgroups:** examine limits, placement, and service runtime.
8. **Diagnose:** review evidence and competing explanations before taking action.

All views show the same simulated incident. The demo does not load probes, modify host controls, or claim a real incident occurred. Latency and overhead values are fixtures. It does not demonstrate live kernel compatibility or prove any proposed cause.

## Preview artifacts

- [Animated GIF](../screenshots/demo/kernwatch-demo.gif), suitable for the README.
- A local MP4 is generated alongside the GIF for review; it is not tracked by default.
- [Dense screenshot](../screenshots/current/00-dense.png) for a full-resolution still.

Rebuild the preview with:

```sh
python3 scripts/build_demo.py
```

This requires Pillow, FFmpeg with libx264, and Adwaita Mono or DejaVu Sans Mono. Set `KERNWATCH_FONT` to an alternative font file if needed. Frames are rendered by the actual Ratatui application using the same scene definitions as `--demo-tour`.
