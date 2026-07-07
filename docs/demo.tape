# vhs script for docs/provenant-demo.gif — a side-by-side race of Provenant vs. ScanCode
# scanning the same repository with the same flags (https://github.com/charmbracelet/vhs).
#
# Do not run this directly. Regenerate the GIF with:
#   docs/demo/record.sh
# which builds provenant, clones astral-sh/uv, builds the ScanCode Docker image, puts the
# docs/demo/scancode wrapper on PATH (with SCANCODE_IMAGE / SCANCODE_OUT), then runs vhs.
# Everything transient lives under .provenant/ (gitignored), so it never touches tracked files.
#
# Both scanners are throttled to a single process (-n 1) so the progress bars are visible
# and the comparison is apples-to-apples; Provenant uses every core by default.
Output "docs/provenant-demo.gif"

Set Shell "bash"
Set FontSize 18
Set Width 1500
Set Height 660
Set Theme "Catppuccin Mocha"
Set Padding 18
Set TypingSpeed 45ms

Hide
Type "export PATH=$PWD/target/release:$PATH" Enter
Type "export PROVENANT_CACHE=$(mktemp -d)" Enter
# Run inside the gitignored .provenant/ scratch area so the ./uv checkout and the
# results.json scan output never land next to tracked files.
Type "cd .provenant/demo" Enter
# Warm Provenant's license-index cache so the race shows scanning, not index build.
Type "provenant scan --json-pp /dev/null --license --package uv/ >/dev/null 2>&1" Enter
Sleep 8s
Type "clear" Enter
# Two labeled tmux panes: ScanCode (left) and Provenant (right).
Type "tmux kill-server 2>/dev/null; tmux new-session -d -s race -x 250 -y 36" Enter
Type "tmux set -t race status off" Enter
Type "tmux set -t race pane-border-status top" Enter
Type "tmux set -t race pane-border-format ' #{pane_title} '" Enter
Type "tmux split-window -h -t race" Enter
Type "tmux select-pane -t race:0.0 -T 'ScanCode (Docker)'" Enter
Type "tmux select-pane -t race:0.1 -T 'Provenant'" Enter
# ScanCode has a slow startup, so kick it off in the left pane first.
Type "tmux send-keys -t race:0.0 'scancode --json-pp results.json --license --package -n 1 uv/' Enter" Enter
Type "tmux select-pane -t race:0.1" Enter
Type "tmux attach -t race" Enter
Show

Sleep 1200ms
Type "provenant scan --json-pp results.json --license --package -n 1 uv/"
Sleep 500ms
Enter
Sleep 7500ms
Sleep 6500ms
