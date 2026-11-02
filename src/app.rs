use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, FontId, Key, KeyboardShortcut, Layout, Modifiers, RichText, Sense,
    text::{LayoutJob, TextFormat},
};

use crate::editor::{self, EditorArgs, HighlightCache};
use crate::fileformat::{FileFormat, decode_source, encode_source};
use crate::theme::Theme;
use varan::asm::{self, AsmError, Program};
use varan::cpu::{Cpu, StepResult};
use varan::disasm::disassemble;

// Shortcuts use `Modifiers::COMMAND`: Ctrl on Windows/Linux, Cmd on macOS.
const fn cmd(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, key)
}
const SC_NEW: KeyboardShortcut = cmd(Key::N);
const SC_OPEN: KeyboardShortcut = cmd(Key::O);
const SC_SAVE: KeyboardShortcut = cmd(Key::S);
const SC_SAVE_AS: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
const SC_ASSEMBLE: KeyboardShortcut = cmd(Key::B);
const SC_RESET: KeyboardShortcut = cmd(Key::R);
const SC_ZOOM_IN: KeyboardShortcut = cmd(Key::Plus);
const SC_ZOOM_IN_ALT: KeyboardShortcut = cmd(Key::Equals);
const SC_ZOOM_OUT: KeyboardShortcut = cmd(Key::Minus);

/// Shortcut text for this platform, e.g. "Ctrl+S" on Windows, "Cmd+S" on macOS.
fn sc_text(sc: &KeyboardShortcut) -> String {
    sc.format(&egui::ModifierNames::NAMES, cfg!(target_os = "macos"))
}

const SAMPLE: &str = r#"; Welcome to Varan - an ARM emulator for teaching.
;
; F7 assemble   F5 run   F11 step   F10 step over   Shift+F5 pause
; Click the line-number gutter to set breakpoints.
;
; SWI 0  print the character in R0
; SWI 1  read a character into R0
; SWI 2  halt
; SWI 3  print the string at address R0
; SWI 4  print R0 as a decimal number

        ENTRY
start   ADR     R0, hello       ; R0 = address of the string
        SWI     3               ; print it

        MOV     R1, #1          ; counter
        MOV     R2, #0          ; running total
loop    ADD     R2, R2, R1
        ADD     R1, R1, #1
        CMP     R1, #10
        BLE     loop

        ADR     R0, result
        SWI     3
        MOV     R0, R2
        SWI     4               ; print the total (55)
        MOV     R0, #10
        SWI     0               ; newline
        SWI     2               ; halt

hello   DEFB    "Hello from ARM!\n", 0
result  DEFB    "1 + 2 + ... + 10 = ", 0
        ALIGN
"#;

#[derive(Debug, Clone, Copy, PartialEq)]
enum RunState {
    /// No program has been assembled yet (or assembly failed).
    NotLoaded,
    /// Program loaded and paused (at entry or after a step/breakpoint).
    Paused,
    Running,
    WaitingInput,
    Halted,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CenterTab {
    Editor,
    Listing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BottomTab {
    Console,
    Memory,
    Problems,
    Symbols,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Speed {
    Max,
    PerSecond(u32),
}

impl Speed {
    fn label(&self) -> String {
        match self {
            Speed::Max => "Full speed".into(),
            Speed::PerSecond(n) => format!("{n} instr/s"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MemFormat {
    Bytes,
    Words,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Pending {
    New,
    Open,
    Quit,
}

pub struct VaranApp {
    source: String,
    file_path: Option<PathBuf>,
    dirty: bool,
    /// Line endings / encoding of the opened file, reused when saving.
    file_format: FileFormat,
    highlight_cache: HighlightCache,

    program: Option<Program>,
    loaded_source: String,
    errors: Vec<AsmError>,
    error_lines: BTreeSet<usize>,
    code_lines: BTreeSet<usize>,

    cpu: Cpu,
    state: RunState,
    status_msg: String,
    /// Registers before the last step/run (to highlight changes).
    prev_regs: [u32; 16],
    prev_flags: [bool; 4],

    breakpoints: BTreeSet<usize>,
    bp_addrs: HashSet<u32>,
    temp_break: Option<u32>,
    skip_break_once: bool,
    resume_running: bool,
    speed: Speed,
    step_accum: f64,
    last_frame: Instant,

    center_tab: CenterTab,
    bottom_tab: BottomTab,
    follow_pc: bool,
    scroll_editor_to: Option<usize>,
    scroll_listing: bool,
    cursor: (usize, usize),

    console_input: String,
    echo_input: bool,
    focus_console: bool,

    mem_base: u32,
    mem_goto: String,
    mem_goto_err: Option<String>,
    mem_format: MemFormat,
    mem_scroll_reset: bool,

    reg_hex: bool,
    editing_reg: Option<(usize, String)>,

    font_size: f32,
    tab_width: usize,
    theme: Theme,
    pending: Option<Pending>,
    allow_close: bool,
    title: String,
}

impl VaranApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = Theme::dark();
        cc.egui_ctx.set_visuals(theme.visuals());
        editor::install_fonts(&cc.egui_ctx);
        cc.egui_ctx.global_style_mut(|s| {
            s.spacing.item_spacing = egui::vec2(8.0, 5.0);
            s.spacing.button_padding = egui::vec2(8.0, 3.0);
        });
        let mut app = VaranApp {
            source: SAMPLE.to_string(),
            file_path: None,
            dirty: false,
            file_format: FileFormat::default(),
            highlight_cache: HighlightCache::default(),
            program: None,
            loaded_source: String::new(),
            errors: Vec::new(),
            error_lines: BTreeSet::new(),
            code_lines: BTreeSet::new(),
            cpu: Cpu::new(),
            state: RunState::NotLoaded,
            status_msg: "Press F7 to assemble, or F5 to assemble and run.".into(),
            prev_regs: [0; 16],
            prev_flags: [false; 4],
            breakpoints: BTreeSet::new(),
            bp_addrs: HashSet::new(),
            temp_break: None,
            skip_break_once: false,
            resume_running: false,
            speed: Speed::Max,
            step_accum: 0.0,
            last_frame: Instant::now(),
            center_tab: CenterTab::Editor,
            bottom_tab: BottomTab::Console,
            follow_pc: true,
            scroll_editor_to: None,
            scroll_listing: false,
            cursor: (0, 0),
            console_input: String::new(),
            echo_input: true,
            focus_console: false,
            mem_base: 0,
            mem_goto: "0".into(),
            mem_goto_err: None,
            mem_format: MemFormat::Bytes,
            mem_scroll_reset: false,
            reg_hex: true,
            editing_reg: None,
            font_size: 14.0,
            tab_width: 8,
            theme,
            pending: None,
            allow_close: false,
            title: String::new(),
        };
        // Open a file passed on the command line.
        if let Some(path) = std::env::args().nth(1).map(PathBuf::from) {
            app.load_file(path);
        }
        app
    }

    // ------------------------------------------------------------------
    // Program control
    // ------------------------------------------------------------------

    fn source_changed_since_load(&self) -> bool {
        self.program.is_none() || self.source != self.loaded_source
    }

    fn assemble_and_load(&mut self) -> bool {
        match asm::assemble(&self.source) {
            Ok(prog) => {
                self.errors.clear();
                self.error_lines.clear();
                self.code_lines = prog
                    .addr_to_line
                    .values()
                    .copied()
                    .collect();
                let size: usize = prog.segments.iter().map(|(_, b)| b.len()).sum();
                let entry = prog.entry;
                self.program = Some(prog);
                self.loaded_source = self.source.clone();
                self.recompute_breakpoints();
                self.reset();
                self.status_msg = format!(
                    "Assembled successfully: {size} bytes, entry point 0x{entry:08X}."
                );
                if self.bottom_tab == BottomTab::Problems {
                    self.bottom_tab = BottomTab::Console;
                }
                true
            }
            Err(errs) => {
                self.error_lines = errs.iter().map(|e| e.line).collect();
                self.status_msg = format!(
                    "Assembly failed with {} error{}.",
                    errs.len(),
                    if errs.len() == 1 { "" } else { "s" }
                );
                if let Some(first) = errs.first() {
                    self.scroll_editor_to = Some(first.line);
                }
                self.errors = errs;
                self.program = None;
                self.state = RunState::NotLoaded;
                self.code_lines.clear();
                self.bp_addrs.clear();
                self.bottom_tab = BottomTab::Problems;
                self.center_tab = CenterTab::Editor;
                false
            }
        }
    }

    fn ensure_loaded(&mut self) -> bool {
        if self.source_changed_since_load() {
            self.assemble_and_load()
        } else {
            true
        }
    }

    /// Reload the program into a fresh machine.
    fn reset(&mut self) {
        let Some(prog) = &self.program else { return };
        let mut cpu = Cpu::new();
        for (addr, bytes) in &prog.segments {
            cpu.mem.load(*addr, bytes);
        }
        cpu.regs[15] = prog.entry;
        self.cpu = cpu;
        self.prev_regs = self.cpu.regs;
        self.prev_flags = self.flags();
        self.state = RunState::Paused;
        self.temp_break = None;
        self.status_msg = "Program reset.".into();
        self.reveal_pc();
    }

    fn flags(&self) -> [bool; 4] {
        [self.cpu.n, self.cpu.z, self.cpu.c, self.cpu.v]
    }

    fn snapshot(&mut self) {
        self.prev_regs = self.cpu.regs;
        self.prev_flags = self.flags();
    }

    fn recompute_breakpoints(&mut self) {
        self.bp_addrs.clear();
        if let Some(p) = &self.program {
            for line in &self.breakpoints {
                if let Some(a) = p.line_to_addr.get(line) {
                    if p.addr_to_line.get(a) == Some(line) {
                        self.bp_addrs.insert(*a);
                    }
                }
            }
        }
    }

    fn toggle_breakpoint(&mut self, line: usize) {
        if !self.breakpoints.remove(&line) {
            self.breakpoints.insert(line);
        }
        self.recompute_breakpoints();
    }

    fn can_execute(&mut self) -> bool {
        if !self.ensure_loaded() {
            return false;
        }
        match self.state {
            RunState::Halted | RunState::Error => {
                self.status_msg = format!(
                    "The program has stopped. Press Reset ({}) to run it again.",
                    sc_text(&SC_RESET)
                );
                false
            }
            _ => true,
        }
    }

    fn run(&mut self) {
        if !self.ensure_loaded() {
            return;
        }
        if matches!(self.state, RunState::Halted | RunState::Error) {
            self.reset();
        }
        if self.state == RunState::WaitingInput {
            self.focus_console = true;
            return;
        }
        self.snapshot();
        self.state = RunState::Running;
        self.skip_break_once = true;
        self.step_accum = 0.0;
        self.last_frame = Instant::now();
        self.status_msg = "Running...".into();
    }

    fn pause(&mut self) {
        if matches!(self.state, RunState::Running | RunState::WaitingInput) {
            self.state = RunState::Paused;
            self.temp_break = None;
            self.status_msg = format!("Paused at 0x{:08X}.", self.cpu.pc());
            self.reveal_pc();
        }
    }

    fn step(&mut self) {
        if !self.can_execute() {
            return;
        }
        if self.state == RunState::Running {
            self.pause();
            return;
        }
        if self.state == RunState::WaitingInput {
            self.focus_console = true;
            return;
        }
        self.snapshot();
        let r = self.cpu.step();
        self.handle_result(r, false);
        if self.state == RunState::Paused {
            self.status_msg = format!("Stepped. Next instruction at 0x{:08X}.", self.cpu.pc());
        }
        self.reveal_pc();
    }

    /// Step, but run over BL calls until they return.
    fn step_over(&mut self) {
        if !self.can_execute() {
            return;
        }
        if self.state != RunState::Paused {
            return;
        }
        let pc = self.cpu.pc();
        let instr = self.cpu.mem.read_u32(pc);
        let is_bl = (instr >> 24) & 0xF == 0xB && instr >> 28 != 0xF;
        if is_bl && self.cpu.cond_passed(instr >> 28) {
            self.temp_break = Some(pc.wrapping_add(4));
            self.snapshot();
            self.state = RunState::Running;
            self.skip_break_once = true;
            self.status_msg = "Stepping over call...".into();
        } else {
            self.step();
        }
    }

    fn handle_result(&mut self, r: StepResult, running: bool) {
        match r {
            StepResult::Ok => {}
            StepResult::Halted => {
                self.state = RunState::Halted;
                self.temp_break = None;
                self.status_msg = format!(
                    "Program halted (SWI 2) after {} instructions.",
                    self.cpu.steps
                );
                self.reveal_pc();
            }
            StepResult::WaitingForInput => {
                self.state = RunState::WaitingInput;
                self.resume_running = running;
                self.status_msg = "Waiting for keyboard input (SWI 1) - type in the console below.".into();
                self.bottom_tab = BottomTab::Console;
                self.focus_console = true;
            }
            StepResult::Error(e) => {
                self.state = RunState::Error;
                self.temp_break = None;
                self.status_msg = format!("Error: {e}");
                self.reveal_pc();
            }
        }
    }

    fn run_frame(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.25);
        self.last_frame = now;
        let mut max_steps = match self.speed {
            Speed::Max => u64::MAX,
            Speed::PerSecond(n) => {
                self.step_accum += dt * n as f64;
                let k = self.step_accum.floor();
                self.step_accum -= k;
                k as u64
            }
        };
        if matches!(self.speed, Speed::PerSecond(_)) && self.follow_pc && max_steps > 0 {
            // In slow mode, highlight what changed in each instruction.
            self.snapshot();
        }
        let start = Instant::now();
        let budget = Duration::from_millis(14);
        let mut n: u64 = 0;
        while max_steps > 0 {
            let pc = self.cpu.pc();
            if !self.skip_break_once {
                if self.temp_break == Some(pc) {
                    self.temp_break = None;
                    self.state = RunState::Paused;
                    self.status_msg = format!("Stepped over call. Next instruction at 0x{pc:08X}.");
                    self.reveal_pc();
                    return;
                }
                if self.bp_addrs.contains(&pc) {
                    self.state = RunState::Paused;
                    self.temp_break = None;
                    self.status_msg = format!("Breakpoint hit at 0x{pc:08X}.");
                    self.reveal_pc();
                    return;
                }
            }
            self.skip_break_once = false;
            let r = self.cpu.step();
            if r != StepResult::Ok {
                self.handle_result(r, true);
                return;
            }
            n += 1;
            max_steps -= 1;
            if n % 2048 == 0 && start.elapsed() > budget {
                break;
            }
        }
        if matches!(self.speed, Speed::PerSecond(_)) {
            self.reveal_pc();
        }
    }

    fn submit_input(&mut self) {
        let mut text = std::mem::take(&mut self.console_input);
        text.push('\n');
        if self.echo_input {
            self.cpu.output.push_str(&text);
        }
        self.cpu.input.extend(text.bytes());
        if self.state == RunState::WaitingInput {
            if self.resume_running {
                self.state = RunState::Running;
                self.last_frame = Instant::now();
                self.skip_break_once = true;
                self.status_msg = "Running...".into();
            } else {
                self.state = RunState::Paused;
                let r = self.cpu.step();
                self.handle_result(r, false);
                if self.state == RunState::Paused {
                    self.status_msg = format!("Stepped. Next instruction at 0x{:08X}.", self.cpu.pc());
                }
                self.reveal_pc();
            }
        }
    }

    fn reveal_pc(&mut self) {
        if !self.follow_pc {
            return;
        }
        if let Some(line) = self.pc_line() {
            self.scroll_editor_to = Some(line);
        }
        self.scroll_listing = true;
    }

    fn pc_line(&self) -> Option<usize> {
        if self.state == RunState::NotLoaded || self.source_changed_since_load() {
            return None;
        }
        self.program
            .as_ref()
            .and_then(|p| p.addr_to_line.get(&self.cpu.pc()).copied())
    }

    fn label_for(&self, addr: u32) -> Option<String> {
        let p = self.program.as_ref()?;
        // Prefer a label that isn't an EQU constant: pick any symbol whose
        // value matches and which labels a line.
        p.symbols
            .iter()
            .find(|(_, v)| **v == addr)
            .map(|(k, _)| k.clone())
    }

    // ------------------------------------------------------------------
    // Files
    // ------------------------------------------------------------------

    fn file_name(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled.s".into())
    }

    fn load_file(&mut self, path: PathBuf) {
        match std::fs::read(&path) {
            Ok(bytes) => {
                let (text, format) = decode_source(&bytes);
                self.source = text;
                self.file_format = format;
                self.file_path = Some(path);
                self.dirty = false;
                self.breakpoints.clear();
                self.program = None;
                self.errors.clear();
                self.error_lines.clear();
                self.code_lines.clear();
                self.state = RunState::NotLoaded;
                self.cpu = Cpu::new();
                self.status_msg = format!("Opened {}.", self.file_name());
            }
            Err(e) => {
                self.status_msg = format!("Could not open file: {e}");
            }
        }
    }

    fn dialog() -> rfd::FileDialog {
        rfd::FileDialog::new()
            .add_filter("ARM assembly", &["s", "asm", "S", "txt"])
            .add_filter("All files", &["*"])
    }

    fn open_dialog(&mut self) {
        if let Some(path) = Self::dialog().pick_file() {
            self.load_file(path);
        }
    }

    fn save(&mut self) -> bool {
        match self.file_path.clone() {
            Some(p) => self.write_to(p),
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) -> bool {
        if let Some(path) = Self::dialog().set_file_name(self.file_name()).save_file() {
            self.write_to(path)
        } else {
            false
        }
    }

    fn write_to(&mut self, path: PathBuf) -> bool {
        let bytes = encode_source(&self.source, &self.file_format);
        match std::fs::write(&path, bytes) {
            Ok(()) => {
                self.file_path = Some(path);
                self.dirty = false;
                self.status_msg = format!("Saved {}.", self.file_name());
                true
            }
            Err(e) => {
                self.status_msg = format!("Could not save: {e}");
                false
            }
        }
    }

    /// Ask to save unsaved changes. Returns true if it's OK to proceed.
    fn confirm_discard(&mut self) -> bool {
        if !self.dirty {
            return true;
        }
        let res = rfd::MessageDialog::new()
            .set_title("Unsaved changes")
            .set_description(format!("Save changes to {}?", self.file_name()))
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .set_level(rfd::MessageLevel::Warning)
            .show();
        match res {
            rfd::MessageDialogResult::Yes => self.save(),
            rfd::MessageDialogResult::No => true,
            _ => false,
        }
    }

    fn do_pending(&mut self, ctx: &egui::Context) {
        let Some(p) = self.pending.take() else { return };
        if !self.confirm_discard() {
            return;
        }
        match p {
            Pending::New => {
                self.source = "        ENTRY\nstart   \n        SWI     2\n".into();
                self.file_path = None;
                self.file_format = FileFormat::default();
                self.dirty = false;
                self.breakpoints.clear();
                self.program = None;
                self.errors.clear();
                self.error_lines.clear();
                self.code_lines.clear();
                self.state = RunState::NotLoaded;
                self.cpu = Cpu::new();
                self.status_msg = "New file.".into();
            }
            Pending::Open => self.open_dialog(),
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    // ------------------------------------------------------------------
    // Input
    // ------------------------------------------------------------------

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let sc = |m: Modifiers, k: Key| KeyboardShortcut::new(m, k);
        let fire = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        if fire(sc(Modifiers::SHIFT, Key::F5)) {
            self.pause();
        }
        if fire(sc(Modifiers::NONE, Key::F5)) {
            self.run();
        }
        if fire(sc(Modifiers::NONE, Key::F7)) || fire(SC_ASSEMBLE) {
            self.assemble_and_load();
        }
        if fire(sc(Modifiers::NONE, Key::F11)) {
            self.step();
        }
        if fire(sc(Modifiers::NONE, Key::F10)) {
            self.step_over();
        }
        if fire(sc(Modifiers::NONE, Key::F9)) {
            self.toggle_breakpoint(self.cursor.0);
        }
        if fire(SC_RESET) {
            if self.ensure_loaded() {
                self.reset();
            }
        }
        if fire(SC_SAVE_AS) {
            self.save_as();
        }
        if fire(SC_SAVE) {
            self.save();
        }
        if fire(SC_OPEN) {
            self.pending = Some(Pending::Open);
        }
        if fire(SC_NEW) {
            self.pending = Some(Pending::New);
        }
        if fire(SC_ZOOM_IN) || fire(SC_ZOOM_IN_ALT) {
            self.font_size = (self.font_size + 1.0).min(32.0);
        }
        if fire(SC_ZOOM_OUT) {
            self.font_size = (self.font_size - 1.0).max(8.0);
        }
    }

    fn handle_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = format!(
            "{}{} - Varan ARM Emulator",
            self.file_name(),
            if self.dirty { " *" } else { "" }
        );
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    // ------------------------------------------------------------------
    // UI
    // ------------------------------------------------------------------

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.add(egui::Button::new("New").shortcut_text(sc_text(&SC_NEW))).clicked() {
                    self.pending = Some(Pending::New);
                }
                if ui.add(egui::Button::new("Open...").shortcut_text(sc_text(&SC_OPEN))).clicked() {
                    self.pending = Some(Pending::Open);
                }
                if ui.add(egui::Button::new("Save").shortcut_text(sc_text(&SC_SAVE))).clicked() {
                    self.save();
                }
                if ui
                    .add(egui::Button::new("Save As...").shortcut_text(sc_text(&SC_SAVE_AS)))
                    .clicked()
                {
                    self.save_as();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    self.pending = Some(Pending::Quit);
                }
            });
            ui.menu_button("Run", |ui| {
                if ui.add(egui::Button::new("Assemble").shortcut_text("F7")).clicked() {
                    self.assemble_and_load();
                }
                if ui.add(egui::Button::new("Run / Continue").shortcut_text("F5")).clicked() {
                    self.run();
                }
                if ui.add(egui::Button::new("Pause").shortcut_text("Shift+F5")).clicked() {
                    self.pause();
                }
                if ui.add(egui::Button::new("Step").shortcut_text("F11")).clicked() {
                    self.step();
                }
                if ui.add(egui::Button::new("Step Over").shortcut_text("F10")).clicked() {
                    self.step_over();
                }
                if ui.add(egui::Button::new("Reset").shortcut_text(sc_text(&SC_RESET))).clicked() {
                    if self.ensure_loaded() {
                        self.reset();
                    }
                }
                ui.separator();
                if ui.add(egui::Button::new("Toggle Breakpoint").shortcut_text("F9")).clicked() {
                    self.toggle_breakpoint(self.cursor.0);
                }
                if ui.button("Clear All Breakpoints").clicked() {
                    self.breakpoints.clear();
                    self.recompute_breakpoints();
                }
            });
            ui.menu_button("View", |ui| {
                let mut dark = self.theme.dark;
                if ui.checkbox(&mut dark, "Dark theme").changed() {
                    self.theme = if dark { Theme::dark() } else { Theme::light() };
                    ui.ctx().set_visuals(self.theme.visuals());
                }
                ui.checkbox(&mut self.follow_pc, "Follow PC while stepping");
                ui.checkbox(&mut self.echo_input, "Echo console input");
                ui.horizontal(|ui| {
                    ui.label("Tab width");
                    for w in [2usize, 4, 8] {
                        ui.selectable_value(&mut self.tab_width, w, w.to_string());
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Editor font size");
                    ui.add(egui::Slider::new(&mut self.font_size, 8.0..=32.0).step_by(1.0));
                });
            });
            ui.menu_button("Help", |ui| {
                ui.label(RichText::new("Varan ARM Emulator").strong());
                ui.label("A KoMoDo-compatible ARM assembler and emulator.");
                ui.separator();
                ui.label("SWI 0  print character in R0");
                ui.label("SWI 1  read character into R0");
                ui.label("SWI 2  halt");
                ui.label("SWI 3  print string at address R0");
                ui.label("SWI 4  print R0 as a decimal integer");
                ui.separator();
                ui.label("Directives: ENTRY, ORIGIN, ALIGN, EQU, DEFW, DEFB, DEFS, LTORG");
                ui.label("Pseudo-ops: NOP, ADR, ADRL, LDR Rd,=value, PUSH, POP");
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let running = self.state == RunState::Running;
            let big = |s: &str| RichText::new(s).size(14.5);
            if ui
                .add(egui::Button::new(big("🔨 Assemble")))
                .on_hover_text("Assemble and load the program (F7)")
                .clicked()
            {
                self.assemble_and_load();
            }
            ui.separator();
            if running {
                if ui.add(egui::Button::new(big("⏸ Pause"))).on_hover_text("Shift+F5").clicked() {
                    self.pause();
                }
            } else if ui
                .add(egui::Button::new(big("▶ Run")).fill(self.theme.accent.linear_multiply(0.35)))
                .on_hover_text("Run / continue (F5)")
                .clicked()
            {
                self.run();
            }
            if ui
                .add_enabled(!running, egui::Button::new(big("⤵ Step")))
                .on_hover_text("Execute one instruction (F11)")
                .clicked()
            {
                self.step();
            }
            if ui
                .add_enabled(!running, egui::Button::new(big("↷ Step Over")))
                .on_hover_text("Step, running over BL calls (F10)")
                .clicked()
            {
                self.step_over();
            }
            if ui
                .add(egui::Button::new(big("⟲ Reset")))
                .on_hover_text(format!(
                    "Reload the program and reset registers ({})",
                    sc_text(&SC_RESET)
                ))
                .clicked()
                && self.ensure_loaded()
            {
                self.reset();
            }
            ui.separator();
            ui.label("Speed:");
            egui::ComboBox::from_id_salt("speed")
                .selected_text(self.speed.label())
                .show_ui(ui, |ui| {
                    for s in [
                        Speed::PerSecond(1),
                        Speed::PerSecond(4),
                        Speed::PerSecond(20),
                        Speed::PerSecond(100),
                        Speed::PerSecond(1000),
                        Speed::Max,
                    ] {
                        ui.selectable_value(&mut self.speed, s, s.label());
                    }
                });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (text, color) = self.state_badge();
                ui.label(RichText::new(text).color(color).strong());
            });
        });
    }

    fn state_badge(&self) -> (&'static str, Color32) {
        let stale = self.state != RunState::NotLoaded && self.source_changed_since_load();
        if stale {
            return ("● MODIFIED - reassemble", self.theme.warn_text);
        }
        match self.state {
            RunState::NotLoaded => ("● NOT ASSEMBLED", self.theme.dim),
            RunState::Paused => ("● PAUSED", self.theme.accent),
            RunState::Running => ("● RUNNING", self.theme.ok_text),
            RunState::WaitingInput => ("● WAITING FOR INPUT", self.theme.warn_text),
            RunState::Halted => ("● HALTED", self.theme.dim),
            RunState::Error => ("● ERROR", self.theme.error_text),
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let color = match self.state {
                RunState::Error => self.theme.error_text,
                _ if !self.errors.is_empty() => self.theme.error_text,
                _ => self.theme.text,
            };
            ui.label(RichText::new(&self.status_msg).color(color));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("Ln {}, Col {}", self.cursor.0 + 1, self.cursor.1 + 1))
                        .color(self.theme.dim),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("{} instructions", self.cpu.steps)).color(self.theme.dim),
                );
                ui.separator();
                ui.label(RichText::new(self.file_name()).color(self.theme.dim));
            });
        });
    }

    fn registers_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading("Registers");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.selectable_value(&mut self.reg_hex, false, "Dec");
                ui.selectable_value(&mut self.reg_hex, true, "Hex");
            });
        });
        ui.separator();
        let mono = FontId::monospace(13.5);
        let editable = self.state != RunState::Running;
        let mut commit: Option<(usize, u32)> = None;
        let mut cancel_edit = false;

        egui::Grid::new("regs")
            .num_columns(3)
            .striped(true)
            .spacing([10.0, 3.0])
            .show(ui, |ui| {
                for i in 0..16 {
                    let v = self.cpu.regs[i];
                    let changed = v != self.prev_regs[i];
                    let name = match i {
                        13 => "R13 SP".to_string(),
                        14 => "R14 LR".to_string(),
                        15 => "R15 PC".to_string(),
                        _ => format!("R{i}"),
                    };
                    ui.label(RichText::new(name).font(mono.clone()).color(self.theme.dim));
                    let color = if changed { self.theme.changed } else { self.theme.text };

                    let editing_this = matches!(&self.editing_reg, Some((r, _)) if *r == i);
                    if editing_this {
                        let (_, buf) = self.editing_reg.as_mut().unwrap();
                        let resp = ui.add(
                            egui::TextEdit::singleline(buf)
                                .font(mono.clone())
                                .desired_width(110.0),
                        );
                        resp.request_focus();
                        if ui.input(|inp| inp.key_pressed(Key::Escape)) {
                            cancel_edit = true;
                        } else if resp.lost_focus() {
                            let syms = self.program.as_ref().map(|p| p.symbols.clone()).unwrap_or_default();
                            match asm::evaluate(buf, &syms) {
                                Ok(val) => commit = Some((i, val)),
                                Err(_) => cancel_edit = true,
                            }
                        }
                    } else {
                        let text = if self.reg_hex {
                            format!("0x{v:08X}")
                        } else {
                            format!("{v}")
                        };
                        let resp = ui.add(
                            egui::Label::new(RichText::new(text).font(mono.clone()).color(color))
                                .sense(Sense::click()),
                        );
                        if editable && resp.double_clicked() {
                            self.editing_reg = Some((i, format!("0x{v:X}")));
                        }
                        resp.on_hover_text(format!(
                            "Hex: 0x{v:08X}\nUnsigned: {v}\nSigned: {}\nBinary: {:032b}\n\nDouble-click to edit",
                            v as i32, v
                        ));
                    }
                    ui.label(
                        RichText::new(format!("{}", v as i32))
                            .font(FontId::monospace(12.0))
                            .color(self.theme.dim),
                    );
                    ui.end_row();
                }
            });
        if let Some((r, v)) = commit {
            if r == 15 {
                self.cpu.regs[15] = v & !3;
                self.reveal_pc();
            } else {
                self.cpu.regs[r] = v;
            }
            self.prev_regs[r] = self.cpu.regs[r];
            self.editing_reg = None;
        }
        if cancel_edit {
            self.editing_reg = None;
        }

        ui.add_space(8.0);
        ui.heading("Flags");
        ui.separator();
        ui.horizontal(|ui| {
            let flags = [
                ("N", "Negative", self.cpu.n, self.prev_flags[0]),
                ("Z", "Zero", self.cpu.z, self.prev_flags[1]),
                ("C", "Carry", self.cpu.c, self.prev_flags[2]),
                ("V", "Overflow", self.cpu.v, self.prev_flags[3]),
            ];
            let mut new_vals = [self.cpu.n, self.cpu.z, self.cpu.c, self.cpu.v];
            for (idx, (short, long, val, prev)) in flags.iter().enumerate() {
                let changed = val != prev;
                let fill = if *val { self.theme.accent.linear_multiply(0.6) } else { Color32::TRANSPARENT };
                let text_color = if changed { self.theme.changed } else if *val { Color32::WHITE } else { self.theme.dim };
                let btn = egui::Button::new(
                    RichText::new(format!("{short} {}", *val as u8)).font(FontId::monospace(14.0)).color(text_color),
                )
                .fill(fill)
                .min_size(egui::vec2(48.0, 26.0));
                let resp = ui.add_enabled(editable, btn).on_hover_text(format!("{long} flag - click to toggle"));
                if resp.clicked() {
                    new_vals[idx] = !new_vals[idx];
                }
            }
            self.cpu.n = new_vals[0];
            self.cpu.z = new_vals[1];
            self.cpu.c = new_vals[2];
            self.cpu.v = new_vals[3];
        });
        ui.label(
            RichText::new(format!("CPSR = 0x{:08X}", self.cpu.cpsr()))
                .font(FontId::monospace(12.5))
                .color(self.theme.dim),
        );

        ui.add_space(8.0);
        ui.heading("Next instruction");
        ui.separator();
        if self.state == RunState::NotLoaded {
            ui.label(RichText::new("-").color(self.theme.dim));
        } else {
            let pc = self.cpu.pc();
            let instr = self.cpu.mem.read_u32(pc);
            let text = disassemble(instr, pc, &|a| self.label_for(a));
            let cond_ok = self.cpu.cond_passed(instr >> 28);
            ui.label(
                RichText::new(text)
                    .font(FontId::monospace(14.0))
                    .color(self.theme.syn_mnemonic),
            );
            ui.label(
                RichText::new(format!("at 0x{pc:08X}  ({instr:08X})"))
                    .font(FontId::monospace(12.0))
                    .color(self.theme.dim),
            );
            if instr >> 28 != 0xE {
                ui.label(
                    RichText::new(if cond_ok { "condition: will execute" } else { "condition: will be skipped" })
                        .color(if cond_ok { self.theme.ok_text } else { self.theme.warn_text }),
                );
            }
        }

        ui.add_space(8.0);
        ui.heading("Stack");
        ui.separator();
        let sp = self.cpu.regs[13];
        egui::Grid::new("stack").num_columns(2).spacing([10.0, 2.0]).show(ui, |ui| {
            for k in 0..8u32 {
                let a = sp.wrapping_add(4 * k);
                let v = self.cpu.mem.read_u32(a);
                let lbl = if k == 0 { format!("SP→ {a:08X}") } else { format!("    {a:08X}") };
                ui.label(RichText::new(lbl).font(FontId::monospace(12.0)).color(self.theme.dim));
                ui.label(RichText::new(format!("{v:08X}")).font(FontId::monospace(12.0)));
                ui.end_row();
            }
        });
    }

    fn bottom_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let problems = if self.errors.is_empty() {
                "Problems".to_string()
            } else {
                format!("Problems ({})", self.errors.len())
            };
            ui.selectable_value(&mut self.bottom_tab, BottomTab::Console, "Console");
            ui.selectable_value(&mut self.bottom_tab, BottomTab::Memory, "Memory");
            let p = ui.selectable_value(&mut self.bottom_tab, BottomTab::Problems, problems);
            if !self.errors.is_empty() {
                ui.painter().circle_filled(p.rect.right_top() + egui::vec2(-3.0, 4.0), 3.0, self.theme.error_text);
            }
            ui.selectable_value(&mut self.bottom_tab, BottomTab::Symbols, "Symbols");
        });
        ui.separator();
        match self.bottom_tab {
            BottomTab::Console => self.console(ui),
            BottomTab::Memory => self.memory(ui),
            BottomTab::Problems => self.problems(ui),
            BottomTab::Symbols => self.symbols(ui),
        }
    }

    fn console(&mut self, ui: &mut egui::Ui) {
        let waiting = self.state == RunState::WaitingInput;
        egui::Panel::bottom("console_input")
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let prompt = if waiting {
                        RichText::new("Input ▸").color(self.theme.warn_text).strong()
                    } else {
                        RichText::new("Input ▸").color(self.theme.dim)
                    };
                    ui.label(prompt);
                    let w = ui.available_width() - 150.0;
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.console_input)
                            .font(FontId::monospace(13.0))
                            .hint_text(if waiting {
                                "Program is waiting - type and press Enter"
                            } else {
                                "Type input for SWI 1 and press Enter"
                            })
                            .desired_width(w.max(100.0)),
                    );
                    if self.focus_console {
                        resp.request_focus();
                        self.focus_console = false;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        self.submit_input();
                        resp.request_focus();
                    }
                    if ui.button("Send").clicked() {
                        self.submit_input();
                    }
                    if ui.button("Clear").clicked() {
                        self.cpu.output.clear();
                    }
                });
                if !self.cpu.input.is_empty() {
                    ui.label(
                        RichText::new(format!("{} character(s) queued", self.cpu.input.len()))
                            .small()
                            .color(self.theme.dim),
                    );
                }
            });
        egui::Frame::new()
            .fill(self.theme.console_bg)
            .inner_margin(6.0)
            .corner_radius(4.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("console_out")
                    .stick_to_bottom(true)
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        let text = if self.cpu.output.is_empty() {
                            RichText::new("Program output appears here.").italics().color(self.theme.dim)
                        } else {
                            RichText::new(&self.cpu.output).color(self.theme.text)
                        };
                        ui.add(egui::Label::new(text.font(FontId::monospace(self.font_size))).wrap());
                    });
            });
    }

    fn memory(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Address:");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.mem_goto)
                    .font(FontId::monospace(13.0))
                    .desired_width(140.0)
                    .hint_text("0x1000 or label"),
            );
            let mut go = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            go |= ui.button("Go").clicked();
            if go {
                let syms = self.program.as_ref().map(|p| p.symbols.clone()).unwrap_or_default();
                match asm::evaluate(&self.mem_goto, &syms) {
                    Ok(a) => {
                        self.mem_base = a & !0xF;
                        self.mem_goto_err = None;
                        self.mem_scroll_reset = true;
                    }
                    Err(e) => self.mem_goto_err = Some(e),
                }
            }
            if ui.button("PC").clicked() {
                self.mem_base = self.cpu.pc() & !0xF;
                self.mem_goto = format!("0x{:X}", self.cpu.pc());
                self.mem_scroll_reset = true;
            }
            if ui.button("SP").clicked() {
                self.mem_base = self.cpu.regs[13] & !0xF;
                self.mem_goto = format!("0x{:X}", self.cpu.regs[13]);
                self.mem_scroll_reset = true;
            }
            ui.separator();
            ui.selectable_value(&mut self.mem_format, MemFormat::Bytes, "Bytes");
            ui.selectable_value(&mut self.mem_format, MemFormat::Words, "Words");
            if let Some(e) = &self.mem_goto_err {
                ui.label(RichText::new(e).color(self.theme.error_text));
            }
        });
        ui.add_space(2.0);

        let font = FontId::monospace(13.0);
        let row_h = ui.fonts_mut(|f| f.row_height(&font)) + 2.0;
        let rows = 4096usize; // 64 KiB window
        let base = self.mem_base;
        let written: HashSet<u32> = self.cpu.last_writes.iter().map(|(a, _)| *a).collect();
        let pc = self.cpu.pc();
        let sp = self.cpu.regs[13];
        let theme = self.theme.clone();
        let mut sa = egui::ScrollArea::vertical()
            .id_salt(("mem", base))
            .auto_shrink(false);
        if self.mem_scroll_reset {
            sa = sa.vertical_scroll_offset(0.0);
            self.mem_scroll_reset = false;
        }
        let mem = &self.cpu.mem;
        let fmt_words = self.mem_format == MemFormat::Words;
        sa.show_rows(ui, row_h, rows, |ui, range| {
            for r in range {
                let addr = base.wrapping_add((r as u32) * 16);
                let mut job = LayoutJob::default();
                let tf = |c: Color32| TextFormat { font_id: font.clone(), color: c, ..Default::default() };
                let tf_bg = |c: Color32, bg: Color32| TextFormat { font_id: font.clone(), color: c, background: bg, ..Default::default() };
                job.append(&format!("{addr:08X}  "), 0.0, tf(theme.dim));
                if fmt_words {
                    for w in 0..4u32 {
                        let a = addr.wrapping_add(w * 4);
                        let v = mem.read_u32(a);
                        let ch = (0..4).any(|k| written.contains(&a.wrapping_add(k)));
                        let color = if ch { theme.changed } else if v == 0 { theme.dim } else { theme.text };
                        let bg = if a == pc { theme.pc_line_bg } else if a == sp { theme.accent.linear_multiply(0.25) } else { Color32::TRANSPARENT };
                        job.append(&format!("{v:08X}"), 0.0, tf_bg(color, bg));
                        job.append("  ", 0.0, tf(theme.dim));
                    }
                } else {
                    for b in 0..16u32 {
                        let a = addr.wrapping_add(b);
                        let v = mem.read_u8(a);
                        let color = if written.contains(&a) { theme.changed } else if v == 0 { theme.dim } else { theme.text };
                        let in_pc = a >= pc && a < pc.wrapping_add(4);
                        let bg = if in_pc { theme.pc_line_bg } else { Color32::TRANSPARENT };
                        job.append(&format!("{v:02X}"), 0.0, tf_bg(color, bg));
                        job.append(if b == 7 { "  " } else { " " }, 0.0, tf(theme.dim));
                    }
                }
                job.append(" ", 0.0, tf(theme.dim));
                let ascii: String = (0..16u32)
                    .map(|b| {
                        let v = mem.read_u8(addr.wrapping_add(b));
                        if (0x20..0x7F).contains(&v) { v as char } else { '·' }
                    })
                    .collect();
                job.append(&ascii, 0.0, tf(theme.syn_string));
                ui.label(job);
            }
        });
    }

    fn problems(&mut self, ui: &mut egui::Ui) {
        if self.errors.is_empty() {
            ui.label(RichText::new("No problems.").color(self.theme.dim));
            return;
        }
        let mut goto = None;
        egui::ScrollArea::vertical().id_salt("problems").auto_shrink(false).show(ui, |ui| {
            for e in &self.errors {
                let text = RichText::new(format!("Line {}: {}", e.line + 1, e.msg))
                    .font(FontId::monospace(13.0))
                    .color(self.theme.error_text);
                if ui.add(egui::Label::new(text).sense(Sense::click())).on_hover_text("Click to go to this line").clicked() {
                    goto = Some(e.line);
                }
            }
        });
        if let Some(line) = goto {
            self.center_tab = CenterTab::Editor;
            self.scroll_editor_to = Some(line);
            editor::set_cursor_line(ui.ctx(), &self.source, line);
        }
    }

    fn symbols(&mut self, ui: &mut egui::Ui) {
        let Some(p) = &self.program else {
            ui.label(RichText::new("Assemble the program to see its symbols.").color(self.theme.dim));
            return;
        };
        let mut goto = None;
        egui::ScrollArea::vertical().id_salt("symbols").auto_shrink(false).show(ui, |ui| {
            egui::Grid::new("symgrid").striped(true).num_columns(3).spacing([24.0, 3.0]).show(ui, |ui| {
                ui.label(RichText::new("Symbol").strong());
                ui.label(RichText::new("Hex").strong());
                ui.label(RichText::new("Decimal").strong());
                ui.end_row();
                let mut syms: Vec<_> = p.symbols.iter().collect();
                syms.sort_by_key(|(_, v)| **v);
                for (name, v) in syms {
                    let r = ui.add(
                        egui::Label::new(RichText::new(name).font(FontId::monospace(13.0)).color(self.theme.syn_label))
                            .sense(Sense::click()),
                    );
                    if r.on_hover_text("Click to view in memory").clicked() {
                        goto = Some((name.clone(), *v));
                    }
                    ui.label(RichText::new(format!("0x{v:08X}")).font(FontId::monospace(13.0)));
                    ui.label(RichText::new(format!("{}", *v as i32)).font(FontId::monospace(13.0)).color(self.theme.dim));
                    ui.end_row();
                }
            });
        });
        if let Some((name, v)) = goto {
            self.mem_base = v & !0xF;
            self.mem_goto = name;
            self.mem_scroll_reset = true;
            self.bottom_tab = BottomTab::Memory;
        }
    }

    fn center(&mut self, ui: &mut egui::Ui) {
        let editor_title = format!("📝 {}{}", self.file_name(), if self.dirty { " *" } else { "" });
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.center_tab, CenterTab::Editor, editor_title);
            ui.selectable_value(&mut self.center_tab, CenterTab::Listing, "🔍 Listing / Disassembly");
        });
        ui.separator();
        match self.center_tab {
            CenterTab::Editor => self.editor(ui),
            CenterTab::Listing => self.listing(ui),
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui) {
        let pc_line = if matches!(self.state, RunState::NotLoaded) { None } else { self.pc_line() };
        let args = EditorArgs {
            font_size: self.font_size,
            tab_width: self.tab_width,
            breakpoints: &self.breakpoints,
            code_lines: &self.code_lines,
            pc_line,
            error_lines: &self.error_lines,
            scroll_to_line: self.scroll_editor_to.take(),
            read_only: self.state == RunState::Running,
            theme: &self.theme,
        };
        let res = editor::show(ui, &mut self.source, &mut self.highlight_cache, &args);
        if res.changed {
            self.dirty = true;
            if !self.error_lines.is_empty() {
                // Errors may no longer apply; keep the list but drop the highlight.
                self.error_lines.clear();
            }
        }
        if let Some(c) = res.cursor {
            self.cursor = c;
        }
        if let Some(line) = res.toggled_breakpoint {
            self.toggle_breakpoint(line);
        }
    }

    fn listing(&mut self, ui: &mut egui::Ui) {
        let Some(prog) = &self.program else {
            ui.label(RichText::new("Assemble the program (F7) to see the listing.").color(self.theme.dim));
            return;
        };
        let font = FontId::monospace(self.font_size - 1.0);
        let row_h = ui.fonts_mut(|f| f.row_height(&font)) + 3.0;
        let src_lines: Vec<&str> = self.loaded_source.split('\n').collect();
        let pc = self.cpu.pc();
        let n = prog.listing.len();
        let pc_row = prog.listing.iter().position(|r| r.addr == pc && r.is_code);
        let mut sa = egui::ScrollArea::both().id_salt("listing").auto_shrink(false);
        if self.scroll_listing {
            if let Some(i) = pc_row {
                let h = ui.available_height();
                sa = sa.vertical_scroll_offset((i as f32 * row_h - h / 2.0).max(0.0));
            }
            self.scroll_listing = false;
        }
        let theme = self.theme.clone();
        let mut toggle = None;
        sa.show_rows(ui, row_h, n, |ui, range| {
            for i in range {
                let row = &prog.listing[i];
                let is_pc = row.is_code && row.addr == pc && self.state != RunState::NotLoaded;
                let has_bp = row.is_code && self.bp_addrs.contains(&row.addr);
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(900.0), row_h), Sense::click());
                let painter = ui.painter();
                if is_pc {
                    painter.rect_filled(rect, 0.0, theme.pc_line_bg);
                } else if resp.hovered() {
                    painter.rect_filled(rect, 0.0, ui.visuals().widgets.hovered.bg_fill.linear_multiply(0.3));
                }
                let cy = rect.center().y;
                if has_bp {
                    painter.circle_filled(egui::pos2(rect.left() + 8.0, cy), 5.0, theme.breakpoint);
                }
                if is_pc {
                    painter.text(egui::pos2(rect.left() + 16.0, cy), egui::Align2::LEFT_CENTER, "▶", font.clone(), theme.pc_arrow);
                }
                let mut x = rect.left() + 34.0;
                let mut put = |s: &str, c: Color32, w: f32| {
                    painter.text(egui::pos2(x, cy), egui::Align2::LEFT_CENTER, s, font.clone(), c);
                    x += w;
                };
                let cw = self.font_size * 0.62;
                put(&format!("{:08X}", row.addr), theme.dim, cw * 10.0);
                let hex = if row.bytes.len() == 4 && row.is_code {
                    format!("{:08X}", u32::from_le_bytes([row.bytes[0], row.bytes[1], row.bytes[2], row.bytes[3]]))
                } else {
                    row.bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
                };
                put(&hex, theme.syn_number, cw * 13.0);
                let dis = if row.is_code {
                    let w = u32::from_le_bytes([row.bytes[0], row.bytes[1], row.bytes[2], row.bytes[3]]);
                    disassemble(w, row.addr, &|a| prog.symbols.iter().find(|(_, v)| **v == a).map(|(k, _)| k.clone()))
                } else if row.line.is_none() && row.bytes.len() == 4 {
                    "(literal pool)".to_string()
                } else {
                    String::new()
                };
                put(&dis, theme.syn_mnemonic, cw * 30.0);
                if let Some(l) = row.line {
                    let src = src_lines.get(l).copied().unwrap_or("").trim_end();
                    put(&format!("{:>4}  {}", l + 1, src), theme.dim, 0.0);
                }
                if resp.clicked() && row.is_code {
                    if let Some(l) = row.line.or_else(|| prog.addr_to_line.get(&row.addr).copied()) {
                        toggle = Some(l);
                    }
                }
            }
        });
        if let Some(l) = toggle {
            self.toggle_breakpoint(l);
        }
    }
}

impl eframe::App for VaranApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_close(&ctx);
        self.handle_shortcuts(&ctx);
        self.do_pending(&ctx);
        if self.state == RunState::Running {
            self.run_frame();
            ctx.request_repaint();
        }
        self.update_title(&ctx);

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(3.0);
            self.toolbar(ui);
            ui.add_space(3.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::right("registers")
            .resizable(true)
            .default_size(300.0)
            .min_size(240.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("regs_scroll").show(ui, |ui| self.registers_panel(ui));
            });
        egui::Panel::bottom("bottom")
            .resizable(true)
            .default_size(240.0)
            .min_size(120.0)
            .show(ui, |ui| self.bottom_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.center(ui));
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_labels_match_platform() {
        let (save, save_as) = (sc_text(&SC_SAVE), sc_text(&SC_SAVE_AS));
        if cfg!(target_os = "macos") {
            assert_eq!((save.as_str(), save_as.as_str()), ("Cmd+S", "Shift+Cmd+S"));
        } else {
            assert_eq!((save.as_str(), save_as.as_str()), ("Ctrl+S", "Ctrl+Shift+S"));
        }
    }
}
