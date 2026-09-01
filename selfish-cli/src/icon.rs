//! The icon a package gets when the caller does not supply one.
//!
//! # Why there is a default at all, and why it is not blank
//!
//! An icon is not one of the console's formats, so it is not the library's business - this
//! lives in the tool rather than in `selfish-pkg` for that reason. But a package must carry
//! one, and something has to go in when nobody said what.
//!
//! The first attempt was a single blank pixel: valid, minimal, and useless. A blank tile on a
//! home screen tells you nothing. **A recognisable mark tells you two things at once** - that
//! selfish built this package, and that nobody supplied an icon - which is exactly the state
//! of affairs, and exactly what you want to know when a package you are debugging appears on a
//! console.
//!
//! # Committed rather than drawn, now that there is something to commit
//!
//! This used to build the image here from a glyph table, spelling `SELFISH` in a border, on the
//! reasoning that a drawn mark can be read and reviewed in a diff like everything else in this
//! repository - and it said, in as many words, that when a real logo existed it could replace
//! `default_icon` wholesale with nothing else moving. That is what happened, so the glyph table,
//! the border, the word and the hand-rolled encoder are gone.
//!
//! The asset is [`assets/logo.svg`], and [`assets/logo.png`] is a 512×512 raster of it. **The
//! source of truth is the SVG**; the PNG is generated from it, because a console wants a raster
//! at a fixed size and nothing here can rasterise one, so it is rendered out of band and committed.
//!
//! Two logos is the failure this avoids. A drawn mark here and a real one in the readme would
//! diverge the first time either changed, and the one nobody looks at is the one that ends up on
//! a console.
//!
//! [`assets/logo.svg`]: https://github.com/project-oops/SELFish/blob/main/assets/logo.svg
//! [`assets/logo.png`]: https://github.com/project-oops/SELFish/blob/main/assets/logo.png

/// The icon a package gets when none was supplied: selfish's own logo, 512×512.
///
/// Embedded at compile time, so a build carries it without reading anything at run time - and put
/// through [`normalise`] like any supplied icon, because the default is not exempt from the
/// requirement it exists to satisfy.
///
/// It was exempt, briefly, and that is the whole reason this says so. `normalise` was written for
/// icons a *caller* supplies, while the default went into the package as authored: a PNG with an
/// alpha channel and a transparent margin. So the one icon this tool ships was the one icon it did
/// not convert, and the tile it produced was the one on a home screen that read square beside
/// everything else - the exact fault the conversion was written to fix, still shipping under it.
///
/// # Errors
///
/// If the embedded logo is not a PNG this can convert, which would be a broken build rather than
/// anything a caller did.
pub(crate) fn default_icon() -> Result<Vec<u8>, String> {
    normalise(LOGO, "selfish's own logo")
}

/// The logo, as the PNG bytes a package entry holds.
const LOGO: &[u8] = include_bytes!("../../assets/logo.png");

/// What a console wants an icon to be, on both axes.
const ICON_SIDE: u32 = 512;

/// Make a supplied PNG into the icon a console expects, or say why it cannot.
///
/// # Why this is here rather than in every project that builds a package
///
/// A console wants `icon0.png` at 512x512 with **no alpha channel**: all three real packages
/// examined are colour type 2 with their artwork running edge to edge, and the rounded corners on
/// a home screen are the console's own mask laid over the top. An icon exported with transparency
/// is not rejected - it is accepted and then composited differently, so the artwork sits inset
/// inside its own margin and the tile reads square beside everything else. That is a difference
/// you find by looking at a television, which is the worst place to keep a format requirement.
///
/// Four projects here build packages. Asking each to export correctly means four chances to get
/// it wrong and no single place to fix it, so the conversion happens once, where the requirement
/// is already written down.
///
/// # What it does, and what it refuses
///
/// Transparency is composited over black rather than dropped, because dropping it leaves whatever
/// colour happened to sit under a transparent pixel - usually white fringing on antialiased art.
///
/// **It does not resize.** A wrong-sized icon is refused with its actual size in the message.
/// Scaling is a judgement about someone else's artwork - which filter, whether to letterbox a
/// non-square image, whether to sharpen pixel art that nearest-neighbour would keep crisp and
/// bilinear would turn to mush - and guessing it silently is how a logo ends up blurred with
/// nobody able to say which step did it. Refusing names the problem where the artwork is.
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

/// Three bytes a pixel, with any transparency laid over black.
///
/// Composited rather than dropped: discarding an alpha channel leaves whatever colour sat under a
/// transparent pixel, which on antialiased artwork is white fringing around every edge.
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

/// The finished icon: eight-bit RGB at the size a console expects.
fn encode(flat: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, ICON_SIDE, ICON_SIDE);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|why| format!("could not write the icon: {why}"))?;
    writer
        .write_image_data(flat)
        .map_err(|why| format!("could not write the icon: {why}"))?;
    drop(writer);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::LOGO;

    /// The default icon is converted like any other, and carries no alpha channel.
    ///
    /// The one this tool ships was the one it did not convert: `normalise` was written for icons a
    /// caller supplies, and the embedded logo went into a package as authored, with the
    /// transparent margin that makes a tile read square on a home screen. The requirement is not
    /// something a default is exempt from, and this is what says so.
    ///
    /// `IHDR` records the colour type at byte 25, where 2 is RGB and 6 is RGBA.
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

    /// The bytes are a PNG, and one a console will recognise as such.
    ///
    /// Worth a test rather than a glance: the file is generated by a separate step, and a
    /// truncated or mis-copied asset would otherwise reach a package and only be noticed as a
    /// tile that will not draw.
    #[test]
    fn the_logo_is_a_png() {
        assert_eq!(
            LOGO.get(..8),
            Some(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A][..]),
            "the embedded logo does not start with the PNG signature"
        );
    }

    /// A console expects the icon at 512×512, so the raster's own header has to say so.
    ///
    /// `IHDR` is always the first chunk: eight bytes of signature, a four-byte length, the type,
    /// then width and height as big-endian `u32`.
    #[test]
    fn the_logo_is_512_square() {
        let width = LOGO.get(16..20).expect("an IHDR width");
        let height = LOGO.get(20..24).expect("an IHDR height");
        assert_eq!(width, &[0, 0, 2, 0], "the logo is not 512 wide");
        assert_eq!(height, &[0, 0, 2, 0], "the logo is not 512 tall");
    }
}
