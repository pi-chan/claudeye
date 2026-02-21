# ccmonitor

A transparent always-on-top overlay that monitors Claude Code sessions running in tmux panes.

## What it does

ccmonitor periodically polls all tmux panes, finds those running the `claude` command, and displays their status in a small floating window at the top center of your screen. The window is click-through, so it never interferes with your workflow.

Each session is shown with a colored indicator:

| Indicator | Color  | Meaning                               |
|-----------|--------|---------------------------------------|
| `●`       | Green  | Working (Claude is processing)        |
| `●`       | Yellow | Waiting for your approval             |
| `●`       | Blue   | Waiting for your answer               |
| `○`       | Gray   | Idle (prompt shown, waiting for input)|
| `✕`       | Red    | Stopped                               |

## Requirements

- **tmux** — must be installed and running. ccmonitor uses `tmux list-panes` and `tmux capture-pane` to discover and read Claude sessions.
- **Claude Code** (`claude` CLI) — sessions must be running inside tmux panes. ccmonitor identifies panes where the current command is `claude`.
- **Rust toolchain** — install via [rustup](https://rustup.rs/)
- **Linux with X11** — tested on X11; Wayland is untested. Standard X11 libraries are required (typically pre-installed).

## Installation

```sh
cargo install ccmonitor
```

## Usage

```sh
ccmonitor [--opacity <VALUE>]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--opacity <VALUE>` | `0.24` | Background opacity (0.0 = fully transparent, 1.0 = fully opaque) |

### Examples

```sh
# Run with default opacity
ccmonitor

# Run with a more visible background
ccmonitor --opacity 0.6

# Run fully transparent background (text only)
ccmonitor --opacity 0.0
```

## How it works

1. A background thread periodically polls `tmux list-panes -a` to find all panes running `claude`.
2. For each matching pane, it runs `tmux capture-pane` to read the terminal content.
3. The terminal content is analyzed with regex patterns to determine Claude's current state (working, waiting for approval, idle, etc.).
4. The egui overlay window updates to reflect the latest state of each session.

The overlay window is:
- Positioned at the top center of your primary monitor
- Always on top of other windows
- Click-through (mouse events pass through to windows below)
- Transparent background with configurable opacity

## Development

```sh
cargo test    # Run tests
cargo clippy  # Run linter
cargo fmt     # Format code
cargo run     # Run in development mode
```
