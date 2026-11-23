use std::{fs, io::Cursor, sync::Arc};

use corner_table_jigsaw::{OutlinePoint, Size, piece_outline};
use gpui::{AssetSource as _, RenderImage};
use image::{DynamicImage, Frame, ImageDecoder as _, ImageFormat, ImageReader, RgbImage, Rgba};

use crate::assets::Assets;

use super::PictureSource;

pub(super) struct PieceArt {
    pub outline: Arc<[OutlinePoint]>,
    pub image: Arc<RenderImage>,
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub aspect: f32,
}

pub(super) fn cut_painting(source: &PictureSource, size: Size) -> anyhow::Result<Vec<PieceArt>> {
    let bytes = match source {
        PictureSource::Bundled(asset) => Assets
            .load(asset)?
            .ok_or_else(|| anyhow::anyhow!("Missing painting {asset}"))?,
        PictureSource::Local(path) => fs::read(path)?.into(),
    };
    let mut decoder =
        ImageReader::with_format(Cursor::new(bytes), ImageFormat::Jpeg).into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut painting = DynamicImage::from_decoder(decoder)?;
    painting.apply_orientation(orientation);
    let painting = painting.into_rgb8();

    Ok((0..size.piece_count())
        .map(|piece| cut_piece(&painting, size, piece))
        .collect())
}

fn cut_piece(painting: &RgbImage, size: Size, piece: usize) -> PieceArt {
    let (columns, rows) = size.dimensions();
    let picture_width = painting.width() as f32;
    let picture_height = painting.height() as f32;
    let outline = piece_outline(size, piece).expect("piece is in the painting");
    let mut pixels: Vec<_> = outline
        .iter()
        .map(|p| OutlinePoint {
            x: p.x * picture_width / columns as f32,
            y: p.y * picture_height / rows as f32,
        })
        .collect();

    let left = pixels.iter().map(|p| p.x).reduce(f32::min).unwrap().floor() as u32;
    let top = pixels.iter().map(|p| p.y).reduce(f32::min).unwrap().floor() as u32;
    let right =
        (pixels.iter().map(|p| p.x).reduce(f32::max).unwrap().ceil() as u32).min(painting.width());
    let bottom =
        (pixels.iter().map(|p| p.y).reduce(f32::max).unwrap().ceil() as u32).min(painting.height());
    let (width, height) = (right - left, bottom - top);
    for p in &mut pixels {
        p.x -= left as f32;
        p.y -= top as f32;
    }
    pixels.push(pixels[0]);

    let mut coverage = vec![0.; width as usize];
    let mut crossings = Vec::new();
    let mut cutout = image::RgbaImage::new(width, height);
    for y in 0..height {
        coverage.fill(0.);
        // Integrate horizontal coverage across four subpixel scanlines. Both sides
        // of each cut sample the same source pixels and retain antialiased edges.
        for subpixel_y in [0.125, 0.375, 0.625, 0.875] {
            let sample_y = y as f32 + subpixel_y;
            crossings.clear();
            for edge in pixels.windows(2) {
                let [a, b] = [edge[0], edge[1]];
                if (a.y <= sample_y && sample_y < b.y) || (b.y <= sample_y && sample_y < a.y) {
                    crossings.push(a.x + (b.x - a.x) * (sample_y - a.y) / (b.y - a.y));
                }
            }
            crossings.sort_unstable_by(f32::total_cmp);
            for &[start, end] in crossings.as_chunks::<2>().0 {
                let first = start.floor() as usize;
                let last = (end.ceil() as usize).min(coverage.len());
                for (x, alpha) in coverage.iter_mut().enumerate().take(last).skip(first) {
                    *alpha += end.min(x as f32 + 1.) - start.max(x as f32);
                }
            }
        }

        for (x, alpha) in coverage.iter().enumerate() {
            // GPUI's texture atlas expects straight-alpha BGRA pixels.
            let [r, g, b] = painting.get_pixel(left + x as u32, top + y).0;
            let alpha = (alpha * (255. / 4.)).round() as u8;
            cutout.put_pixel(x as u32, y, Rgba([b, g, r, alpha]));
        }
    }

    PieceArt {
        outline: outline.into(),
        image: Arc::new(RenderImage::new([Frame::new(cutout)])),
        left: left as f32 / picture_width,
        top: top as f32 / picture_height,
        width: width as f32 / picture_width,
        height: height as f32 / picture_height,
        aspect: width as f32 / height as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_bundled_paintings_decode_for_swap_and_cut_at_every_size() {
        let renderer = gpui::SvgRenderer::new(Arc::new(Assets));
        for painting in super::super::GALLERY {
            let PictureSource::Bundled(asset) = painting.source else {
                panic!("{} must be a bundled scene", painting.id);
            };
            let bytes = Assets.load(asset).unwrap().unwrap().into_owned();
            let image = gpui::Image::from_bytes(gpui::ImageFormat::Jpeg, bytes)
                .to_image_data(renderer.clone())
                .unwrap_or_else(|error| panic!("decoding {asset}: {error}"));
            let dimensions = image.size(0);
            let aspect = u32::from(dimensions.width) as f32 / u32::from(dimensions.height) as f32;
            assert_eq!(painting.aspect, aspect, "{asset}");

            for size in Size::ALL {
                let pieces = cut_painting(&painting.source, size)
                    .unwrap_or_else(|error| panic!("cutting {asset} at {size:?}: {error}"));
                assert_eq!(pieces.len(), size.piece_count(), "{asset} at {size:?}");
            }
        }
    }

    #[test]
    fn the_cutouts_cover_each_source_pixel_once() {
        let painting = RgbImage::from_pixel(103, 79, image::Rgb([65, 130, 179]));
        for size in Size::ALL {
            let mut coverage = vec![0_u16; (painting.width() * painting.height()) as usize];
            for piece in 0..size.piece_count() {
                let art = cut_piece(&painting, size, piece);
                let left = (art.left * painting.width() as f32).round() as usize;
                let top = (art.top * painting.height() as f32).round() as usize;
                let width = u32::from(art.image.size(0).width) as usize;
                let (pixels, _) = art.image.as_bytes(0).unwrap().as_chunks::<4>();
                for (index, pixel) in pixels.iter().enumerate() {
                    assert_eq!(&pixel[..3], &[179, 130, 65]);
                    let x = left + index % width;
                    let y = top + index / width;
                    coverage[y * painting.width() as usize + x] += u16::from(pixel[3]);
                }
            }
            // Independent alpha rounding can differ by one at the shared cuts.
            assert!(coverage.iter().all(|alpha| (254..=256).contains(alpha)));
        }
    }
}
