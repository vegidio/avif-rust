//! Encoder configuration tests: non-default builder settings must still produce a
//! valid AVIF that decodes back to the original dimensions.
//!
//! The scenarios share a single `#[test]` so their encodes run sequentially. The
//! bundled SVT-AV1 (v3.1.2) segfaults when encodes with *different* configurations
//! run concurrently in one process (shared global encoder state); these scenarios use
//! different settings, so as separate parallel tests they crash. Parallel encoding with
//! a *uniform* config is fine — only the heterogeneous case is unsafe. Keeping these
//! serial sidesteps the library limitation.

mod common;

use common::{is_avif, source_image};

use avif::AvifEncoder;
use image::{DynamicImage, GenericImageView};

#[test]
fn encodes_with_custom_config() {
    let img = source_image();
    let (w, h) = (img.width(), img.height());

    // Custom quality + speed.
    let mut buf = Vec::new();
    img.write_with_encoder(AvifEncoder::new(&mut buf).with_quality(20).with_speed(9))
        .expect("encode with custom quality/speed");
    assert!(is_avif(&buf), "custom quality/speed output should be a valid AVIF stream");
    assert_eq!(avif::decode(&buf).expect("decode").dimensions(), (w, h));

    // Explicit single worker thread.
    let mut buf = Vec::new();
    img.write_with_encoder(AvifEncoder::new(&mut buf).with_threads(1))
        .expect("encode single-threaded");
    assert!(is_avif(&buf), "single-threaded output should be a valid AVIF stream");

    // RGBA with custom alpha quality (exercises the separate alpha-plane encoder).
    let rgba = DynamicImage::ImageRgba8(source_image().to_rgba8());
    let mut buf = Vec::new();
    rgba.write_with_encoder(AvifEncoder::new(&mut buf).with_quality_alpha(30))
        .expect("encode rgba with custom alpha quality");
    assert!(is_avif(&buf), "rgba output should be a valid AVIF stream");
    assert_eq!(avif::decode(&buf).expect("decode rgba").dimensions(), (w, h));
}
