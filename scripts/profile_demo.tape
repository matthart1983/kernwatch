# Real CPU captures inside an isolated two-CPU Linux VM; no synthetic samples.
Output screenshots/demo/kernwatch-profiling.gif
Output screenshots/demo/kernwatch-profiling.mp4
Set Shell bash
Set FontFamily "Adwaita Mono"
Set FontSize 12
Set Padding 10
Set Width 1332
Set Height 824
Set Framerate 6
Set Theme { "background": "#0b1015", "foreground": "#c9d4de", "cursor": "#c9d4de", "selection": "#1b2733" }
Hide
Type "python3 scripts/profile_demo.py --run"
Enter
Sleep 20s
Show
Sleep 3s
Type "F"
Sleep 2s
Type "/profile-demo"
Enter
Sleep 2s
Enter
Sleep 15s
Type "x"
Sleep 2s
Screenshot screenshots/demo/profiling-capture.png
# Preserve and export the baseline. The bounded workload changes its mix when
# this export appears; no profile samples or symbols are manufactured.
Type "B"
Sleep 2s
Type "e"
Sleep 3s
Type "P"
Sleep 1s
Type "/profile-demo"
Enter
Sleep 2s
Enter
Sleep 15s
Type "x"
Sleep 2s
Type "D"
Sleep 5s
Screenshot screenshots/demo/profiling-comparison.png
Type "/parse_headers"
Enter
Sleep 4s
Escape
Type "D"
Sleep 3s
Type "e"
Sleep 3s
Hide
Type "q"
Sleep 5s
