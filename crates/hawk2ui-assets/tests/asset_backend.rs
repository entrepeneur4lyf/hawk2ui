use hawk2ui_api::{Diagnostic, DiagnosticSeverity};
use hawk2ui_assets::{AssetBackend, AssetBackendError, AssetHash, AssetKind, AssetLimits};
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
fn asset_backend_rejects_pixel_limits_before_accepting_decoded_images() {
    let bytes = png_rgba(2, 2, [255, 0, 0, 255]);
    let mut backend = AssetBackend::new(AssetLimits::default().with_max_pixels(1));

    let error = backend
        .compile_image(
            "hero",
            "assets/hero.png",
            &bytes,
            &AssetHash::sha256_bytes(&bytes),
        )
        .expect_err("image above pixel limit must fail");

    assert_eq!(error.diagnostic().rule(), "asset.limit.pixels-exceeded");
}

#[test]
fn asset_backend_validates_and_lowers_vectors() {
    let safe_svg = br#"<svg viewBox="0 0 10 10"><path d="M0 0L10 10"/></svg>"#;
    let unsafe_svg = br"<svg><script>alert(1)</script></svg>";
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
    let lowered_svg =
        std::str::from_utf8(vector.compiled_bytes()).expect("lowered vector remains UTF-8 SVG");
    assert!(lowered_svg.starts_with("<svg"));
    assert!(lowered_svg.contains("<path"));
    assert!(!lowered_svg.contains("viewBox=\"0 0 10 10\""));
    assert!(!lowered_svg.contains("<script"));

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
fn asset_backend_allows_internal_svg_refs_and_rejects_external_svg_refs() {
    let gradient_svg = br##"<svg viewBox="0 0 10 10">
        <defs>
            <linearGradient id="grad"><stop offset="0%" stop-color="red"/></linearGradient>
            <path id="shape" d="M0 0L10 10"/>
        </defs>
        <rect width="10" height="10" fill="url(#grad)"/>
        <use href="#shape"/>
    </svg>"##;
    let external_svg = br#"<svg><use href="/etc/passwd"/></svg>"#;
    let mut backend = AssetBackend::new(AssetLimits::default());

    let vector = backend
        .compile_vector(
            "gradient",
            "assets/gradient.svg",
            gradient_svg,
            &AssetHash::sha256_bytes(gradient_svg),
        )
        .expect("internal SVG references compile");
    let external = backend
        .compile_vector(
            "external",
            "assets/external.svg",
            external_svg,
            &AssetHash::sha256_bytes(external_svg),
        )
        .expect_err("external SVG references must fail");

    assert_eq!(vector.kind(), AssetKind::Vector);
    assert_eq!(
        external.diagnostic().rule(),
        "asset.vector.external-reference"
    );
}

#[test]
fn asset_backend_rejects_invalid_fonts_and_loads_parseable_fonts_when_available() {
    let mut backend = AssetBackend::new(AssetLimits::default());
    let invalid = backend
        .load_font(
            "display",
            "assets/display.ttf",
            b"not-a-font",
            &AssetHash::sha256_bytes(b"not-a-font"),
        )
        .expect_err("invalid font bytes must fail");
    assert_eq!(invalid.diagnostic().rule(), "asset.font.parse-failed");

    let Some(font_bytes) = fixture_font_bytes() else {
        return;
    };
    let font = backend
        .load_font(
            "display",
            "assets/display.ttf",
            &font_bytes,
            &AssetHash::sha256_bytes(&font_bytes),
        )
        .expect("parseable font loads");

    assert_eq!(font.kind(), AssetKind::Font);
    assert!(!font.sanitized());
    assert_eq!(backend.manifest().assets().len(), 1);
    assert_eq!(
        backend.manifest().asset("display").unwrap().hash(),
        font.hash()
    );
}

#[test]
fn asset_backend_cache_generation_changes_only_when_compiled_payload_changes() {
    let first_bytes = png_rgba(1, 1, [255, 0, 0, 255]);
    let second_bytes = png_rgba(1, 1, [0, 255, 0, 255]);
    let mut backend = AssetBackend::new(AssetLimits::default());
    let first = backend
        .compile_image(
            "hero",
            "assets/hero.png",
            &first_bytes,
            &AssetHash::sha256_bytes(&first_bytes),
        )
        .expect("first image compiles");
    let repeated = backend
        .compile_image(
            "hero",
            "assets/hero.png",
            &first_bytes,
            &AssetHash::sha256_bytes(&first_bytes),
        )
        .expect("identical image compile is stable");
    let changed = backend
        .compile_image(
            "hero",
            "assets/hero.png",
            &second_bytes,
            &AssetHash::sha256_bytes(&second_bytes),
        )
        .expect("changed image compiles");

    assert_eq!(first.cache_generation(), 1);
    assert_eq!(repeated.cache_generation(), first.cache_generation());
    assert!(changed.cache_generation() > repeated.cache_generation());
}

#[test]
fn asset_backend_error_converts_to_shared_diagnostic() {
    let error = AssetBackendError::new("asset.test", "asset failed");
    let diagnostic = Diagnostic::from(error);

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.rule.as_str(), "asset.test");
    assert_eq!(diagnostic.message, "asset failed");
}

fn png_1x1() -> Vec<u8> {
    png_rgba(1, 1, [255, 0, 0, 255])
}

fn png_rgba(width: u32, height: u32, pixel: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut pixels = Vec::new();
    for _ in 0..(width * height) {
        pixels.extend_from_slice(&pixel);
    }
    let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder
        .write_image(&pixels, width, height, ColorType::Rgba8.into())
        .expect("test PNG encodes");
    bytes
}

fn fixture_font_bytes() -> Option<Vec<u8>> {
    for path in [
        "/usr/share/fonts/truetype/jsmath/jsMath-lasy10.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansLycian-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            return Some(bytes);
        }
    }
    None
}
