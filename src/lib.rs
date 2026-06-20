//! `avif-rust` — encode and decode AVIF images via libavif.
//!
//! The libavif C library (plus its codec/support dependencies) is downloaded as a
//! prebuilt **static** library at build time and linked directly into this crate, so
//! consumers do not need libavif installed on the host. See `build.rs`.
//!
//! Encoding uses **SVT-AV1** and decoding uses **dav1d**, both statically linked.
//!
//! The API mirrors the `image` crate's codec conventions:
//! * [`AvifEncoder`] / [`AvifDecoder`] implement `image`'s `ImageEncoder` / `ImageDecoder`
//!   traits, so they plug into `DynamicImage::write_with_encoder` / `DynamicImage::from_decoder`
//!   just like the codecs bundled with `image`.
//! * a thin facade ([`encode`], [`encode_buffer`], [`decode`], [`probe`]) wraps those for
//!   one-line convenience.

mod decoder;
mod encoder;
mod error;
mod ffi;
mod info;
mod sys;

pub use decoder::{AvifDecoder, DecoderConfig};
pub use encoder::{AvifEncoder, EncoderConfig};
pub use error::{AvifError, Result};
pub use info::{BitDepth, ImageInfo};

use std::ffi::CStr;
use std::io::Cursor;
use std::ops::Deref;

use image::{DynamicImage, EncodableLayout, ImageBuffer, ImageDecoder, PixelWithColorType};

/// Returns the version string of the linked libavif library, e.g. `"1.4.2"`.
pub fn libavif_version() -> String {
    // SAFETY: `avifVersion` returns a pointer to a static, NUL-terminated C string.
    unsafe {
        let ptr = sys::avifVersion();
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

/// Encode a [`DynamicImage`] to AV1/AVIF bytes using sensible defaults.
///
/// # Example
/// ```no_run
/// let img = image::open("photo.png")?;
/// let avif_bytes = avif_rust::encode(&img)?;
/// # Ok::<(), avif_rust::AvifError>(())
/// ```
pub fn encode(image: &DynamicImage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    image.write_with_encoder(AvifEncoder::new(&mut buf))?;
    Ok(buf)
}

/// Encode a typed [`ImageBuffer`] directly, avoiding the runtime dispatch
/// overhead of [`DynamicImage`]. Prefer this when you already know your
/// pixel type at compile time.
///
/// # Example
/// ```no_run
/// use image::RgbaImage;
/// let img: RgbaImage = image::open("photo.png")?.into_rgba8();
/// let avif_bytes = avif_rust::encode_buffer(&img)?;
/// # Ok::<(), avif_rust::AvifError>(())
/// ```
pub fn encode_buffer<P, C>(buffer: &ImageBuffer<P, C>) -> Result<Vec<u8>>
where
    P: PixelWithColorType,
    [P::Subpixel]: EncodableLayout,
    C: Deref<Target = [P::Subpixel]>,
{
    let mut buf = Vec::new();
    buffer.write_with_encoder(AvifEncoder::new(&mut buf))?;
    Ok(buf)
}

/// Decode AV1/AVIF bytes into a [`DynamicImage`].
///
/// # Example
/// ```no_run
/// # let avif_bytes: Vec<u8> = Vec::new();
/// let img = avif_rust::decode(&avif_bytes)?;
/// img.save("output.png")?;
/// # Ok::<(), avif_rust::AvifError>(())
/// ```
pub fn decode(data: &[u8]) -> Result<DynamicImage> {
    let decoder = AvifDecoder::new(Cursor::new(data))?;
    Ok(DynamicImage::from_decoder(decoder)?)
}

/// Read only the image header — no pixel decode.
/// Useful for validation or thumbnailing pipelines.
///
/// # Example
/// ```no_run
/// # let avif_bytes: Vec<u8> = Vec::new();
/// let info = avif_rust::probe(&avif_bytes)?;
/// println!("{}x{} @ {:?}", info.width, info.height, info.bit_depth);
/// # Ok::<(), avif_rust::AvifError>(())
/// ```
pub fn probe(data: &[u8]) -> Result<ImageInfo> {
    let decoder = AvifDecoder::new(Cursor::new(data))?;
    let (width, height) = decoder.dimensions();
    Ok(ImageInfo {
        width,
        height,
        color_type: decoder.color_type(),
        bit_depth: decoder.bit_depth(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: calling into libavif proves the static binaries are linked and
    /// callable end-to-end.
    #[test]
    fn reports_libavif_version() {
        let version = libavif_version();
        println!("linked libavif version: {version}");
        assert!(!version.is_empty());
    }
}
