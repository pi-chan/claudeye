# claudeye

A transparent always-on-top overlay that monitors Claude Code sessions running in tmux panes.

## What it does

claudeye periodically polls all tmux panes, finds those running the `claude` command, and displays their status in a small floating window at the top center of your screen. The window is click-through, so it never interferes with your workflow.

Each session is shown as a mini Clawd (the Claude robot mascot) with a speech bubble indicating its current state:

| Indicator | Color  | Meaning                               |
|-----------|--------|---------------------------------------|
| `●`       | Green  | Working (Claude is processing)        |
| `●`       | Yellow | Waiting for your approval             |
| `○`       | Gray   | Idle (prompt shown, waiting for input)|

## Requirements

- **tmux** — must be installed and running. claudeye uses `tmux list-panes` and `tmux capture-pane` to discover and read Claude sessions.
- **Claude Code** (`claude` CLI) — sessions must be running inside tmux panes. claudeye identifies panes where the current command is `claude`.
- **Rust toolchain** — install via [rustup](https://rustup.rs/)
- **Linux with X11** — tested on X11; Wayland is untested. Standard X11 libraries are required (typically pre-installed).

## Installation

```sh
cargo install claudeye
```

## Usage

```sh
claudeye [--compact]
claudeye picker
```

### Overlay mode

```sh
# Run with default settings (show all sessions)
claudeye

# Run in compact mode (cycle through one session at a time, one per second)
claudeye --compact
```

| Option | Description |
|--------|-------------|
| `--compact` | Show one session at a time, cycling every second |

![Overlay mode][1]

### Picker mode

```sh
claudeye picker
```

An interactive TUI session picker. Use it to quickly switch to any Claude session:

| Key | Action |
|-----|--------|
| `1`–`9` | Jump directly to that session |
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `Enter` | Switch to selected session |
| `q` / `Esc` | Quit without switching |

Sessions beyond the 9th are accessible via `j`/`k` navigation.

![Picker mode][2]

## How it works

### Overlay mode

1. A background thread periodically polls `tmux list-panes -a` to find all panes running `claude`.
2. For each matching pane, it runs `tmux capture-pane` to read the terminal content.
3. The terminal content is analyzed with regex patterns to determine Claude's current state (working, waiting for approval, idle, etc.).
4. The egui overlay window updates to reflect the latest state of each session.

The overlay window is:
- Positioned at the top center of your primary monitor
- Always on top of other windows
- Click-through (mouse events pass through to windows below)
- Fully transparent background

### Picker mode

1. Runs `tmux list-panes -a` once to collect all panes running `claude`.
2. Captures each pane's content to determine its current state.
3. Displays the sessions in a ratatui TUI list with state indicators and numeric labels.
4. On selection, runs `tmux switch-client` to jump to the chosen pane.

## Development

```sh
cargo test    # Run tests
cargo clippy  # Run linter
cargo fmt     # Format code
cargo run     # Run in development mode
```

[1]: https://raw.githubusercontent.com/maedana/claudeye/main/demo/demo.png
[2]: https://raw.githubusercontent.com/maedana/claudeye/main/demo/demo2.gif
