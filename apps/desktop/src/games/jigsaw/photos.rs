use std::{
    env,
    fs::{self, File},
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use image::{
    DynamicImage, GenericImageView as _, ImageDecoder as _, ImageError, ImageFormat, ImageReader,
    Limits, Rgb, RgbImage,
    codecs::jpeg::JpegEncoder,
    imageops::{FilterType, resize},
};

const MAX_FILE: u64 = 32 * 1024 * 1024;
const MAX_SIDE: u32 = 8192;
const MAX_PIXELS: u64 = 32_000_000;
const MAX_DECODED: u64 = 192 * 1024 * 1024;
const WIDTH: u32 = 1200;
const HEIGHT: u32 = 800;
pub(super) const ASPECT: f32 = WIDTH as f32 / HEIGHT as f32;

const ID_PREFIX: &str = "photo-";
const STAGING_PREFIX: &str = ".import-";
pub(super) const PICTURE: &str = "puzzle.jpg";
pub(super) const THUMBNAIL: &str = "thumbnail.jpg";

pub(super) fn directory() -> Option<PathBuf> {
    // Follow the desktop profile convention used by storage::Session.
    env::var_os("CORNER_TABLE_DATA_DIR")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir().map(|dir| dir.join("dk.karbowiak.CornerTable")))
        .map(|dir| dir.join("jigsaw-photos"))
}

pub(super) fn stored(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut photos = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name
            .to_str()
            .is_some_and(|name| name.starts_with(ID_PREFIX))
        {
            continue;
        }
        let directory = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir())
            && is_regular_file(&directory.join(PICTURE))
            && is_regular_file(&directory.join(THUMBNAIL))
        {
            photos.push(directory);
        }
    }
    photos.sort_unstable();
    Ok(photos)
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
}

pub(super) fn import(source: &Path, directory: &Path) -> Result<PathBuf> {
    let picture = normalize(decode(source)?);
    let thumbnail = resize(&picture, 276, 184, FilterType::Triangle);
    save(directory, &picture, &thumbnail).context("Couldn’t save the photo to your local gallery.")
}

fn decode(source: &Path) -> Result<DynamicImage> {
    let metadata = fs::metadata(source).context("Couldn’t open that file.")?;
    if !metadata.is_file() {
        bail!("Choose a JPEG, PNG, or WebP file.");
    }
    if metadata.len() > MAX_FILE {
        bail!("Choose an image that’s 32 MB or smaller.");
    }
    let mut bytes = Vec::new();
    File::open(source)
        .context("Couldn’t open that file.")?
        .take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .context("Couldn’t read that file.")?;
    if bytes.len() as u64 > MAX_FILE {
        bail!("Choose an image that’s 32 MB or smaller.");
    }
    let format = match image::guess_format(&bytes) {
        Ok(format @ (ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)) => format,
        _ => bail!("Choose a JPEG, PNG, or WebP file."),
    };

    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODED);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS || decoder.total_bytes() > MAX_DECODED {
        bail!("That image has too many pixels. Choose a smaller copy.");
    }
    let orientation = decoder.orientation().map_err(decode_error)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn decode_error(error: ImageError) -> anyhow::Error {
    match error {
        ImageError::Limits(_) => anyhow::anyhow!("That image is too large. Choose a smaller copy."),
        _ => anyhow::anyhow!("Couldn’t decode that image. Try a different JPEG, PNG, or WebP."),
    }
}

fn normalize(image: DynamicImage) -> RgbImage {
    let (width, height) = image.dimensions();
    // Crop before resampling so even panoramas fit without resizing a large unused area.
    let units = (width / 3).min(height / 2).max(1);
    let crop_width = width.min(units * 3);
    let crop_height = height.min(units * 2);
    let left = (width - crop_width) / 2;
    let top = (height - crop_height) / 2;
    let crop = RgbImage::from_fn(crop_width, crop_height, |x, y| {
        let [r, g, b, alpha] = image.get_pixel(x + left, y + top).0;
        let alpha = u16::from(alpha);
        // JPEG is opaque. Composite transparent input onto white before resampling.
        Rgb([r, g, b].map(|channel| (255 - ((255 - u16::from(channel)) * alpha + 127) / 255) as u8))
    });
    resize(&crop, WIDTH, HEIGHT, FilterType::Triangle)
}

fn save(directory: &Path, picture: &RgbImage, thumbnail: &RgbImage) -> Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let staging = tempfile::Builder::new()
        .prefix(STAGING_PREFIX)
        .tempdir_in(directory)?;
    write_jpeg(&staging.path().join(PICTURE), picture)?;
    write_jpeg(&staging.path().join(THUMBNAIL), thumbnail)?;
    let random = staging
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix(STAGING_PREFIX))
        .expect("tempfile uses an ASCII prefix and suffix");
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let photo = directory.join(format!("{ID_PREFIX}{timestamp:020}-{random}"));
    // Publish both files at once. An interrupted import is invisible to the gallery.
    fs::rename(staging.path(), &photo)?;
    Ok(photo)
}

fn write_jpeg(path: &Path, picture: &RgbImage) -> Result<()> {
    let mut file = File::create(path)?;
    JpegEncoder::new_with_quality(&mut file, 90).encode_image(picture)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder as _;

    #[test]
    fn supported_files_become_local_landscape_pictures_and_thumbnails() {
        let test = tempfile::tempdir().unwrap();
        let directory = test.path().join("gallery");
        let portrait = RgbImage::from_fn(90, 150, |_, y| {
            if (40..110).contains(&y) {
                Rgb([20, 220, 40])
            } else {
                Rgb([230, 10, 20])
            }
        });
        for format in [ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::WebP] {
            let source = test.path().join("Original private filename");
            portrait.save_with_format(&source, format).unwrap();
            let original = fs::read(&source).unwrap();
            let photo = import(&source, &directory).unwrap();
            assert_eq!(fs::read(&source).unwrap(), original);
            assert!(
                !photo
                    .to_string_lossy()
                    .contains("Original private filename")
            );
            fs::remove_file(source).unwrap();

            for (file, dimensions) in [(PICTURE, (WIDTH, HEIGHT)), (THUMBNAIL, (276, 184))] {
                let image = image::open(photo.join(file)).unwrap();
                assert_eq!(image.dimensions(), dimensions);
                for y in [0, dimensions.1 - 1] {
                    let [red, green, _, _] = image.get_pixel(dimensions.0 / 2, y).0;
                    assert!(green > 200 && red < 40);
                }
            }
        }
        assert_eq!(stored(&directory).unwrap().len(), 3);
    }

    #[test]
    fn jpeg_orientation_is_applied_before_cropping_and_removed_from_the_copy() {
        let test = tempfile::tempdir().unwrap();
        let source = test.path().join("portrait.jpg");
        let portrait = RgbImage::from_fn(80, 120, |_, y| {
            if y < 60 {
                Rgb([230, 20, 0])
            } else {
                Rgb([0, 20, 230])
            }
        });
        let mut encoder = JpegEncoder::new_with_quality(File::create(&source).unwrap(), 90);
        // Little-endian TIFF header and one EXIF orientation tag, 6 = rotate 90° clockwise.
        encoder
            .set_exif_metadata(vec![
                b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0,
                0, 0,
            ])
            .unwrap();
        encoder.encode_image(&portrait).unwrap();

        let photo = import(&source, &test.path().join("gallery")).unwrap();
        let mut decoder = ImageReader::open(photo.join(PICTURE))
            .unwrap()
            .into_decoder()
            .unwrap();
        assert!(decoder.exif_metadata().unwrap().is_none());
        let picture = DynamicImage::from_decoder(decoder).unwrap();
        assert!(picture.get_pixel(0, HEIGHT / 2).0[2] > 200);
        assert!(picture.get_pixel(WIDTH - 1, HEIGHT / 2).0[0] > 200);
    }

    #[test]
    fn rejected_inputs_publish_no_gallery_entries() {
        let test = tempfile::tempdir().unwrap();
        let directory = test.path().join("gallery");
        let source = test.path().join("invalid");
        File::create(&source)
            .unwrap()
            .set_len(MAX_FILE + 1)
            .unwrap();
        assert!(
            import(&source, &directory)
                .unwrap_err()
                .to_string()
                .contains("32 MB")
        );

        RgbImage::new(MAX_SIDE + 1, 1)
            .save_with_format(&source, ImageFormat::Png)
            .unwrap();
        assert!(
            import(&source, &directory)
                .unwrap_err()
                .to_string()
                .contains("too large")
        );

        fs::write(&source, [0xff, 0xd8, 0xff]).unwrap();
        assert!(
            import(&source, &directory)
                .unwrap_err()
                .to_string()
                .contains("decode")
        );

        fs::write(&source, b"unsupported").unwrap();
        assert!(
            import(&source, &directory)
                .unwrap_err()
                .to_string()
                .contains("JPEG, PNG, or WebP")
        );
        assert!(stored(&directory).unwrap().is_empty());
    }
}
