#[path = "../src/viewer/drawing.rs"]
mod drawing;

use drawing::{PreparedScene, prepare_scene};
use nsh_core::{
    canvas::{
        CanvasFont, CanvasImage, CanvasOp, CanvasTextAlign, CanvasTextBaseline, FillRule, LineCap,
        LineJoin, Num, Paint, Path, PathOp, TextMetrics, point_in_path, point_in_stroke,
    },
    geometry::{point, px},
};
use oshwhub_core::pcb::{Bounds, DrawBatch, Metrics, Scene, SceneImage, SceneLayer, TextLabel};
use std::sync::Arc;

const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+ip1sAAAAASUVORK5CYII=";
const COPPER: &str = "M2 2H22V22H2Z M8 8V16H16V8Z M18 2H30V22H18Z";

fn scene() -> Scene {
    Scene {
        id: "offline-drawing".into(),
        name: "Authored drawing fixture".into(),
        revision: "1".into(),
        bounds: Bounds {
            min_x: 0.,
            min_y: 0.,
            max_x: 40.,
            max_y: 30.,
        },
        layers: vec![SceneLayer {
            id: 1,
            name: "Copper".into(),
            color: "#ff0000".into(),
            visible: true,
            order: 1,
        }],
        batches: vec![DrawBatch {
            layer: 1,
            path: COPPER.into(),
            fill: true,
            width: 0.,
            bounds: Bounds::default(),
            color: None,
        }],
        texts: Vec::new(),
        images: Vec::new(),
        object_count: 3,
        warnings: Vec::new(),
        metrics: Metrics::default(),
    }
}

fn label(text: &str) -> TextLabel {
    TextLabel {
        layer: 1,
        x: 32.,
        y: 32.,
        text: text.into(),
        size: 10.,
        rotation: 90.,
        align: "center".into(),
        vertical_align: "top".into(),
        color: Some("#00ff00".into()),
        bold: false,
    }
}

fn image() -> SceneImage {
    SceneImage {
        layer: 1,
        content: PNG.into(),
        width: 10.,
        height: 20.,
        transform: [-1., 0.25, 0.5, 1., 5., 7.],
    }
}

fn font_metrics(_: &CanvasFont) -> Result<TextMetrics, String> {
    let mut metrics = TextMetrics::default();
    metrics.ascent = px(8.);
    metrics.descent = px(2.);
    metrics.height = px(10.);
    Ok(metrics)
}

fn prepare(scene: &Scene) -> Result<PreparedScene, String> {
    prepare_scene(scene, font_metrics, |_| {
        Err("image decoder is unavailable".into())
    })
}

fn paths(scene: &PreparedScene) -> Vec<&Path> {
    scene
        .layers
        .iter()
        .flat_map(|layer| &layer.drawing.ops)
        .filter_map(|op| match op {
            CanvasOp::Fill(Some(path), _) | CanvasOp::Stroke(Some(path)) => Some(path),
            _ => None,
        })
        .collect()
}

#[test]
fn stable_layer_order_reuses_each_distinct_parsed_path() {
    let mut scene = scene();
    scene.layers = vec![
        SceneLayer {
            id: 2,
            order: 20,
            ..scene.layers[0].clone()
        },
        SceneLayer {
            id: 1,
            order: 10,
            ..scene.layers[0].clone()
        },
        SceneLayer {
            id: 3,
            order: 20,
            visible: false,
            ..scene.layers[0].clone()
        },
    ];
    scene.batches.push(DrawBatch {
        layer: 2,
        fill: false,
        width: 2.,
        color: Some("#abcdef".into()),
        ..scene.batches[0].clone()
    });
    scene.batches.push(DrawBatch {
        layer: 3,
        path: "M0 0L10 10".into(),
        ..scene.batches[0].clone()
    });
    let prepared = prepare(&scene).unwrap();
    assert_eq!(
        prepared
            .layers
            .iter()
            .map(|layer| layer.layer.id)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(prepared.counts.paths, 2);
    assert_eq!(prepared.counts.fonts, 0);
    assert_eq!(prepared.counts.texts, 0);
    assert_eq!(prepared.counts.images, 0);
    let paths = paths(&prepared);
    assert!(Arc::ptr_eq(paths[0], paths[1]));
    let drawing = prepared.drawing(|id| id != 3);
    assert_eq!(drawing.ops.iter().filter(|op| op.paints()).count(), 2);
    assert!(drawing.nodes.is_empty());
    let repeated = prepared.drawing(|id| id == 1);
    let path = repeated
        .ops
        .iter()
        .find_map(|op| match op {
            CanvasOp::Fill(Some(path), _) => Some(path),
            _ => None,
        })
        .unwrap();
    assert!(Arc::ptr_eq(paths[0], path));
    assert!(drawing.ops.iter().any(
        |op| matches!(op, CanvasOp::StrokeStyle(Paint::Color(color)) if color.0 == 0xab_cd_ef_ff)
    ));
}

#[test]
fn nonzero_fill_keeps_copper_overlap_solid_and_reverse_winding_holes_empty() {
    let prepared = prepare(&scene()).unwrap();
    let (path, rule) = prepared.layers[0]
        .drawing
        .ops
        .iter()
        .find_map(|op| match op {
            CanvasOp::Fill(Some(path), rule) => Some((path, *rule)),
            _ => None,
        })
        .unwrap();
    assert_eq!(rule, FillRule::NonZero);
    assert!(point_in_path(
        &[],
        Some(path),
        rule,
        point(px(20.), px(10.)),
        |_| 0.
    ));
    assert!(!point_in_path(
        &[],
        Some(path),
        rule,
        point(px(12.), px(12.)),
        |_| 0.
    ));
    assert!(!point_in_path(
        &[],
        Some(path),
        rule,
        point(px(35.), px(25.)),
        |_| 0.
    ));
}

#[test]
fn strokes_preserve_width_and_round_ends_and_joins() {
    let mut scene = scene();
    scene.batches[0].path = "M2 10L22 10".into();
    scene.batches[0].fill = false;
    scene.batches[0].width = 4.;
    let prepared = prepare(&scene).unwrap();
    let ops = &prepared.layers[0].drawing.ops;
    assert!(ops.contains(&CanvasOp::LineCap(LineCap::Round)));
    assert!(ops.contains(&CanvasOp::LineJoin(LineJoin::Round)));
    assert!(ops.contains(&CanvasOp::LineWidth(Num::Const(4.))));
    let (index, path) = ops
        .iter()
        .enumerate()
        .find_map(|(i, op)| match op {
            CanvasOp::Stroke(Some(path)) => Some((i, path)),
            _ => None,
        })
        .unwrap();
    assert!(point_in_stroke(
        &ops[..index],
        Some(path),
        point(px(0.5), px(10.)),
        |_| 0.
    ));
    assert!(!point_in_stroke(
        &ops[..index],
        Some(path),
        point(px(-1.), px(10.)),
        |_| 0.
    ));
}

#[test]
fn relative_commands_arcs_and_smooth_curves_are_prepared_as_explicit_paths() {
    let mut scene = scene();
    scene.batches[0].path = "m2,3,4,0 v4 h-4 z M20,10a5,5 0 0 1 10,0".into();
    let prepared = prepare(&scene).unwrap();
    let path = paths(&prepared)[0];
    assert_eq!(path[0], PathOp::MoveTo(Num::Const(2.), Num::Const(3.)));
    assert_eq!(path[1], PathOp::LineTo(Num::Const(6.), Num::Const(3.)));
    assert!(path.iter().any(|op| matches!(op, PathOp::CubicTo(_))));
    scene.batches[0].path = "M0 0 C1 2 3 4 5 6 S7 8 9 10 Q11 12 13 14 T15 16".into();
    let prepared = prepare(&scene).unwrap();
    let path = paths(&prepared)[0];
    assert_eq!(
        path[2],
        PathOp::CubicTo([7., 8., 7., 8., 9., 10.].map(Num::Const))
    );
    assert_eq!(
        path[4],
        PathOp::QuadTo([15., 16., 15., 16.].map(Num::Const))
    );
}

#[test]
fn malformed_path_tails_fail_instead_of_recording_a_partial_board() {
    for path in [
        "broken path",
        "M0 0 L10",
        "M0 0 L10 10 broken",
        "M0 0 A4 4 0 2 0 10 10",
        "M0 0 L1e999 3",
        "M3e38 0l3e38 0",
        "M0 0 L1 2 Z 3",
        "M0 0L1 2 M",
        "M0 0L1 2 !",
        "L1 2",
        "M,0 0",
        "M0,,0",
        "M0 0,",
        "M0 0,L1 2",
        "M0 0Z,",
    ] {
        let mut scene = scene();
        scene.batches[0].path = path.into();
        let error = prepare(&scene).unwrap_err();
        assert!(error.contains("路径"), "{path}: {error}");
        assert!(error.contains("Copper"), "{path}: {error}");
    }
    let mut scene = scene();
    scene.batches[0].path.clear();
    assert!(paths(&prepare(&scene).unwrap())[0].is_empty());
}

#[test]
fn label_baselines_use_font_metrics_independent_of_label_contents() {
    for (vertical_align, y) in [("top", 8.), ("middle", 3.), ("bottom", -2.), ("", 0.)] {
        let mut scene = scene();
        scene.batches.clear();
        scene.texts = ["CLK", "CLKgj", "CLK中文"]
            .map(|text| TextLabel {
                vertical_align: vertical_align.into(),
                ..label(text)
            })
            .into();
        let prepared = prepare(&scene).unwrap();
        assert_eq!(prepared.counts.fonts, 1);
        assert_eq!(prepared.counts.texts, 3);
        let ops = &prepared.layers[0].drawing.ops;
        let baselines: Vec<_> = ops
            .iter()
            .filter_map(|op| match op {
                CanvasOp::FillText {
                    x: Num::Const(x),
                    y: Num::Const(y),
                    ..
                } => Some((*x, *y)),
                _ => None,
            })
            .collect();
        assert_eq!(baselines, [(0., y), (0., y), (0., y)]);
        assert!(ops.contains(&CanvasOp::TextAlign(CanvasTextAlign::Center)));
        assert!(ops.contains(&CanvasOp::TextBaseline(CanvasTextBaseline::Alphabetic)));
        assert!(ops.contains(&CanvasOp::Translate(Num::Const(32.), Num::Const(32.))));
        assert!(ops.iter().any(|op| matches!(op, CanvasOp::Rotate(Num::Const(angle)) if (*angle - std::f32::consts::FRAC_PI_2).abs() < 1e-6)));
    }
}

#[test]
fn labels_keep_bold_weight_alignment_and_color_overrides() {
    let mut scene = scene();
    scene.texts = vec![
        TextLabel {
            bold: true,
            align: "right".into(),
            rotation: -90.,
            ..label("U1中文")
        },
        label("CLK"),
    ];
    let prepared = prepare(&scene).unwrap();
    assert_eq!(prepared.counts.fonts, 2);
    let ops = &prepared.layers[0].drawing.ops;
    assert!(ops.iter().any(|op| matches!(op, CanvasOp::Font(font) if font.size == px(10.) && font.weight == 700 && font.family.is_none())));
    assert!(ops.contains(&CanvasOp::TextAlign(CanvasTextAlign::Right)));
    assert!(ops.iter().any(
        |op| matches!(op, CanvasOp::FillStyle(Paint::Color(color)) if color.0 == 0x00_ff_00_ff)
    ));
}

#[test]
fn embedded_images_keep_affine_mirrors_and_draw_before_paths_and_labels() {
    let mut scene = scene();
    scene.images.push(image());
    scene.texts.push(label("U1"));
    let prepared = prepare_scene(&scene, font_metrics, |image| {
        Ok((
            CanvasImage {
                uri: image.content.clone().into(),
            },
            [1, 1],
        ))
    })
    .unwrap();
    assert_eq!(prepared.counts.images, 1);
    let ops = &prepared.layers[0].drawing.ops;
    assert!(ops.contains(&CanvasOp::Transform(
        [-1., 0.25, 0.5, 1., 5., 7.].map(Num::Const)
    )));
    let image = ops
        .iter()
        .find_map(|op| match op {
            CanvasOp::DrawImage {
                image,
                source,
                dest,
            } => Some((image, source, dest)),
            _ => None,
        })
        .unwrap();
    assert_eq!(image.0.uri.as_ref(), PNG);
    assert_eq!(*image.1, None);
    assert_eq!(*image.2, [0., 0., 10., 20.].map(Num::Const));
    let paints: Vec<_> = ops
        .iter()
        .filter_map(|op| match op {
            CanvasOp::DrawImage { .. } => Some("image"),
            CanvasOp::Fill(..) => Some("path"),
            CanvasOp::FillText { .. } => Some("text"),
            _ => None,
        })
        .collect();
    assert_eq!(paints, ["image", "path", "text"]);
}

#[test]
fn image_decode_failures_and_oversized_bitmaps_are_visible_errors() {
    let mut scene = scene();
    scene.images.push(image());
    let error = prepare(&scene).unwrap_err();
    assert!(error.contains("image decoder is unavailable"));
    for size in [[0, 1], [5000, 5000]] {
        let error = prepare_scene(&scene, font_metrics, |image| {
            Ok((
                CanvasImage {
                    uri: image.content.clone().into(),
                },
                size,
            ))
        })
        .unwrap_err();
        assert!(error.contains("图片尺寸"), "{error}");
    }
    assert!(
        prepare_scene(&scene, font_metrics, |image| Ok((
            CanvasImage {
                uri: image.content.clone().into()
            },
            [4096, 4096]
        )))
        .is_ok()
    );
    scene.images[0].content = "https://example.com/image.png".into();
    let error = prepare_scene(&scene, font_metrics, |_| {
        panic!("external image reached the decoder")
    })
    .unwrap_err();
    assert!(error.contains("嵌入图片"));
}

#[test]
fn undeclared_and_duplicate_layers_are_rejected() {
    for kind in ["path", "text", "image"] {
        let mut scene = scene();
        match kind {
            "path" => scene.batches[0].layer = 99,
            "text" => scene.texts.push(TextLabel {
                layer: 99,
                ..label("U1")
            }),
            _ => scene.images.push(SceneImage {
                layer: 99,
                ..image()
            }),
        }
        assert!(prepare(&scene).unwrap_err().contains("99"));
    }
    let mut scene = scene();
    scene.layers.push(scene.layers[0].clone());
    assert!(prepare(&scene).unwrap_err().contains("重复"));
}

#[test]
fn invalid_geometry_styles_and_font_readiness_fail_preparation() {
    let mut invalid_color = scene();
    invalid_color.batches[0].color = Some("invalid color".into());
    let mut invalid_width = scene();
    invalid_width.batches[0].width = -1.;
    let mut invalid_text = scene();
    invalid_text.texts.push(TextLabel {
        rotation: f64::NAN,
        ..label("U1")
    });
    let mut invalid_image = scene();
    invalid_image.images.push(SceneImage {
        transform: [f64::INFINITY; 6],
        ..image()
    });
    for scene in [invalid_color, invalid_width, invalid_text, invalid_image] {
        assert!(prepare(&scene).is_err());
    }
    let mut scene = scene();
    scene.texts.push(label("U1"));
    let error = prepare_scene(
        &scene,
        |_| Err("font is not ready".into()),
        |_| unreachable!(),
    )
    .unwrap_err();
    assert!(error.contains("font is not ready"));
}
