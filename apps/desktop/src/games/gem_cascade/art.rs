use std::sync::{Arc, OnceLock};

use corner_table_gem_cascade::Gem;
use gpui::{App, Bounds, Image, ImageFormat, Pixels, RenderImage, Window};

const fn index(gem: Gem) -> usize {
    match gem {
        Gem::Ruby => 0,
        Gem::Amber => 1,
        Gem::Jade => 2,
        Gem::Amethyst => 3,
    }
}

pub(super) fn name(gem: Gem) -> &'static str {
    ["Ruby", "Amber", "Jade", "Amethyst"][index(gem)]
}

pub(super) fn glint(gem: Gem) -> u32 {
    [0xff8898, 0xffcf6f, 0x7ff6cd, 0xc8b5ff][index(gem)]
}

struct Cut {
    solid: Arc<RenderImage>,
    outline: Arc<RenderImage>,
}

pub(super) struct Artwork {
    cuts: [Cut; 4],
    pub lining: Arc<RenderImage>,
}

impl Artwork {
    pub fn shared(cx: &App) -> &'static Self {
        static ART: OnceLock<Artwork> = OnceLock::new();
        ART.get_or_init(|| {
            let decode = |bytes: &[u8]| {
                Image::from_bytes(ImageFormat::Png, bytes.to_vec())
                    .to_image_data(cx.svg_renderer())
                    .expect("bundled Gem Cascade artwork decodes")
            };
            macro_rules! png {
                ($name:literal) => {
                    decode(include_bytes!(concat!(
                        "../../../assets/games/gem-cascade/",
                        $name,
                        ".png"
                    )))
                };
            }
            Self {
                cuts: [
                    Cut {
                        solid: png!("ruby"),
                        outline: png!("ruby-outline"),
                    },
                    Cut {
                        solid: png!("amber"),
                        outline: png!("amber-outline"),
                    },
                    Cut {
                        solid: png!("jade"),
                        outline: png!("jade-outline"),
                    },
                    Cut {
                        solid: png!("amethyst"),
                        outline: png!("amethyst-outline"),
                    },
                ],
                lining: png!("lining"),
            }
        })
    }

    pub fn solid(&self, gem: Gem) -> Arc<RenderImage> {
        self.cuts[index(gem)].solid.clone()
    }

    pub fn outline(&self, gem: Gem) -> Arc<RenderImage> {
        self.cuts[index(gem)].outline.clone()
    }
}

pub(super) fn paint_sprite(window: &mut Window, bounds: Bounds<Pixels>, sprite: Arc<RenderImage>) {
    // Preview, landed and moving stones reuse the same atlas entries.
    let _ = window.paint_image(bounds, bounds, Default::default(), sprite, 0, false);
}
