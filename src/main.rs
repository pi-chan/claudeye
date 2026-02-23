mod claude_state;
mod monitor;
mod picker;
mod tmux;

use clap::{Parser, Subcommand};
use eframe::egui::{self, Color32, RichText, Ui, Vec2};
use monitor::{ClaudeSession, start_polling};
use claude_state::ClaudeState;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(about = "Claude session monitor overlay")]
struct Args {
    /// Background opacity (0.0 = fully transparent, 1.0 = fully opaque)
    #[arg(long, default_value_t = 0.24, value_parser = parse_opacity)]
    opacity: f32,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive TUI session picker
    Picker,
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
const WINDOW_WIDTH: f32 = 300.0;
const WINDOW_EMPTY_HEIGHT: f32 = 40.0;
const ROW_HEIGHT: f32 = 20.0;
const WINDOW_PADDING: f32 = 8.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    match args.command {
        Some(Commands::Picker) => picker::run_picker()?,
        None => run_gui(args.opacity)?,
    }
    Ok(())
}

fn run_gui(opacity: f32) -> eframe::Result<()> {
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
        "claudeye",
        options,
        Box::new(|_cc| Ok(Box::new(CcMonitorApp { sessions, positioned: false, opacity }))),
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

        let has_working = sessions.iter().any(|s| matches!(
            s.state,
            ClaudeState::Working | ClaudeState::WaitingForApproval
        ));
        if has_working {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs(REPAINT_INTERVAL_SECS));
        }

        let time = ctx.input(|i| i.time);

        let window_height = if sessions.is_empty() {
            WINDOW_EMPTY_HEIGHT
        } else {
            sessions.len() as f32 * ROW_HEIGHT + WINDOW_PADDING * 2.0
        };

        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
            WINDOW_WIDTH,
            window_height,
        )));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(20, 20, 20, (self.opacity * 255.0) as u8))
                    .inner_margin(egui::Margin::symmetric(8.0, WINDOW_PADDING)),
            )
            .show(ctx, |ui| {
                if sessions.is_empty() {
                    ui.label(
                        RichText::new("No Claude sessions found")
                            .color(Color32::from_gray(120))
                            .size(12.0),
                    );
                } else {
                    for session in &sessions {
                        render_session_row(ui, session, time);
                    }
                }
            });
    }
}

fn render_session_row(ui: &mut Ui, session: &ClaudeSession, time: f64) {
    let (state_color, label) = match &session.state {
        ClaudeState::Working => (Color32::from_rgb(80, 200, 80), "WORKING"),
        ClaudeState::WaitingForApproval => (Color32::from_rgb(220, 180, 0), "APPROVAL"),
        ClaudeState::Idle => (Color32::from_gray(160), "IDLE"),
    };

    ui.horizontal(|ui| {
        // Mini robot art or spinner (fixed-width column, center-aligned)
        ui.allocate_ui(egui::Vec2::new(40.0, ROW_HEIGHT), |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let p = Color32::from_rgb(130, 80, 200);  // purple
                let o = Color32::from_rgb(210, 110, 30);  // orange
                let head_color = if matches!(
                    session.state,
                    ClaudeState::Working | ClaudeState::WaitingForApproval
                ) {
                    if (time * 2.0) as usize % 2 == 0 { p } else { state_color }
                } else {
                    p
                };
                let lines: [(&str, Color32); 4] = [
                    ("▟█▙", head_color),
                    ("▐▛███▜▌", o),
                    ("▝▜█████▛▘", o),
                    ("▘▘ ▝▝", o),
                ];
                for (text, color) in lines {
                    ui.label(RichText::new(text).size(5.0).color(color).monospace());
                }
            });
        });

        ui.add_space(4.0);

        // Session info, roughly vertically centered
        ui.vertical(|ui| {
            ui.add_space(3.0);
            ui.label(
                RichText::new(format!(
                    "{}  {}  [{}]",
                    session.pane.id, session.pane.project_name, label
                ))
                .color(state_color)
                .size(13.0),
            );
        });
    });
}
