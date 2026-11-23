use gpui::{Div, FontWeight, ParentElement as _, SharedString, Styled as _, div, px, rgb};

pub const INK: u32 = 0x18332d;
pub const MUTED: u32 = 0x4e665a;
pub const RAIL: u32 = 0x132e2a;
pub const RAIL_ACTIVE: u32 = 0x244b3c;
pub const PAPER: u32 = 0xf6f2e9;
pub const SURFACE: u32 = 0xfffcf5;
pub const BORDER: u32 = 0xddd3c3;
pub const GOLD: u32 = 0xefb94d;
pub const GOLD_HOVER: u32 = 0xf7cc72;
pub const SAGE: u32 = 0xe7ecdb;

pub const BODY: &str = "Open Sans";
pub const DISPLAY: &str = "DejaVu Serif";

pub fn heading(text: impl Into<SharedString>) -> Div {
    display_type(text, 49.)
}

pub fn display_type(text: impl Into<SharedString>, size: f32) -> Div {
    div()
        .font_family(DISPLAY)
        .font_weight(FontWeight::BOLD)
        .text_size(px(size))
        .line_height(px(size * 1.2))
        .child(text.into())
}

pub fn surface() -> Div {
    div()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .rounded_xl()
}
