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
    frame: 0x394a66,
    rule: 0xc4c8c5,
    ink: 0x243553,
    entry: 0x185c52,
    pencil: 0x43536b,
    hover: 0xebecdf,
    focus: 0xdce8f8,
    focus_rule: 0x295388,
    peer: 0xecedf0,
    match_digit: 0xd6e9de,
    hint_target: 0xf6dca2,
    hint_source: 0xf6e5bb,
    hint_evidence: 0xf9efd4,
    hint_ink: 0x75490d,
    error: 0xf9ddd6,
    error_ink: 0x8f352e,
    cage: 0x627087,
    cage_focus: 0xe0eee7,
    cage_error: 0xfae6dc,
    grain_alpha: 0.14,
};

const DARK: Paper = Paper {
    ground: 0x232d42,
    givens: 0x2d384c,
    frame: 0x8193ad,
    rule: 0x48546c,
    ink: 0xf6ead5,
    entry: 0xa5e3cc,
    pencil: 0xcfdaed,
    hover: 0x36465d,
    focus: 0x314d73,
    focus_rule: 0xa3cdff,
    peer: 0x303d56,
    match_digit: 0x30544e,
    hint_target: 0x5c431f,
    hint_source: 0x4d402d,
    hint_evidence: 0x3b352e,
    hint_ink: 0xffd68f,
    error: 0x563039,
    error_ink: 0xffb2a6,
    cage: 0x9daac2,
    cage_focus: 0x31483f,
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
