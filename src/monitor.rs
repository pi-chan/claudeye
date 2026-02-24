use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const POLL_INTERVAL_SECS: u64 = 2;

use crate::claude_state::{detect_state, ClaudeState};
use crate::tmux::{self, PaneInfo};

#[derive(Debug, Clone)]
pub struct ClaudeSession {
    pub pane: PaneInfo,
    pub state: ClaudeState,
    pub state_changed_at: Instant,
}

pub fn start_polling(sessions: Arc<Mutex<Vec<ClaudeSession>>>) {
    thread::spawn(move || loop {
        let panes = tmux::list_claude_panes();
        let prev = sessions.lock().ok().map(|g| g.clone()).unwrap_or_default();
        let now = Instant::now();
        let updated: Vec<ClaudeSession> = panes
            .into_iter()
            .map(|pane| {
                let content = tmux::capture_pane(&pane.id);
                let state = detect_state(&content);
                let state_changed_at = prev
                    .iter()
                    .find(|s| s.pane.id == pane.id && s.state == state)
                    .map(|s| s.state_changed_at)
                    .unwrap_or(now);
                ClaudeSession { pane, state, state_changed_at }
            })
            .collect();

        if let Ok(mut lock) = sessions.lock() {
            *lock = updated;
        }

        thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
    });
}
