//! Code editor widget: syntax highlighting, line-number gutter with
//! breakpoints, and current-line / error-line highlighting.

use std::collections::BTreeSet;
use std::sync::Arc;

use eframe::egui::{
    self, Align, FontFamily, FontId, Galley, Id, Margin, Pos2, Rect, Sense, Shape, Stroke, TextBuffer,
    TextEdit, Vec2,
    text::{CCursor, CCursorRange, LayoutJob, TextFormat},
};

use crate::theme::Theme;
use varan::asm::{is_keyword, parse_reg};

pub struct EditorArgs<'a> {
    pub font_size: f32,
    /// Columns per tab stop.
    pub tab_width: usize,
    pub breakpoints: &'a BTreeSet<usize>,
    /// Lines that contain assembled instructions (breakpoint-able).
    pub code_lines: &'a BTreeSet<usize>,
    pub pc_line: Option<usize>,
    pub error_lines: &'a BTreeSet<usize>,
    pub scroll_to_line: Option<usize>,
    pub read_only: bool,
    pub theme: &'a Theme,
}

#[derive(Default)]
pub struct EditorResult {
    pub changed: bool,
    pub toggled_breakpoint: Option<usize>,
    /// (line, column), 0-based.
    pub cursor: Option<(usize, usize)>,
}

#[derive(Default)]
pub struct HighlightCache {
    text: String,
    key: (u32, bool, usize),
    job: Option<LayoutJob>,
}

pub const EDITOR_ID: &str = "varan_code_editor";

pub fn editor_id() -> Id {
    Id::unique(EDITOR_ID)
}

/// Move the editor cursor to the start of `line` and focus it.
pub fn set_cursor_line(ctx: &egui::Context, text: &str, line: usize) {
    let idx: usize = text
        .split('\n')
        .take(line)
        .map(|l| l.chars().count() + 1)
        .sum();
    let id = editor_id();
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(CCursorRange::one(CCursor::new(idx))));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

pub fn show(
    ui: &mut egui::Ui,
    text: &mut String,
    cache: &mut HighlightCache,
    args: &EditorArgs,
) -> EditorResult {
    let mut result = EditorResult::default();
    let font_id = FontId::monospace(args.font_size);
    let theme = args.theme;
    let n_lines = text.split('\n').count().max(1);
    let digits = n_lines.to_string().len().max(3);
    let char_w = ui.fonts_mut(|f| f.glyph_width(&font_id, '0'));
    let row_h = ui.fonts_mut(|f| f.row_height(&font_id));
    let gutter_w = char_w * digits as f32 + 34.0;

    let mut layouter = |ui: &egui::Ui, buf: &dyn TextBuffer, _wrap: f32| -> Arc<Galley> {
        let key = (args.font_size.to_bits(), theme.dark, args.tab_width);
        if cache.job.is_none() || cache.text != buf.as_str() || cache.key != key {
            cache.text = buf.as_str().to_string();
            cache.key = key;
            cache.job = Some(highlight(buf.as_str(), &font_id, theme, char_w, args.tab_width));
        }
        ui.painter().layout_job(cache.job.clone().unwrap())
    };

    egui::ScrollArea::both()
        .id_salt("editor_scroll")
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let min_h = ui.available_height().max(n_lines as f32 * row_h + row_h);
                let (gutter_rect, gutter_resp) =
                    ui.allocate_exact_size(Vec2::new(gutter_w, min_h), Sense::click());
                let bg_idx = ui.painter().add(Shape::Noop);

                let visible_rows = ((ui.available_height() / row_h) as usize).max(n_lines);
                let output = TextEdit::multiline(text)
                    .id(editor_id())
                    .font(font_id.clone())
                    .code_editor()
                    .frame(egui::Frame::NONE)
                    .margin(Margin::ZERO)
                    .desired_width(f32::INFINITY)
                    .desired_rows(visible_rows)
                    .lock_focus(true)
                    .interactive(!args.read_only)
                    .layouter(&mut layouter)
                    .show(ui);

                result.changed = output.response.changed();

                let galley = &output.galley;
                let origin = output.galley_pos;
                let full_right = ui.clip_rect().right().max(output.response.rect.right());
                let row_rect = |i: usize| -> Rect {
                    let y = match galley.rows.get(i) {
                        Some(r) => origin.y + r.rect().min.y,
                        None => origin.y + i as f32 * row_h,
                    };
                    Rect::from_min_max(
                        Pos2::new(gutter_rect.left(), y),
                        Pos2::new(full_right, y + row_h),
                    )
                };

                // Line backgrounds (painted behind the text).
                let mut bg_shapes = Vec::new();
                for &l in args.error_lines {
                    if l < n_lines {
                        bg_shapes.push(Shape::rect_filled(row_rect(l), 0.0, theme.error_line_bg));
                    }
                }
                if let Some(l) = args.pc_line {
                    if l < n_lines {
                        let r = row_rect(l);
                        bg_shapes.push(Shape::rect_filled(r, 0.0, theme.pc_line_bg));
                    }
                }
                ui.painter().set(bg_idx, Shape::Vec(bg_shapes));

                // Gutter.
                let painter = ui.painter();
                painter.rect_filled(gutter_rect, 0.0, theme.gutter_bg);
                let cursor_line = output.cursor_range.map(|c| {
                    let idx = c.primary.index;
                    line_col_of(text, idx.0)
                });
                if let Some(lc) = cursor_line {
                    result.cursor = Some(lc);
                }
                let clip = ui.clip_rect();
                for i in 0..n_lines {
                    let r = row_rect(i);
                    if r.bottom() < clip.top() || r.top() > clip.bottom() {
                        continue;
                    }
                    let is_cur = cursor_line.map(|c| c.0) == Some(i);
                    let color = if is_cur { theme.text } else { theme.gutter_text };
                    painter.text(
                        Pos2::new(gutter_rect.right() - 8.0, r.center().y),
                        egui::Align2::RIGHT_CENTER,
                        (i + 1).to_string(),
                        font_id.clone(),
                        color,
                    );
                    let dot_c = Pos2::new(gutter_rect.left() + 9.0, r.center().y);
                    if args.breakpoints.contains(&i) {
                        let active = args.code_lines.contains(&i);
                        let col = if active { theme.breakpoint } else { theme.breakpoint_inactive };
                        painter.circle_filled(dot_c, (row_h * 0.3).min(6.0), col);
                    }
                    if args.pc_line == Some(i) {
                        let s = (row_h * 0.32).min(6.0);
                        let x = gutter_rect.left() + 18.0;
                        painter.add(Shape::convex_polygon(
                            vec![
                                Pos2::new(x - s, dot_c.y - s),
                                Pos2::new(x + s * 0.7, dot_c.y),
                                Pos2::new(x - s, dot_c.y + s),
                            ],
                            theme.pc_arrow,
                            Stroke::NONE,
                        ));
                    }
                }
                painter.vline(
                    gutter_rect.right() - 2.0,
                    gutter_rect.y_range(),
                    Stroke::new(1.0, theme.gutter_border),
                );

                if gutter_resp.clicked() {
                    if let Some(p) = gutter_resp.interact_pointer_pos() {
                        let line = ((p.y - origin.y) / row_h).floor();
                        if line >= 0.0 && (line as usize) < n_lines {
                            result.toggled_breakpoint = Some(line as usize);
                        }
                    }
                }
                gutter_resp.on_hover_text("Click to toggle a breakpoint");

                if let Some(l) = args.scroll_to_line {
                    if l < n_lines {
                        ui.scroll_to_rect(row_rect(l), Some(Align::Center));
                    }
                }
            });
        });
    result
}

fn line_col_of(text: &str, char_idx: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for (i, c) in text.chars().enumerate() {
        if i == char_idx {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[derive(Clone, Copy, PartialEq)]
enum Tok {
    Plain,
    Comment,
    Label,
    Mnemonic,
    Directive,
    Register,
    Number,
    Str,
    Symbol,
}

fn is_directive_word(w: &str) -> bool {
    matches!(
        w.to_ascii_uppercase().as_str(),
        "ENTRY" | "ORIGIN" | "ORG" | "ALIGN" | "EQU" | "DEFW" | "DCD" | "DEFH" | "DCW" | "DEFB"
            | "DCB" | "DEFS" | "SPACE" | "LTORG" | "END"
    )
}

/// Syntax-highlight assembly source into a layout job.
pub fn highlight(src: &str, font_id: &FontId, theme: &Theme, char_w: f32, tab_width: usize) -> LayoutJob {
    let mut job = TabJob::new(font_id, char_w, tab_width);
    let fmt = |t: Tok| TextFormat {
        font_id: font_id.clone(),
        color: match t {
            Tok::Plain => theme.text,
            Tok::Comment => theme.syn_comment,
            Tok::Label => theme.syn_label,
            Tok::Mnemonic => theme.syn_mnemonic,
            Tok::Directive => theme.syn_directive,
            Tok::Register => theme.syn_register,
            Tok::Number => theme.syn_number,
            Tok::Str => theme.syn_string,
            Tok::Symbol => theme.syn_symbol,
        },
        ..Default::default()
    };

    for (li, line) in src.split('\n').enumerate() {
        if li > 0 {
            job.newline(fmt(Tok::Plain));
        }
        let chars: Vec<(usize, char)> = line.char_indices().collect();
        let mut i = 0;
        let mut seen_word = false;
        while i < chars.len() {
            let (start, c) = chars[i];
            let end_of = |j: usize| if j < chars.len() { chars[j].0 } else { line.len() };
            if c == ';' {
                job.add(&line[start..], fmt(Tok::Comment));
                break;
            }
            if c == '"' || c == '\'' {
                let mut j = i + 1;
                while j < chars.len() {
                    if chars[j].1 == '\\' {
                        j += 2;
                        continue;
                    }
                    if chars[j].1 == c {
                        j += 1;
                        break;
                    }
                    j += 1;
                }
                let j = j.min(chars.len());
                job.add(&line[start..end_of(j)], fmt(Tok::Str));
                i = j;
                continue;
            }
            if c.is_ascii_alphabetic() || c == '_' {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].1.is_ascii_alphanumeric() || chars[j].1 == '_' || chars[j].1 == '.') {
                    j += 1;
                }
                let word = &line[start..end_of(j)];
                let followed_by_colon = chars.get(j).map(|x| x.1) == Some(':');
                let tok = if parse_reg(word).is_some() {
                    Tok::Register
                } else if followed_by_colon || (start == 0 && !is_keyword(word)) {
                    Tok::Label
                } else if is_directive_word(word) {
                    Tok::Directive
                } else if !seen_word {
                    if is_keyword(word) { Tok::Mnemonic } else { Tok::Label }
                } else if matches!(word.to_ascii_uppercase().as_str(), "LSL" | "LSR" | "ASR" | "ROR" | "RRX" | "ASL") {
                    Tok::Mnemonic
                } else {
                    Tok::Symbol
                };
                // A label followed by a mnemonic: the mnemonic is the next word.
                if tok == Tok::Label {
                    seen_word = false;
                } else {
                    seen_word = true;
                }
                job.add(word, fmt(tok));
                i = j;
                continue;
            }
            if c == '#' || c.is_ascii_digit() || c == '&' {
                let mut j = i + 1;
                if c == '#' && j < chars.len() && chars[j].1 == '-' {
                    j += 1;
                }
                while j < chars.len() && (chars[j].1.is_ascii_alphanumeric() || chars[j].1 == '_') {
                    j += 1;
                }
                let s = &line[start..end_of(j)];
                // '#label' -> keep # as number colour but label as symbol.
                if c == '#' && s.len() > 1 && s[1..].starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_') {
                    job.add("#", fmt(Tok::Number));
                    i += 1;
                    continue;
                }
                job.add(s, fmt(Tok::Number));
                i = j;
                continue;
            }
            // Plain run.
            let mut j = i + 1;
            while j < chars.len() {
                let ch = chars[j].1;
                if ch == ';' || ch == '"' || ch == '\'' || ch == '#' || ch == '&' || ch.is_ascii_alphanumeric() || ch == '_' {
                    break;
                }
                j += 1;
            }
            job.add(&line[start..end_of(j)], fmt(Tok::Plain));
            i = j;
        }
    }
    job.job
}

/// Font family whose tab character has zero width. Tabs are drawn in this
/// family with explicit leading space, so they line up on real tab stops
/// (egui itself renders every tab as a fixed width).
pub const TAB_FAMILY: &str = "varan_tab";

/// Register the zero-width-tab font family. Call once at startup.
pub fn install_fonts(ctx: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();
    let mono = defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    if let Some(data) = mono.first().and_then(|name| defs.font_data.get(name)).cloned() {
        let mut data = (*data).clone();
        data.tweak.tab_size = 0.0;
        defs.font_data.insert(TAB_FAMILY.to_string(), Arc::new(data));
        let mut family = vec![TAB_FAMILY.to_string()];
        family.extend(mono.iter().skip(1).cloned());
        defs.families.insert(FontFamily::Name(TAB_FAMILY.into()), family);
    }
    ctx.set_fonts(defs);
}

/// Builds a layout job, tracking the visual column so tabs advance to the
/// next multiple of `tab_width` columns. The text is kept unchanged (tabs
/// stay tabs), so cursor positions map 1:1 onto the source.
struct TabJob {
    job: LayoutJob,
    col: usize,
    char_w: f32,
    tab_width: usize,
    tab_font: FontId,
}

impl TabJob {
    fn new(font_id: &FontId, char_w: f32, tab_width: usize) -> Self {
        TabJob {
            job: LayoutJob::default(),
            col: 0,
            char_w,
            tab_width: tab_width.max(1),
            tab_font: FontId::new(font_id.size, FontFamily::Name(TAB_FAMILY.into())),
        }
    }

    fn newline(&mut self, format: TextFormat) {
        self.job.append("\n", 0.0, format);
        self.col = 0;
    }

    fn add(&mut self, text: &str, format: TextFormat) {
        let mut rest = text;
        while let Some(pos) = rest.find('\t') {
            let before = &rest[..pos];
            if !before.is_empty() {
                self.job.append(before, 0.0, format.clone());
                self.col += before.chars().count();
            }
            let stop = (self.col / self.tab_width + 1) * self.tab_width;
            let width = (stop - self.col) as f32 * self.char_w;
            let tab_format = TextFormat { font_id: self.tab_font.clone(), ..format.clone() };
            self.job.append("\t", width, tab_format);
            self.col = stop;
            rest = &rest[pos + 1..];
        }
        if !rest.is_empty() {
            self.job.append(rest, 0.0, format);
            self.col += rest.chars().count();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// x position (in columns) of the first glyph `ch` on each row.
    fn columns_of(src: &str, ch: char, tab_width: usize) -> Vec<f32> {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        ctx.begin_pass(egui::RawInput::default());
        let font = FontId::monospace(14.0);
        let char_w = ctx.fonts_mut(|f| f.glyph_width(&font, '0'));
        let job = highlight(src, &font, &Theme::dark(), char_w, tab_width);
        let galley = ctx.fonts_mut(|f| f.layout_job(job));
        ctx.end_pass().textures_delta.clear();
        galley
            .rows
            .iter()
            .filter_map(|r| r.glyphs.iter().find(|g| g.chr == ch).map(|g| (r.pos.x + g.pos.x) / char_w))
            .collect()
    }


    /// Expand tabs to spaces the way a classic editor does.
    fn expand(src: &str, tab: usize) -> String {
        src.split('\n')
            .map(|line| {
                let mut out = String::new();
                for c in line.chars() {
                    if c == '\t' {
                        let n = tab - out.chars().count() % tab;
                        out.extend(std::iter::repeat_n(' ', n));
                    } else {
                        out.push(c);
                    }
                }
                out
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn tabs_render_like_tab_stops() {
        let src = "ab\tMOV\tR0\n\tMOV\tR0\nlabel12\tMOV\tR0\nlabel123\tMOV\tR0\n    \tMOV\tR0, #1\t; c";
        for tab in [4, 8] {
            let spaced = expand(src, tab);
            for ch in ['M', 'R', ';'] {
                let with_tabs = columns_of(src, ch, tab);
                let with_spaces = columns_of(&spaced, ch, tab);
                assert_eq!(with_tabs.len(), with_spaces.len());
                for (a, b) in with_tabs.iter().zip(&with_spaces) {
                    assert!((a - b).abs() < 0.01, "tab={tab} '{ch}': {with_tabs:?} vs {with_spaces:?}");
                }
            }
        }
    }
}
