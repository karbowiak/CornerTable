use std::sync::Arc;

use corner_table_memory_match::Face;
use gpui::{App, Image, ImageFormat, RenderImage};

pub(super) struct CardArt {
    pub back: Arc<RenderImage>,
    pub seal: Arc<RenderImage>,
    faces: [(Face, Arc<RenderImage>); Face::ALL.len()],
}

impl CardArt {
    pub fn load(cx: &App) -> Self {
        Self {
            back: decode(
                "back",
                include_bytes!("../../../assets/games/memory-match/back.png"),
                cx,
            ),
            seal: decode(
                "matched",
                include_bytes!("../../../assets/games/memory-match/matched.png"),
                cx,
            ),
            faces: Face::ALL.map(|face| {
                let (name, art) = specimen(face);
                (face, decode(name, art, cx))
            }),
        }
    }

    pub fn face(&self, face: Face) -> Arc<RenderImage> {
        self.faces
            .iter()
            .find(|(kind, _)| *kind == face)
            .expect("every Memory Match face has artwork")
            .1
            .clone()
    }
}

fn decode(name: &str, bytes: &[u8], cx: &App) -> Arc<RenderImage> {
    // Decode the small bundled deck up front so the mismatch clock always
    // includes the time the player can actually see both illustrations.
    Image::from_bytes(ImageFormat::Png, bytes.to_vec())
        .to_image_data(cx.svg_renderer())
        .unwrap_or_else(|err| panic!("invalid Memory Match artwork {name}: {err}"))
}

pub(super) fn name(face: Face) -> &'static str {
    specimen(face).0
}

fn specimen(face: Face) -> (&'static str, &'static [u8]) {
    macro_rules! card {
        ($name:literal, $asset:literal) => {
            (
                $name,
                include_bytes!(concat!(
                    "../../../assets/games/memory-match/",
                    $asset,
                    ".png"
                )),
            )
        };
    }

    match face {
        Face::FanShell => card!("Fan shell", "fan-shell"),
        Face::Crab => card!("Crab", "crab"),
        Face::Jellyfish => card!("Jellyfish", "jellyfish"),
        Face::Kelp => card!("Kelp", "kelp"),
        Face::SeaStar => card!("Sea star", "sea-star"),
        Face::Lanternfish => card!("Lanternfish", "lanternfish"),
        Face::Urchin => card!("Urchin", "urchin"),
        Face::SandDollar => card!("Sand dollar", "sand-dollar"),
        Face::Seahorse => card!("Seahorse", "seahorse"),
        Face::Anemone => card!("Anemone", "anemone"),
        Face::Snail => card!("Snail", "snail"),
        Face::Mussel => card!("Mussel", "mussel"),
    }
}
