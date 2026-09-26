//! The icon a package gets when the caller does not supply one.
//!
//! An icon is not a hardware format, so this lives in the tool rather than in `selfish-pkg`.
//! A package must carry one; the default is the project logo, so a tile shows both who built
//! the package and that no icon was supplied.
//!
//! The source of truth is [`assets/logo.svg`]; [`assets/logo.png`] is a 512x512 raster of it,
//! rendered out of band and committed because nothing here rasterises SVG.
//!
//! [`assets/logo.svg`]: https://github.com/project-oops/SELFish/blob/main/assets/logo.svg
//! [`assets/logo.png`]: https://github.com/project-oops/SELFish/blob/main/assets/logo.png

/// The icon a package gets when none was supplied: selfish's own logo, 512x512.
///
/// Embedded at compile time and put through [`normalise`] like any supplied icon, since the
/// authored logo carries an alpha channel and a transparent margin.
///
/// # Errors
///
/// If the embedded logo is not a PNG this can convert, which is a broken build.
pub(crate) fn default_icon() -> Result<Vec<u8>, String> {
    normalise(LOGO, "selfish's own logo")
}

/// The default background wallpaper (`pic0.png`): 1920x1080 24-bit RGB.
pub(crate) fn default_background() -> Result<Vec<u8>, String> {
    normalise_background(BACKGROUND, "selfish's default background")
}

/// The default title logo badge (`logo.png`): transparent RGBA PNG.
pub(crate) fn default_logo() -> Result<Vec<u8>, String> {
    normalise_logo(BADGE, "selfish's default title badge")
}

/// The logo, as the PNG bytes a package entry holds.
const LOGO: &[u8] = include_bytes!("../../assets/logo.png");

/// The default background wallpaper, 1920x1080 RGB.
const BACKGROUND: &[u8] = include_bytes!("../../assets/background.png");

/// The default title logo badge, transparent RGBA PNG.
const BADGE: &[u8] = include_bytes!("../../assets/badge.png");

/// The side of the square icon the hardware expects.
const ICON_SIDE: u32 = 512;

/// Make a supplied PNG into the icon the hardware expects, or say why it cannot.
///
/// `icon0.png` is 512x512 with no alpha channel: the measured packages are colour type 2 with
/// artwork running edge to edge, and the home screen applies its own corner mask. An icon with
/// transparency is accepted but composited inset, reading square beside the others. Every
/// package builder in the collection goes through this one conversion (D073).
///
/// Transparency is composited over black, not dropped, so antialiased edges do not fringe.
/// A wrong-sized icon is refused with its size in the message; scaling is a judgement about
/// the artwork and is left to its author.
pub(crate) fn normalise(bytes: &[u8], what: &str) -> Result<Vec<u8>, String> {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder
        .read_info()
        .map_err(|why| format!("{what} is not a PNG this can read: {why}"))?;
    let mut buffer = vec![0_u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|why| format!("{what} could not be decoded: {why}"))?;

    if info.width != ICON_SIDE || info.height != ICON_SIDE {
        return Err(format!(
            "{what} is {}x{}; the hardware wants {ICON_SIDE}x{ICON_SIDE}. Nothing here resizes it, \
             because which filter to use is a decision about your artwork - export it at \
             {ICON_SIDE}x{ICON_SIDE}",
            info.width, info.height
        ));
    }

    let source = buffer
        .get(..info.buffer_size())
        .ok_or_else(|| format!("{what} decoded to fewer bytes than it declared"))?;
    let flat = flatten(source, info.color_type, what)?;
    encode(&flat)
}

/// Make a supplied PNG into the background wallpaper the hardware expects: 1920x1080 or
/// 3840x2160, no alpha.
pub(crate) fn normalise_background(bytes: &[u8], what: &str) -> Result<Vec<u8>, String> {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder
        .read_info()
        .map_err(|why| format!("{what} is not a PNG this can read: {why}"))?;
    let mut buffer = vec![0_u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|why| format!("{what} could not be decoded: {why}"))?;

    let is_1080p = info.width == 1920 && info.height == 1080;
    let is_4k = info.width == 3840 && info.height == 2160;
    if !is_1080p && !is_4k {
        return Err(format!(
            "{what} is {}x{}; background wallpaper must be 1920x1080 or 3840x2160. \
             Export it at 1920x1080 or 3840x2160",
            info.width, info.height
        ));
    }

    let source = buffer
        .get(..info.buffer_size())
        .ok_or_else(|| format!("{what} decoded to fewer bytes than it declared"))?;
    let flat = flatten(source, info.color_type, what)?;
    encode_sized(&flat, info.width, info.height)
}

/// Make a supplied PNG into the title logo the hardware expects: within 1920x1080, RGBA.
pub(crate) fn normalise_logo(bytes: &[u8], what: &str) -> Result<Vec<u8>, String> {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder
        .read_info()
        .map_err(|why| format!("{what} is not a PNG this can read: {why}"))?;
    let mut buffer = vec![0_u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|why| format!("{what} could not be decoded: {why}"))?;

    if info.width == 0 || info.height == 0 || info.width > 1920 || info.height > 1080 {
        return Err(format!(
            "{what} is {}x{}; logo must be within 1920x1080 bounds",
            info.width, info.height
        ));
    }

    let source = buffer
        .get(..info.buffer_size())
        .ok_or_else(|| format!("{what} decoded to fewer bytes than it declared"))?;
    let rgba = to_rgba(source, info.color_type, what)?;
    encode_rgba(&rgba, info.width, info.height)
}

/// Three bytes a pixel, with any transparency laid over black.
///
/// Composited rather than dropped: discarding alpha exposes the colour under transparent
/// pixels, which fringes antialiased edges.
fn flatten(source: &[u8], colour: png::ColorType, what: &str) -> Result<Vec<u8>, String> {
    /// Scale a channel by an alpha value, both eight-bit.
    fn over_black(channel: u8, alpha: u8) -> u8 {
        let scaled = u16::from(channel).saturating_mul(u16::from(alpha)) / 255;
        u8::try_from(scaled).unwrap_or(u8::MAX)
    }

    let mut flat = Vec::with_capacity(source.len());
    match colour {
        png::ColorType::Rgb => flat.extend_from_slice(source),
        png::ColorType::Rgba => {
            for pixel in source.as_chunks::<4>().0 {
                let alpha = pixel[3];
                flat.extend_from_slice(&[
                    over_black(pixel[0], alpha),
                    over_black(pixel[1], alpha),
                    over_black(pixel[2], alpha),
                ]);
            }
        }
        png::ColorType::Grayscale => {
            for grey in source {
                flat.extend_from_slice(&[*grey, *grey, *grey]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in source.as_chunks::<2>().0 {
                let value = over_black(pixel[0], pixel[1]);
                flat.extend_from_slice(&[value, value, value]);
            }
        }
        png::ColorType::Indexed => {
            return Err(format!(
                "{what} is an indexed PNG, which this does not expand. Export it as RGB or RGBA"
            ));
        }
    }
    Ok(flat)
}

/// The finished icon: eight-bit RGB at the size the hardware expects.
fn encode(flat: &[u8]) -> Result<Vec<u8>, String> {
    encode_sized(flat, ICON_SIDE, ICON_SIDE)
}

/// Sized RGB PNG encoder.
fn encode_sized(flat: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|why| format!("could not write image header: {why}"))?;
    writer
        .write_image_data(flat)
        .map_err(|why| format!("could not write image data: {why}"))?;
    drop(writer);
    Ok(out)
}

/// Sized RGBA PNG encoder.
fn encode_rgba(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|why| format!("could not write logo header: {why}"))?;
    writer
        .write_image_data(rgba)
        .map_err(|why| format!("could not write logo data: {why}"))?;
    drop(writer);
    Ok(out)
}

/// Convert source pixels to RGBA preserving or adding full alpha.
fn to_rgba(source: &[u8], colour: png::ColorType, what: &str) -> Result<Vec<u8>, String> {
    let mut rgba = Vec::with_capacity(source.len());
    match colour {
        png::ColorType::Rgba => rgba.extend_from_slice(source),
        png::ColorType::Rgb => {
            for pixel in source.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in source.as_chunks::<2>().0 {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        png::ColorType::Grayscale => {
            for grey in source {
                rgba.extend_from_slice(&[*grey, *grey, *grey, 255]);
            }
        }
        png::ColorType::Indexed => {
            return Err(format!(
                "{what} is an indexed PNG, which this does not expand. Export it as RGB or RGBA"
            ));
        }
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::LOGO;

    /// The default icon is converted like a supplied one: RGB (`IHDR` byte 25 is 2), 512x512.
    #[test]
    fn the_default_icon_is_converted_like_a_supplied_one() {
        let icon = super::default_icon().expect("the embedded logo converts");
        assert_eq!(
            icon.get(25),
            Some(&2),
            "the default icon should be RGB, not RGBA"
        );
        assert_eq!(icon.get(16..20), Some(&[0, 0, 2, 0][..]), "512 wide");
        assert_eq!(icon.get(20..24), Some(&[0, 0, 2, 0][..]), "512 tall");
    }

    /// The embedded logo, generated out of band, starts with the PNG signature.
    #[test]
    fn the_logo_is_a_png() {
        assert_eq!(
            LOGO.get(..8),
            Some(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A][..]),
            "the embedded logo does not start with the PNG signature"
        );
    }

    /// The embedded logo's `IHDR` (the first chunk, width and height at bytes 16-24) is 512x512.
    #[test]
    fn the_logo_is_512_square() {
        let width = LOGO.get(16..20).expect("an IHDR width");
        let height = LOGO.get(20..24).expect("an IHDR height");
        assert_eq!(width, &[0, 0, 2, 0], "the logo is not 512 wide");
        assert_eq!(height, &[0, 0, 2, 0], "the logo is not 512 tall");
    }

    /// A solid RGBA PNG of a given side, for feeding [`normalise`].
    fn rgba_png(side: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, side, side);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        let pixels = (side as usize).saturating_mul(side as usize);
        let data: Vec<u8> = std::iter::repeat_n([200_u8, 50, 25, 128], pixels)
            .flatten()
            .collect();
        writer.write_image_data(&data).expect("pixels");
        drop(writer);
        out
    }

    /// A wrong-size icon is refused with both sizes named, not resized.
    #[test]
    fn a_non_512_icon_is_refused_rather_than_resized() {
        let err = super::normalise(&rgba_png(64), "test.png").expect_err("64x64 must be refused");
        assert!(err.contains("64x64"), "names the size it got: {err}");
        assert!(err.contains("512"), "names the size it wants: {err}");
    }

    /// A supplied 512x512 RGBA icon is flattened to RGB rather than written as authored.
    #[test]
    fn a_supplied_rgba_icon_is_flattened_to_rgb() {
        let out = super::normalise(&rgba_png(super::ICON_SIDE), "test.png")
            .expect("a 512 RGBA icon normalises");
        assert_eq!(
            out.get(25),
            Some(&2),
            "IHDR colour type must be RGB (2), not RGBA (6)"
        );
        assert_eq!(out.get(16..20), Some(&[0, 0, 2, 0][..]), "512 wide");
    }

    /// The default background is 1920x1080 RGB.
    #[test]
    fn the_default_background_is_1080p_rgb() {
        let bg = super::default_background().expect("default background normalises");
        assert_eq!(
            bg.get(25),
            Some(&2),
            "IHDR colour type must be RGB (2), not RGBA (6)"
        );
        assert_eq!(bg.get(16..20), Some(&[0, 0, 0x07, 0x80][..]), "1920 wide");
        assert_eq!(bg.get(20..24), Some(&[0, 0, 0x04, 0x38][..]), "1080 tall");
    }

    /// The default title logo keeps its alpha channel.
    #[test]
    fn the_default_logo_is_rgba() {
        let logo = super::default_logo().expect("default logo normalises");
        assert_eq!(logo.get(25), Some(&6), "IHDR colour type must be RGBA (6)");
    }

    /// A background that is neither 1080p nor 4K is refused with the accepted sizes named.
    #[test]
    fn wrong_size_background_is_refused() {
        let err = super::normalise_background(&rgba_png(512), "bad_bg.png")
            .expect_err("512x512 background must be refused");
        assert!(err.contains("512x512"));
        assert!(err.contains("1920x1080 or 3840x2160"));
    }
}
