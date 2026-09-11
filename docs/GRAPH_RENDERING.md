# Graph redraw consistency

The follow-up to the timeline-spacing fix addresses the shared renderer and live frame loop.

- **One time grid:** all histories use absolute one-second buckets, with exactly one terminal character per bucket. The visible time range follows the plot width; resizing reveals or clips older samples instead of stretching or compressing them. Every bucket requires an actual observation in that second. Uncollected seconds and explicit missing samples stay blank. No previous or future value is copied into them.
- **Stable vertical scales:** history and large-graph scales round upward to 1/2/5 × powers of ten. A small increase within one scale band does not resize every historical bar. Crossing a band or explicitly using Scheduler auto-scale can still change the scale. Large graphs label the scale they actually use.
- **Historical colors:** alarm color is based on the value in each braille cell rather than the cell's vertical position. CPU and health history styles do not toggle with current issue state. Card alarms change the current value's color while keeping the history style stable.
- **Explicit clearing:** the plot renderer clears its rectangle before repainting, including when all samples disappear or become invalid. Non-finite values are not plotted.
- **Complete terminal frames:** interactive Linux and portable viewers bracket each frame with synchronized-update commands. The end command is sent on draw failure and terminal cleanup too. Supporting terminals present the completed frame together; other terminals may ignore these commands.
- **Steady collection cadence:** the Linux collector schedules against a one-second deadline rather than repeatedly oversleeping in 50ms steps. Overruns skip ahead rather than producing a burst of catch-up samples.

Six new regressions cover scale boundaries, preservation of completed pixels when a small new peak arrives, uncollected seconds remaining blank, removal of old bars, sample-based alarm colors and card-history stability. PTY tests also require synchronized frames and verify that synchronization is closed on normal and signal-driven exit.

The previous 1.5-second carry-forward tolerance has been removed: insufficient history must remain blank. Time-series samples are never resampled to fit the plot. Each sample fills the two braille dot columns inside one terminal character; unavailable samples occupy a blank character. Old samples clip at the left edge and new samples enter at the right. Event markers and time-axis labels use the same one-second-per-character mapping. Histogram buckets remain categorical and may fill their available plot width.

Validation: all 97 tests pass, including six new redraw regressions. Formatting, Clippy with warnings denied, refreshed screen captures, release build, synchronized-frame PTY keyboard/SIGTERM checks and the guided demo test pass. The rebuilt binary is installed at `~/.local/bin/kernwatch`; restart existing sessions to use it.

Fixed sample width supersedes earlier fixed-duration timeline behavior. Three additional regressions verify exact one-character bars at five panel widths, left clipping/right padding without value aggregation, and matching event-marker spacing after resize.

Fixed-width validation: all 100 tests pass, plus Clippy, refreshed reference captures and release build. The rebuilt executable is installed locally.

CPU graphs and timeline histories now call the same filled-series drawing primitive. New regression tests compare their pixels directly and prove that two sparse observations occupy only two columns in 12-, 80- and 240-column plots.

No-fill validation: all 102 tests pass, including the sparse-history and CPU/timeline pixel-equivalence regressions. Existing reference captures pass unchanged; formatting, Clippy and release build pass. The local executable has been updated.
