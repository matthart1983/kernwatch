# A general tour of real data inside an isolated Linux VM; no synthetic samples.
Output screenshots/demo/kernwatch-tour.gif
Set Shell bash
Set FontFamily "Adwaita Mono"
Set FontSize 12
Set Padding 10
Set Width 1332
Set Height 824
Set Framerate 6
Set Theme { "background": "#0b1015", "foreground": "#c9d4de", "cursor": "#c9d4de", "selection": "#1b2733" }
# Collect history off-camera. Graphs plot one second per column, so the recording
# only looks alive with a minute behind it. Then start every probe family.
Hide
Type "python3 scripts/profile_demo.py --run --tour"
Enter
Sleep 80s
Type ":"
Type "probe all seconds=60"
Enter
Sleep 12s
Show
# Dense: the whole machine at once.
Sleep 5s
# Tasks: find the busy threads and inspect one.
Type "2"
Sleep 2s
Down
Sleep 400ms
Down
Sleep 400ms
Enter
Sleep 3s
Escape
Sleep 500ms
# Scheduler, memory and disk.
Type "3"
Sleep 3500ms
Type "4"
Sleep 3s
Type "5"
Sleep 3500ms
# What the process is asking the kernel for, and how interrupts land.
Type "6"
Sleep 3500ms
Type "7"
Sleep 500ms
Type "s"
Type "Rate/s"
Enter
Type "S"
Sleep 3s
Type "s"
Type "concern"
Enter
Type "S"
# Limits, modules, BPF and the kernel log.
Type "8"
Sleep 3500ms
Type "9"
Sleep 2500ms
Type "b"
Sleep 3500ms
Type "m"
Sleep 2500ms
# Diagnose reads the same evidence.
Type "d"
Sleep 4500ms
# Finish on where the CPU actually goes.
Type "F"
Sleep 1s
Type "/webapp"
Enter
Sleep 1s
Enter
Sleep 7s
Type "h"
Sleep 3s
Hide
Type "e"
Sleep 3s
Type "q"
Sleep 5s
