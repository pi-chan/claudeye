# Changelog

## [Unreleased]

### Added

- `picker` subcommand — interactive TUI session picker using ratatui/crossterm
  - Number keys `1`–`9` jump directly to the corresponding session
  - `j`/`k` (or arrow keys) for navigation, `Enter` to switch, `q`/`Esc` to quit
  - `tmux switch-client` integration to jump to the selected pane

## [0.1.0] - 2026-02-23

### Added

- Transparent always-on-top overlay window showing Claude session states
- `--opacity` option to control overlay background transparency (default: `0.24`)
- Overlay window positioned at top center of primary monitor on startup
- Click-through overlay (mouse events pass through to windows below)
- State detection via regex analysis of captured tmux pane content
- MIT License

### Changed

- Project renamed from `ccmonitor` to `claudeye`
- Overlay window height adjusts dynamically per session row count

[Unreleased]: https://github.com/maedana/claudeye/compare/v0.1...HEAD
[0.1.0]: https://github.com/maedana/claudeye/releases/tag/v0.1
