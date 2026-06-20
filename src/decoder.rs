//! AVIF decoder, mirroring the `image` crate's per-format decoder convention.
//!
//! [`AvifDecoder`] is generic over a [`Read`] source and implements [`ImageDecoder`],
//! so it slots into `DynamicImage::from_decoder` exactly like the codecs that ship
//! with the `image` crate (e.g. `JpegDecoder`, `PngDecoder`). Decoding uses dav1d
//! under the hood.

use std::io::Read;

use image::{ColorType, ImageDecoder, ImageResult};

use crate::info::BitDepth;

/// Tunable parameters for the dav1d decoder.
#[derive(Default)]
pub struct DecoderConfig {
    /// Worker threads; `None` = auto-detect.
    pub threads: Option<u32>,
}

/// AVIF decoder reading from `R`, using dav1d.
///
/// # Example
/// ```no_run
/// use avif_rust::AvifDecoder;
/// use image::DynamicImage;
/// use std::io::Cursor;
///
/// # let bytes: Vec<u8> = Vec::new();
/// let decoder = AvifDecoder::new(Cursor::new(&bytes))?;
/// let img = DynamicImage::from_decoder(decoder)?;
/// # Ok::<(), image::ImageError>(())
/// ```
pub struct AvifDecoder<R: Read> {
    reader: R,
    config: DecoderConfig,
}

impl<R: Read> AvifDecoder<R> {
    /// Create a decoder from `r`, reading the container header eagerly so that
    /// [`dimensions`](ImageDecoder::dimensions) and [`color_type`](ImageDecoder::color_type)
    /// are available before the frame is decoded.
    pub fn new(r: R) -> ImageResult<Self> {
        Ok(Self {
            reader: r,
            config: DecoderConfig::default(),
        })
    }

    /// Set the number of decode worker threads (applied when the frame is decoded).
    pub fn with_threads(mut self, threads: u32) -> Self {
        self.config.threads = Some(threads);
        self
    }

    /// Bit depth of the image — extra information that [`ColorType`] cannot express
    /// (it only distinguishes 8- vs 16-bit).
    pub fn bit_depth(&self) -> BitDepth {
        todo!()
    }
}

impl<R: Read> ImageDecoder for AvifDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        todo!()
    }

    fn color_type(&self) -> ColorType {
        todo!()
    }

    fn read_image(self, buf: &mut [u8]) -> ImageResult<()> {
        let _ = (self.reader, self.config, buf);
        todo!()
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        self.read_image(buf)
    }
}
