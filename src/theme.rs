use eframe::egui::{self, Color32, CornerRadius, Shadow, Stroke, Visuals};

#[derive(Clone)]
pub struct Theme {
    pub dark: bool,
    pub text: Color32,
    pub dim: Color32,
    pub accent: Color32,
    /// Text drawn on top of an `accent` fill.
    pub on_accent: Color32,
    pub changed: Color32,
    /// Background tint for a value that changed in the last step.
    pub changed_bg: Color32,
    /// Window background behind panels (toolbar, side bars).
    pub chrome_bg: Color32,
    /// Background of the editor and the other content areas.
    pub surface: Color32,
    /// Subtle 1px borders between regions.
    pub border: Color32,
    /// Rows striped in tables.
    pub stripe: Color32,
    pub gutter_bg: Color32,
    pub gutter_text: Color32,
    pub gutter_border: Color32,
    pub breakpoint: Color32,
    pub breakpoint_inactive: Color32,
    pub pc_arrow: Color32,
    pub pc_line_bg: Color32,
    pub error_line_bg: Color32,
    pub error_text: Color32,
    pub ok_text: Color32,
    pub run: Color32,
    pub warn_text: Color32,
    pub console_bg: Color32,
    pub syn_comment: Color32,
    pub syn_label: Color32,
    pub syn_mnemonic: Color32,
    pub syn_directive: Color32,
    pub syn_register: Color32,
    pub syn_number: Color32,
    pub syn_string: Color32,
    pub syn_symbol: Color32,
}

impl Theme {
    pub fn dark() -> Self {
        Theme {
            dark: true,
            text: Color32::from_rgb(0xD8, 0xDB, 0xE2),
            dim: Color32::from_rgb(0x84, 0x8B, 0x98),
            accent: Color32::from_rgb(0x4F, 0x9D, 0xF7),
            on_accent: Color32::WHITE,
            changed: Color32::from_rgb(0xFF, 0xB8, 0x4D),
            changed_bg: Color32::from_rgba_unmultiplied(0xFF, 0xB8, 0x4D, 0x1C),
            chrome_bg: Color32::from_rgb(0x1B, 0x1E, 0x24),
            surface: Color32::from_rgb(0x22, 0x26, 0x2D),
            border: Color32::from_rgb(0x30, 0x35, 0x3E),
            stripe: Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x06),
            gutter_bg: Color32::from_rgb(0x1F, 0x23, 0x29),
            gutter_text: Color32::from_rgb(0x5C, 0x63, 0x70),
            gutter_border: Color32::from_rgb(0x2C, 0x31, 0x3A),
            breakpoint: Color32::from_rgb(0xE5, 0x48, 0x4D),
            breakpoint_inactive: Color32::from_rgb(0x7A, 0x3B, 0x3E),
            pc_arrow: Color32::from_rgb(0xF5, 0xC5, 0x42),
            pc_line_bg: Color32::from_rgba_unmultiplied(0xF5, 0xC5, 0x42, 0x2A),
            error_line_bg: Color32::from_rgba_unmultiplied(0xE5, 0x48, 0x4D, 0x30),
            error_text: Color32::from_rgb(0xF2, 0x6D, 0x72),
            ok_text: Color32::from_rgb(0x7B, 0xD8, 0x8F),
            run: Color32::from_rgb(0x2E, 0x9E, 0x5B),
            warn_text: Color32::from_rgb(0xF5, 0xC5, 0x42),
            console_bg: Color32::from_rgb(0x16, 0x18, 0x1D),
            syn_comment: Color32::from_rgb(0x6A, 0x99, 0x55),
            syn_label: Color32::from_rgb(0xE5, 0xC0, 0x7B),
            syn_mnemonic: Color32::from_rgb(0x61, 0xAF, 0xEF),
            syn_directive: Color32::from_rgb(0xC6, 0x78, 0xDD),
            syn_register: Color32::from_rgb(0x56, 0xB6, 0xC2),
            syn_number: Color32::from_rgb(0xD1, 0x9A, 0x66),
            syn_string: Color32::from_rgb(0x98, 0xC3, 0x79),
            syn_symbol: Color32::from_rgb(0xD8, 0xDB, 0xE2),
        }
    }

    pub fn light() -> Self {
        Theme {
            dark: false,
            text: Color32::from_rgb(0x1F, 0x23, 0x28),
            dim: Color32::from_rgb(0x65, 0x6D, 0x76),
            accent: Color32::from_rgb(0x09, 0x69, 0xDA),
            on_accent: Color32::WHITE,
            changed: Color32::from_rgb(0xBC, 0x4C, 0x00),
            changed_bg: Color32::from_rgba_unmultiplied(0xFF, 0x9A, 0x3C, 0x26),
            chrome_bg: Color32::from_rgb(0xEE, 0xF0, 0xF3),
            surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            border: Color32::from_rgb(0xD6, 0xDA, 0xDF),
            stripe: Color32::from_rgba_unmultiplied(0x00, 0x20, 0x50, 0x07),
            gutter_bg: Color32::from_rgb(0xF6, 0xF7, 0xF9),
            gutter_text: Color32::from_rgb(0x9A, 0xA1, 0xAB),
            gutter_border: Color32::from_rgb(0xE3, 0xE6, 0xEA),
            breakpoint: Color32::from_rgb(0xD7, 0x3A, 0x49),
            breakpoint_inactive: Color32::from_rgb(0xEB, 0xA8, 0xAE),
            pc_arrow: Color32::from_rgb(0xD9, 0x8E, 0x04),
            pc_line_bg: Color32::from_rgba_unmultiplied(0xFF, 0xD3, 0x3D, 0x55),
            error_line_bg: Color32::from_rgba_unmultiplied(0xD7, 0x3A, 0x49, 0x30),
            error_text: Color32::from_rgb(0xCB, 0x24, 0x31),
            ok_text: Color32::from_rgb(0x1A, 0x7F, 0x37),
            run: Color32::from_rgb(0x1F, 0x88, 0x3D),
            warn_text: Color32::from_rgb(0x9A, 0x67, 0x00),
            console_bg: Color32::from_rgb(0xFA, 0xFB, 0xFC),
            syn_comment: Color32::from_rgb(0x6A, 0x73, 0x7D),
            syn_label: Color32::from_rgb(0x95, 0x38, 0x00),
            syn_mnemonic: Color32::from_rgb(0x00, 0x5C, 0xC5),
            syn_directive: Color32::from_rgb(0x82, 0x50, 0xDF),
            syn_register: Color32::from_rgb(0x0A, 0x7E, 0x8C),
            syn_number: Color32::from_rgb(0xB3, 0x5E, 0x00),
            syn_string: Color32::from_rgb(0x1A, 0x7F, 0x37),
            syn_symbol: Color32::from_rgb(0x1F, 0x23, 0x28),
        }
    }

    pub fn visuals(&self) -> Visuals {
        let mut v = if self.dark { Visuals::dark() } else { Visuals::light() };
        v.override_text_color = None;
        v.panel_fill = self.chrome_bg;
        v.window_fill = self.surface;
        v.extreme_bg_color = if self.dark {
            Color32::from_rgb(0x17, 0x1A, 0x1F)
        } else {
            Color32::WHITE
        };
        v.text_edit_bg_color = Some(v.extreme_bg_color);
        v.faint_bg_color = self.stripe;
        v.code_bg_color = self.surface;
        v.window_stroke = Stroke::new(1.0, self.border);
        v.window_corner_radius = CornerRadius::same(8);
        v.menu_corner_radius = CornerRadius::same(6);
        v.window_shadow = Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(if self.dark { 110 } else { 40 }),
        };
        v.popup_shadow = Shadow {
            offset: [0, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(if self.dark { 90 } else { 30 }),
        };

        let r = CornerRadius::same(5);
        let (idle, hover, active) = if self.dark {
            (
                Color32::from_rgb(0x2A, 0x2F, 0x37),
                Color32::from_rgb(0x34, 0x3A, 0x44),
                Color32::from_rgb(0x3E, 0x45, 0x51),
            )
        } else {
            (
                Color32::from_rgb(0xE4, 0xE7, 0xEB),
                Color32::from_rgb(0xD9, 0xDD, 0xE2),
                Color32::from_rgb(0xCC, 0xD1, 0xD8),
            )
        };
        let w = &mut v.widgets;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.text);
        w.noninteractive.corner_radius = r;
        w.inactive.weak_bg_fill = idle;
        w.inactive.bg_fill = idle;
        w.inactive.bg_stroke = Stroke::NONE;
        w.inactive.fg_stroke = Stroke::new(1.0, self.text);
        w.inactive.corner_radius = r;
        w.hovered.weak_bg_fill = hover;
        w.hovered.bg_fill = hover;
        w.hovered.bg_stroke = Stroke::new(1.0, self.accent.linear_multiply(0.5));
        w.hovered.fg_stroke = Stroke::new(1.5, self.text);
        w.hovered.corner_radius = r;
        w.hovered.expansion = 0.0;
        w.active.weak_bg_fill = active;
        w.active.bg_fill = active;
        w.active.bg_stroke = Stroke::new(1.0, self.accent);
        w.active.fg_stroke = Stroke::new(2.0, self.text);
        w.active.corner_radius = r;
        w.active.expansion = 0.0;
        w.open.weak_bg_fill = hover;
        w.open.bg_fill = hover;
        w.open.corner_radius = r;

        v.selection.bg_fill = self.accent.linear_multiply(if self.dark { 0.45 } else { 0.3 });
        v.selection.stroke = Stroke::new(1.0, self.accent);
        v.hyperlink_color = self.accent;
        v.striped = false;
        v
    }

    /// Apply this theme's visuals and spacing to the context.
    pub fn apply(&self, ctx: &egui::Context) {
        ctx.set_visuals(self.visuals());
    }
}
