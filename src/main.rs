mod claude_state;
mod monitor;
mod tmux;

use eframe::egui::{self, Color32, RichText, Vec2};
use monitor::{ClaudeSession, start_polling};
use claude_state::ClaudeState;
use std::sync::{Arc, Mutex};

/// UI の再描画間隔（秒）
const REPAINT_INTERVAL_SECS: u64 = 2;
/// ウィンドウ幅（px）
const WINDOW_WIDTH: f32 = 280.0;
/// ウィンドウの初期高さ（px）
const WINDOW_INITIAL_HEIGHT: f32 = 120.0;
/// セッションが空の場合のウィンドウ高さ（px）
const WINDOW_EMPTY_HEIGHT: f32 = 80.0;
/// セッション行の高さ（px）
const ROW_HEIGHT: f32 = 50.0;
/// ヘッダー部分の高さ（px）
const WINDOW_HEADER_HEIGHT: f32 = 30.0;

fn main() -> eframe::Result<()> {
    let sessions: Arc<Mutex<Vec<ClaudeSession>>> = Arc::new(Mutex::new(vec![]));
    start_polling(Arc::clone(&sessions));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_always_on_top()
            .with_inner_size([WINDOW_WIDTH, WINDOW_INITIAL_HEIGHT])
            .with_position([20.0, 20.0])
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "ccmonitor",
        options,
        Box::new(|_cc| Ok(Box::new(CcMonitorApp { sessions }))),
    )
}

struct CcMonitorApp {
    sessions: Arc<Mutex<Vec<ClaudeSession>>>,
}

impl eframe::App for CcMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // REPAINT_INTERVAL_SECS 秒ごとに再描画してポーリング結果を反映
        ctx.request_repaint_after(std::time::Duration::from_secs(REPAINT_INTERVAL_SECS));

        // 常に最前面を維持
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));

        let sessions = match self.sessions.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return, // ポーリングスレッドがパニックして Mutex がポイズンされた場合はスキップ
        };
        let window_height = if sessions.is_empty() {
            WINDOW_EMPTY_HEIGHT
        } else {
            sessions.len() as f32 * ROW_HEIGHT + WINDOW_HEADER_HEIGHT
        };

        // ウィンドウサイズを動的に調整
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
            WINDOW_WIDTH,
            window_height,
        )));

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(20, 20, 20, 60)))
            .show(ctx, |ui| {
                // タイトルバー（ドラッグ可能）
                let title_response = ui.add(
                    egui::Label::new(
                        RichText::new("● Claude Code Monitor")
                            .color(Color32::from_gray(180))
                            .size(11.0),
                    )
                    .sense(egui::Sense::click_and_drag()),
                );
                if title_response.dragged() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                ui.separator();

                if sessions.is_empty() {
                    ui.label(
                        RichText::new("No Claude sessions found")
                            .color(Color32::from_gray(120))
                            .size(12.0),
                    );
                } else {
                    for session in &sessions {
                        let row_resp = ui.add(
                            egui::Label::new(format_session_line(session))
                                .sense(egui::Sense::click()),
                        );
                        if row_resp.clicked() {
                            tmux::switch_to_pane(&session.pane.id);
                        }
                        if row_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                }
            });
    }
}

fn format_session_line(session: &ClaudeSession) -> RichText {
    let (indicator, color, label) = match &session.state {
        ClaudeState::Working => ("●", Color32::from_rgb(80, 200, 80), "WORKING"),
        ClaudeState::WaitingForApproval => ("●", Color32::from_rgb(220, 180, 0), "APPROVAL"),
        ClaudeState::WaitingForAnswer => ("●", Color32::from_rgb(80, 150, 220), "ANSWER"),
        ClaudeState::Idle => ("○", Color32::from_gray(160), "IDLE"),
        ClaudeState::NotRunning => ("✕", Color32::from_rgb(200, 60, 60), "STOPPED"),
    };

    let text = format!(
        "{} {}  {}  [{}]",
        indicator, session.pane.id, session.pane.project_name, label
    );

    RichText::new(text).color(color).size(13.0)
}
