use ccmonitor::tmux::parse_pane_line;

#[test]
fn parse_valid_pane_line_claude() {
    let line = "main:0.1 12345 /home/user/projects/myapp claude";
    let result = parse_pane_line(line);
    assert!(result.is_some());
    let pane = result.unwrap();
    assert_eq!(pane.id, "main:0.1");
    assert_eq!(pane.pid, 12345);
    assert_eq!(pane.cwd, "/home/user/projects/myapp");
    assert_eq!(pane.project_name, "myapp");
}

#[test]
fn parse_pane_line_non_claude_command_returns_none() {
    let line = "main:0.0 9999 /home/user bash";
    let result = parse_pane_line(line);
    assert!(result.is_none());
}

#[test]
fn parse_pane_line_insufficient_fields_returns_none() {
    let line = "main:0.1 12345 /home/user";
    let result = parse_pane_line(line);
    assert!(result.is_none());
}

#[test]
fn parse_pane_line_invalid_pid_returns_none() {
    let line = "main:0.1 notanumber /home/user claude";
    let result = parse_pane_line(line);
    assert!(result.is_none());
}

#[test]
fn project_name_is_basename_of_cwd() {
    let line = "work:2.3 54321 /home/maedana/tmp/ccmonitor claude";
    let result = parse_pane_line(line);
    assert!(result.is_some());
    let pane = result.unwrap();
    assert_eq!(pane.project_name, "ccmonitor");
}
