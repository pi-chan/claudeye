use std::process::Command;

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

    if command.trim() != "claude" {
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
