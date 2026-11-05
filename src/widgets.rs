//! Small custom widgets: vector icons (no emoji-font dependency), toolbar
//! buttons, underline tab bars, section headers and status pills.

use std::f32::consts::PI;

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Pos2, Rect, Response, RichText, Sense, Shape,
    Stroke, Ui, Vec2, pos2, vec2,
};

use crate::theme::Theme;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Icon {
    Assemble,
    Run,
    Pause,
    Step,
    StepOver,
    Reset,
    Book,
}

fn arc(c: Pos2, r: f32, from: f32, to: f32) -> Vec<Pos2> {
    let n = 20;
    (0..=n)
        .map(|i| {
            let a = from + (to - from) * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

fn arrow_head(tip: Pos2, dir: Vec2, size: f32, color: Color32) -> Shape {
    let d = dir.normalized();
    let n = vec2(-d.y, d.x);
    Shape::convex_polygon(
        vec![tip, tip - d * size + n * size * 0.75, tip - d * size - n * size * 0.75],
        color,
        Stroke::NONE,
    )
}

/// Paint `icon` centred in `rect`.
pub fn paint_icon(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.5;
    let stroke = Stroke::new((s * 0.2).max(1.4), color);
    match icon {
        Icon::Run => {
            painter.add(Shape::convex_polygon(
                vec![c + vec2(-s * 0.55, -s * 0.8), c + vec2(s * 0.85, 0.0), c + vec2(-s * 0.55, s * 0.8)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            for dx in [-0.42, 0.42] {
                let bar = Rect::from_center_size(c + vec2(s * dx, 0.0), vec2(s * 0.42, s * 1.6));
                painter.rect_filled(bar, CornerRadius::same(1), color);
            }
        }
        Icon::Step => {
            // Arrow pointing down onto a line: "execute one instruction".
            let top = c + vec2(0.0, -s * 0.9);
            let tip = c + vec2(0.0, s * 0.35);
            painter.line_segment([top, tip - vec2(0.0, s * 0.3)], stroke);
            painter.add(arrow_head(tip, vec2(0.0, 1.0), s * 0.55, color));
            painter.line_segment([c + vec2(-s * 0.7, s * 0.85), c + vec2(s * 0.7, s * 0.85)], stroke);
        }
        Icon::StepOver => {
            // Arc jumping over a dot.
            let ac = c + vec2(0.0, s * 0.25);
            let pts = arc(ac, s * 0.75, PI, 1.75 * PI);
            let end = *pts.last().unwrap();
            painter.add(Shape::line(pts, stroke));
            let tip = ac + vec2(s * 0.75, 0.05 * s);
            painter.add(arrow_head(tip, tip - end + vec2(0.0, s * 0.2), s * 0.5, color));
            painter.circle_filled(c + vec2(0.0, s * 0.7), s * 0.2, color);
        }
        Icon::Reset => {
            let r = s * 0.72;
            let pts = arc(c, r, -0.35 * PI, 1.25 * PI);
            painter.add(Shape::line(pts, stroke));
            let a = -0.35 * PI;
            let tip = c + vec2(a.cos(), a.sin()) * r;
            let dir = vec2(-a.sin(), a.cos()) * -1.0;
            painter.add(arrow_head(tip + dir * s * 0.25, dir, s * 0.55, color));
        }
        Icon::Assemble => {
            // Lightning bolt: "build".
            let p = |x: f32, y: f32| c + vec2(x * s, y * s);
            let fill = |pts: Vec<Pos2>| Shape::convex_polygon(pts, color, Stroke::NONE);
            painter.add(fill(vec![p(0.35, -1.0), p(0.1, 0.15), p(-0.6, 0.15)]));
            painter.add(fill(vec![p(-0.6, 0.15), p(-0.05, -0.15), p(0.6, -0.15), p(0.1, 0.15)]));
            painter.add(fill(vec![p(-0.05, -0.15), p(0.6, -0.15), p(-0.35, 1.0)]));
        }
        Icon::Book => {
            let p = |x: f32, y: f32| c + vec2(x * s, y * s);
            let thin = Stroke::new(stroke.width * 0.8, color);
            painter.add(Shape::closed_line(vec![p(0.0, -0.55), p(-0.9, -0.8), p(-0.9, 0.6), p(0.0, 0.85)], thin));
            painter.add(Shape::closed_line(vec![p(0.0, -0.55), p(0.9, -0.8), p(0.9, 0.6), p(0.0, 0.85)], thin));
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ButtonKind {
    Normal,
    /// Filled with a colour (e.g. green "Run").
    Filled(Color32),
}

/// A toolbar button with a vector icon and a text label.
pub fn tool_button(
    ui: &mut Ui,
    theme: &Theme,
    icon: Icon,
    label: &str,
    kind: ButtonKind,
    enabled: bool,
) -> Response {
    let font = FontId::proportional(14.0);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER);
    let icon_sz = 14.0;
    let pad = vec2(10.0, 6.0);
    let size = vec2(pad.x * 2.0 + icon_sz + 7.0 + galley.size().x, 30.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    let resp = resp.on_hover_cursor(if enabled { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Default });
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let (mut fill, mut fg) = match kind {
            ButtonKind::Normal => (v.widgets.inactive.weak_bg_fill, theme.text),
            ButtonKind::Filled(c) => (c, Color32::WHITE),
        };
        if enabled {
            if resp.is_pointer_button_down_on() {
                fill = match kind {
                    ButtonKind::Normal => v.widgets.active.weak_bg_fill,
                    ButtonKind::Filled(c) => c.gamma_multiply(0.8),
                };
            } else if resp.hovered() {
                fill = match kind {
                    ButtonKind::Normal => v.widgets.hovered.weak_bg_fill,
                    ButtonKind::Filled(c) => lighten(c, 0.12),
                };
            }
        } else {
            fill = fill.gamma_multiply(0.5);
            fg = theme.dim.gamma_multiply(0.7);
        }
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(6), fill);
        let icon_rect = Rect::from_center_size(
            pos2(rect.left() + pad.x + icon_sz / 2.0, rect.center().y),
            Vec2::splat(icon_sz),
        );
        paint_icon(painter, icon_rect, icon, fg);
        painter.galley_with_override_text_color(
            pos2(icon_rect.right() + 7.0, rect.center().y - galley.size().y / 2.0),
            galley,
            fg,
        );
    }
    resp
}

fn lighten(c: Color32, t: f32) -> Color32 {
    let l = |x: u8| (x as f32 + (255.0 - x as f32) * t) as u8;
    Color32::from_rgb(l(c.r()), l(c.g()), l(c.b()))
}

/// A horizontal tab strip with an underline under the active tab.
/// `badge` adds a small count bubble (e.g. number of errors).
pub fn tab_bar<T: PartialEq + Copy>(
    ui: &mut Ui,
    theme: &Theme,
    current: &mut T,
    tabs: &[(T, String, Option<(usize, Color32)>)],
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (value, label, badge) in tabs {
            let selected = *current == *value;
            let font = FontId::proportional(13.5);
            let color = if selected { theme.text } else { theme.dim };
            let galley = ui.painter().layout_no_wrap(label.clone(), font.clone(), color);
            let badge_galley = badge.map(|(n, _)| {
                ui.painter().layout_no_wrap(n.to_string(), FontId::proportional(11.0), Color32::WHITE)
            });
            let badge_w = badge_galley.as_ref().map_or(0.0, |g| g.size().x.max(8.0) + 14.0);
            let size = vec2(galley.size().x + 22.0 + badge_w, 30.0);
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, selected, label));
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                *current = *value;
            }
            let painter = ui.painter();
            if resp.hovered() && !selected {
                painter.rect_filled(rect.shrink2(vec2(0.0, 3.0)), CornerRadius::same(5), theme.stripe.gamma_multiply(2.5));
            }
            let text_pos = pos2(rect.left() + 11.0, rect.center().y - galley.size().y / 2.0);
            let text_w = galley.size().x;
            painter.galley(text_pos, galley, color);
            if let (Some(g), Some((_, bc))) = (badge_galley, badge) {
                let bw = g.size().x.max(8.0) + 8.0;
                let br = Rect::from_center_size(
                    pos2(text_pos.x + text_w + 6.0 + bw / 2.0, rect.center().y),
                    vec2(bw, 16.0),
                );
                painter.rect_filled(br, CornerRadius::same(8), *bc);
                painter.galley(br.center() - g.size() / 2.0, g, Color32::WHITE);
            }
            if selected {
                let line = Rect::from_min_max(
                    pos2(rect.left() + 6.0, rect.bottom() - 2.5),
                    pos2(rect.right() - 6.0, rect.bottom()),
                );
                painter.rect_filled(line, CornerRadius::same(1), theme.accent);
            }
        }
    });
}

/// Small uppercase caption used to title a group in a side panel.
pub fn section_header(ui: &mut Ui, theme: &Theme, title: &str) {
    ui.add_space(6.0);
    ui.label(
        RichText::new(title.to_uppercase())
            .size(11.5)
            .strong()
            .extra_letter_spacing(0.8)
            .color(theme.dim),
    );
    ui.add_space(2.0);
}

/// A rounded "pill" with a coloured dot and text, used for the run state.
pub fn status_pill(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let font = FontId::proportional(12.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let size = vec2(galley.size().x + 32.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(12),
        color.gamma_multiply(0.14),
        Stroke::new(1.0, color.gamma_multiply(0.5)),
        egui::StrokeKind::Inside,
    );
    painter.circle_filled(pos2(rect.left() + 13.0, rect.center().y), 4.0, color);
    painter.galley(pos2(rect.left() + 23.0, rect.center().y - galley.size().y / 2.0), galley, color);
    resp
}

/// Draw `text` with a keyboard-key look (for shortcut lists).
pub fn key_cap(ui: &mut Ui, theme: &Theme, text: &str) {
    let font = FontId::monospace(12.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, theme.text);
    let size = galley.size() + vec2(12.0, 6.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(4),
        theme.stripe.gamma_multiply(3.0),
        Stroke::new(1.0, theme.border),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.center() - galley.size() / 2.0, galley, theme.text);
}
