use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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

    let version_names = claude_version_names();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .filter_map(|line| parse_pane_line_with_versions(line, &version_names))
                .collect()
        }
        Err(e) => {
            eprintln!("[claudeye] tmux list-panes failed: {e}");
            vec![]
        }
    }
}

/// Parse a tmux pane line, using the caller-provided version name set.
fn parse_pane_line_with_versions(line: &str, version_names: &HashSet<String>) -> Option<PaneInfo> {
    let parts: Vec<&str> = line.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return None;
    }
    let id = parts[0].to_string();
    let pid: u32 = parts[1].parse().ok()?;
    let cwd = parts[2].to_string();
    let command = parts[3].trim();

    if !is_claude_command_with_versions(command, version_names) {
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

/// Public wrapper that resolves version names on each call.
/// Kept for use in tests and external callers.
pub fn parse_pane_line(line: &str) -> Option<PaneInfo> {
    let version_names = claude_version_names();
    parse_pane_line_with_versions(line, &version_names)
}


pub fn switch_to_pane(pane_id: &str) {
    let result = Command::new("tmux")
        .args(["switch-client", "-t", pane_id])
        .output();
    if let Err(e) = result {
        eprintln!("[claudeye] tmux switch-client failed: {e}");
    }
}

fn is_claude_command_with_versions(command: &str, version_names: &HashSet<String>) -> bool {
    command == "claude" || version_names.contains(command)
}

const VERSION_CACHE_TTL: Duration = Duration::from_secs(30);

struct VersionCache {
    names: HashSet<String>,
    versions_dir: Option<PathBuf>,
    last_refresh: Instant,
}

fn version_cache() -> &'static Mutex<VersionCache> {
    static CACHE: OnceLock<Mutex<VersionCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let (versions_dir, names) = init_version_cache();
        Mutex::new(VersionCache {
            names,
            versions_dir,
            last_refresh: Instant::now(),
        })
    })
}

/// On macOS, the `claude` binary is a symlink to a versioned path
/// (e.g. `~/.local/share/claude/versions/2.1.50`), so tmux resolves
/// the symlink and reports the version number as the command name.
/// The versions directory path is resolved once at startup (`which claude`),
/// while its contents are refreshed every 30 seconds so that new CLI
/// versions installed while claudeye is running are detected.
fn claude_version_names() -> HashSet<String> {
    let mut cache = version_cache().lock().unwrap_or_else(|e| e.into_inner());
    if cache.last_refresh.elapsed() >= VERSION_CACHE_TTL {
        reload_entries(&mut cache);
    }
    cache.names.clone()
}

/// Force-refresh the version cache regardless of TTL.
pub fn refresh_version_cache() {
    let mut cache = version_cache().lock().unwrap_or_else(|e| e.into_inner());
    reload_entries(&mut cache);
}

fn reload_entries(cache: &mut VersionCache) {
    if let Some(ref dir) = cache.versions_dir
        && let Some(entries) = read_version_entries(dir)
    {
        cache.names = entries;
    }
    cache.last_refresh = Instant::now();
}

fn init_version_cache() -> (Option<PathBuf>, HashSet<String>) {
    let Some(dir) = resolve_versions_dir() else {
        return (None, HashSet::new());
    };
    let names = read_version_entries(&dir).unwrap_or_default();
    (Some(dir), names)
}

fn resolve_versions_dir() -> Option<PathBuf> {
    let output = Command::new("which").arg("claude").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let target = std::fs::read_link(&path).ok()?;
    Some(target.parent()?.to_path_buf())
}

pub fn read_version_entries(dir: &Path) -> Option<HashSet<String>> {
    let entries = std::fs::read_dir(dir)
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
