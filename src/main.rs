mod claude_state;
mod monitor;
mod tmux;

use clap::Parser;
use eframe::egui::{self, Color32, RichText, Vec2};
use monitor::{ClaudeSession, start_polling};
use claude_state::ClaudeState;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(about = "Claude session monitor overlay")]
struct Args {
    /// Background opacity (0.0 = fully transparent, 1.0 = fully opaque)
    #[arg(long, default_value_t = 0.24, value_parser = parse_opacity)]
    opacity: f32,
}

fn parse_opacity(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| format!("'{s}' is not a valid number"))?;
    if (0.0..=1.0).contains(&v) {
        Ok(v)
    } else {
        Err(format!("opacity must be between 0.0 and 1.0, got {v}"))
    }
}

const REPAINT_INTERVAL_SECS: u64 = 2;
const WINDOW_WIDTH: f32 = 280.0;
const WINDOW_EMPTY_HEIGHT: f32 = 80.0;
const ROW_HEIGHT: f32 = 50.0;

fn main() -> eframe::Result<()> {
    let args = Args::parse();
    let sessions: Arc<Mutex<Vec<ClaudeSession>>> = Arc::new(Mutex::new(vec![]));
    start_polling(Arc::clone(&sessions));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_always_on_top()
            .with_mouse_passthrough(true)
            .with_inner_size([WINDOW_WIDTH, WINDOW_EMPTY_HEIGHT])
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "ccmonitor",
        options,
        Box::new(|_cc| Ok(Box::new(CcMonitorApp { sessions, positioned: false, opacity: args.opacity }))),
    )
}

const WINDOW_TOP_MARGIN: f32 = 20.0;

struct CcMonitorApp {
    sessions: Arc<Mutex<Vec<ClaudeSession>>>,
    positioned: bool,
    opacity: f32,
}

impl eframe::App for CcMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_secs(REPAINT_INTERVAL_SECS));
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
        ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(true));

        if !self.positioned {
            if let Some(monitor_size) = ctx.input(|i| i.viewport().monitor_size) {
                let x = (monitor_size.x - WINDOW_WIDTH) / 2.0;
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, WINDOW_TOP_MARGIN)));
                self.positioned = true;
            }
        }

        let sessions = match self.sessions.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return, // poisoned mutex: polling thread panicked
        };
        let window_height = if sessions.is_empty() {
            WINDOW_EMPTY_HEIGHT
        } else {
            sessions.len() as f32 * ROW_HEIGHT
        };

        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
            WINDOW_WIDTH,
            window_height,
        )));

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(20, 20, 20, (self.opacity * 255.0) as u8)))
            .show(ctx, |ui| {
                if sessions.is_empty() {
                    ui.label(
                        RichText::new("No Claude sessions found")
                            .color(Color32::from_gray(120))
                            .size(12.0),
                    );
                } else {
                    for session in &sessions {
                        ui.label(format_session_line(session));
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
