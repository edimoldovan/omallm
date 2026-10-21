//! omallm: ask Omarchy in your own words, get the command, run it after a look.

mod model;
mod omarchy;
mod runner;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Context, Key, Modifiers, RichText, ScrollArea, TextEdit, TextStyle};
use omarchy_kit::theme::{self, ThemeWatcher};
use omarchy_kit::{Button, Choice, CommandPalette, Size, StatusLine, TextField, field, fonts, icons};

use omarchy::{Answer, Scripts};

struct Turn {
    question: String,
    answer: Answer,
    /// The proposed command line; the owner can edit it before running.
    command: String,
    note: String,
    output: String,
    run: Option<runner::Run>,
    finished: bool,
}

struct App {
    model: Result<model::Model, String>,
    model_name: String,
    scripts: Scripts,
    turns: Vec<Turn>,
    input: String,
    status: String,
    theme_watcher: ThemeWatcher,
    palette: CommandPalette,
    focus_input: bool,
}

/// The model file: the first argument, else $OMALLM_MODEL, else ~/models/omallm.gguf.
fn model_path() -> PathBuf {
    model_path_from(std::env::args().nth(1))
}

fn model_path_from(arg: Option<String>) -> PathBuf {
    if let Some(arg) = arg {
        return PathBuf::from(arg);
    }
    if let Ok(path) = std::env::var("OMALLM_MODEL") {
        return PathBuf::from(path);
    }
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("models/omallm.gguf")
}

fn main() -> eframe::Result {
    // `omallm --ask "question"` prints the model's answer and exits; nothing is run.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--ask") {
        let question = args[2..].join(" ");
        let path = model_path_from(None);
        match model::Model::load(&path).and_then(|m| m.answer(&question)) {
            Ok(answer) => println!("{answer}"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("omallm")
            .with_inner_size([760.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "omallm",
        options,
        Box::new(|cc| Ok(Box::new(App::new(&cc.egui_ctx)))),
    )
}

impl App {
    fn new(ctx: &Context) -> App {
        ctx.set_visuals(theme::visuals(&theme::load()));
        fonts::install(ctx);
        let path = model_path();
        let model = model::Model::load(&path);
        let scripts = Scripts::load();
        let status = match &model {
            Ok(_) => format!("{} Omarchy scripts known · ask away", scripts.len()),
            Err(e) => e.clone(),
        };
        App {
            model,
            model_name: path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            scripts,
            turns: Vec::new(),
            input: String::new(),
            status,
            theme_watcher: ThemeWatcher::new(),
            palette: CommandPalette::default(),
            focus_input: true,
        }
    }

    fn ask(&mut self) {
        let question = self.input.trim().to_string();
        if question.is_empty() {
            return;
        }
        self.input.clear();
        let Ok(model) = &self.model else {
            self.status = "no model loaded".into();
            return;
        };
        let t0 = Instant::now();
        let (answer, command, note) = match model.answer(&question) {
            Ok(text) => {
                let answer = omarchy::classify(&self.scripts, &text);
                let (command, note) = match &answer {
                    Answer::Command { name, args, placeholders } => {
                        let script = self.scripts.get(name);
                        let mut note = script.map(|s| s.summary.clone()).unwrap_or_default();
                        if script.is_some_and(|s| s.sudo) {
                            note.push_str(" · needs sudo, runs in a terminal");
                        }
                        if !placeholders.is_empty() {
                            note.push_str(" · fill in the <…> parts before running");
                        }
                        (omarchy::command_line(name, args), note)
                    }
                    Answer::Keybinding(key) => (key.clone(), "keybinding".into()),
                    Answer::Prose(text) => (text.clone(), "from the manual".into()),
                    Answer::Unknown(text) => (text.clone(), "not an Omarchy command".into()),
                };
                (answer, command, note)
            }
            Err(e) => (Answer::Unknown(String::new()), String::new(), e),
        };
        self.status = format!("answered in {} ms", t0.elapsed().as_millis());
        self.turns.push(Turn {
            question,
            answer,
            command,
            note,
            output: String::new(),
            run: None,
            finished: false,
        });
    }

    fn run(&mut self, i: usize) {
        let text = self.turns[i].command.trim().to_string();
        let words = omarchy::split(&text);
        let Some(name) = words.first() else {
            return;
        };
        let Some(script) = self.scripts.get(name) else {
            self.status = format!("{name} is not an Omarchy script");
            return;
        };
        if words.iter().any(|w| omarchy::is_placeholder(w)) {
            self.status = "fill in the <…> parts first".into();
            return;
        }
        if script.sudo {
            match runner::in_terminal(&text) {
                Ok(()) => {
                    self.turns[i].output = "opened in a terminal window (needs sudo)\n".into();
                    self.status = format!("{name} runs in a terminal");
                }
                Err(e) => self.status = e,
            }
            return;
        }
        match runner::Run::start(&script.path, &words[1..]) {
            Ok(run) => {
                self.turns[i].output.clear();
                self.turns[i].finished = false;
                self.turns[i].run = Some(run);
                self.status = format!("running {name}");
            }
            Err(e) => self.status = e,
        }
    }

    fn open_in_terminal(&mut self, i: usize) {
        let command = self.turns[i].command.clone();
        if let Some(run) = &mut self.turns[i].run {
            run.stop();
        }
        match runner::in_terminal(&command) {
            Ok(()) => self.status = "opened in a terminal window".into(),
            Err(e) => self.status = e,
        }
    }

    fn poll_runs(&mut self) {
        for turn in &mut self.turns {
            let Some(run) = &mut turn.run else {
                continue;
            };
            for line in run.poll() {
                turn.output.push_str(&line);
                turn.output.push('\n');
            }
            if let (Some(code), false) = (run.exit, turn.finished) {
                turn.finished = true;
                turn.output.push_str(&format!("[exit {code}]\n"));
            }
        }
    }

    fn turn_ui(&mut self, ui: &mut egui::Ui, i: usize, newest: bool) {
        ui.add_space(10.0);
        ui.label(RichText::new(format!("› {}", self.turns[i].question)).strong());
        ui.add_space(4.0);
        let is_command = matches!(self.turns[i].answer, Answer::Command { .. });
        if is_command {
            field(ui, Size::Regular, |ui| {
                let edit = ui.add(
                    TextEdit::singleline(&mut self.turns[i].command)
                        .frame(false)
                        .font(TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                );
                (edit.has_focus(), ())
            });
            ui.label(RichText::new(&self.turns[i].note).weak());
            ui.horizontal(|ui| {
                let running = self.turns[i].run.as_ref().is_some_and(|r| r.running());
                if running {
                    if ui.add(Button::new("Stop").danger()).clicked() {
                        if let Some(run) = &mut self.turns[i].run {
                            run.stop();
                        }
                    }
                } else {
                    let button = if newest { Button::new("Run").primary() } else { Button::new("Run") };
                    if ui.add(button).clicked() {
                        self.run(i);
                    }
                }
                if ui.add(Button::new("Copy")).clicked() {
                    ui.ctx().copy_text(self.turns[i].command.clone());
                    self.status = "copied".into();
                }
                if ui.add(Button::new("In terminal")).clicked() {
                    self.open_in_terminal(i);
                }
            });
        } else {
            let (text, note) = (self.turns[i].command.clone(), self.turns[i].note.clone());
            if matches!(self.turns[i].answer, Answer::Prose(_)) {
                ui.add(egui::Label::new(RichText::new(text)).wrap());
            } else {
                ui.label(RichText::new(text).monospace());
            }
            ui.label(RichText::new(note).weak());
        }
        if !self.turns[i].output.is_empty() {
            field(ui, Size::Regular, |ui| {
                let shown = ui.add(
                    TextEdit::multiline(&mut self.turns[i].output)
                        .frame(false)
                        .font(TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .interactive(false),
                );
                (false, shown)
            });
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if let Some(palette) = self.theme_watcher.changed(ctx) {
            ctx.set_visuals(theme::visuals(&palette));
        }
        self.poll_runs();
        let newest = self.turns.len().checked_sub(1);

        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::K)) {
            self.palette.toggle();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Enter)) {
            if let Some(i) = newest {
                self.run(i);
            }
        }

        StatusLine::new("status")
            .message(&self.status)
            .end(|ui| {
                ui.label(RichText::new(&self.model_name).weak());
                if ui
                    .add(
                        Button::icon_only(|p, r, c| {
                            icons::draw(p, r, "keyboard", c);
                        })
                            .quiet()
                            .tip("Commands")
                            .hint("Ctrl+K"),
                    )
                    .clicked()
                {
                    self.palette.toggle();
                }
            })
            .show(ctx);

        egui::TopBottomPanel::bottom("input").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let width = ui.available_width() - 80.0;
                let response = ui.add(
                    TextField::new(&mut self.input)
                        .hint("Ask Omarchy")
                        .key("/")
                        .width(width),
                );
                if self.focus_input {
                    response.request_focus();
                    self.focus_input = false;
                }
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if ui.add(Button::new("Ask")).clicked() || enter {
                    self.ask();
                    self.focus_input = true;
                }
            });
            ui.add_space(8.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical()
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if self.turns.is_empty() {
                        ui.add_space(24.0);
                        ui.label(RichText::new("Ask how to do something in Omarchy. The answer is a command; look at it, then Run.").weak());
                    }
                    for i in 0..self.turns.len() {
                        self.turn_ui(ui, i, Some(i) == newest);
                    }
                    ui.add_space(12.0);
                });
        });

        let choices = vec![
            Choice { label: "Run".into(), key: Some("Ctrl+Enter".into()), keywords: "execute go".into() },
            Choice { label: "Copy command".into(), key: None, keywords: "clipboard".into() },
            Choice { label: "Open in terminal".into(), key: None, keywords: "shell window".into() },
            Choice { label: "Stop".into(), key: None, keywords: "kill cancel".into() },
            Choice { label: "Clear chat".into(), key: None, keywords: "reset new empty".into() },
        ];
        if let Some(choice) = self.palette.show(ctx, "Ctrl+K", &choices) {
            match (choice, newest) {
                (0, Some(i)) => self.run(i),
                (1, Some(i)) => {
                    ctx.copy_text(self.turns[i].command.clone());
                    self.status = "copied".into();
                }
                (2, Some(i)) => self.open_in_terminal(i),
                (3, Some(i)) => {
                    if let Some(run) = &mut self.turns[i].run {
                        run.stop();
                    }
                }
                (4, _) => self.turns.clear(),
                _ => {}
            }
        }

        if self.turns.iter().any(|t| t.run.as_ref().is_some_and(|r| r.running())) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}
