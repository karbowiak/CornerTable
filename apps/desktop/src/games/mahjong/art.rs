use std::sync::{Arc, OnceLock};

use corner_table_mahjong::Face;
use gpui::{AssetSource, Div, FontWeight, ObjectFit, RenderImage, div, img, prelude::*, px, rgb};
use image::{Frame, ImageFormat};

use crate::{assets::Assets, theme};

pub(super) struct Stamp {
    index: usize,
    label: &'static str,
    pub(super) name: String,
    ink: u32,
}

impl From<Face> for Stamp {
    fn from(face: Face) -> Self {
        const COBALT: u32 = 0x193c54;
        const JADE: u32 = 0x214633;
        const CINNABAR: u32 = 0x74302a;
        const RANKS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

        let (index, rank, name, ink) = match face {
            Face::Dots(rank) => (0, rank, "Dots", COBALT),
            Face::Bamboo(rank) => (9, rank, "Bamboo", JADE),
            Face::Characters(rank) => (18, rank, "Characters", CINNABAR),
            Face::Wind('E') => return Self::honor(27, "E", "East Wind", COBALT),
            Face::Wind('S') => return Self::honor(28, "S", "South Wind", COBALT),
            Face::Wind('W') => return Self::honor(29, "W", "West Wind", COBALT),
            Face::Wind(_) => return Self::honor(30, "N", "North Wind", COBALT),
            Face::Dragon('R') => return Self::honor(31, "R", "Red Dragon", CINNABAR),
            Face::Dragon('G') => return Self::honor(32, "G", "Green Dragon", JADE),
            Face::Dragon(_) => return Self::honor(33, "W", "White Dragon", COBALT),
            Face::Flower => return Self::honor(34, "F", "Flower", 0x78353b),
            Face::Season => return Self::honor(35, "S", "Season", 0x5f4125),
        };
        let offset = usize::from(rank - 1);
        Self {
            index: index + offset,
            label: RANKS[offset],
            name: format!("{name} {rank}"),
            ink,
        }
    }
}

impl Stamp {
    fn honor(index: usize, label: &'static str, name: &str, ink: u32) -> Self {
        Self {
            index,
            label,
            name: name.into(),
            ink,
        }
    }
}

pub(super) fn face_name(face: Face) -> String {
    Stamp::from(face).name
}

#[derive(Clone, Copy)]
pub(super) enum Finish {
    Free = 36,
    Blocked,
    Selected,
    Hinted,
    Matched,
}

pub(super) fn glaze(size: f32, stamp: &Stamp, finish: Finish) -> Div {
    let label_size = (size * 0.23).max(9.).min(size * 0.4);
    let art = atlas();
    div()
        .absolute()
        .inset_0()
        .child(
            img(art[finish as usize].clone())
                .absolute()
                .size_full()
                .object_fit(ObjectFit::Fill),
        )
        .child(
            img(art[stamp.index].clone())
                .absolute()
                .size_full()
                .object_fit(ObjectFit::Fill),
        )
        .child(
            div()
                .absolute()
                .top(px(size * 0.049))
                .left(px(size * 0.139))
                .font_family(theme::BODY)
                .font_weight(FontWeight::BOLD)
                .text_size(px(label_size))
                .line_height(px(label_size))
                .text_color(rgb(stamp.ink))
                .child(stamp.label),
        )
}

const SPRITES: usize = Finish::Matched as usize + 1;

fn atlas() -> &'static [Arc<RenderImage>; SPRITES] {
    const WIDTH: u32 = 144;
    const HEIGHT: u32 = 164;
    const COLUMNS: u32 = 9;
    static ATLAS: OnceLock<[Arc<RenderImage>; SPRITES]> = OnceLock::new();

    ATLAS.get_or_init(|| {
        let data = Assets
            .load("games/mahjong/tiles.webp")
            .expect("embedded asset is readable")
            .expect("bundled Mahjong artwork exists");
        let mut sheet = image::load_from_memory_with_format(&data, ImageFormat::WebP)
            .expect("bundled Mahjong artwork decodes")
            .into_rgba8();
        assert_eq!(sheet.dimensions(), (COLUMNS * WIDTH, 5 * HEIGHT));
        // GPUI uploads RenderImage frames in BGRA channel order.
        for pixel in sheet.pixels_mut() {
            pixel.0.swap(0, 2);
        }
        std::array::from_fn(|index| {
            let col = index as u32 % COLUMNS;
            let row = index as u32 / COLUMNS;
            let cell = image::imageops::crop_imm(&sheet, col * WIDTH, row * HEIGHT, WIDTH, HEIGHT);
            Arc::new(RenderImage::new([Frame::new(cell.to_image())]))
        })
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use corner_table_mahjong::{Board, DAILY_SEED};

    use super::*;

    #[test]
    fn the_complete_deck_and_each_finish_have_an_atlas_cell() {
        let deck: BTreeSet<_> = Board::new(DAILY_SEED)
            .all_tiles()
            .iter()
            .map(|tile| Stamp::from(tile.face).index)
            .collect();
        assert!(deck.into_iter().eq(0..Finish::Free as usize));
        for (index, sprite) in atlas().iter().enumerate() {
            let (pixels, _) = sprite.as_bytes(0).unwrap().as_chunks::<4>();
            assert!(
                pixels.iter().any(|pixel| pixel[3] >= 128),
                "empty cell {index}"
            );
        }
    }
}
