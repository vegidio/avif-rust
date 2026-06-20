//! End-to-end encode/decode tests against the images in `assets/`.

use image::{DynamicImage, GenericImageView};

const JPG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/image.jpg");
const AVIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/image.avif");

fn source_image() -> DynamicImage {
    image::open(JPG).expect("load assets/image.jpg")
}

/// The encoded stream starts with an ISO-BMFF `ftyp` box advertising `avif`.
fn is_avif(bytes: &[u8]) -> bool {
    bytes.len() > 12 && &bytes[4..8] == b"ftyp" && &bytes[8..12] == b"avif"
}

#[test]
fn encode_produces_valid_avif() {
    let img = source_image();
    let bytes = avif_rust::encode(&img).expect("encode");
    assert!(!bytes.is_empty(), "encoded output should not be empty");
    assert!(is_avif(&bytes), "output should be a valid AVIF stream");
}

#[test]
fn roundtrip_preserves_dimensions() {
    let img = source_image();
    let (w, h) = (img.width(), img.height());

    let bytes = avif_rust::encode(&img).expect("encode");
    let decoded = avif_rust::decode(&bytes).expect("decode");

    assert_eq!(decoded.dimensions(), (w, h));
}

#[test]
fn probe_reads_header_only() {
    let bytes = std::fs::read(AVIF).expect("read assets/image.avif");
    let info = avif_rust::probe(&bytes).expect("probe");

    assert!(info.width > 0 && info.height > 0);
    // The sample asset is an 8-bit AVIF.
    assert_eq!(info.bit_depth, avif_rust::BitDepth::Eight);
}

#[test]
fn decodes_bundled_avif_asset() {
    let bytes = std::fs::read(AVIF).expect("read assets/image.avif");
    let img = avif_rust::decode(&bytes).expect("decode bundled asset");
    assert!(img.width() > 0 && img.height() > 0);
}

/// Encode an image with explicit alpha so the RGBA path is exercised.
#[test]
fn roundtrip_rgba() {
    let img = DynamicImage::ImageRgba8(source_image().to_rgba8());
    let bytes = avif_rust::encode(&img).expect("encode rgba");
    let decoded = avif_rust::decode(&bytes).expect("decode rgba");
    assert_eq!(decoded.dimensions(), img.dimensions());
}

/// Grayscale input must be expanded to RGB (SVT-AV1 rejects 4:0:0 monochrome).
#[test]
fn roundtrip_grayscale() {
    let img = DynamicImage::ImageLuma8(source_image().to_luma8());
    let bytes = avif_rust::encode(&img).expect("encode grayscale");
    let decoded = avif_rust::decode(&bytes).expect("decode grayscale");
    assert_eq!(decoded.dimensions(), img.dimensions());
}
