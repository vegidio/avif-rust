//! AVIF encoder, mirroring the `image` crate's per-format encoder convention.
//!
//! [`AvifEncoder`] is generic over a [`Write`] sink and implements [`ImageEncoder`],
//! so it slots into `DynamicImage::write_with_encoder` exactly like the codecs that
//! ship with the `image` crate (e.g. `JpegEncoder`, `WebPEncoder`). Encoding uses
//! SVT-AV1 under the hood.

use std::io::Write;

use image::{ExtendedColorType, ImageEncoder, ImageResult};

use crate::info::BitDepth;

/// Tunable parameters for the SVT-AV1 encoder.
pub struct EncoderConfig {
    /// SVT-AV1 speed preset 0–13, default 8.
    pub preset: u8,
    /// Constant Rate Factor 0–63, default 35.
    pub crf: u8,
    /// Worker threads; `None` = auto-detect.
    pub threads: Option<u32>,
    /// Output bit depth, default [`BitDepth::Eight`].
    pub bit_depth: BitDepth,
    /// Tile columns, default 0 (auto).
    pub tile_columns: u8,
    /// Tile rows, default 0 (auto).
    pub tile_rows: u8,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            preset: 8,
            crf: 35,
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

    pub fn with_preset(mut self, preset: u8) -> Self {
        self.config.preset = preset;
        self
    }

    pub fn with_crf(mut self, crf: u8) -> Self {
        self.config.crf = crf;
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

impl<W: Write> ImageEncoder for AvifEncoder<W> {
    fn write_image(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        let _ = (self.writer, self.config, buf, width, height, color_type);
        todo!()
    }
}
