//! Decoder- and probe-focused integration tests against the bundled AVIF asset.

mod common;

use common::AVIF;

#[test]
fn probe_reads_header_only() {
    let bytes = std::fs::read(AVIF).expect("read assets/image.avif");
    let info = avif::probe(&bytes).expect("probe");

    assert!(info.width > 0 && info.height > 0);
    // The sample asset is an 8-bit AVIF.
    assert_eq!(info.bit_depth, avif::BitDepth::Eight);
}

#[test]
fn decodes_bundled_avif_asset() {
    let bytes = std::fs::read(AVIF).expect("read assets/image.avif");
    let img = avif::decode(&bytes).expect("decode bundled asset");
    assert!(img.width() > 0 && img.height() > 0);
}
