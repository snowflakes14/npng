use std::{collections::HashMap, fs, path::Path};

use image::ImageReader;

extern crate npng_crate;

use npng_crate::{CompressMap, types::*, *};

fn require_in_png() {
    let p = Path::new("in.png");
    println!("in.png!");
    assert!(p.exists(), "in.png not found");
}

fn get_test_configs() -> Vec<Config> {
    vec![
        Config {
            save_alpha: true,
            varint: true,
        },
        Config {
            save_alpha: true,
            varint: false,
        },
        Config {
            save_alpha: false,
            varint: true,
        },
        Config {
            save_alpha: false,
            varint: false,
        },
    ]
}

#[test]
fn test_encode_image_to_npng_image_with_configs() {
    require_in_png();
    let metadata = Metadata::new("TEST", HashMap::new());
    let out_path = "out.npng";

    let compress_maps = vec![
        CompressMap::plain(),
        CompressMap::zlib(3),
        CompressMap::zstd(1),
    ];

    for (i, config) in get_test_configs().iter().enumerate() {
        println!(
            "==> Config {}: save_alpha={}, varint={}",
            i, config.save_alpha, config.varint
        );

        for (j, cmap) in compress_maps.iter().enumerate() {
            println!("  -- Using CompressMap #{}: encoder={}", j, cmap.encoder());

            if Path::new(out_path).exists() {
                let _ = fs::remove_file(out_path);
            }

            println!("    -> Encoding image...");
            encode_image_to_npng_image(
                "in.png",
                out_path,
                metadata.clone(),
                true,
                config.clone(),
                cmap.clone(),
            )
            .expect("encode_image_to_npng_image failed");

            let md = fs::metadata(out_path).expect("cannot read out.npng");
            println!("    <- Saved image, size={} bytes", md.len());
            assert!(md.len() > 0, "out.npng is empty");

            let _ = fs::remove_file(out_path);
        }
    }
}

#[test]
fn test_encode_bytes_and_decode_bytes_roundtrip_with_configs() {
    require_in_png();
    let reader = ImageReader::open("in.png")
        .expect("cannot open in.png")
        .with_guessed_format()
        .expect("cannot guess format");
    let _img = reader.decode().expect("cannot decode in.png");

    let metadata = Metadata::new("TEST", HashMap::new());

    let compress_maps = vec![
        CompressMap::plain(),
        CompressMap::zlib(3),
        CompressMap::zstd(1),
    ];

    for (i, config) in get_test_configs().iter().enumerate() {
        println!(
            "==> Config {}: save_alpha={}, varint={}",
            i, config.save_alpha, config.varint
        );

        for (j, cmap) in compress_maps.iter().enumerate() {
            println!("  -- Using CompressMap #{}: encoder={}", j, cmap.encoder());

            println!("    -> Encoding to bytes...");
            let bytes = encode_image_to_npng_bytes(
                "in.png",
                metadata.clone(),
                config.clone(),
                cmap.clone(),
            )
            .expect("encode_image_to_npng_bytes failed");

            assert!(!bytes.is_empty(), "Encoded bytes are empty");
            println!("    <- Bytes encoded, length={}", bytes.len());

            let out_decoded = "decoded.png";
            if Path::new(out_decoded).exists() {
                let _ = fs::remove_file(out_decoded);
            }

            println!("    -> Decoding bytes to image...");
            let (version, decoded_meta) =
                decode_bytes_to_image(&bytes, out_decoded, false, cmap.clone())
                    .expect("decode_bytes_to_image failed");

            let md = fs::metadata(out_decoded).expect("cannot read decoded.png");
            println!("    <- Decoded image saved, size={} bytes", md.len());
            assert!(md.len() > 0, "decoded.png is empty");

            let _ = fs::remove_file(out_decoded);
        }
    }
}

#[test]
fn test_coordinates_duplicates() {
    let pixels = vec![Pixel::new(1, 1, 0), Pixel::new(1, 1, 0)];

    let r = encode_pixel_vec_with_metadata(
        pixels,
        Metadata::new("TEST", HashMap::new()),
        Config::default(),
        Encoding::Plain,
    );
    assert!(matches!(r, Err(NPNGError::DuplicatePixel(_, _))));
    println!(
        "test_coordinates_duplicates: {:?}",
        r.err().unwrap().to_string()
    );
}
