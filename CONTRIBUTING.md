# Contributing

Use Linux x86-64 with Rust 1.98 or newer. Run `cargo build --locked` to fetch and build dependencies; `--offline` works once they are cached.

Before opening a pull request:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
python3 scripts/pty_smoke.py
python3 scripts/demo_smoke.py
```

Explain the problem, resulting behavior, and validation. Keep measurements tied to a named source, interval, unit, and acquisition state. Preserve task/cgroup identity checks and explicit action previews. Missing data must not become a numeric zero or a synthetic live value.

UI changes should be reviewed at 80×24 and the reference size for the affected screen. If the change is intentional, run `python3 scripts/capture_all.py`, inspect the resulting PNGs, then rerun the layout tests. The renderer requires Pillow and either Adwaita Mono or DejaVu Sans Mono.

Changes to `probes/kernwatch.bpf.c` require a BPF-capable Clang and regeneration of the embedded object with `sh probes/build.sh`. Privileged tests belong in a disposable VM. The VM smoke test writes to its expendable virtio disk; do not run its guest mode on the host. See `docs/VALIDATION.md` for the test boundary.

Do not commit live recordings, reports, host audit output, credentials, or generated build directories. Open issues should include the view, terminal size, build revision, and a minimal reproduction; use demo captures when possible.

## Binary releases

Update the package version and `docs/RELEASE_NOTES.md`, then push a matching `v` tag (for example, `v0.1.0`). `.github/workflows/release.yml` builds and tests Linux x86-64 GNU and musl targets, packages executable archives with SHA-256 checksums, and publishes only after both builds pass. The manual dispatch accepts an existing tag. Existing published assets are not overwritten; a failed draft publication can be inspected and completed through GitHub Releases.

The older `scripts/package_release.py` packages a local evidence bundle and requires separate privileged VM logs. It is not the binary release pipeline.
