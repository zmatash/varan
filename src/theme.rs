use eframe::egui::{Color32, Visuals};

#[derive(Clone)]
pub struct Theme {
    pub dark: bool,
    pub text: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub changed: Color32,
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
            text: Color32::from_rgb(0xD4, 0xD7, 0xDD),
            dim: Color32::from_rgb(0x7A, 0x80, 0x8C),
            accent: Color32::from_rgb(0x4F, 0xA3, 0xF7),
            changed: Color32::from_rgb(0xFF, 0xB8, 0x4D),
            gutter_bg: Color32::from_rgb(0x1E, 0x21, 0x27),
            gutter_text: Color32::from_rgb(0x5C, 0x63, 0x70),
            gutter_border: Color32::from_rgb(0x2C, 0x31, 0x3A),
            breakpoint: Color32::from_rgb(0xE5, 0x48, 0x4D),
            breakpoint_inactive: Color32::from_rgb(0x7A, 0x3B, 0x3E),
            pc_arrow: Color32::from_rgb(0xF5, 0xC5, 0x42),
            pc_line_bg: Color32::from_rgba_unmultiplied(0xF5, 0xC5, 0x42, 0x2A),
            error_line_bg: Color32::from_rgba_unmultiplied(0xE5, 0x48, 0x4D, 0x30),
            error_text: Color32::from_rgb(0xF2, 0x6D, 0x72),
            ok_text: Color32::from_rgb(0x7B, 0xD8, 0x8F),
            warn_text: Color32::from_rgb(0xF5, 0xC5, 0x42),
            console_bg: Color32::from_rgb(0x14, 0x16, 0x1A),
            syn_comment: Color32::from_rgb(0x6A, 0x99, 0x55),
            syn_label: Color32::from_rgb(0xE5, 0xC0, 0x7B),
            syn_mnemonic: Color32::from_rgb(0x61, 0xAF, 0xEF),
            syn_directive: Color32::from_rgb(0xC6, 0x78, 0xDD),
            syn_register: Color32::from_rgb(0x56, 0xB6, 0xC2),
            syn_number: Color32::from_rgb(0xD1, 0x9A, 0x66),
            syn_string: Color32::from_rgb(0x98, 0xC3, 0x79),
            syn_symbol: Color32::from_rgb(0xD4, 0xD7, 0xDD),
        }
    }

    pub fn light() -> Self {
        Theme {
            dark: false,
            text: Color32::from_rgb(0x24, 0x29, 0x2E),
            dim: Color32::from_rgb(0x6A, 0x73, 0x7D),
            accent: Color32::from_rgb(0x05, 0x69, 0xDA),
            changed: Color32::from_rgb(0xC2, 0x4E, 0x00),
            gutter_bg: Color32::from_rgb(0xF3, 0xF4, 0xF6),
            gutter_text: Color32::from_rgb(0x9A, 0xA1, 0xAB),
            gutter_border: Color32::from_rgb(0xDD, 0xE0, 0xE4),
            breakpoint: Color32::from_rgb(0xD7, 0x3A, 0x49),
            breakpoint_inactive: Color32::from_rgb(0xEB, 0xA8, 0xAE),
            pc_arrow: Color32::from_rgb(0xD9, 0x8E, 0x04),
            pc_line_bg: Color32::from_rgba_unmultiplied(0xFF, 0xD3, 0x3D, 0x55),
            error_line_bg: Color32::from_rgba_unmultiplied(0xD7, 0x3A, 0x49, 0x30),
            error_text: Color32::from_rgb(0xCB, 0x24, 0x31),
            ok_text: Color32::from_rgb(0x1A, 0x7F, 0x37),
            warn_text: Color32::from_rgb(0x9A, 0x67, 0x00),
            console_bg: Color32::from_rgb(0xFA, 0xFB, 0xFC),
            syn_comment: Color32::from_rgb(0x6A, 0x73, 0x7D),
            syn_label: Color32::from_rgb(0x95, 0x38, 0x00),
            syn_mnemonic: Color32::from_rgb(0x00, 0x5C, 0xC5),
            syn_directive: Color32::from_rgb(0x82, 0x50, 0xDF),
            syn_register: Color32::from_rgb(0x0A, 0x7E, 0x8C),
            syn_number: Color32::from_rgb(0xB3, 0x5E, 0x00),
            syn_string: Color32::from_rgb(0x1A, 0x7F, 0x37),
            syn_symbol: Color32::from_rgb(0x24, 0x29, 0x2E),
        }
    }

    pub fn visuals(&self) -> Visuals {
        let mut v = if self.dark { Visuals::dark() } else { Visuals::light() };
        if self.dark {
            v.panel_fill = Color32::from_rgb(0x21, 0x25, 0x2B);
            v.extreme_bg_color = Color32::from_rgb(0x1A, 0x1D, 0x22);
            v.faint_bg_color = Color32::from_rgb(0x26, 0x2A, 0x31);
        }
        v.selection.bg_fill = self.accent.linear_multiply(0.45);
        v.hyperlink_color = self.accent;
        v
    }
}
