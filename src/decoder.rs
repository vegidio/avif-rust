//! AVIF decoder, mirroring the `image` crate's per-format decoder convention.
//!
//! [`AvifDecoder`] is generic over a [`Read`] source and implements [`ImageDecoder`],
//! so it slots into `DynamicImage::from_decoder` exactly like the codecs that ship
//! with the `image` crate (e.g. `JpegDecoder`, `PngDecoder`). Decoding uses dav1d
//! under the hood.

use std::io::Read;

use image::error::{DecodingError, ImageFormatHint};
use image::{ColorType, ImageError, ImageDecoder, ImageResult};

use crate::error::AvifError;
use crate::ffi;
use crate::info::BitDepth;
use crate::sys;

/// Tunable parameters for the dav1d decoder.
#[derive(Default)]
pub struct DecoderConfig {
    /// Worker threads; `None` = auto-detect.
    pub threads: Option<u32>,
}

/// AVIF decoder reading from `R`, using dav1d.
///
/// The container header is parsed eagerly in [`new`](AvifDecoder::new) so that
/// [`dimensions`](ImageDecoder::dimensions), [`color_type`](ImageDecoder::color_type),
/// and [`bit_depth`](AvifDecoder::bit_depth) are available before the frame is decoded.
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
    /// Raw libavif decoder; destroyed in `Drop`.
    decoder: *mut sys::avifDecoder,
    /// Owned compressed bytes. libavif's memory IO references this buffer without
    /// copying, so it must outlive `decoder`. Never moved out.
    _data: Vec<u8>,
    config: DecoderConfig,
    width: u32,
    height: u32,
    depth: u32,
    alpha_present: bool,
    /// Marker to keep the `R` type parameter; the reader is fully drained in `new`.
    _reader: std::marker::PhantomData<R>,
}

impl<R: Read> AvifDecoder<R> {
    /// Create a decoder from `r`, reading the container header eagerly so that
    /// [`dimensions`](ImageDecoder::dimensions) and [`color_type`](ImageDecoder::color_type)
    /// are available before the frame is decoded.
    pub fn new(mut r: R) -> ImageResult<Self> {
        let mut data = Vec::new();
        r.read_to_end(&mut data).map_err(ImageError::IoError)?;

        // SAFETY: pointers are checked; the decoder is destroyed on every error path
        // and in `Drop`. `data` outlives `decoder` (stored alongside it below).
        unsafe {
            let decoder = sys::avifDecoderCreate();
            if decoder.is_null() {
                return Err(to_image_error(AvifError::DecoderInit("avifDecoderCreate returned null".into())));
            }
            (*decoder).codecChoice = sys::avifCodecChoice_AVIF_CODEC_CHOICE_DAV1D;

            let res = sys::avifDecoderSetIOMemory(decoder, data.as_ptr(), data.len());
            if !ffi::is_ok(res) {
                sys::avifDecoderDestroy(decoder);
                return Err(to_image_error(AvifError::DecoderInit(ffi::result_message(res))));
            }

            let res = sys::avifDecoderParse(decoder);
            if !ffi::is_ok(res) {
                sys::avifDecoderDestroy(decoder);
                return Err(to_image_error(AvifError::Decode(ffi::result_message(res))));
            }

            let image = (*decoder).image;
            Ok(Self {
                decoder,
                width: (*image).width,
                height: (*image).height,
                depth: (*image).depth,
                alpha_present: (*decoder).alphaPresent == sys::AVIF_TRUE as sys::avifBool,
                _data: data,
                config: DecoderConfig::default(),
                _reader: std::marker::PhantomData,
            })
        }
    }

    /// Set the number of decode worker threads (applied when the frame is decoded).
    pub fn with_threads(mut self, threads: u32) -> Self {
        self.config.threads = Some(threads);
        self
    }

    /// Bit depth of the image — extra information that [`ColorType`] cannot express
    /// (it only distinguishes 8- vs 16-bit).
    pub fn bit_depth(&self) -> BitDepth {
        match self.depth {
            12 => BitDepth::Twelve,
            10 => BitDepth::Ten,
            _ => BitDepth::Eight,
        }
    }

    /// Channels in the decoded RGB output (3 without alpha, 4 with).
    fn channels(&self) -> usize {
        if self.alpha_present { 4 } else { 3 }
    }

    /// Bytes per output sample (1 for 8-bit, 2 for >8-bit).
    fn sample_bytes(&self) -> usize {
        if self.depth > 8 { 2 } else { 1 }
    }
}

impl<R: Read> ImageDecoder for AvifDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn color_type(&self) -> ColorType {
        match (self.depth > 8, self.alpha_present) {
            (false, false) => ColorType::Rgb8,
            (false, true) => ColorType::Rgba8,
            (true, false) => ColorType::Rgb16,
            (true, true) => ColorType::Rgba16,
        }
    }

    fn read_image(self, buf: &mut [u8]) -> ImageResult<()> {
        let expected = self.width as usize * self.height as usize * self.channels() * self.sample_bytes();
        if buf.len() != expected {
            return Err(to_image_error(AvifError::Decode(format!(
                "output buffer length {} does not match expected {expected}",
                buf.len()
            ))));
        }

        // SAFETY: `self.decoder` is a valid decoder created and parsed in `new`.
        unsafe {
            (*self.decoder).maxThreads = self.config.threads.unwrap_or(0) as i32;

            let res = sys::avifDecoderNextImage(self.decoder);
            if !ffi::is_ok(res) {
                return Err(to_image_error(AvifError::Decode(ffi::result_message(res))));
            }

            let image = (*self.decoder).image;
            let mut rgb: sys::avifRGBImage = std::mem::zeroed();
            sys::avifRGBImageSetDefaults(&mut rgb, image);
            rgb.format = if self.alpha_present {
                sys::avifRGBFormat_AVIF_RGB_FORMAT_RGBA
            } else {
                sys::avifRGBFormat_AVIF_RGB_FORMAT_RGB
            };
            rgb.depth = if self.depth > 8 { 16 } else { 8 };
            rgb.pixels = buf.as_mut_ptr();
            rgb.rowBytes = self.width * (self.channels() * self.sample_bytes()) as u32;

            let res = sys::avifImageYUVToRGB(image, &mut rgb);
            if !ffi::is_ok(res) {
                return Err(to_image_error(AvifError::Decode(ffi::result_message(res))));
            }
        }
        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        self.read_image(buf)
    }
}

impl<R: Read> Drop for AvifDecoder<R> {
    fn drop(&mut self) {
        // SAFETY: `decoder` was created in `new` and is not destroyed elsewhere.
        unsafe { sys::avifDecoderDestroy(self.decoder) };
    }
}

/// Wraps an [`AvifError`] as an `image` decoding error.
fn to_image_error(err: AvifError) -> ImageError {
    ImageError::Decoding(DecodingError::new(ImageFormatHint::Name("AVIF".into()), err))
}
