use std::collections::HashSet;
use std::process::Command;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct PaneInfo {
    pub id: String,
    #[allow(dead_code)]
    pub pid: u32,
    #[allow(dead_code)]
    pub cwd: String,
    pub project_name: String,
}

pub fn list_claude_panes() -> Vec<PaneInfo> {
    let output = Command::new("tmux")
        .args([
            "list-panes",
            "-a",
            "-F",
            "#{session_name}:#{window_index}.#{pane_index} #{pane_pid} #{pane_current_path} #{pane_current_command}",
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .filter_map(parse_pane_line)
                .collect()
        }
        Err(e) => {
            eprintln!("[claudeye] tmux list-panes failed: {e}");
            vec![]
        }
    }
}

pub fn parse_pane_line(line: &str) -> Option<PaneInfo> {
    let parts: Vec<&str> = line.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return None;
    }
    let id = parts[0].to_string();
    let pid: u32 = parts[1].parse().ok()?;
    let cwd = parts[2].to_string();
    let command = parts[3].to_string();

    if !is_claude_command(command.trim()) {
        return None;
    }

    let project_name = std::path::Path::new(&cwd)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Some(PaneInfo {
        id,
        pid,
        cwd,
        project_name,
    })
}


pub fn switch_to_pane(pane_id: &str) {
    let result = Command::new("tmux")
        .args(["switch-client", "-t", pane_id])
        .output();
    if let Err(e) = result {
        eprintln!("[claudeye] tmux switch-client failed: {e}");
    }
}

/// Check whether the tmux pane command corresponds to a claude process.
fn is_claude_command(command: &str) -> bool {
    if command == "claude" {
        return true;
    }
    claude_version_names().contains(command)
}

/// On macOS, the `claude` binary is a symlink to a versioned path
/// (e.g. `~/.local/share/claude/versions/2.1.50`), so tmux resolves
/// the symlink and reports the version number as the command name.
/// Since multiple versions may coexist (older sessions survive across
/// upgrades), we cache all filenames in the versions directory at startup.
/// On Linux (or when `claude` is not a symlink), this returns an empty set
/// and detection falls back to the `command == "claude"` check above.
fn claude_version_names() -> &'static HashSet<String> {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| resolve_claude_versions().unwrap_or_default())
}

fn resolve_claude_versions() -> Option<HashSet<String>> {
    let output = Command::new("which").arg("claude").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let target = std::fs::read_link(&path).ok()?;
    let versions_dir = target.parent()?;
    let entries = std::fs::read_dir(versions_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    Some(entries)
}

pub fn capture_pane(pane_id: &str) -> String {
    let output = Command::new("tmux")
        .args(["capture-pane", "-p", "-t", pane_id])
        .output();

    match output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
        Err(e) => {
            eprintln!("[claudeye] tmux capture-pane failed for {pane_id}: {e}");
            String::new()
        }
    }
}
