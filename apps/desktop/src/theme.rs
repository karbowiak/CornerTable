use gpui::{
    App, Div, FontWeight, Global, ParentElement as _, SharedString, Styled as _, Window,
    WindowAppearance, div, px, rgb,
};
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub fn native(self) -> Option<WindowAppearance> {
        match self {
            Self::System => None,
            Self::Light => Some(WindowAppearance::Light),
            Self::Dark => Some(WindowAppearance::Dark),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub paper: u32,
    pub surface: u32,
    pub border: u32,
    pub ink: u32,
    pub muted: u32,
    pub hover: u32,
    pub gold_text: u32,
    pub rail: u32,
    pub rail_active: u32,
}

const LIGHT: Palette = Palette {
    paper: PAPER,
    surface: SURFACE,
    border: BORDER,
    ink: INK,
    muted: MUTED,
    hover: SAGE,
    gold_text: 0x855713,
    rail: RAIL,
    rail_active: RAIL_ACTIVE,
};

const DARK: Palette = Palette {
    paper: 0x192521,
    surface: 0x23312c,
    border: 0x42534b,
    ink: 0xf3ecdf,
    muted: 0xb9c9bd,
    hover: 0x344a3e,
    gold_text: GOLD,
    rail: 0x111d19,
    rail_active: 0x294539,
};

impl Global for Palette {}

pub fn colors(cx: &App) -> Palette {
    cx.try_global::<Palette>().copied().unwrap_or(LIGHT)
}

pub fn apply(appearance: Appearance, window: &mut Window, cx: &mut App) {
    let palette = match appearance
        .native()
        .unwrap_or_else(|| cx.window_appearance())
    {
        WindowAppearance::Light | WindowAppearance::VibrantLight => LIGHT,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => DARK,
    };
    if cx.try_global::<Palette>() != Some(&palette) {
        cx.set_global(palette);
        window.refresh();
    }
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
        .text_color(rgb(INK))
        .rounded_xl()
}

pub fn panel(cx: &App) -> Div {
    let palette = colors(cx);
    div()
        .bg(rgb(palette.surface))
        .border_1()
        .border_color(rgb(palette.border))
        .text_color(rgb(palette.ink))
        .rounded_xl()
}
