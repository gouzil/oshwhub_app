//! Native canvases accept bitmaps; NSH's SVG decoder omits text and embedded images.

use super::raster::RasterPlan;
use nsh_core::canvas::{CanvasImage, CanvasOp, Drawing, Num};
use resvg::{tiny_skia, usvg};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[path = "svg_budget.rs"]
mod svg_budget;

const MAX_PIXELS: u64 = 16_777_216;

pub struct Svg {
    tree: usvg::Tree,
}

pub struct Tile {
    pub uri: Arc<str>,
    pub dest: [f32; 4],
    pub pixels: u64,
}

impl Svg {
    pub fn parse(content: &str) -> Result<Option<Arc<Self>>, String> {
        let (mime, bytes) = nsh_core::image::decode_data_uri(content)?;
        if mime != "image/svg+xml" {
            return Ok(None);
        }
        std::panic::catch_unwind(|| parse(&bytes))
            .map_err(|_| "嵌入 SVG 解码失败".to_owned())?
            .map(|svg| Some(Arc::new(svg)))
    }

    pub fn size(&self) -> [u32; 2] {
        [
            self.tree.size().width().ceil() as u32,
            self.tree.size().height().ceil() as u32,
        ]
    }

    pub fn render(
        &self,
        world: [f64; 6],
        size: [f64; 2],
        plan: RasterPlan,
    ) -> Result<Option<Tile>, String> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.render_tile(world, size, plan)
        }))
        .map_err(|_| "嵌入 SVG 绘制失败".to_owned())?
    }

    fn render_tile(
        &self,
        [a, b, c, d, e, f]: [f64; 6],
        [w, h]: [f64; 2],
        plan: RasterPlan,
    ) -> Result<Option<Tile>, String> {
        let corners = [[0., 0.], [w, 0.], [w, h], [0., h]]
            .map(|[x, y]| [a * x + c * y + e, b * x + d * y + f]);
        let min_x = corners
            .iter()
            .map(|p| p[0])
            .fold(f64::INFINITY, f64::min)
            .max(plan.x);
        let min_y = corners
            .iter()
            .map(|p| p[1])
            .fold(f64::INFINITY, f64::min)
            .max(plan.y);
        let max_x = corners
            .iter()
            .map(|p| p[0])
            .fold(f64::NEG_INFINITY, f64::max)
            .min(plan.x + plan.width);
        let max_y = corners
            .iter()
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max)
            .min(plan.y + plan.height);
        if min_x >= max_x || min_y >= max_y {
            return Ok(None);
        }
        let dx = f64::from(plan.pixel_width) / plan.width;
        let dy = f64::from(plan.pixel_height) / plan.height;
        let x0 = ((min_x - plan.x) * dx).floor().max(0.);
        let y0 = ((min_y - plan.y) * dy).floor().max(0.);
        let x1 = ((max_x - plan.x) * dx)
            .ceil()
            .min(f64::from(plan.pixel_width));
        let y1 = ((max_y - plan.y) * dy)
            .ceil()
            .min(f64::from(plan.pixel_height));
        let width = (x1 - x0) as u32;
        let height = (y1 - y0) as u32;
        if width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || u64::from(width) * u64::from(height) > 4_194_304
        {
            return Err("嵌入 SVG 缓存尺寸超过限制".into());
        }
        let x = plan.x + x0 / dx;
        let y = plan.y + y0 / dy;
        let sx = w / f64::from(self.tree.size().width());
        let sy = h / f64::from(self.tree.size().height());
        let transform = tiny_skia::Transform::from_row(
            (a * sx * dx) as f32,
            (b * sx * dy) as f32,
            (c * sy * dx) as f32,
            (d * sy * dy) as f32,
            ((e - x) * dx) as f32,
            ((f - y) * dy) as f32,
        );
        let peak = svg_budget::peak(&self.tree, transform, (width, height))
            .ok_or("嵌入 SVG 绘制次数超过限制")?;
        if !peak.is_finite() || peak > (4 * MAX_PIXELS) as f64 {
            return Err("嵌入 SVG 图层超过内存限制".into());
        }
        let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or("嵌入 SVG 尺寸无效")?;
        resvg::render(&self.tree, transform, &mut pixmap.as_mut());
        // Clip the transformed viewport too: rotated images and filters cannot spill outside it.
        let mut path = tiny_skia::PathBuilder::new();
        for (i, p) in corners.iter().enumerate() {
            let (px, py) = (((p[0] - x) * dx) as f32, ((p[1] - y) * dy) as f32);
            if i == 0 {
                path.move_to(px, py);
            } else {
                path.line_to(px, py);
            }
        }
        path.close();
        let path = path.finish().ok_or("嵌入 SVG 变换无效")?;
        let mut mask = tiny_skia::Mask::new(width, height).ok_or("嵌入 SVG 裁剪尺寸无效")?;
        mask.fill_path(
            &path,
            tiny_skia::FillRule::Winding,
            true,
            tiny_skia::Transform::identity(),
        );
        pixmap.apply_mask(&mask);
        let png = pixmap.encode_png().map_err(|error| error.to_string())?;
        Ok(Some(Tile {
            uri: Arc::from(nsh_core::platform::clipboard::data_uri("image/png", &png)),
            dest: [
                x as f32,
                y as f32,
                (f64::from(width) / dx) as f32,
                (f64::from(height) / dy) as f32,
            ],
            pixels: u64::from(width) * u64::from(height),
        }))
    }
}

/// Keep parsed paths/fonts intact; only replace SVG image operations for this bounded cache.
pub async fn raster_drawing(
    drawing: &Drawing,
    images: &[(Arc<str>, Arc<Svg>)],
    plan: RasterPlan,
) -> Result<Drawing, String> {
    let mut output = drawing.clone();
    output.ops.clear();
    let mut pixels = 0;
    for op in &drawing.ops {
        if let CanvasOp::DrawImage {
            image,
            dest,
            source: None,
        } = op
            && let Some((_, svg)) = images.iter().find(|(uri, _)| Arc::ptr_eq(uri, &image.uri))
        {
            let Some(CanvasOp::Transform(transform)) = output.ops.last() else {
                return Err("嵌入 SVG 缺少场景变换".into());
            };
            let constant = |n: &Num| match n {
                Num::Const(n) => Ok(f64::from(*n)),
                _ => Err("嵌入 SVG 变换必须为常量".to_owned()),
            };
            let mut world = [0.; 6];
            for (out, n) in world.iter_mut().zip(transform) {
                *out = constant(n)?;
            }
            let size = [constant(&dest[2])?, constant(&dest[3])?];
            let svg = Arc::clone(svg);
            let tile = crate::services::compute(move || {
                svg.render(world, size, plan)
                    .map_err(|error| oshwhub_core::error::AppError::new("INVALID_IMAGE", error))
            })
            .await
            .map_err(|error| error.message)?;
            if let Some(tile) = tile {
                pixels += tile.pixels;
                if pixels > MAX_PIXELS {
                    return Err("嵌入 SVG 缓存总量超过限制".into());
                }
                *output.ops.last_mut().unwrap() =
                    CanvasOp::Transform([1., 0., 0., 1., 0., 0.].map(Num::Const));
                output.ops.push(CanvasOp::DrawImage {
                    image: CanvasImage { uri: tile.uri },
                    source: None,
                    dest: tile.dest.map(Num::Const),
                });
            }
        } else {
            output.ops.push(op.clone());
        }
    }
    Ok(output)
}

fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    Arc::clone(FONTS.get_or_init(|| {
        let mut fonts = usvg::fontdb::Database::new();
        fonts.load_font_source(usvg::fontdb::Source::Binary(Arc::new(
            include_bytes!("../../../../packages/pcb-viewer/src/assets/NotoSansSC-Regular.ttf")
                .as_slice(),
        )));
        fonts.set_sans_serif_family("Noto Sans SC");
        Arc::new(fonts)
    }))
}

fn parse(bytes: &[u8]) -> Result<Svg, String> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("嵌入 SVG 超过大小限制".into());
    }
    // Preserve NSH's bounded XML / SVGZ parsing before resvg inflates or builds its tree.
    let intrinsic = nsh_core::svg::size(bytes).map_err(|error| error.to_string())?;
    let inflated = if bytes.starts_with(&[0x1f, 0x8b]) {
        Some(usvg::decompress_svgz(bytes).map_err(|error| error.to_string())?)
    } else {
        None
    };
    let text = std::str::from_utf8(inflated.as_deref().unwrap_or(bytes))
        .map_err(|error| error.to_string())?;
    let document = usvg::roxmltree::Document::parse_with_options(
        text,
        usvg::roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .map_err(|error| error.to_string())?;
    let invalid = AtomicBool::new(false);
    let image_pixels = AtomicU64::new(0);
    let options = usvg::Options {
        fontdb: fonts(),
        font_family: "Noto Sans SC".into(),
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, data, _| {
                let size = nsh_core::platform::clipboard::image_size(&data);
                let pixels = size.map(|(w, h)| u64::from(w) * u64::from(h));
                if data.len() > 16 * 1024 * 1024
                    || !pixels.is_some_and(|pixels| {
                        pixels > 0
                            && pixels <= MAX_PIXELS
                            && image_pixels.fetch_add(pixels, Ordering::Relaxed) + pixels
                                <= MAX_PIXELS
                    })
                {
                    invalid.store(true, Ordering::Relaxed);
                    return None;
                }
                let decoded = (|| {
                    use std::io::Cursor;
                    let format = match nsh_core::platform::clipboard::image_mime(&data) {
                        Some("image/png") => image::ImageFormat::Png,
                        Some("image/jpeg") => image::ImageFormat::Jpeg,
                        Some("image/gif") => image::ImageFormat::Gif,
                        Some("image/webp") => image::ImageFormat::WebP,
                        _ => return None,
                    };
                    let mut reader = image::ImageReader::with_format(Cursor::new(&*data), format);
                    let mut limits = image::Limits::default();
                    limits.max_alloc = Some(MAX_PIXELS * 4);
                    reader.limits(limits);
                    let bitmap = reader.decode().ok()?.into_rgba8();
                    let mut png = Cursor::new(Vec::new());
                    // Canonical PNG also gives resvg a format its renderer can always decode.
                    bitmap.write_to(&mut png, image::ImageFormat::Png).ok()?;
                    Some(usvg::ImageKind::PNG(Arc::new(png.into_inner())))
                })();
                if decoded.is_none() {
                    invalid.store(true, Ordering::Relaxed);
                }
                decoded
            }),
            resolve_string: Box::new(|_, _| {
                invalid.store(true, Ordering::Relaxed);
                None
            }),
        },
        ..usvg::Options::default()
    };
    let root = document.root_element();
    let missing = root.attribute("viewBox").and_then(|_| {
        match (root.attribute("width"), root.attribute("height")) {
            (Some(width), None) if !width.trim_end().ends_with('%') => {
                Some(("height", intrinsic.height.0))
            }
            (None, Some(height)) if !height.trim_end().ends_with('%') => {
                Some(("width", intrinsic.width.0))
            }
            _ => None,
        }
    });
    let tree = if let Some((name, value)) = missing {
        // usvg treats an omitted side as 100% of viewBox; preserve NSH/browser aspect sizing.
        let start = root.range().start + 1;
        let at = start
            + text[start..]
                .find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
                .ok_or("嵌入 SVG 根元素无效")?;
        usvg::Tree::from_str(
            &format!("{} {name}='{value}'{}", &text[..at], &text[at..]),
            &options,
        )
    } else {
        usvg::Tree::from_xmltree(&document, &options)
    }
    .map_err(|error| error.to_string())?;
    if invalid.load(Ordering::Relaxed) {
        return Err("SVG 图片只能引用有界的 data PNG/JPEG/GIF/WebP 位图".into());
    }
    Ok(Svg { tree })
}
