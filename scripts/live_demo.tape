# Record kernwatch monitoring the real host. Driven by scripts/live_demo.py,
# which starts a bounded workload first so the graphs have something to plot.
Output screenshots/demo/kernwatch-live.gif
Output screenshots/demo/kernwatch-live.mp4

Set Shell bash
Set FontFamily "Adwaita Mono"
Set FontSize 12
Set Padding 10
Set Width 1332
Set Height 824
Set Framerate 8
Set Theme { "background": "#0b1015", "foreground": "#c9d4de", "cursor": "#c9d4de", "selection": "#1b2733" }

# Start live and collect history off-camera. The graphs plot one second per
# column, so the recording only looks alive once there is a minute behind it.
Hide
Type "clear && ./target/release/kernwatch --view dense"
Enter
Sleep 75s
Show

# Dense: live CPU, memory, block, IRQ and the running timeline.
Sleep 5s

# Tasks: walk the live task list and inspect one thread.
Type "2"
Sleep 2s
Down
Sleep 400ms
Down
Sleep 400ms
Down
Sleep 1s
Enter
Sleep 3s
Escape
Sleep 1s

# Scheduler, memory and block under the generated load.
Type "3"
Sleep 4s
Type "4"
Sleep 4s
Type "5"
Sleep 4s

# Interrupts and cgroups.
Type "7"
Sleep 4s
Type "8"
Sleep 3s

# Kernel surface: modules, BPF and the kernel log.
Type "9"
Sleep 2s
Type "b"
Sleep 3s
Type "m"
Sleep 3s

# Diagnose reads the same live evidence.
Type "d"
Sleep 4s

# Back to Dense with several minutes of history collected.
Type "0"
Sleep 4s

Type "q"
Sleep 1s
