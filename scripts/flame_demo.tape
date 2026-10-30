# Real CPU captures inside an isolated two-CPU Linux VM; no synthetic samples.
Output screenshots/demo/kernwatch-flame.gif
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
Type "F"
Sleep 1s
Type "/webapp"
Enter
Sleep 1s
Show
Sleep 2s
Enter
Sleep 12s
Type "x"
Sleep 2s
# Follow the hottest path, then zoom out to the request that contains it.
Type "h"
Sleep 3s
Up 4
Sleep 1s
Enter
Sleep 3s
Down
Sleep 1s
Right
Sleep 1500ms
Right
Sleep 1500ms
# One Escape clears the cursor and ten unwind the zoom, one frame at a time.
Escape@100ms 11
Sleep 1s
Type "/expand_block"
Enter
Sleep 3s
# Reopening the filter and escaping clears it; a plain Escape would not.
Type "/"
Escape
Sleep 1s
# Preserve and export the baseline. The bounded workload changes its mix when
# this export appears; no profile samples or symbols are manufactured.
Type "B"
Sleep 1s
Type "e"
Sleep 2s
Hide
Type "P"
Sleep 1s
Type "/webapp"
Enter
Sleep 1s
Enter
Sleep 8s
Show
Sleep 5s
Type "x"
Sleep 2s
Type "D"
Sleep 5s
Type "/parse_headers"
Enter
Sleep 4s
Hide
Type "/"
Escape
Sleep 1s
Type "e"
Sleep 3s
Type "q"
Sleep 5s
