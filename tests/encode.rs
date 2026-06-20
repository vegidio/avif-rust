//! Encoder-focused integration tests.

mod common;

use common::{is_avif, source_image};

#[test]
fn encode_produces_valid_avif() {
    let img = source_image();
    let bytes = avif::encode(&img).expect("encode");
    assert!(!bytes.is_empty(), "encoded output should not be empty");
    assert!(is_avif(&bytes), "output should be a valid AVIF stream");
}
