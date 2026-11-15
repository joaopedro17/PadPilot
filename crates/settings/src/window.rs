//! The window: large sliders and buttons, so a trackpad is all it takes, and
//! no text fields that would need the on-screen keyboard.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use eframe::egui::{self, FontId, RichText, TextStyle, Vec2};
use padpilot_core::config::{Config, LogLevel, SHOW_SETTINGS, TEMPLATE};
use padpilot_core::stick::{LiftBehavior, StickSettings};
use tracing::{Level, error, info};

/// Changes are written once the slider has rested this long.
const SAVE_AFTER: Duration = Duration::from_millis(300);
const TRAVEL_MM: std::ops::RangeInclusive<f32> = 3.0..=30.0;
const DEAD_ZONE_PERCENT: std::ops::RangeInclusive<f32> = 0.0..=30.0;

/// What the joystick asked for, from the thread that listens to it.
#[derive(Default)]
struct Requests {
    show: AtomicBool,
    /// The joystick closed: time to go.
    quit: AtomicBool,
}

pub fn run(hidden: bool) -> ExitCode {
    // Never returns normally: see the end.
    let logs = padpilot_logging::logs_dir();
    let Ok(log_guard) = padpilot_logging::init(&logs, "settings", Level::INFO) else {
        return ExitCode::FAILURE;
    };
    let path = padpilot_logging::config_path();
    let (config, problem) = read(&path);
    info!(target: "settings", path = %path.display(), ?config, ?problem, hidden, "settings window started");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("PadPilot settings")
            .with_inner_size([460.0, 640.0])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_visible(!hidden)
            // Waiting hidden must never take focus from the game.
            .with_active(!hidden)
            .with_icon(icon()),
        centered: true,
        ..Default::default()
    };
    let window = SettingsWindow {
        path,
        logs,
        config,
        problem,
        unsaved_since: None,
        requests: hidden.then(|| Arc::new(Requests::default())),
        quitting: false,
    };
    let result = eframe::run_native(
        "PadPilot settings",
        options,
        Box::new(|creation| {
            large_style(&creation.egui_ctx);
            if let Some(requests) = &window.requests {
                listen(Arc::clone(requests), creation.egui_ctx.clone());
            }
            Ok(Box::new(window))
        }),
    );
    let code = match result {
        Ok(()) => {
            info!(target: "settings", "settings window closed");
            0
        }
        Err(err) => {
            error!(target: "settings", "cannot show the settings window: {err}");
            1
        }
    };
    // The graphics library can leave the process hanging after its window
    // is gone; flush the log, then end the process for sure.
    drop(log_guard);
    std::process::exit(code)
}

struct SettingsWindow {
    path: PathBuf,
    logs: PathBuf,
    config: Config,
    /// What was wrong with the file when the window opened.
    problem: Option<String>,
    unsaved_since: Option<Instant>,
    /// Set when started hidden by the joystick.
    requests: Option<Arc<Requests>>,
    quitting: bool,
}

/// Reads the joystick's requests on a separate thread and wakes the window.
fn listen(requests: Arc<Requests>, ctx: egui::Context) {
    std::thread::spawn(move || {
        for byte in std::io::stdin().lock().bytes() {
            match byte {
                Ok(SHOW_SETTINGS) => requests.show.store(true, Ordering::SeqCst),
                Ok(_) => continue,
                Err(_) => break,
            }
            ctx.request_repaint();
        }
        requests.quit.store(true, Ordering::SeqCst);
        ctx.request_repaint();
    });
}

fn read(path: &Path) -> (Config, Option<String>) {
    match std::fs::read_to_string(path) {
        Ok(text) => match Config::parse(&text) {
            Ok(config) => (config, None),
            Err(err) => (Config::default(), Some(err.to_string())),
        },
        Err(_) => (Config::default(), None),
    }
}

impl eframe::App for SettingsWindow {
    /// Shows and hides the window for the joystick; closing only hides it.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Some(requests) = &self.requests else {
            return;
        };
        if requests.quit.load(Ordering::SeqCst) && !self.quitting {
            self.quitting = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if requests.show.swap(false, Ordering::SeqCst) {
            // The file may have changed since it was last shown.
            if self.unsaved_since.is_none() {
                (self.config, self.problem) = read(&self.path);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            info!(target: "settings", "settings window shown");
        }
        if !self.quitting && ctx.input(|input| input.viewport().close_requested()) {
            if self.unsaved_since.is_some() {
                self.save();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            info!(target: "settings", "settings window hidden");
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            let before = self.config.clone();
            self.controls(ui);
            if self.config != before {
                self.unsaved_since = Some(Instant::now());
            }
        });
        let resting = !ui.ctx().input(|input| input.pointer.any_down());
        match self.unsaved_since {
            Some(since) if resting && since.elapsed() >= SAVE_AFTER => self.save(),
            Some(_) => ui.ctx().request_repaint_after(Duration::from_millis(50)),
            None => {}
        }
    }

    fn on_exit(&mut self) {
        if self.unsaved_since.is_some() {
            self.save();
        }
    }
}

impl SettingsWindow {
    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        if let Some(problem) = &self.problem {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("config.toml has a mistake and was not used:\n{problem}\nChanging a setting rewrites the file."),
            );
            ui.add_space(12.0);
        }

        let stick = self.config.stick;
        let mut travel = stick.full_travel();
        let mut dead_zone = stick.dead_zone() * 100.0;
        let mut lift = stick.lift_behavior();

        heading(ui, "Sensitivity");
        ui.label("Finger travel from the center to 100%. Smaller is sharper.");
        ui.horizontal(|ui| {
            ui.add(
                egui::Slider::new(&mut travel, TRAVEL_MM)
                    .step_by(0.5)
                    .show_value(false),
            );
            ui.label(format!("{travel:.1} mm"));
        });
        ui.add_space(16.0);

        heading(ui, "Dead zone");
        ui.label("The part around the center that does nothing.");
        ui.horizontal(|ui| {
            ui.add(
                egui::Slider::new(&mut dead_zone, DEAD_ZONE_PERCENT)
                    .step_by(1.0)
                    .show_value(false),
            );
            ui.label(format!("{dead_zone:.0} %"));
        });
        ui.add_space(16.0);

        heading(ui, "When you lift your finger");
        ui.horizontal(|ui| {
            big_choice(ui, &mut lift, LiftBehavior::ReturnToCenter, "Spring back");
            big_choice(ui, &mut lift, LiftBehavior::HoldPosition, "Hold position");
        });
        ui.add_space(16.0);

        if let Ok(changed) = StickSettings::new(lift, travel, dead_zone / 100.0) {
            self.config.stick = changed;
        }

        heading(ui, "Log detail");
        ui.label("Takes effect the next time PadPilot starts.");
        ui.horizontal(|ui| {
            let level = &mut self.config.log_level;
            big_choice(ui, level, LogLevel::Warn, "Problems");
            big_choice(ui, level, LogLevel::Info, "Normal");
            big_choice(ui, level, LogLevel::Debug, "Detailed");
            big_choice(ui, level, LogLevel::Trace, "Everything");
        });
        ui.add_space(24.0);

        ui.horizontal(|ui| {
            if ui
                .add_sized([200.0, 44.0], egui::Button::new("Open logs folder"))
                .clicked()
            {
                self.open_logs();
            }
            if ui
                .add_sized([200.0, 44.0], egui::Button::new("Restore defaults"))
                .clicked()
            {
                info!(target: "settings", "defaults restored");
                self.config = Config::default();
            }
        });
        ui.add_space(16.0);
        ui.label(RichText::new("Changes apply to the joystick right away.").weak());
    }

    /// Writes the settings into the file as it is now, keeping its comments.
    fn save(&mut self) {
        self.unsaved_since = None;
        let current = std::fs::read_to_string(&self.path).unwrap_or_else(|_| TEMPLATE.to_owned());
        let text = self
            .config
            .write_into(&current.replace("\r\n", "\n"))
            .replace('\n', "\r\n");
        let temporary = self.path.with_extension("toml.new");
        let written = self
            .path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&temporary, text))
            .and_then(|()| std::fs::rename(&temporary, &self.path));
        match written {
            Ok(()) => {
                self.problem = None;
                info!(target: "settings", config = ?self.config, "settings saved");
            }
            Err(err) => {
                error!(target: "settings", path = %self.path.display(), "cannot save the settings: {err}")
            }
        }
    }

    fn open_logs(&self) {
        let _ = std::fs::create_dir_all(&self.logs);
        if let Err(err) = std::process::Command::new("explorer.exe")
            .arg(&self.logs)
            .spawn()
        {
            error!(target: "settings", "cannot open the logs folder: {err}");
        }
    }
}

/// The icon's 64-pixel image, from the same file that is built into the
/// program. Without it, the window shows egui's own logo.
fn icon() -> Arc<egui::IconData> {
    const ICO: &[u8] = include_bytes!("../../../assets/padpilot.ico");
    const SIDE: usize = 64;
    let image = (0..usize::from(u16::from_le_bytes([ICO[4], ICO[5]]))).find_map(|index| {
        let entry = &ICO[6 + 16 * index..22 + 16 * index];
        let offset = u32::from_le_bytes(entry[12..16].try_into().ok()?) as usize;
        (usize::from(entry[0]) == SIDE).then(|| &ICO[offset..])
    });
    let Some(image) = image else {
        return Arc::default();
    };
    // A 32-bit bitmap after its 40-byte header, bottom row first, BGRA.
    let pixels = &image[40..40 + SIDE * SIDE * 4];
    let rgba = pixels
        .chunks(SIDE * 4)
        .rev()
        .flat_map(|row| row.chunks(4).flat_map(|p| [p[2], p[1], p[0], p[3]]))
        .collect();
    Arc::new(egui::IconData {
        rgba,
        width: SIDE as u32,
        height: SIDE as u32,
    })
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).heading());
}

/// A wide button that stays highlighted while its choice is selected.
fn big_choice<T: PartialEq + Copy>(ui: &mut egui::Ui, current: &mut T, choice: T, text: &str) {
    let selected = *current == choice;
    let button = egui::Button::selectable(selected, text).min_size(Vec2::new(96.0, 44.0));
    if ui.add(button).clicked() {
        *current = choice;
    }
}

/// Larger text and targets than egui's defaults, for a trackpad.
fn large_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(22.0)),
            (TextStyle::Body, FontId::proportional(17.0)),
            (TextStyle::Button, FontId::proportional(17.0)),
            (TextStyle::Monospace, FontId::monospace(15.0)),
            (TextStyle::Small, FontId::proportional(13.0)),
        ]
        .into();
        style.spacing.slider_width = 300.0;
        style.spacing.slider_rail_height = 10.0;
        style.spacing.interact_size = Vec2::new(48.0, 40.0);
        style.spacing.button_padding = Vec2::new(14.0, 10.0);
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    });
}
