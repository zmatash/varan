//! The COMP1054 ARM instruction summary (docs/ARMInst.pdf) as a searchable,
//! non-modal reference window, plus a way to open the original PDF.

use eframe::egui::{self, Color32, FontId, RichText, text::LayoutJob, text::TextFormat};

use crate::theme::Theme;
use varan::asm::parse_reg;

/// The original handout, embedded so the single-file exe can still open it.
pub const PDF: &[u8] = include_bytes!("../docs/ARMInst.pdf");

pub struct Section {
    pub title: &'static str,
    /// Column headings (syntax, meaning[, extra]).
    pub columns: &'static [&'static str],
    pub rows: &'static [&'static [&'static str]],
    pub note: &'static str,
}

pub const SECTIONS: &[Section] = &[
    Section {
        title: "Data processing",
        columns: &["Syntax", "Meaning"],
        rows: &[
            &["ADD  Rd, Rn, Op", "Rd = Rn + Op"],
            &["SUB  Rd, Rn, Op", "Rd = Rn - Op"],
            &["RSB  Rd, Rn, Op", "Rd = Op - Rn (\"reverse subtract\")"],
            &["AND  Rd, Rn, Op", "Rd = Rn AND Op"],
            &["ORR  Rd, Rn, Op", "Rd = Rn OR Op"],
            &["EOR  Rd, Rn, Op", "Rd = Rn exclusive-OR Op"],
            &["BIC  Rd, Rn, Op", "Bit clear: Rd = Rn AND NOT Op"],
            &["MOV  Rd, Op", "Rd = Op"],
            &["MVN  Rd, Op", "Rd = NOT Op"],
            &["CMP  Rn, Op", "Set flags on Rn - Op"],
            &["CMN  Rn, Op", "Set flags on Rn + Op"],
            &["TST  Rn, Op", "Set flags on Rn AND Op"],
            &["TEQ  Rn, Op", "Set flags on Rn EOR Op"],
            &["MUL  Rd, Rm, Rs", "Rd = Rm * Rs"],
            &["MLA  Rd, Rm, Rs, Rn", "Rd = (Rm * Rs) + Rn"],
        ],
        note: "Op can be a literal (#10), a register (R1) or a shifted register (R1, LSL #3). \
               Add S to set the flags (ADDS); it goes after any condition code (ADDNES).",
    },
    Section {
        title: "Shifts",
        columns: &["Syntax", "Meaning"],
        rows: &[
            &["ADD  R1, R4, R5, LSL #3", "R1 = R4 + R5 * 8 (shift by a literal)"],
            &["ADD  R1, R4, R5, LSL R6", "R1 = R4 + R5 * 2^R6 (shift by a register)"],
            &["LSL  #n", "Logical shift left 0-31 places, 0s into the low end"],
            &["LSR  #n", "Logical shift right 1-32 places, 0s into the high end"],
            &["ASR  #n", "Arithmetic shift right 1-32 places, copying the sign bit"],
            &["ROR  #n", "Rotate right 1-31 places, bits wrap around"],
            &["RRX", "Rotate right one place through the carry flag"],
        ],
        note: "",
    },
    Section {
        title: "Loads and stores",
        columns: &["Syntax", "Meaning"],
        rows: &[
            &["LDR   Rd, Address", "Load a 32-bit word into Rd"],
            &["LDRB  Rd, Address", "Load a byte into Rd; the top 24 bits are 0"],
            &["STR   Rd, Address", "Store Rd to memory"],
            &["STRB  Rd, Address", "Store the bottom byte of Rd"],
            &["LDMFD Rn!, {reg list}", "Load multiple registers (pop from a full-descending stack)"],
            &["STMFD Rn!, {reg list}", "Store multiple registers (push); Rn is updated"],
        ],
        note: "An Address can be a label (fred), a number, or computed from registers - see below.",
    },
    Section {
        title: "Addressing modes",
        columns: &["Operand", "Address used", "R0 afterwards"],
        rows: &[
            &["[R0]", "R0", "unchanged"],
            &["[R0, R1]", "R0 + R1", "unchanged"],
            &["[R0, #1]", "R0 + 1", "unchanged"],
            &["[R0, R1]!", "R0 + R1", "R0 + R1 (pre-indexed, write-back)"],
            &["[R0, #1]!", "R0 + 1", "R0 + 1 (pre-indexed, write-back)"],
            &["[R0], R1", "R0", "R0 + R1 (post-indexed)"],
            &["[R0], #1", "R0", "R0 + 1 (post-indexed)"],
            &["[R0, R1, LSL #3]", "R0 + R1 * 8", "unchanged"],
        ],
        note: "",
    },
    Section {
        title: "Control transfer",
        columns: &["Syntax", "Meaning"],
        rows: &[
            &["B    label", "Branch: R15 = label"],
            &["BL   function", "Branch and link: R14 = return address, R15 = function"],
            &["MOV  PC, LR", "Return from a function called with BL"],
            &["SWI  number", "Software interrupt (see SWI calls below)"],
        ],
        note: "",
    },
    Section {
        title: "Condition codes",
        columns: &["Code", "Meaning", "Flags"],
        rows: &[
            &["EQ", "Equal", "Z"],
            &["NE", "Not equal", "!Z"],
            &["GE", "Greater than or equal (signed)", "N = V"],
            &["GT", "Greater than (signed)", "!Z and N = V"],
            &["LT", "Less than (signed)", "N != V"],
            &["LE", "Less than or equal (signed)", "Z or N != V"],
            &["HI", "Higher (unsigned)", "C and !Z"],
            &["LS", "Lower or same (unsigned)", "!C or Z"],
            &["CS / HS", "Carry set / higher or same (unsigned)", "C"],
            &["CC / LO", "Carry clear / lower (unsigned)", "!C"],
            &["MI", "Minus (negative)", "N"],
            &["PL", "Plus (positive or zero)", "!N"],
            &["VS", "Overflow set", "V"],
            &["VC", "Overflow clear", "!V"],
            &["AL", "Always (the default)", "-"],
        ],
        note: "Any instruction can be conditional: ADD becomes ADDEQ, B becomes BNE, etc.",
    },
    Section {
        title: "Pseudo-operations",
        columns: &["Syntax", "Meaning"],
        rows: &[
            &["NOP", "No operation"],
            &["MOV  Rd, #-nn", "Replaced by MVN when needed"],
            &["CMP  Rn, #-nn", "Replaced by CMN when needed"],
            &["ADR  Rd, label", "Rd = address of label (becomes ADD/SUB Rd, R15, #nn)"],
            &["ADRL Rd, label", "Like ADR, for labels out of ADR's range"],
            &["LDR  Rd, =nnnnnnnn", "Load any 32-bit constant or address"],
            &["PUSH {reg list}", "Same as STMFD SP!, {reg list}"],
            &["POP  {reg list}", "Same as LDMFD SP!, {reg list}"],
        ],
        note: "",
    },
    Section {
        title: "Directives",
        columns: &["Syntax", "Meaning"],
        rows: &[
            &["ENTRY", "Mark the code entry point"],
            &["ORIGIN 0x1000", "Set the start address of the code or data that follows"],
            &["ALIGN", "Align to the next 4-byte boundary"],
            &["name  EQU  0x2A", "Define a compile-time constant"],
            &["DEFW 0x12345678", "Define the value of the next word(s)"],
            &["DEFB 0, 2, \"bytes\"", "Define the value of the next byte(s)"],
            &["DEFS 0x20", "Reserve the next 0x20 bytes of storage"],
        ],
        note: "DEFB, DEFW and DEFS plant data amongst the instructions of a program.",
    },
    Section {
        title: "SWI calls",
        columns: &["Call", "Effect"],
        rows: &[
            &["SWI 0", "Print the character in R0"],
            &["SWI 1", "Read one character typed in the console into R0"],
            &["SWI 2", "Halt the program"],
            &["SWI 3", "Print the zero-terminated string whose address is in R0"],
            &["SWI 4", "Print R0 as a signed decimal integer"],
        ],
        note: "",
    },
];

#[derive(Default)]
pub struct CheatSheet {
    pub open: bool,
    pub filter: String,
    pub focus_filter: bool,
}

fn matches(filter: &str, cells: &[&str]) -> bool {
    filter.is_empty() || cells.iter().any(|c| c.to_ascii_lowercase().contains(filter))
}

/// Syntax-coloured layout for an instruction template like `ADD  Rd, Rn, Op`.
fn syntax_job(s: &str, theme: &Theme, font: &FontId, all_keywords: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    let fmt = |c: Color32| TextFormat { font_id: font.clone(), color: c, ..Default::default() };
    let mut first = true;
    let mut rest = s;
    while !rest.is_empty() {
        let split = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .map(|i| if i == 0 { rest.chars().next().unwrap().len_utf8() } else { i })
            .unwrap_or(rest.len());
        let (tok, tail) = rest.split_at(split);
        let is_word = tok.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
        let up = tok.to_ascii_uppercase();
        let color = if tok.starts_with('#') || tok.starts_with('"') || tok.chars().all(|c| c.is_ascii_digit()) {
            theme.syn_number
        } else if is_word && (first || all_keywords) {
            first = false;
            theme.syn_mnemonic
        } else if is_word && (parse_reg(tok).is_some() || matches!(up.as_str(), "RD" | "RN" | "RM" | "RS" | "OP")) {
            theme.syn_register
        } else if is_word && matches!(up.as_str(), "LSL" | "LSR" | "ASR" | "ROR" | "RRX" | "EQU") {
            theme.syn_mnemonic
        } else if is_word {
            theme.syn_label
        } else {
            theme.text
        };
        job.append(tok, 0.0, fmt(color));
        rest = tail;
    }
    job
}

/// Draw one section as a bordered table. Column widths are measured over all
/// of the section's rows (not just the filtered ones) so they stay put while
/// searching. The last column wraps.
fn table(ui: &mut egui::Ui, theme: &Theme, sec: &Section, rows: &[&&[&str]], mono: &FontId) {
    let body = FontId::proportional(13.5);
    let head = FontId::proportional(11.5);
    let (pad, gap) = (10.0, 20.0);
    let all_kw = sec.title == "Condition codes";
    let painter = ui.painter().clone();
    let measure = |job: LayoutJob| painter.layout_job(job).size().x;
    let col0 = sec
        .rows
        .iter()
        .map(|r| measure(syntax_job(r[0], theme, mono, all_kw)))
        .fold(0.0, f32::max);
    let outer = ui.available_width();
    let inner = outer - 2.0 * pad;
    let mut widths = vec![col0];
    if sec.columns.len() == 3 {
        let natural = sec
            .rows
            .iter()
            .map(|r| painter.layout_no_wrap(r[1].to_string(), body.clone(), theme.text).size().x)
            .fold(0.0, f32::max);
        widths.push(natural.min((inner - col0 - 2.0 * gap) * 0.6));
    }
    let used: f32 = widths.iter().sum::<f32>() + gap * widths.len() as f32;
    widths.push((inner - used).max(80.0));

    egui::Frame::new()
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.border))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(0, 4))
        .show(ui, |ui| {
            ui.set_width(outer - 2.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            let draw_row = |ui: &mut egui::Ui, galleys: Vec<std::sync::Arc<egui::Galley>>, stripe: bool| {
                let h = galleys.iter().map(|g| g.size().y).fold(0.0, f32::max) + 8.0;
                let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::hover());
                if stripe {
                    ui.painter().rect_filled(rect, 0.0, theme.stripe);
                }
                let mut x = rect.left() + pad;
                for (g, w) in galleys.into_iter().zip(&widths) {
                    ui.painter().galley(egui::pos2(x, rect.top() + 4.0), g, theme.text);
                    x += w + gap;
                }
            };
            let header = sec
                .columns
                .iter()
                .map(|c| painter.layout_no_wrap(c.to_uppercase(), head.clone(), theme.dim))
                .collect();
            draw_row(ui, header, false);
            for (i, row) in rows.iter().enumerate() {
                let mut galleys = vec![painter.layout_job(syntax_job(row[0], theme, mono, all_kw))];
                for (c, w) in row[1..].iter().zip(&widths[1..]) {
                    galleys.push(painter.layout(c.to_string(), body.clone(), theme.text, *w));
                }
                draw_row(ui, galleys, i % 2 == 0);
            }
        });
}

impl CheatSheet {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.focus_filter = self.open;
    }

    pub fn show(&mut self, ctx: &egui::Context, theme: &Theme) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("ARM Instruction Reference")
            .open(&mut open)
            .default_size([660.0, 640.0])
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .min_width(420.0)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| self.contents(ui, theme));
        self.open = open;
    }

    fn contents(&mut self, ui: &mut egui::Ui, theme: &Theme) {
        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("Search, e.g. LDR, carry, signed, SWI...")
                    .desired_width(ui.available_width() - 120.0),
            );
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search reference"));
            if self.focus_filter {
                resp.request_focus();
                self.focus_filter = false;
            }
            if ui
                .button("Open PDF")
                .on_hover_text("Open the original COMP1054 instruction summary")
                .clicked()
            {
                if let Err(e) = open_pdf() {
                    log_error(&e);
                }
            }
        });
        ui.add_space(4.0);
        let filter = self.filter.trim().to_ascii_lowercase();
        let mono = FontId::monospace(13.0);
        let mut any = false;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            for sec in SECTIONS {
                let title_hit = matches(&filter, &[sec.title]);
                let rows: Vec<_> = sec
                    .rows
                    .iter()
                    .filter(|r| title_hit || matches(&filter, r))
                    .collect();
                if rows.is_empty() {
                    continue;
                }
                any = true;
                ui.add_space(8.0);
                ui.label(RichText::new(sec.title).size(15.5).strong().color(theme.text));
                ui.add_space(2.0);
                table(ui, theme, sec, &rows, &mono);
                if !sec.note.is_empty() && (title_hit || filter.is_empty()) {
                    ui.add_space(2.0);
                    ui.add(egui::Label::new(RichText::new(sec.note).color(theme.dim).size(12.5)).wrap());
                }
            }
            if !any {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(format!("Nothing matches \"{}\".", self.filter.trim())).color(theme.dim));
                });
            }
            ui.add_space(8.0);
        });
    }
}

fn log_error(e: &str) {
    rfd::MessageDialog::new()
        .set_title("Could not open PDF")
        .set_description(e)
        .set_level(rfd::MessageLevel::Error)
        .show();
}

/// Write the embedded PDF to the temp directory and open it in the default viewer.
pub fn open_pdf() -> Result<(), String> {
    let path = std::env::temp_dir().join("COMP1054 ARM Instruction Summary.pdf");
    std::fs::write(&path, PDF).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    #[cfg(windows)]
    let mut cmd = {
        let mut c = std::process::Command::new("explorer");
        c.arg(&path);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(&path);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(&path);
        c
    };
    cmd.spawn().map(|_| ()).map_err(|e| format!("Could not open the PDF viewer: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_pdf_is_a_pdf() {
        assert!(PDF.starts_with(b"%PDF"));
    }

    #[test]
    fn every_row_fills_its_columns() {
        for s in SECTIONS {
            for r in s.rows {
                assert_eq!(r.len(), s.columns.len(), "{}: {:?}", s.title, r);
            }
        }
    }

    #[test]
    fn filter_is_case_insensitive_substring() {
        assert!(matches("ldr", &["LDR  Rd, Address"]));
        assert!(matches("", &["anything"]));
        assert!(!matches("swi", &["ADD  Rd, Rn, Op", "Rd = Rn + Op"]));
    }

    /// The reference's examples must actually assemble in Varan.
    #[test]
    fn instruction_examples_assemble() {
        let src = "        ENTRY\n\
                   start   ADD  R1, R4, R5, LSL #3\n\
                   \x20       ADD  R1, R4, R5, LSL R6\n\
                   \x20       LDR  R2, [R0, R1, LSL #3]\n\
                   \x20       LDR  R2, [R0], #1\n\
                   \x20       STMFD SP!, {R0-R2, LR}\n\
                   \x20       LDMFD SP!, {R0-R2, LR}\n\
                   \x20       PUSH {R4}\n\
                   \x20       POP  {R4}\n\
                   \x20       ADDNES R0, R0, #1\n\
                   \x20       NOP\n\
                   \x20       LDR  R0, =0x12345678\n\
                   \x20       ADR  R0, start\n\
                   \x20       ADRL R0, start\n\
                   \x20       SWI  2\n\
                   name    EQU  0x2A\n\
                   \x20       DEFW 0x12345678\n\
                   \x20       DEFB 0, 2, \"bytes\"\n\
                   \x20       ALIGN\n\
                   \x20       DEFS 0x20\n";
        if let Err(e) = varan::asm::assemble(src) {
            panic!("{e:?}");
        }
    }
}
