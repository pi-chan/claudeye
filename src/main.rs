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
#[command(about = "Claude session monitor overlay", version)]
struct Args {
    /// Show one session at a time, cycling every second
    #[arg(long)]
    compact: bool,

    /// Overlay window position on screen
    #[arg(long, short, default_value = "top-center", value_enum)]
    position: Position,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive TUI session picker
    Picker,
}

#[derive(Clone, Copy, Default, clap::ValueEnum)]
enum Position {
    TopLeft,
    #[default]
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Position {
    fn compute(self, monitor: Vec2, window: Vec2) -> egui::Pos2 {
        let x = match self {
            Position::TopLeft | Position::MiddleLeft | Position::BottomLeft => MARGIN,
            Position::TopCenter | Position::MiddleCenter | Position::BottomCenter => {
                (monitor.x - window.x) / 2.0
            }
            Position::TopRight | Position::MiddleRight | Position::BottomRight => {
                monitor.x - window.x - MARGIN
            }
        };
        let y = match self {
            Position::TopLeft | Position::TopCenter | Position::TopRight => MARGIN,
            Position::MiddleLeft | Position::MiddleCenter | Position::MiddleRight => {
                (monitor.y - window.y) / 2.0
            }
            Position::BottomLeft | Position::BottomCenter | Position::BottomRight => {
                monitor.y - window.y - MARGIN
            }
        };
        egui::pos2(x, y)
    }
}

const REPAINT_INTERVAL_SECS: u64 = 2;
const WINDOW_WIDTH: f32 = 300.0;
const WINDOW_EMPTY_HEIGHT: f32 = 40.0;
const ROW_HEIGHT: f32 = 22.0;
const WINDOW_PADDING: f32 = 8.0;
const MARGIN: f32 = 2.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    match args.command {
        Some(Commands::Picker) => picker::run_picker()?,
        None => run_gui(args.compact, args.position)?,
    }
    Ok(())
}

fn run_gui(compact: bool, position: Position) -> eframe::Result<()> {
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
        Box::new(|_cc| Ok(Box::new(CcMonitorApp { sessions, compact, position }))),
    )
}

struct CcMonitorApp {
    sessions: Arc<Mutex<Vec<ClaudeSession>>>,
    compact: bool,
    position: Position,
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

        let sessions = match self.sessions.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return, // poisoned mutex: polling thread panicked
        };

        let needs_fast_repaint = sessions.iter().any(|s| matches!(s.state, ClaudeState::Working | ClaudeState::WaitingForApproval));
        if needs_fast_repaint || self.compact {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        } else if !sessions.is_empty() {
            // Repaint every second to keep elapsed time display up to date
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
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

        if let Some(monitor_size) = ctx.input(|i| i.viewport().monitor_size) {
            let pos = self.position.compute(monitor_size, Vec2::new(WINDOW_WIDTH, window_height));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
        }

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

fn calc_stroke_width(state: &ClaudeState, time: f64) -> f32 {
    match state {
        ClaudeState::WaitingForApproval => {
            let pulse = ((time * 16.0).sin() as f32 + 1.0) / 2.0;
            1.0 + pulse * 2.0
        }
        ClaudeState::Working | ClaudeState::Idle => 1.0,
    }
}

fn render_session_row(ui: &mut Ui, session: &ClaudeSession, time: f64) {
    let (state_color, label) = match &session.state {
        ClaudeState::Working => (Color32::from_rgb(80, 200, 80), "Running"),
        ClaudeState::WaitingForApproval => (Color32::from_rgb(220, 180, 0), "Approval"),
        ClaudeState::Idle => (Color32::from_gray(160), "Idle"),
    };

    let stroke_width = calc_stroke_width(&session.state, time);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        // Mini robot art or spinner (fixed-width column, center-aligned)
        ui.allocate_ui(egui::Vec2::new(40.0, ROW_HEIGHT), |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let o = Color32::from_rgb(210, 110, 30);  // orange
                let lines: [(&str, Color32); 4] = [
                    ("▟█▙", state_color),
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
            .stroke(egui::Stroke::new(stroke_width, state_color))
            .rounding(egui::Rounding::same(5.0))
            .inner_margin(egui::Margin::symmetric(6.0, 2.0))
            .show(ui, |ui: &mut Ui| {
                ui.set_max_width(max_label_width);
                let elapsed = session.state_changed_at.elapsed().as_secs();
                ui.label(
                    RichText::new(format!(
                        "{}  {}  [{}] {}s",
                        session.pane.id, session.pane.project_name, label, elapsed
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
        painter.line_segment([tail_tip, tail_top], egui::Stroke::new(stroke_width, state_color));
        painter.line_segment([tail_tip, tail_bot], egui::Stroke::new(stroke_width, state_color));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stroke_width_working_is_always_one() {
        assert_eq!(calc_stroke_width(&ClaudeState::Working, 0.0), 1.0);
        assert_eq!(calc_stroke_width(&ClaudeState::Working, 5.0), 1.0);
    }

    #[test]
    fn stroke_width_idle_is_always_one() {
        assert_eq!(calc_stroke_width(&ClaudeState::Idle, 0.0), 1.0);
        assert_eq!(calc_stroke_width(&ClaudeState::Idle, 5.0), 1.0);
    }

    #[test]
    fn position_top_center_default() {
        let monitor = Vec2::new(1920.0, 1080.0);
        let window = Vec2::new(300.0, 40.0);
        let pos = Position::TopCenter.compute(monitor, window);
        assert_eq!(pos.x, (1920.0 - 300.0) / 2.0);
        assert_eq!(pos.y, MARGIN);
    }

    #[test]
    fn position_bottom_right() {
        let monitor = Vec2::new(1920.0, 1080.0);
        let window = Vec2::new(300.0, 40.0);
        let pos = Position::BottomRight.compute(monitor, window);
        assert_eq!(pos.x, 1920.0 - 300.0 - MARGIN);
        assert_eq!(pos.y, 1080.0 - 40.0 - MARGIN);
    }

    #[test]
    fn position_middle_center() {
        let monitor = Vec2::new(1920.0, 1080.0);
        let window = Vec2::new(300.0, 40.0);
        let pos = Position::MiddleCenter.compute(monitor, window);
        assert_eq!(pos.x, (1920.0 - 300.0) / 2.0);
        assert_eq!(pos.y, (1080.0 - 40.0) / 2.0);
    }

    #[test]
    fn stroke_width_approval_always_pulses_strongly() {
        let mut saw_peak = false;
        for t in 0..100 {
            let time = t as f64 * 0.1;
            let w = calc_stroke_width(&ClaudeState::WaitingForApproval, time);
            assert!(w >= 1.0 && w <= 3.0, "got {w} at time {time}");
            if w > 2.5 {
                saw_peak = true;
            }
        }
        assert!(saw_peak, "should reach near 3.0");
    }
}
