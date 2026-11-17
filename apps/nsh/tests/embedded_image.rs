#![cfg(not(target_arch = "wasm32"))]

#[path = "../src/viewer/embedded_image.rs"]
mod embedded_image;

use nsh_core::platform::clipboard::data_uri;
use oshwhub_nsh::services;
use oshwhub_nsh::viewer::raster::{self, RasterPlan};
use resvg::tiny_skia::Pixmap;

const SVG: &str = concat!(
    "<svg xmlns='http://www.w3.org/2000/svg' width='64' height='48'>",
    "<rect width='64' height='48' fill='#00ff00'/>",
    "<text x='4' y='20' font-size='12' fill='white'>SVG中文</text>",
    "<image href='data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+ip1sAAAAASUVORK5CYII=' x='4' y='28' width='16' height='16'/>",
    "</svg>"
);

fn raster(svg: &str) -> Result<Pixmap, String> {
    let svg = embedded_image::Svg::parse(&data_uri("image/svg+xml", svg.as_bytes()))?.unwrap();
    let [width, height] = svg.size();
    let tile = svg
        .render(
            [1., 0., 0., 1., 0., 0.],
            [f64::from(width), f64::from(height)],
            RasterPlan {
                x: 0.,
                y: 0.,
                width: f64::from(width),
                height: f64::from(height),
                pixel_width: width * 4,
                pixel_height: height * 4,
            },
        )?
        .unwrap();
    let (mime, bytes) = nsh_core::image::decode_data_uri(&tile.uri)?;
    assert_eq!(mime, "image/png", "native canvases require a bitmap source");
    Pixmap::decode_png(&bytes).map_err(|error| error.to_string())
}

#[test]
fn zoom_rasterizes_only_the_visible_region_at_the_requested_density() {
    let svg = embedded_image::Svg::parse(&data_uri("image/svg+xml", b"<svg xmlns='http://www.w3.org/2000/svg' width='64' height='48'><rect width='64' height='48' fill='lime'/><path d='M32 0V48' stroke='white' stroke-width='.25'/></svg>")).unwrap().unwrap();
    let plan = RasterPlan {
        x: 31.,
        y: 0.,
        width: 2.,
        height: 48.,
        pixel_width: 128,
        pixel_height: 3072,
    };
    let tile = svg
        .render([1., 0., 0., 1., 0., 0.], [64., 48.], plan)
        .unwrap()
        .unwrap();
    let (_, bytes) = nsh_core::image::decode_data_uri(&tile.uri).unwrap();
    let pixels = Pixmap::decode_png(&bytes).unwrap();
    assert_eq!((pixels.width(), pixels.height()), (128, 3072));
    assert_eq!(
        pixels.pixel(64, 1500).unwrap().red(),
        255,
        "thin vector strokes must recover full coverage after zoom"
    );
    assert_eq!(tile.dest, [31., 0., 2., 48.]);
    assert!(
        svg.render([1., 0., 0., 1., 100., 0.], [64., 48.], plan)
            .unwrap()
            .is_none()
    );
}

#[test]
fn rotated_svg_tiles_preserve_image_transforms_and_clip_the_viewport() {
    let svg = embedded_image::Svg::parse(&data_uri("image/svg+xml", b"<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><rect x='-100' y='-100' width='210' height='210' fill='red'/></svg>")).unwrap().unwrap();
    // A 10x10 viewport becomes a diamond; pixels in its bounding-box corners stay transparent.
    let plan = RasterPlan {
        x: 0.,
        y: 0.,
        width: 20.,
        height: 20.,
        pixel_width: 80,
        pixel_height: 80,
    };
    let tile = svg
        .render([1., 1., -1., 1., 10., 0.], [10., 10.], plan)
        .unwrap()
        .unwrap();
    let (_, bytes) = nsh_core::image::decode_data_uri(&tile.uri).unwrap();
    let pixels = Pixmap::decode_png(&bytes).unwrap();
    assert_eq!(pixels.pixel(40, 40).unwrap().red(), 255);
    assert_eq!(pixels.pixel(1, 1).unwrap().alpha(), 0);
}

#[test]
fn drawing_replaces_svg_ops_without_changing_other_images() {
    use nsh_core::canvas::{CanvasImage, CanvasOp, Drawing, Num};
    use std::sync::Arc;
    let uri: Arc<str> = Arc::from(data_uri("image/svg+xml", SVG.as_bytes()));
    let svg = embedded_image::Svg::parse(&uri).unwrap().unwrap();
    let bitmap: Arc<str> = Arc::from("other bitmap");
    let drawing = Drawing {
        ops: vec![
            CanvasOp::Save,
            CanvasOp::Transform([1., 0., 0., 1., 10., 20.].map(Num::Const)),
            CanvasOp::DrawImage {
                image: CanvasImage {
                    uri: Arc::clone(&uri),
                },
                source: None,
                dest: [0., 0., 64., 48.].map(Num::Const),
            },
            CanvasOp::Restore,
            CanvasOp::DrawImage {
                image: CanvasImage {
                    uri: Arc::clone(&bitmap),
                },
                source: None,
                dest: [0., 0., 1., 1.].map(Num::Const),
            },
        ],
        ..Default::default()
    };
    let plan = RasterPlan {
        x: 20.,
        y: 30.,
        width: 10.,
        height: 10.,
        pixel_width: 40,
        pixel_height: 40,
    };
    let output = pollster::block_on(embedded_image::raster_drawing(
        &drawing,
        &[(uri, svg)],
        plan,
    ))
    .unwrap();
    assert_eq!(
        output.ops[1],
        CanvasOp::Transform([1., 0., 0., 1., 0., 0.].map(Num::Const))
    );
    let CanvasOp::DrawImage { image, dest, .. } = &output.ops[2] else {
        panic!("missing SVG tile")
    };
    assert_eq!(*dest, [20., 30., 10., 10.].map(Num::Const));
    assert!(image.uri.starts_with("data:image/png;base64,"));
    let CanvasOp::DrawImage { image, .. } = &output.ops[4] else {
        panic!("missing original bitmap")
    };
    assert!(Arc::ptr_eq(&image.uri, &bitmap));
}

#[test]
fn native_svg_bitmap_preserves_chinese_text_and_embedded_png() {
    let raster = raster(SVG).unwrap();
    let scale = raster.width() / 64;
    let pixel = |x, y| raster.pixel(x * scale, y * scale).unwrap();
    assert_eq!(pixel(55, 35).green(), 255);
    assert_eq!(pixel(12, 36).red(), 255);
    assert_eq!(pixel(12, 36).blue(), 255);
    let mut chinese = 0;
    for y in 7 * scale..20 * scale {
        for x in 28 * scale..60 * scale {
            let p = raster.pixel(x, y).unwrap();
            if p.red() > 220 && p.green() > 220 && p.blue() > 220 {
                chinese += 1;
            }
        }
    }
    assert!(
        chinese > 20,
        "Chinese SVG glyphs must survive rasterization"
    );
}

#[test]
fn svg_with_one_dimension_keeps_the_viewbox_aspect_ratio() {
    let raster = raster(
        "<svg xmlns='http://www.w3.org/2000/svg' width='80' viewBox='0 0 40 20'><rect width='40' height='20' fill='red'/></svg>",
    )
    .unwrap();
    assert_eq!(raster.width(), 2 * raster.height());
    assert_eq!(raster.pixel(1, 1).unwrap().red(), 255);
}

#[test]
fn svg_cannot_resolve_external_images_or_overallocate_filters() {
    for href in ["file:///etc/passwd", "https://image.lceda.cn/image.png"] {
        assert!(raster(&format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='64' height='48'><image href='{href}' width='64' height='48'/></svg>"
        )).is_err());
    }
    assert!(raster(concat!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='64' height='48'>",
        "<defs><filter id='f' filterUnits='userSpaceOnUse' x='0' y='0' width='100000' height='100000'>",
        "<feGaussianBlur stdDeviation='1'/></filter></defs>",
        "<rect width='64' height='48' filter='url(#f)'/></svg>"
    )).is_err());
}

#[test]
fn oversized_embedded_raster_is_rejected_before_decoding() {
    let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82];
    png.extend(100_000_u32.to_be_bytes());
    png.extend(100_000_u32.to_be_bytes());
    png.extend([8, 6, 0, 0, 0]);
    let image = data_uri("image/png", &png);
    assert!(raster(&format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='64' height='48'><image href='{image}' width='64' height='48'/></svg>"
    )).is_err());
}

#[test]
fn corrupt_embedded_bitmap_is_reported_instead_of_silently_omitted() {
    let invalid_crc = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+yMc8AAAAASUVORK5CYII=";
    for source in [
        format!("data:image/png;base64,{invalid_crc}"),
        data_uri("image/png", &[137, 80, 78, 71, 13, 10, 26, 10]),
    ] {
        assert!(raster(&format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='64' height='48'><image href='{source}' width='64' height='48'/></svg>"
        )).is_err(), "a broken image must fail the SVG, not disappear");
    }
}
