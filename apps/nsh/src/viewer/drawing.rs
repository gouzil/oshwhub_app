use std::{collections::HashMap, sync::Arc};

use nsh_core::{
    Rgba,
    canvas::{
        CanvasFont, CanvasImage, CanvasOp, CanvasTextAlign, CanvasTextBaseline, Drawing, FillRule,
        LineCap, LineJoin, Num, Paint, Path, PathOp, TextMetrics, parse_svg_path,
    },
    geometry::px,
};
use oshwhub_core::pcb::{Scene, SceneImage, SceneLayer};

#[derive(Debug, Default)]
pub struct PreparationCounts {
    pub paths: usize,
    pub fonts: usize,
    pub texts: usize,
    pub images: usize,
}

#[derive(Debug)]
pub struct PreparedLayer {
    pub layer: SceneLayer,
    pub drawing: Drawing,
}

#[derive(Debug)]
pub struct PreparedScene {
    pub layers: Vec<PreparedLayer>,
    pub counts: PreparationCounts,
}

impl PreparedScene {
    pub fn drawing(&self, visible: impl Fn(i32) -> bool) -> Drawing {
        let mut drawing = Drawing::default();
        for layer in &self.layers {
            if visible(layer.layer.id) {
                drawing.ops.extend(layer.drawing.ops.iter().cloned());
            }
        }
        drawing
    }
}

/// Font metrics must describe the font box, independent of a label's characters.
/// The image resolver must decode the embedded data and return its source and pixel size.
pub fn prepare_scene(
    scene: &Scene,
    mut measure_font: impl FnMut(&CanvasFont) -> Result<TextMetrics, String>,
    mut resolve_image: impl FnMut(&SceneImage) -> Result<(CanvasImage, [u32; 2]), String>,
) -> Result<PreparedScene, String> {
    let mut layers: Vec<_> = scene
        .layers
        .iter()
        .map(|layer| PreparedLayer {
            layer: layer.clone(),
            drawing: Drawing {
                ops: vec![
                    CanvasOp::Save,
                    CanvasOp::LineCap(LineCap::Round),
                    CanvasOp::LineJoin(LineJoin::Round),
                ],
                ..Drawing::default()
            },
        })
        .collect();
    layers.sort_by_key(|layer| layer.layer.order);
    let mut by_id = HashMap::new();
    for (index, layer) in layers.iter().enumerate() {
        if by_id.insert(layer.layer.id, index).is_some() {
            return Err(format!("场景包含重复图层 {}", layer.layer.id));
        }
    }
    for image in &scene.images {
        let index = *by_id
            .get(&image.layer)
            .ok_or_else(|| format!("图片包含未声明的图层 {}", image.layer))?;
        let layer = &mut layers[index];
        let width = scalar(image.width)?;
        let height = scalar(image.height)?;
        let mut transform = [Num::Const(0.); 6];
        for (out, value) in transform.iter_mut().zip(image.transform) {
            *out = Num::Const(scalar(value)?);
        }
        let embedded = image.content.len() <= 16 * 1024 * 1024
            && image.content.split_once(',').is_some_and(|(header, _)| {
                matches!(
                    header,
                    "data:image/png;base64"
                        | "data:image/jpeg;base64"
                        | "data:image/jpg;base64"
                        | "data:image/webp;base64"
                        | "data:image/svg+xml;base64"
                )
            });
        if width <= 0. || height <= 0. || !embedded {
            return Err(format!("图层「{}」包含无效的嵌入图片", layer.layer.name));
        }
        let (_, bytes) = nsh_core::image::decode_data_uri(&image.content)
            .map_err(|error| format!("嵌入图片无法解码：{error}"))?;
        if bytes.is_empty() {
            return Err("嵌入图片内容为空".into());
        }
        let (source, [pixel_width, pixel_height]) = resolve_image(image)
            .map_err(|error| format!("图层「{}」嵌入图片无法解码：{error}", layer.layer.name))?;
        if pixel_width == 0
            || pixel_height == 0
            || u64::from(pixel_width) * u64::from(pixel_height) > 16_777_216
        {
            return Err("原理图图片尺寸超过限制或为空".into());
        }
        layer.drawing.ops.extend([
            CanvasOp::Save,
            CanvasOp::Transform(transform),
            CanvasOp::DrawImage {
                image: source,
                source: None,
                dest: [0., 0., width, height].map(Num::Const),
            },
            CanvasOp::Restore,
        ]);
    }
    let mut paths: HashMap<&str, Path> = HashMap::new();
    for batch in &scene.batches {
        let index = *by_id
            .get(&batch.layer)
            .ok_or_else(|| format!("场景包含未声明的图层 {}", batch.layer))?;
        let layer = &mut layers[index];
        let width = scalar(batch.width)?;
        if width < 0. {
            return Err(format!("图层「{}」的线宽无效", layer.layer.name));
        }
        let paint = paint(
            batch
                .color
                .as_deref()
                .filter(|color| !color.is_empty())
                .unwrap_or(&layer.layer.color),
        )?;
        let path = match paths.get(batch.path.as_str()) {
            Some(path) => Arc::clone(path),
            None => {
                let path = strict_path(&batch.path)
                    .map_err(|_| format!("图层「{}」包含无法绘制的路径", layer.layer.name))?;
                paths.insert(&batch.path, Arc::clone(&path));
                path
            }
        };
        if batch.fill {
            layer.drawing.ops.extend([
                CanvasOp::FillStyle(paint),
                CanvasOp::Fill(Some(path), FillRule::NonZero),
            ]);
        } else {
            layer.drawing.ops.extend([
                CanvasOp::StrokeStyle(paint),
                CanvasOp::LineWidth(Num::Const(width)),
                CanvasOp::Stroke(Some(path)),
            ]);
        }
    }
    let mut fonts: HashMap<(u32, bool), (CanvasFont, TextMetrics)> = HashMap::new();
    for text in &scene.texts {
        let index = *by_id
            .get(&text.layer)
            .ok_or_else(|| format!("文字包含未声明的图层 {}", text.layer))?;
        let layer = &mut layers[index];
        let (x, y, size, rotation) = (
            scalar(text.x)?,
            scalar(text.y)?,
            scalar(text.size)?,
            scalar(text.rotation.to_radians())?,
        );
        if size <= 0.
            || text.text.chars().any(|c| {
                matches!(
                    c,
                    '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
                )
            })
        {
            return Err(format!("图层「{}」的单行文字或字号无效", layer.layer.name));
        }
        let align = match text.align.as_str() {
            "" | "left" => CanvasTextAlign::Left,
            "center" => CanvasTextAlign::Center,
            "right" => CanvasTextAlign::Right,
            _ => return Err("文字水平对齐方式不受支持".into()),
        };
        let key = (size.to_bits(), text.bold);
        if let std::collections::hash_map::Entry::Vacant(entry) = fonts.entry(key) {
            let font = CanvasFont {
                size: px(size),
                weight: if text.bold { 700 } else { 400 },
                ..CanvasFont::default()
            };
            let metrics =
                measure_font(&font).map_err(|error| format!("PCB 字体尚未就绪：{error}"))?;
            if !metrics.ascent.0.is_finite()
                || !metrics.descent.0.is_finite()
                || metrics.ascent.0 < 0.
                || metrics.descent.0 < 0.
                || metrics.ascent.0 + metrics.descent.0 <= 0.
            {
                return Err("PCB 字体度量无效".into());
            }
            entry.insert((font, metrics));
        }
        let (font, metrics) = &fonts[&key];
        let baseline = match text.vertical_align.as_str() {
            "" => 0.,
            "top" => metrics.ascent.0,
            "middle" => (metrics.ascent.0 - metrics.descent.0) / 2.,
            "bottom" => -metrics.descent.0,
            _ => return Err("文字垂直对齐方式不受支持".into()),
        };
        layer.drawing.ops.extend([
            CanvasOp::Save,
            CanvasOp::Font(font.clone()),
            CanvasOp::FillStyle(paint(
                text.color
                    .as_deref()
                    .filter(|color| !color.is_empty())
                    .unwrap_or(&layer.layer.color),
            )?),
            CanvasOp::TextAlign(align),
            // Native non-alphabetic baselines can depend on the current string's glyphs.
            CanvasOp::TextBaseline(CanvasTextBaseline::Alphabetic),
            CanvasOp::Translate(Num::Const(x), Num::Const(y)),
            CanvasOp::Rotate(Num::Const(rotation)),
            CanvasOp::FillText {
                text: text.text.as_str().into(),
                x: Num::Const(0.),
                y: Num::Const(baseline),
                max_width: None,
            },
            CanvasOp::Restore,
        ]);
    }
    for layer in &mut layers {
        layer.drawing.ops.push(CanvasOp::Restore);
    }
    Ok(PreparedScene {
        layers,
        counts: PreparationCounts {
            paths: paths.len(),
            fonts: fonts.len(),
            texts: scene.texts.len(),
            images: scene.images.len(),
        },
    })
}

fn scalar(value: f64) -> Result<f32, String> {
    let value = value as f32;
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| "绘制数值无效或超出精度范围".into())
}

fn paint(color: &str) -> Result<Paint, String> {
    Rgba::try_from(color)
        .map(|color| Paint::Color(color.into()))
        .map_err(|_| format!("无法绘制颜色「{color}」"))
}

fn strict_path(source: &str) -> Result<Path, ()> {
    validate_path(source)?;
    let path = parse_svg_path(source);
    let finite = |n: &Num| matches!(n, Num::Const(value) if value.is_finite());
    if !path.iter().all(|op| match op {
        PathOp::MoveTo(x, y) | PathOp::LineTo(x, y) => finite(x) && finite(y),
        PathOp::QuadTo(values) => values.iter().all(finite),
        PathOp::CubicTo(values) => values.iter().all(finite),
        PathOp::Close => true,
        _ => false,
    }) {
        return Err(());
    }
    Ok(path.into())
}

// NSH's parser keeps a valid prefix on errors; reject malformed tails before parsing once.
fn validate_path(source: &str) -> Result<(), ()> {
    let bytes = source.as_bytes();
    let mut at = 0;
    let mut command = None;
    let mut first = true;
    loop {
        whitespace(bytes, &mut at);
        if bytes.get(at) == Some(&b',') {
            if command.is_none() {
                return Err(());
            }
            at += 1;
            whitespace(bytes, &mut at);
            if bytes
                .get(at)
                .is_none_or(|b| b.is_ascii_alphabetic() || *b == b',')
            {
                return Err(());
            }
        }
        let Some(&byte) = bytes.get(at) else {
            return Ok(());
        };
        if byte.is_ascii_alphabetic() {
            if first && !matches!(byte, b'M' | b'm') {
                return Err(());
            }
            command = Some(byte);
            at += 1;
        }
        let op = command.ok_or(())?;
        let count = match op.to_ascii_uppercase() {
            b'M' | b'L' | b'T' => 2,
            b'H' | b'V' => 1,
            b'C' => 6,
            b'S' | b'Q' => 4,
            b'A' => 7,
            b'Z' => {
                command = None;
                continue;
            }
            _ => return Err(()),
        };
        for argument in 0..count {
            whitespace(bytes, &mut at);
            if argument > 0 && bytes.get(at) == Some(&b',') {
                at += 1;
                whitespace(bytes, &mut at);
            }
            if op.eq_ignore_ascii_case(&b'A') && matches!(argument, 3 | 4) {
                if !matches!(bytes.get(at), Some(b'0' | b'1')) {
                    return Err(());
                }
                at += 1;
            } else {
                number(source, &mut at)?;
            }
        }
        first = false;
        command = Some(match op {
            b'M' => b'L',
            b'm' => b'l',
            other => other,
        });
    }
}

fn whitespace(bytes: &[u8], at: &mut usize) {
    while bytes.get(*at).is_some_and(u8::is_ascii_whitespace) {
        *at += 1;
    }
}

fn number(source: &str, at: &mut usize) -> Result<(), ()> {
    let bytes = source.as_bytes();
    let start = *at;
    if matches!(bytes.get(*at), Some(b'+' | b'-')) {
        *at += 1;
    }
    let mut digits = false;
    while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
        *at += 1;
        digits = true;
    }
    if bytes.get(*at) == Some(&b'.') {
        *at += 1;
        while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
            *at += 1;
            digits = true;
        }
    }
    if !digits {
        return Err(());
    }
    if matches!(bytes.get(*at), Some(b'e' | b'E')) {
        *at += 1;
        if matches!(bytes.get(*at), Some(b'+' | b'-')) {
            *at += 1;
        }
        let exponent = *at;
        while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
            *at += 1;
        }
        if exponent == *at {
            return Err(());
        }
    }
    source[start..*at]
        .parse::<f32>()
        .ok()
        .filter(|n| n.is_finite())
        .map(|_| ())
        .ok_or(())
}
