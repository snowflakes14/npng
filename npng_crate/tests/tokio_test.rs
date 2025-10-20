extern crate npng_crate;
extern crate __tk_rt_private as tokio;
use npng_crate::*;
use npng_crate::tokio::*;
use std::{collections::HashMap, fs, path::Path};
use image::ImageReader;
use npng_crate::compress::CompressMap;
use npng_crate::types::Metadata;

fn require_in_png() {
    let p = Path::new("in.png");
    assert!(p.exists(), "in.png not found");
}

fn get_test_configs() -> Vec<Config> {
    vec![
        Config { save_alpha: true, varint: true },
        Config { save_alpha: true, varint: false },
        Config { save_alpha: false, varint: true },
        Config { save_alpha: false, varint: false },
    ]
}

#[tokio::test]
async fn test_encode_image_to_npng_image_with_configs_tokio() {
    require_in_png();
    let metadata = Metadata::new_str("TEST", HashMap::new());
    let out_path = "out.npng";

    let compress_maps = vec![
        CompressMap::plain(),
        CompressMap::zlib(3),
        CompressMap::zstd(1),
    ];

    for config in get_test_configs() {
        for cmap in &compress_maps {
            if Path::new(out_path).exists() {
                let _ = fs::remove_file(out_path);
            }

            encode_image_to_npng_image_tokio(
                "in.png",
                out_path,
                metadata.clone(),
                true,
                config.clone(),
                cmap.clone(),
            )
                .await
                .expect("encoding failed");

            let md = fs::metadata(out_path).expect("cannot read out.npng");
            assert!(md.len() > 0, "out.npng is empty");

            let _ = fs::remove_file(out_path);
        }
    }
}

#[tokio::test]
async fn test_encode_bytes_and_decode_bytes_roundtrip_with_configs_tokio() {
    require_in_png();
    let metadata = Metadata::new_str("TEST", HashMap::new());
    let compress_maps = vec![
        CompressMap::plain(),
        CompressMap::zlib(3),
        CompressMap::zstd(1),
    ];

    for config in get_test_configs() {
        for cmap in &compress_maps {
            // encode to bytes
            let bytes = encode_image_to_npng_bytes_tokio(
                "in.png",
                metadata.clone(),
                config.clone(),
                cmap.clone(),
            )
                .await
                .expect("encoding failed")
                .expect("encoding failed");

            assert!(!bytes.is_empty(), "encoded bytes empty");

            let out_decoded = "decoded.png";
            if Path::new(out_decoded).exists() {
                let _ = fs::remove_file(out_decoded);
            }

            // decode back to image
            decode_bytes_to_image_tokio(bytes, out_decoded, false, cmap.clone())
                .await
                .expect("decoding failed");

            let md = fs::metadata(out_decoded).expect("cannot read decoded.png");
            assert!(md.len() > 0, "decoded.png is empty");

            let _ = fs::remove_file(out_decoded);
        }
    }
}
