//! AVIF encoder, mirroring the `image` crate's per-format encoder convention.
//!
//! [`AvifEncoder`] is generic over a [`Write`] sink and implements [`ImageEncoder`],
//! so it slots into `DynamicImage::write_with_encoder` exactly like the codecs that
//! ship with the `image` crate (e.g. `JpegEncoder`, `WebPEncoder`). Encoding uses
//! SVT-AV1 under the hood.

use std::io::Write;
use std::ptr;

use image::error::{EncodingError, ImageFormatHint, UnsupportedError, UnsupportedErrorKind};
use image::{ExtendedColorType, ImageError, ImageEncoder, ImageResult};

use crate::error::AvifError;
use crate::ffi;
use crate::info::BitDepth;
use crate::sys;

/// Tunable parameters for the SVT-AV1 encoder.
///
/// Field names, ranges, and defaults mirror libavif / the `avifenc` CLI.
pub struct EncoderConfig {
    /// Encoder speed, range 0–10 (slower = better quality per byte); default 6.
    /// Maps to `avifEncoder.speed`.
    pub speed: u8,
    /// Color quality, range 0–100 (higher = better); default 60.
    /// Maps to `avifEncoder.quality`.
    pub quality: u8,
    /// Alpha quality, range 0–100 (higher = better); default 60.
    /// Maps to `avifEncoder.qualityAlpha`.
    pub quality_alpha: u8,
    /// Worker threads; `None` = auto-detect. Maps to `avifEncoder.maxThreads`.
    pub threads: Option<u32>,
    /// Output bit depth, default [`BitDepth::Eight`].
    ///
    /// Chroma subsampling is fixed at 4:2:0: the bundled SVT-AV1 (v3.1.2) rejects
    /// 4:2:2 / 4:4:4 with "Only support 420 now", so no subsampling knob is exposed.
    pub bit_depth: BitDepth,
    /// Tile columns, default 0 (auto). Maps to `avifEncoder.tileColsLog2` / `autoTiling`.
    pub tile_columns: u8,
    /// Tile rows, default 0 (auto). Maps to `avifEncoder.tileRowsLog2` / `autoTiling`.
    pub tile_rows: u8,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            speed: 6,
            quality: 60,
            quality_alpha: 60,
            threads: None,
            bit_depth: BitDepth::Eight,
            tile_columns: 0,
            tile_rows: 0,
        }
    }
}

/// AVIF encoder writing to `W`, using SVT-AV1.
///
/// # Example
/// ```no_run
/// use avif_rust::AvifEncoder;
/// use image::ImageEncoder;
///
/// let img = image::open("photo.png")?;
/// let mut buf = Vec::new();
/// img.write_with_encoder(AvifEncoder::new(&mut buf))?;
/// # Ok::<(), image::ImageError>(())
/// ```
pub struct AvifEncoder<W: Write> {
    writer: W,
    config: EncoderConfig,
}

impl<W: Write> AvifEncoder<W> {
    /// Create an encoder writing to `w` with default settings.
    pub fn new(w: W) -> Self {
        Self {
            writer: w,
            config: EncoderConfig::default(),
        }
    }

    /// Create an encoder writing to `w` with an explicit configuration.
    pub fn new_with_config(w: W, config: EncoderConfig) -> Self {
        Self { writer: w, config }
    }

    pub fn with_speed(mut self, speed: u8) -> Self {
        self.config.speed = speed;
        self
    }

    pub fn with_quality(mut self, quality: u8) -> Self {
        self.config.quality = quality;
        self
    }

    pub fn with_quality_alpha(mut self, quality_alpha: u8) -> Self {
        self.config.quality_alpha = quality_alpha;
        self
    }

    pub fn with_threads(mut self, threads: u32) -> Self {
        self.config.threads = Some(threads);
        self
    }

    pub fn with_bit_depth(mut self, bit_depth: BitDepth) -> Self {
        self.config.bit_depth = bit_depth;
        self
    }
}

/// How an input pixel buffer maps onto the RGB image libavif consumes.
struct Layout {
    /// Channels per pixel in the *input* `buf` (1=L, 2=La, 3=Rgb, 4=Rgba).
    src_channels: usize,
    /// Channels in the RGB buffer handed to libavif (3=RGB, 4=RGBA).
    rgb_channels: usize,
    /// Bytes per sample (1 for 8-bit, 2 for 16-bit, native-endian).
    sample_bytes: usize,
    rgb_format: sys::avifRGBFormat,
    /// Depth of the input RGB samples (8 or 16); libavif scales to the image depth.
    rgb_depth: u32,
    /// Whether the input is grayscale and must be expanded to RGB/RGBA.
    gray: bool,
    /// Whether the input carries an alpha channel.
    alpha: bool,
}

/// Maps a supported [`ExtendedColorType`] to its [`Layout`], or `None` if unsupported.
fn layout_for(color_type: ExtendedColorType) -> Option<Layout> {
    use ExtendedColorType as E;
    let rgb = sys::avifRGBFormat_AVIF_RGB_FORMAT_RGB;
    let rgba = sys::avifRGBFormat_AVIF_RGB_FORMAT_RGBA;
    let l = |src, rgb_ch, sb, fmt, depth, gray, alpha| {
        Some(Layout {
            src_channels: src,
            rgb_channels: rgb_ch,
            sample_bytes: sb,
            rgb_format: fmt,
            rgb_depth: depth,
            gray,
            alpha,
        })
    };
    match color_type {
        E::L8 => l(1, 3, 1, rgb, 8, true, false),
        E::La8 => l(2, 4, 1, rgba, 8, true, true),
        E::Rgb8 => l(3, 3, 1, rgb, 8, false, false),
        E::Rgba8 => l(4, 4, 1, rgba, 8, false, true),
        E::L16 => l(1, 3, 2, rgb, 16, true, false),
        E::La16 => l(2, 4, 2, rgba, 16, true, true),
        E::Rgb16 => l(3, 3, 2, rgb, 16, false, false),
        E::Rgba16 => l(4, 4, 2, rgba, 16, false, true),
        _ => None,
    }
}

/// Expands a grayscale buffer (L or La) into RGB/RGBA by replicating luma into the
/// three color channels, preserving native-endian samples of `sample_bytes` each.
fn expand_gray(buf: &[u8], sample_bytes: usize, alpha: bool) -> Vec<u8> {
    let in_ch = if alpha { 2 } else { 1 };
    let out_ch = if alpha { 4 } else { 3 };
    let pixels = buf.len() / (in_ch * sample_bytes);
    let mut out = Vec::with_capacity(pixels * out_ch * sample_bytes);
    for i in 0..pixels {
        let base = i * in_ch * sample_bytes;
        let luma = &buf[base..base + sample_bytes];
        out.extend_from_slice(luma);
        out.extend_from_slice(luma);
        out.extend_from_slice(luma);
        if alpha {
            out.extend_from_slice(&buf[base + sample_bytes..base + 2 * sample_bytes]);
        }
    }
    out
}

fn image_depth(bit_depth: BitDepth) -> u32 {
    match bit_depth {
        BitDepth::Eight => 8,
        BitDepth::Ten => 10,
        BitDepth::Twelve => 12,
    }
}

impl EncoderConfig {
    /// Runs the full libavif encode pipeline, returning the encoded AVIF bytes.
    fn encode(&self, buf: &[u8], width: u32, height: u32, color_type: ExtendedColorType) -> Result<Vec<u8>, EncodeError> {
        if width == 0 || height == 0 {
            return Err(EncodeError::Avif(AvifError::InvalidDimensions { width, height }));
        }
        let layout = layout_for(color_type).ok_or(EncodeError::Unsupported(color_type))?;

        let expected = width as usize * height as usize * layout.src_channels * layout.sample_bytes;
        if buf.len() != expected {
            return Err(EncodeError::Avif(AvifError::Encode(format!(
                "buffer length {} does not match {width}x{height} with {} channels of {} byte(s)",
                buf.len(),
                layout.src_channels,
                layout.sample_bytes,
            ))));
        }

        // Either borrow the caller's buffer directly or build an expanded grayscale copy.
        let expanded;
        let pixels: &[u8] = if layout.gray {
            expanded = expand_gray(buf, layout.sample_bytes, layout.alpha);
            &expanded
        } else {
            buf
        };

        // SAFETY: every raw pointer below is checked for null and freed on all paths.
        // Chroma is fixed at 4:2:0 — the bundled SVT-AV1 supports nothing else.
        unsafe {
            let image = sys::avifImageCreate(
                width,
                height,
                image_depth(self.bit_depth),
                sys::avifPixelFormat_AVIF_PIXEL_FORMAT_YUV420,
            );
            if image.is_null() {
                return Err(EncodeError::Avif(AvifError::EncoderInit("avifImageCreate returned null".into())));
            }
            let result = self.encode_into(image, pixels, &layout, width);
            sys::avifImageDestroy(image);
            result
        }
    }

    /// Inner half of [`encode`](Self::encode): assumes `image` is a valid, owned
    /// `avifImage` (freed by the caller) and produces the encoded bytes.
    ///
    /// # Safety
    /// `image` must be a non-null pointer from `avifImageCreate`, and `pixels` must
    /// describe `width` columns laid out per `layout`.
    unsafe fn encode_into(&self, image: *mut sys::avifImage, pixels: &[u8], layout: &Layout, width: u32) -> Result<Vec<u8>, EncodeError> {
        // SAFETY: upheld by this function's contract (see `# Safety`); all libavif
        // handles are checked for null and freed before returning.
        unsafe {
            let mut rgb: sys::avifRGBImage = std::mem::zeroed();
            sys::avifRGBImageSetDefaults(&mut rgb, image);
            rgb.format = layout.rgb_format;
            rgb.depth = layout.rgb_depth;
            rgb.pixels = pixels.as_ptr() as *mut u8;
            rgb.rowBytes = width * (layout.rgb_channels * layout.sample_bytes) as u32;

            let res = sys::avifImageRGBToYUV(image, &rgb);
            if !ffi::is_ok(res) {
                return Err(EncodeError::Avif(AvifError::Encode(ffi::result_message(res))));
            }

            let encoder = sys::avifEncoderCreate();
            if encoder.is_null() {
                return Err(EncodeError::Avif(AvifError::EncoderInit("avifEncoderCreate returned null".into())));
            }

            (*encoder).codecChoice = sys::avifCodecChoice_AVIF_CODEC_CHOICE_SVT;
            (*encoder).speed = self.speed as i32;
            (*encoder).quality = self.quality as i32;
            (*encoder).qualityAlpha = self.quality_alpha as i32;
            (*encoder).maxThreads = self.threads.unwrap_or(0) as i32;
            if self.tile_columns == 0 && self.tile_rows == 0 {
                (*encoder).autoTiling = sys::AVIF_TRUE as sys::avifBool;
            } else {
                (*encoder).tileColsLog2 = self.tile_columns as i32;
                (*encoder).tileRowsLog2 = self.tile_rows as i32;
            }

            let mut output = sys::avifRWData {
                data: ptr::null_mut(),
                size: 0,
            };
            let res = sys::avifEncoderWrite(encoder, image, &mut output);
            let encoded = if ffi::is_ok(res) {
                Ok(std::slice::from_raw_parts(output.data, output.size).to_vec())
            } else {
                Err(EncodeError::Avif(AvifError::Encode(ffi::result_message(res))))
            };

            sys::avifRWDataFree(&mut output);
            sys::avifEncoderDestroy(encoder);
            encoded
        }
    }
}

/// Internal encode failure, distinguishing unsupported inputs (which become
/// `ImageError::Unsupported`) from libavif/runtime failures.
enum EncodeError {
    Unsupported(ExtendedColorType),
    Avif(AvifError),
}

impl From<EncodeError> for ImageError {
    fn from(err: EncodeError) -> Self {
        match err {
            EncodeError::Unsupported(color_type) => ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormatHint::Name("AVIF".into()),
                    UnsupportedErrorKind::Color(color_type),
                ),
            ),
            EncodeError::Avif(e) => {
                ImageError::Encoding(EncodingError::new(ImageFormatHint::Name("AVIF".into()), e))
            }
        }
    }
}

impl<W: Write> ImageEncoder for AvifEncoder<W> {
    fn write_image(
        mut self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        let encoded = self.config.encode(buf, width, height, color_type)?;
        self.writer.write_all(&encoded).map_err(ImageError::IoError)?;
        Ok(())
    }
}
