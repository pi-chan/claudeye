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
    /// Show one session at a time, cycling every second
    #[arg(long)]
    compact: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive TUI session picker
    Picker,
}

const REPAINT_INTERVAL_SECS: u64 = 2;
const WINDOW_WIDTH: f32 = 300.0;
const WINDOW_EMPTY_HEIGHT: f32 = 40.0;
const ROW_HEIGHT: f32 = 22.0;
const WINDOW_PADDING: f32 = 8.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    match args.command {
        Some(Commands::Picker) => picker::run_picker()?,
        None => run_gui(args.compact)?,
    }
    Ok(())
}

fn run_gui(compact: bool) -> eframe::Result<()> {
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
        Box::new(|_cc| Ok(Box::new(CcMonitorApp { sessions, positioned: false, compact }))),
    )
}

const WINDOW_TOP_MARGIN: f32 = 2.0;

struct CcMonitorApp {
    sessions: Arc<Mutex<Vec<ClaudeSession>>>,
    positioned: bool,
    compact: bool,
}

impl eframe::App for CcMonitorApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut visuals = ctx.style().visuals.clone();
        visuals.panel_fill = Color32::TRANSPARENT;
        ctx.set_visuals(visuals);

        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
        ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(true));

        if !self.positioned
            && let Some(monitor_size) = ctx.input(|i| i.viewport().monitor_size)
        {
            let x = (monitor_size.x - WINDOW_WIDTH) / 2.0;
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, WINDOW_TOP_MARGIN)));
            self.positioned = true;
        }

        let sessions = match self.sessions.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return, // poisoned mutex: polling thread panicked
        };

        let has_working = sessions.iter().any(|s| matches!(
            s.state,
            ClaudeState::Working | ClaudeState::WaitingForApproval
        ));
        if has_working || self.compact {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs(REPAINT_INTERVAL_SECS));
        }

        let time = ctx.input(|i| i.time);

        // In compact mode, show one session at a time cycling every second
        let display_sessions: Vec<&ClaudeSession> = if self.compact && !sessions.is_empty() {
            let idx = (time as usize) % sessions.len();
            vec![&sessions[idx]]
        } else {
            sessions.iter().collect()
        };

        let n = display_sessions.len() as f32;
        let window_height = if display_sessions.is_empty() {
            WINDOW_EMPTY_HEIGHT
        } else {
            // ROW_HEIGHT per row + 4px item_spacing between rows + top/bottom padding
            n * ROW_HEIGHT + (n - 1.0) * 4.0 + WINDOW_PADDING * 2.0
        };

        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
            WINDOW_WIDTH,
            window_height,
        )));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(egui::Margin::symmetric(8.0, WINDOW_PADDING)),
            )
            .show(ctx, |ui| {
                if display_sessions.is_empty() {
                    ui.label(
                        RichText::new("No Claude sessions found")
                            .color(Color32::from_gray(120))
                            .size(12.0),
                    );
                } else {
                    for session in &display_sessions {
                        render_session_row(ui, session, time);
                    }
                }
            });
    }
}

fn render_session_row(ui: &mut Ui, session: &ClaudeSession, time: f64) {
    let (state_color, label) = match &session.state {
        ClaudeState::Working => (Color32::from_rgb(80, 200, 80), "Running"),
        ClaudeState::WaitingForApproval => (Color32::from_rgb(220, 180, 0), "Approval"),
        ClaudeState::Idle => (Color32::from_gray(160), "Idle"),
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
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
                    if ((time * 2.0) as usize).is_multiple_of(2) { p } else { state_color }
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

        // Speech bubble with tail pointing left toward robot
        ui.add_space(2.0); // space for the tail triangle

        // Clamp bubble width to remaining available space (minus inner padding + stroke)
        let max_label_width = (ui.available_width() - 14.0).max(0.0);

        let bubble_fill = Color32::from_rgba_unmultiplied(30, 30, 45, 220);
        let inner = egui::Frame::none()
            .fill(bubble_fill)
            .stroke(egui::Stroke::new(1.0, state_color))
            .rounding(egui::Rounding::same(5.0))
            .inner_margin(egui::Margin::symmetric(6.0, 2.0))
            .show(ui, |ui: &mut Ui| {
                ui.set_max_width(max_label_width);
                ui.label(
                    RichText::new(format!(
                        "{}  {}  [{}]",
                        session.pane.id, session.pane.project_name, label
                    ))
                    .color(state_color)
                    .size(11.0),
                );
            });

        // Draw tail triangle pointing left toward the robot
        let rect = inner.response.rect;
        let mid_y = rect.center().y;
        let tail_tip = egui::pos2(rect.left() - 4.0, mid_y);
        let tail_top = egui::pos2(rect.left(), mid_y - 4.0);
        let tail_bot = egui::pos2(rect.left(), mid_y + 4.0);
        let painter = ui.painter();
        painter.add(egui::Shape::convex_polygon(
            vec![tail_tip, tail_top, tail_bot],
            bubble_fill,
            egui::Stroke::NONE,
        ));
        painter.line_segment([tail_tip, tail_top], egui::Stroke::new(1.0, state_color));
        painter.line_segment([tail_tip, tail_bot], egui::Stroke::new(1.0, state_color));
    });
}
