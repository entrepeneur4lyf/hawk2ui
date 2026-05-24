use hawk2ui_assets::{AssetBackend, AssetHash, AssetKind, AssetLimits};
use image::{ColorType, ImageEncoder};

#[test]
fn asset_backend_decodes_images_verifies_hashes_and_exports_render_records() {
    let bytes = png_1x1();
    let expected_hash = AssetHash::sha256_bytes(&bytes);
    let mut backend = AssetBackend::new(AssetLimits::default());

    let asset = backend
        .compile_image("hero", "assets/hero.png", &bytes, &expected_hash)
        .expect("image compiles");

    assert_eq!(asset.kind(), AssetKind::Image);
    assert_eq!(asset.width(), Some(1));
    assert_eq!(asset.height(), Some(1));
    assert!(asset.metadata_stripped());
    assert_eq!(asset.hash(), expected_hash.as_str());
    assert_eq!(asset.cache_generation(), 1);
    assert_eq!(asset.to_render_asset().id(), "hero");
}

#[test]
fn asset_backend_rejects_oversized_assets_and_invalid_hashes() {
    let bytes = png_1x1();
    let mut backend = AssetBackend::new(AssetLimits::default().with_max_bytes(8));

    let size_error = backend
        .compile_image(
            "hero",
            "assets/hero.png",
            &bytes,
            &AssetHash::sha256_bytes(&bytes),
        )
        .expect_err("oversized image must fail");
    assert_eq!(size_error.diagnostic().rule(), "asset.limit.bytes-exceeded");

    let mut backend = AssetBackend::new(AssetLimits::default());
    let hash_error = backend
        .compile_image(
            "hero",
            "assets/hero.png",
            &bytes,
            &AssetHash::new("sha256:bad"),
        )
        .expect_err("invalid hash must fail");
    assert_eq!(hash_error.diagnostic().rule(), "asset.hash.mismatch");
}

#[test]
fn asset_backend_validates_and_lowers_vectors() {
    let safe_svg = br#"<svg viewBox="0 0 10 10"><path d="M0 0L10 10"/></svg>"#;
    let unsafe_svg = br#"<svg><script>alert(1)</script></svg>"#;
    let mut backend = AssetBackend::new(AssetLimits::default());

    let vector = backend
        .compile_vector(
            "logo",
            "assets/logo.svg",
            safe_svg,
            &AssetHash::sha256_bytes(safe_svg),
        )
        .expect("safe vector compiles");

    assert_eq!(vector.kind(), AssetKind::Vector);
    assert!(vector.sanitized());
    assert_eq!(vector.vector_lowering().unwrap().path_count(), 1);

    let error = backend
        .compile_vector(
            "bad",
            "assets/bad.svg",
            unsafe_svg,
            &AssetHash::sha256_bytes(unsafe_svg),
        )
        .expect_err("unsafe vector must fail");
    assert_eq!(error.diagnostic().rule(), "asset.vector.unsafe-content");
}

#[test]
fn asset_backend_loads_fonts_and_tracks_cache_invalidation() {
    let mut backend = AssetBackend::new(AssetLimits::default());
    let first = backend
        .load_font(
            "display",
            "assets/display.ttf",
            b"font-v1",
            &AssetHash::sha256_bytes(b"font-v1"),
        )
        .expect("font loads");
    let second = backend
        .load_font(
            "display",
            "assets/display.ttf",
            b"font-v2",
            &AssetHash::sha256_bytes(b"font-v2"),
        )
        .expect("font reload invalidates cache");

    assert_eq!(first.kind(), AssetKind::Font);
    assert!(second.cache_generation() > first.cache_generation());
    assert_eq!(backend.manifest().assets().len(), 1);
    assert_eq!(
        backend.manifest().asset("display").unwrap().hash(),
        second.hash()
    );
}

fn png_1x1() -> Vec<u8> {
    let mut bytes = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder
        .write_image(&[255, 0, 0, 255], 1, 1, ColorType::Rgba8.into())
        .expect("test PNG encodes");
    bytes
}
