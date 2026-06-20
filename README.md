# avif-rs

A Rust library to encode AVIF images using SVT-AV1 and decode using dav1d.

## ⬇️ Installation

This library can be installed using Cargo. To do that, run the following command in your project's root directory:

```bash
cargo add avif-rs
```

The crate links as `avif`, so you import it with `use avif;` regardless of the package name.

> [!NOTE]
> The first build downloads the prebuilt static binaries for your platform, so an internet connection is required (see [Troubleshooting](#-troubleshooting) for offline builds).

## 🤖 Usage

Here are some examples of how to encode and decode AVIF images using this library. These snippets don't have any error handling for the sake of simplicity, but you should always check for errors in production code.

#### Encoding

```rust
let img = image::open("/path/to/image.png").unwrap(); // an image to be encoded
let bytes = avif::encode(&img).unwrap(); // encode the image with default settings
std::fs::write("/path/to/image.avif", &bytes).unwrap(); // save the AVIF to a file
```

#### Encoding with custom settings

```rust
use avif::AvifEncoder;
use image::ImageEncoder;

let img = image::open("/path/to/image.png").unwrap();
let mut bytes = Vec::new();
img.write_with_encoder(
    AvifEncoder::new(&mut bytes)
        .with_quality(80)   // 0–100, higher = better quality
        .with_speed(4)      // 0–10, slower = better compression
        .with_threads(4),   // worker threads (omit for auto-detect)
).unwrap();
```

#### Decoding

```rust
let bytes = std::fs::read("/path/to/image.avif").unwrap(); // read the AVIF file
let img = avif::decode(&bytes).unwrap(); // decode it into a DynamicImage
img.save("/path/to/image.png").unwrap(); // save it in another format
```

#### Probing (header only)

Read the image dimensions and bit depth without decoding the pixels — useful for validation or thumbnailing pipelines:

```rust
let bytes = std::fs::read("/path/to/image.avif").unwrap();
let info = avif::probe(&bytes).unwrap();
println!("{}x{} @ {:?}", info.width, info.height, info.bit_depth);
```

The public API also exposes [`encode_buffer`] (encode a typed `ImageBuffer` directly), [`AvifEncoder`] / [`AvifDecoder`] for `image`-trait integration, [`EncoderConfig`] / [`DecoderConfig`] for full control, and [`libavif_version`].

#### Runnable examples

The [`examples/`](examples) directory has standalone programs covering each part of the API, runnable out of the box against the bundled assets:

```bash
cargo run --example encode          # encode with defaults
cargo run --example decode          # decode an AVIF to PNG
cargo run --example custom_encoder  # AvifEncoder builder (quality/speed/threads)
cargo run --example encode_buffer   # encode a typed ImageBuffer
cargo run --example probe           # read the header without decoding pixels
cargo run --example roundtrip       # encode then decode
cargo run --example high_bit_depth  # 10-bit encoding via EncoderConfig
cargo run --example parallel_encode # concurrent encoding (safe, uniform config)
cargo run --example version         # print the linked libavif version
```

## 💣 Troubleshooting

### Encoding in parallel with different configurations crashes

The bundled SVT-AV1 encoder keeps **global state that is set per-encode** (preset/speed, quality, threading). When two or more encodes run **concurrently in the same process with *different* configurations**, that shared state is corrupted and the process **segfaults**.

- **Safe:** parallel encoding where every concurrent encode uses the **same** configuration (e.g. all defaults). This was verified stable under heavy load — dozens of concurrent encodes, including RGBA.
- **Unsafe:** parallel encoding where threads use **different** settings at the same time (e.g. one thread at `speed 9 / quality 20` while another runs at the defaults).

If you need to encode with different settings across threads, serialize the encode calls behind a lock:

```rust
use std::sync::Mutex;

static ENCODE_LOCK: Mutex<()> = Mutex::new(());

let bytes = {
    let _guard = ENCODE_LOCK.lock().unwrap();
    avif::encode(&img).unwrap()
};
```

This is a limitation of the underlying SVT-AV1 C library, not of the wrapper. Decoding is unaffected.

### My build fails because it can't download the binaries

The first build fetches the prebuilt static libraries for your platform over the network. For offline or air-gapped builds, download the archive for your target from [binaries-avif](https://github.com/vegidio/binaries-avif/releases), extract it, and point the build at it with the `AVIF_BINARIES_DIR` environment variable:

```
$ AVIF_BINARIES_DIR=/path/to/extracted/libs cargo build
```

## 📝 License

**avif-rs** is released under the Apache 2.0 License. See [LICENSE](LICENSE) for details.

## 👨🏾‍💻 Author

Vinicius Egidio ([vinicius.io](http://vinicius.io))
