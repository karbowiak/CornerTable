use std::sync::{Arc, OnceLock};

use gpui::{App, Image, ImageFormat, RenderImage};

use crate::theme;

#[derive(Clone, Copy)]
pub(super) struct Paper {
    pub ground: u32,
    pub givens: u32,
    pub frame: u32,
    pub rule: u32,
    pub ink: u32,
    pub entry: u32,
    pub pencil: u32,
    pub hover: u32,
    pub focus: u32,
    pub focus_rule: u32,
    pub peer: u32,
    pub match_digit: u32,
    pub hint_target: u32,
    pub hint_source: u32,
    pub hint_evidence: u32,
    pub hint_ink: u32,
    pub error: u32,
    pub error_ink: u32,
    pub cage: u32,
    pub cage_focus: u32,
    pub cage_error: u32,
    pub grain_alpha: f32,
}

const LIGHT: Paper = Paper {
    ground: 0xfffaf0,
    givens: 0xf3ecdc,
    frame: 0x5b4733,
    rule: 0xd6cab6,
    ink: 0x2a2119,
    entry: 0x2b5f8a,
    pencil: 0x5c4d3d,
    hover: 0xf2e8d4,
    focus: 0xf7e2ae,
    focus_rule: 0xb07d1f,
    peer: 0xf2ebdc,
    match_digit: 0xf1dfb0,
    hint_target: 0xf6dca2,
    hint_source: 0xf6e5bb,
    hint_evidence: 0xf9efd4,
    hint_ink: 0x75490d,
    error: 0xf9ddd6,
    error_ink: 0x8f352e,
    cage: 0x7b6650,
    cage_focus: 0xf3e9d3,
    cage_error: 0xfae6dc,
    grain_alpha: 0.14,
};

const DARK: Paper = Paper {
    ground: 0x2a231c,
    givens: 0x332a21,
    frame: 0xa88f6b,
    rule: 0x4d4134,
    ink: 0xf6ead5,
    entry: 0x9cc9e8,
    pencil: 0xd8ccb9,
    hover: 0x3d3328,
    focus: 0x5b4626,
    focus_rule: 0xefb94d,
    peer: 0x362d24,
    match_digit: 0x4a3c22,
    hint_target: 0x5c431f,
    hint_source: 0x4d402d,
    hint_evidence: 0x3b352e,
    hint_ink: 0xffd68f,
    error: 0x563039,
    error_ink: 0xffb2a6,
    cage: 0xbfa98a,
    cage_focus: 0x41372b,
    cage_error: 0x49333d,
    grain_alpha: 0.15,
};

impl Paper {
    pub(super) fn current(cx: &App) -> Self {
        if theme::colors(cx) == theme::LIGHT {
            LIGHT
        } else {
            DARK
        }
    }
}

pub(super) fn grain(cx: &App) -> Arc<RenderImage> {
    static GRAIN: OnceLock<Arc<RenderImage>> = OnceLock::new();
    GRAIN
        .get_or_init(|| {
            let data = include_bytes!("../../../assets/games/sudoku/cotton-stock.jpg");
            Image::from_bytes(ImageFormat::Jpeg, data.to_vec())
                .to_image_data(cx.svg_renderer())
                .expect("bundled Sudoku paper texture")
        })
        .clone()
}
