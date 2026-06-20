//! Shared helpers for the integration tests. Each test file is its own crate, so they
//! pull this in with `mod common;`. Not every file uses every helper.
#![allow(dead_code)]

use image::DynamicImage;

pub const JPG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/image.jpg");
pub const AVIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/image.avif");

/// Loads the sample JPEG used as an encode source.
pub fn source_image() -> DynamicImage {
    image::open(JPG).expect("load assets/image.jpg")
}

/// True when `bytes` begins with an ISO-BMFF `ftyp` box advertising `avif`.
pub fn is_avif(bytes: &[u8]) -> bool {
    bytes.len() > 12 && &bytes[4..8] == b"ftyp" && &bytes[8..12] == b"avif"
}
