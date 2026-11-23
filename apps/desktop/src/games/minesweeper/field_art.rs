use corner_table_minesweeper::Tile;
use gpui::{
    AnyElement, Background, Bounds, Div, ObjectFit, PathBuilder, Pixels, Point, StyledImage,
    Window, canvas, div, fill, img, linear_color_stop, linear_gradient, point, prelude::*, px, rgb,
    rgba, size,
};

use crate::theme;

#[derive(Clone, Copy)]
pub(super) struct Face {
    top: u32,
    base: u32,
    pub(super) edge: u32,
}

impl Face {
    const fn new(top: u32, base: u32, edge: u32) -> Self {
        Self { top, base, edge }
    }

    pub(super) fn finish(self) -> Background {
        finish(self.top, self.base)
    }
}

fn finish(top: u32, base: u32) -> Background {
    linear_gradient(
        180.,
        linear_color_stop(rgb(top), 0.),
        linear_color_stop(rgb(base), 1.),
    )
}

pub(super) struct FieldArt {
    pub(super) frame: Face,
    pub(super) well: Face,
    pub(super) label: u32,
    pub(super) value: u32,
    pub(super) warning: u32,
    pub(super) rule: u32,
    cover: [Face; 2],
    flagged: Face,
    clear: Face,
    clue: Face,
    chord: Face,
    mine: Face,
    misflag: Face,
    pub(super) cover_hover: u32,
    pub(super) cover_pressed: u32,
    pub(super) chord_hover: u32,
    pub(super) chord_pressed: u32,
    focus: u32,
    chord_corners: u32,
    mine_rim: u32,
    misflag_ink: u32,
    clues: [u32; 8],
}

const LIGHT: FieldArt = FieldArt {
    frame: Face::new(0xe7d8be, 0xcaaf87, 0x9c805a),
    well: Face::new(0x263c34, 0x3a4c40, 0x917556),
    label: 0x51402e,
    value: 0x282d24,
    warning: 0x862e21,
    rule: 0xad9675,
    cover: [
        Face::new(0x557f73, 0x2f5a50, 0x284b42),
        Face::new(0x4d786d, 0x2b554b, 0x284b42),
    ],
    flagged: Face::new(0x3c6659, 0x22473d, 0x9d7148),
    clear: Face::new(0xc9d6c8, 0xd4dfd3, 0x9cab9d),
    clue: Face::new(0xe2e7dc, 0xf0eee2, 0xb3bdae),
    chord: Face::new(0xf0dbad, 0xf9edce, 0xba8951),
    mine: Face::new(0xbca78d, 0xd5bca1, 0x9b7b5e),
    misflag: Face::new(0xe3b7a2, 0xf1cdbb, 0xa56e59),
    cover_hover: 0x648c79,
    cover_pressed: 0x3c665a,
    chord_hover: 0xffebbd,
    chord_pressed: 0xf0d6a2,
    focus: 0x1b504b,
    chord_corners: 0x92612c,
    mine_rim: 0x30251e,
    misflag_ink: 0x8a2d23,
    clues: [
        0x1e4d6b, 0x255533, 0x922e24, 0x59447d, 0x753949, 0x1f6264, 0x333e44, 0x594934,
    ],
};

const DARK: FieldArt = FieldArt {
    frame: Face::new(0x3a3a2c, 0x27291e, 0x776345),
    well: Face::new(0x0b1512, 0x111f19, 0x594b35),
    label: 0xc7b89c,
    value: 0xf3e5cd,
    warning: 0xffb28c,
    rule: 0x64573e,
    cover: [
        Face::new(0x466f64, 0x2b5147, 0x1d3d34),
        Face::new(0x40695f, 0x254a41, 0x1b3a32),
    ],
    flagged: Face::new(0x395b49, 0x263f33, 0x93734e),
    clear: Face::new(0x182622, 0x1d2c26, 0x2d4339),
    clue: Face::new(0x1e302a, 0x273a32, 0x3b5146),
    chord: Face::new(0x493a25, 0x3d3324, 0x99734a),
    mine: Face::new(0x573c2e, 0x412e25, 0x775640),
    misflag: Face::new(0x65423a, 0x4a302d, 0x946655),
    cover_hover: 0x577f6f,
    cover_pressed: 0x345c50,
    chord_hover: 0x56472d,
    chord_pressed: 0x483418,
    focus: 0xf9e6b8,
    chord_corners: 0xe3b77a,
    mine_rim: 0xe0ad7d,
    misflag_ink: 0xffb7a0,
    clues: [
        0xa1d7ff, 0xb6e29e, 0xffab96, 0xd3bcf2, 0xf0b3ce, 0x91dcd6, 0xe0eaf1, 0xe9cf9e,
    ],
};

const COPPER: u32 = 0xffc47e;
const IVORY: u32 = 0xffebbd;
const DETONATED: Face = Face::new(0xba482e, 0x78251e, 0xe99062);
pub(super) const OPENING_EDGE: u32 = COPPER;

impl FieldArt {
    pub(super) fn for_palette(palette: theme::Palette) -> &'static Self {
        if palette == theme::LIGHT {
            &LIGHT
        } else {
            &DARK
        }
    }

    pub(super) fn face(&self, tile: Tile, chordable: bool, variant: bool) -> Face {
        match tile {
            Tile::Covered => self.cover[usize::from(variant)],
            Tile::Flagged => self.flagged,
            Tile::Open(0) => self.clear,
            Tile::Open(_) if chordable => self.chord,
            Tile::Open(_) => self.clue,
            Tile::Mine => self.mine,
            Tile::Detonated => DETONATED,
            Tile::Misflagged => self.misflag,
        }
    }

    pub(super) fn clue_ink(&self, count: u8) -> u32 {
        self.clues[usize::from(count - 1)]
    }

    pub(super) fn focus_ink(&self, tile: Tile) -> u32 {
        match tile {
            Tile::Covered | Tile::Flagged | Tile::Detonated => IVORY,
            _ => self.focus,
        }
    }

    pub(super) fn detail(
        &'static self,
        tile: Tile,
        opening: bool,
        chordable: bool,
        impact: Option<f32>,
    ) -> AnyElement {
        // Most cells share one cached inlay, including during a detonation redraw.
        if tile == Tile::Covered && !opening {
            return img("games/minesweeper/covered-inlay.svg")
                .object_fit(ObjectFit::Fill)
                .size_full()
                .into_any_element();
        }

        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                let edge = bounds.size.width.min(bounds.size.height);
                let face = Stamp::new(bounds, edge);
                let mark = Stamp::new(bounds, edge * 0.75);
                match tile {
                    Tile::Covered => {
                        face.bevel(window);
                        mark.opening(window);
                    }
                    Tile::Flagged => {
                        face.bevel(window);
                        mark.flag(window);
                    }
                    Tile::Open(_) if chordable => face.stroke(
                        window,
                        &[
                            &[(0.37, 0.14), (0.14, 0.14), (0.14, 0.37)],
                            &[(0.63, 0.86), (0.86, 0.86), (0.86, 0.63)],
                        ],
                        0.052,
                        rgb(self.chord_corners),
                    ),
                    Tile::Mine => mark.mine(window, self.mine_rim),
                    Tile::Detonated => {
                        face.burst(window, 1., rgb(0xde7448));
                        if let Some(progress) = impact {
                            let light = (190. * (1. - progress).powi(2)) as u32;
                            face.burst(window, 0.74 + 0.36 * progress, rgba(0xffda9c00 | light));
                        }
                        mark.mine(window, 0xffe1a9);
                    }
                    Tile::Misflagged => {
                        mark.flag(window);
                        let slash: &[&[_]] = &[&[(0.1, 0.91), (0.9, 0.09)]];
                        mark.stroke(window, slash, 0.155, rgb(0x4a281d));
                        mark.stroke(window, slash, 0.075, rgb(self.misflag_ink));
                    }
                    Tile::Open(_) => {}
                }
            },
        )
        .size_full()
        .into_any_element()
    }
}

pub(super) fn fastener(at_top: bool, at_left: bool, art: &FieldArt) -> Div {
    let pin = div()
        .absolute()
        .size(px(4.))
        .rounded_full()
        .border_1()
        .border_color(rgb(art.frame.edge))
        .bg(finish(0xf5d8a8, 0xb17f49));
    let pin = if at_top {
        pin.top(px(3.))
    } else {
        pin.bottom(px(3.))
    };
    if at_left {
        pin.left(px(3.))
    } else {
        pin.right(px(3.))
    }
}

// The same normalized geometry is used at every size and display scale.
struct Stamp {
    origin: Point<Pixels>,
    unit: Pixels,
}

impl Stamp {
    fn new(bounds: Bounds<Pixels>, unit: Pixels) -> Self {
        Self {
            origin: bounds.center() - point(unit / 2., unit / 2.),
            unit,
        }
    }

    fn at(&self, (x, y): (f32, f32)) -> Point<Pixels> {
        self.origin + point(self.unit * x, self.unit * y)
    }

    fn stroke(
        &self,
        window: &mut Window,
        lines: &[&[(f32, f32)]],
        width: f32,
        ink: impl Into<Background>,
    ) {
        let mut path = PathBuilder::stroke(self.unit * width);
        for line in lines {
            let [start, rest @ ..] = *line else { continue };
            path.move_to(self.at(*start));
            for &to in rest {
                path.line_to(self.at(to));
            }
        }
        paint(window, path, ink);
    }

    fn polygon(&self, window: &mut Window, points: &[(f32, f32)], ink: impl Into<Background>) {
        let Some((&first, rest)) = points.split_first() else {
            return;
        };
        let mut path = PathBuilder::fill();
        path.move_to(self.at(first));
        for &to in rest {
            path.line_to(self.at(to));
        }
        path.close();
        paint(window, path, ink);
    }

    fn disc(
        &self,
        window: &mut Window,
        (x, y): (f32, f32),
        radius: f32,
        ink: impl Into<Background>,
    ) {
        let diameter = self.unit * (2. * radius);
        let bounds = Bounds::new(self.at((x - radius, y - radius)), size(diameter, diameter));
        window.paint_quad(fill(bounds, ink).corner_radii(diameter / 2.));
    }

    fn bevel(&self, window: &mut Window) {
        self.stroke(
            window,
            &[&[(0.09, 0.78), (0.09, 0.19), (0.18, 0.09), (0.82, 0.09)]],
            0.038,
            rgba(0xc4dfb954),
        );
        self.stroke(
            window,
            &[&[(0.18, 0.90), (0.81, 0.90), (0.91, 0.81), (0.91, 0.20)]],
            0.055,
            rgba(0x071f1b6a),
        );
    }

    fn opening(&self, window: &mut Window) {
        self.polygon(
            window,
            &[(0.50, 0.13), (0.77, 0.50), (0.50, 0.87), (0.23, 0.50)],
            finish(IVORY, 0xd58845),
        );
        self.disc(window, (0.50, 0.50), 0.075, rgb(0x483e29));
    }

    fn flag(&self, window: &mut Window) {
        // A cast peg and a folded copper pennant, with a bright leading edge.
        self.stroke(
            window,
            &[&[(0.28, 0.14), (0.28, 0.89)], &[(0.15, 0.89), (0.53, 0.89)]],
            0.14,
            rgb(0x231d17),
        );
        self.stroke(
            window,
            &[&[(0.28, 0.14), (0.28, 0.87)], &[(0.15, 0.89), (0.52, 0.89)]],
            0.074,
            rgb(COPPER),
        );
        let mut cloth = PathBuilder::fill();
        cloth.move_to(self.at((0.29, 0.19)));
        cloth.curve_to(self.at((0.89, 0.29)), self.at((0.57, 0.06)));
        cloth.curve_to(self.at((0.29, 0.62)), self.at((0.54, 0.62)));
        cloth.close();
        paint(window, cloth, finish(0xffd296, 0xc47b3e));
        self.stroke(
            window,
            &[&[(0.35, 0.23), (0.49, 0.21), (0.61, 0.24)]],
            0.052,
            rgb(IVORY),
        );
        self.disc(window, (0.28, 0.14), 0.072, rgb(IVORY));
    }

    fn mine(&self, window: &mut Window, rim: u32) {
        self.stroke(
            window,
            &[
                &[(0.50, 0.08), (0.50, 0.92)],
                &[(0.08, 0.50), (0.92, 0.50)],
                &[(0.20, 0.20), (0.80, 0.80)],
                &[(0.80, 0.20), (0.20, 0.80)],
            ],
            0.137,
            rgb(rim),
        );
        self.disc(window, (0.50, 0.53), 0.34, rgb(rim));
        self.disc(window, (0.50, 0.50), 0.265, finish(0xe8b77d, 0x835535));
        self.disc(window, (0.50, 0.50), 0.136, rgb(0x453129));
        self.disc(window, (0.44, 0.42), 0.066, rgb(IVORY));
    }

    fn burst(&self, window: &mut Window, scale: f32, ink: impl Into<Background>) {
        self.polygon(
            window,
            &[
                (0.50, 0.03),
                (0.61, 0.32),
                (0.87, 0.18),
                (0.74, 0.42),
                (0.98, 0.50),
                (0.74, 0.60),
                (0.86, 0.86),
                (0.61, 0.73),
                (0.50, 0.97),
                (0.39, 0.72),
                (0.13, 0.87),
                (0.26, 0.60),
                (0.02, 0.50),
                (0.26, 0.40),
                (0.13, 0.15),
                (0.38, 0.31),
            ]
            .map(|(x, y)| (0.5 + (x - 0.5) * scale, 0.5 + (y - 0.5) * scale)),
            ink,
        );
    }
}

fn paint(window: &mut Window, path: PathBuilder, ink: impl Into<Background>) {
    if let Ok(path) = path.build() {
        window.paint_path(path, ink);
    }
}
