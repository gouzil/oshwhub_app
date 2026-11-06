use cache::SceneCache;
use drawing::prepare_scene;
use nsh_core::{canvas::TextMetrics, geometry::px};
use oshwhub_core::pcb::{Bounds, DrawBatch, Metrics, Scene, SceneLayer};
use oshwhub_nsh::viewer::{cache, drawing, two_dimensional, viewport};
use std::sync::Arc;
use two_dimensional::{Camera, GesturePhase, Pointer};
use viewport::Point;

fn scene() -> Scene {
    Scene {
        id: "viewer-fixture".into(),
        name: "中文电路板".into(),
        revision: "1".into(),
        bounds: Bounds {
            min_x: 0.,
            min_y: 0.,
            max_x: 100.,
            max_y: 100.,
        },
        layers: vec![
            SceneLayer {
                id: 1,
                name: "顶层".into(),
                color: "#ff0000".into(),
                visible: true,
                order: 1,
            },
            SceneLayer {
                id: 2,
                name: "底层".into(),
                color: "#0000ff".into(),
                visible: false,
                order: 2,
            },
        ],
        batches: vec![DrawBatch {
            layer: 1,
            path: "M0 0H100V100H0Z".into(),
            fill: true,
            width: 0.,
            color: None,
            bounds: Bounds::default(),
        }],
        texts: vec![],
        images: vec![],
        object_count: 1,
        warnings: vec![],
        metrics: Metrics::default(),
    }
}

fn pointer(id: u32, x: f64, y: f64, changed: bool) -> Pointer {
    Pointer {
        id,
        point: Point { x, y },
        changed,
    }
}

fn camera() -> Camera {
    let mut camera = Camera::new(scene().bounds);
    camera.layout(248., 248.);
    camera
}

#[test]
fn failed_rebuild_retains_last_drawing_and_layer_visibility() {
    let mut cache = SceneCache::default();
    assert!(cache.replace(prepare_scene(
        &scene(),
        |_| Ok(TextMetrics::default()),
        |_| unreachable!()
    )));
    cache.set_visible(2, true).unwrap();
    let prepared = cache.prepared.clone().unwrap();
    let drawing = cache.drawing.clone();
    let revision = cache.revision;
    assert!(!cache.replace(Err("图片解码失败".into())));
    assert!(Arc::ptr_eq(&drawing, &cache.drawing));
    assert!(Arc::ptr_eq(&prepared, cache.prepared.as_ref().unwrap()));
    assert_eq!(cache.visible.iter().copied().collect::<Vec<_>>(), [1, 2]);
    assert_eq!(cache.revision, revision);
    assert_eq!(cache.error.as_deref(), Some("图片解码失败"));
    assert!(cache.set_visible(99, false).is_err());
    assert!(Arc::ptr_eq(&drawing, &cache.drawing));
    cache.set_visible(1, false).unwrap();
    assert!(Arc::ptr_eq(&prepared, cache.prepared.as_ref().unwrap()));
    assert!(!cache.visible.contains(&1));
    assert!(
        !cache
            .drawing
            .ops
            .iter()
            .any(|op| matches!(op, nsh_core::canvas::CanvasOp::Fill(..)))
    );
}

#[test]
fn pan_and_pinch_rebase_when_pointer_count_changes_without_becoming_taps() {
    let mut camera = camera();
    camera.touch(GesturePhase::Start, &[pointer(1, 100., 100., true)]);
    camera.touch(GesturePhase::Move, &[pointer(1, 120., 110., true)]);
    assert_eq!((camera.view.x, camera.view.y), (44., 34.));
    let before_pinch = camera.view;
    camera.touch(
        GesturePhase::Start,
        &[pointer(1, 120., 110., false), pointer(2, 160., 110., true)],
    );
    assert_eq!(camera.view, before_pinch);
    camera.touch(
        GesturePhase::Move,
        &[pointer(2, 180., 110., true), pointer(1, 100., 110., true)],
    );
    assert_eq!(camera.view.scale, 4.);
    let before_lift = camera.view;
    assert!(
        camera
            .touch(
                GesturePhase::End,
                &[pointer(1, 100., 110., false), pointer(2, 180., 110., true)]
            )
            .is_none()
    );
    assert_eq!(camera.view, before_lift);
    camera.touch(GesturePhase::Move, &[pointer(1, 110., 120., true)]);
    assert_eq!(camera.view.x, before_lift.x + 10.);
    assert!(
        camera
            .touch(GesturePhase::End, &[pointer(1, 110., 120., true)])
            .is_none()
    );
}

#[test]
fn tap_has_inclusive_five_pixel_threshold_but_out_and_back_drag_is_not_a_tap() {
    let mut camera = camera();
    camera.touch(GesturePhase::Start, &[pointer(1, 100., 100., true)]);
    assert_eq!(
        camera.touch(GesturePhase::End, &[pointer(1, 103., 104., true)]),
        Some(Point { x: 103., y: 104. })
    );
    camera.touch(GesturePhase::Start, &[pointer(1, 100., 100., true)]);
    camera.touch(GesturePhase::Move, &[pointer(1, 106., 100., true)]);
    camera.touch(GesturePhase::Move, &[pointer(1, 100., 100., true)]);
    assert!(
        camera
            .touch(GesturePhase::End, &[pointer(1, 100., 100., true)])
            .is_none()
    );
    camera.touch(GesturePhase::Start, &[pointer(1, 100., 100., true)]);
    assert!(
        camera
            .touch(GesturePhase::Cancel, &[pointer(1, 100., 100., true)])
            .is_none()
    );
}

#[test]
fn anchored_zoom_clamps_fit_resets_and_resize_preserves_relative_zoom() {
    let mut camera = camera();
    camera.wheel(-100., Point { x: 24., y: 24. });
    assert!((camera.view.scale - 2.6997176151520064).abs() < 1e-12);
    assert_eq!((camera.view.x, camera.view.y), (24., 24.));
    for _ in 0..100 {
        camera.key("+");
    }
    assert_eq!(camera.view.scale, 256.);
    for _ in 0..200 {
        camera.key("-");
    }
    assert_eq!(camera.view.scale, 0.1);
    camera.key("0");
    assert_eq!(camera.view.scale, 2.);
    camera.key("right");
    assert_eq!(camera.view.x, 54.);
    camera.key("+");
    camera.layout(448., 448.);
    assert_eq!(camera.view.scale, 5.);
    assert_eq!((camera.view.x, camera.view.y), (49., -26.));
}

use nsh_core::{
    MountItem,
    element::Root,
    geometry::{Size, point},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;
use std::sync::Mutex;

#[component]
fn ViewerFixture() -> impl IntoElement {
    set_tokens(oshwhub_nsh::app_theme().into_tokens());
    let active = signal(true);
    let mut scene = oshwhub_core::pcb::demo();
    scene.layers.iter_mut().for_each(|layer| {
        if layer.id == 1 {
            layer.name = "顶层".into();
        }
        if layer.id == 2 {
            layer.name = "底层".into();
        }
    });
    scene.texts[0].text = "中文电路标签".into();
    view()
        .flex_1()
        .child(
            view()
                .min_h(px(44.))
                .on_press(move || active.set(!active.get_untracked()))
                .label("切换查看器")
                .child(text("切换")),
        )
        .child(two_dimensional::two_dimensional(Arc::new(scene), active))
}

#[derive(Default)]
struct Observed {
    frames:
        std::collections::BTreeMap<nsh_core::NodeId, nsh_core::Bounds<nsh_core::geometry::Points>>,
    drawings: Vec<Arc<nsh_core::canvas::Drawing>>,
    animation_updates: usize,
}

#[test]
fn viewer_retains_drawing_during_gestures_and_inactive_toggles_then_changes_layers_atomically() {
    let _guard = GUI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let demo_bounds = oshwhub_core::pcb::demo().bounds;
    let observed = Arc::new(Mutex::new(Observed::default()));
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(480.), px(640.)),
        ..EngineConfig::new(Root::of::<ViewerFixture>())
    })
    .unwrap();
    let records = observed.clone();
    app.observe(move |_, batch| {
        let mut records = records.lock().unwrap();
        for item in &batch.items {
            match item {
                MountItem::Canvas { drawing, .. } => {
                    records.drawings.push(drawing.clone());
                }
                MountItem::Frame { id, frame } => {
                    records.frames.insert(*id, *frame);
                }
                MountItem::Animation(_) => records.animation_updates += 1,
                _ => {}
            }
        }
    });
    app.first_render().unwrap();
    app.wait_for("label:二维画布").unwrap();
    let initial = observed.lock().unwrap().drawings.len();
    assert!(initial > 0);
    {
        let records = observed.lock().unwrap();
        let drawing = records.drawings.last().unwrap();
        assert!(drawing.slots.is_empty());
        assert!(drawing.ops.iter().any(|op| matches!(op, nsh_core::canvas::CanvasOp::FillText { text, .. } if text.as_ref() == "中文电路标签")));
        assert!(
            drawing
                .ops
                .iter()
                .any(|op| matches!(op, nsh_core::canvas::CanvasOp::Stroke(..)))
        );
    }
    app.drag("label:二维画布", point(px(20.), px(10.))).unwrap();
    app.press("label:放大二维画布").unwrap();
    app.press_key("right").unwrap();
    assert_eq!(
        observed.lock().unwrap().drawings.len(),
        initial,
        "gestures must not resend Canvas or parse paths"
    );
    assert!(observed.lock().unwrap().animation_updates > 0);
    app.wait_for("text:125%").unwrap();
    app.press("label:切换查看器").unwrap();
    app.press("label:缩小二维画布").unwrap();
    app.press_key("-").unwrap();
    app.wait_for("text:125%").unwrap();
    app.press("label:切换查看器").unwrap();
    app.wait_for("text:125%").unwrap();
    app.press("label:二维图层").unwrap();
    app.press("label:切换图层 顶层").unwrap();
    assert_eq!(observed.lock().unwrap().drawings.len(), initial + 1);
    let records = observed.lock().unwrap();
    let before = &records.drawings[initial - 1];
    let after = records.drawings.last().unwrap();
    assert!(
        after.ops.len() < before.ops.len(),
        "hiding a layer must remove its paths from the Canvas"
    );
    let canvas_id = app
        .screen()
        .press_target("label:二维画布")
        .unwrap()
        .unwrap();
    let canvas_bounds = records.frames[&canvas_id];
    drop(records);
    app.press("label:切换图层 顶层").unwrap();
    app.press("label:二维图层").unwrap();
    app.press("label:适应二维画布").unwrap();
    let camera = viewport::fit_viewport(
        &demo_bounds,
        f64::from(canvas_bounds.size.width.0),
        f64::from(canvas_bounds.size.height.0),
        24.,
    );
    app.press_at(
        "label:二维画布",
        point(
            px((camera.x + 80. * camera.scale) as f32),
            px((camera.y - 80. * camera.scale) as f32),
        ),
    )
    .unwrap();
    app.wait_for("text:类型：PAD").unwrap();
    app.wait_for("text:网络：3V3").unwrap();
    app.wait_for("text:ID：pad").unwrap();
    app.press("label:切换查看器").unwrap();
    app.press("label:切换查看器").unwrap();
    app.wait_for("text:网络：3V3").unwrap();
    app.press("label:二维图层").unwrap();
    app.press("label:切换图层 顶层").unwrap();
    app.press("label:二维属性").unwrap();
    app.wait_for("text:点选图形查看属性").unwrap();
}

#[test]
fn viewer_reports_real_core_pick_errors_instead_of_fabricating_properties() {
    let _guard = GUI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    #[component]
    fn ExpiredFixture() -> impl IntoElement {
        set_tokens(oshwhub_nsh::app_theme().into_tokens());
        two_dimensional::two_dimensional(Arc::new(scene()), signal(true))
    }
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(480.), px(640.)),
        ..EngineConfig::new(Root::of::<ExpiredFixture>())
    })
    .unwrap();
    app.first_render().unwrap();
    app.press_at("label:二维画布", point(px(100.), px(100.)))
        .unwrap();
    app.wait_for("text:场景已过期，请重新打开 PCB").unwrap();
}

static GUI_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn camera_changes_composite_cached_vectors_instead_of_replaying_them() {
    let _guard = GUI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let drawings = Arc::new(Mutex::new(Vec::new()));
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(480.), px(640.)),
        ..EngineConfig::new(Root::of::<ViewerFixture>())
    })
    .unwrap();
    let records = Arc::clone(&drawings);
    app.observe(move |_, batch| {
        for item in &batch.items {
            if let MountItem::Canvas { drawing, .. } = item {
                records.lock().unwrap().push(Arc::clone(drawing));
            }
        }
    });
    app.first_render().unwrap();
    let records = drawings.lock().unwrap();
    let vectors: Vec<_> = records
        .iter()
        .filter(|drawing| drawing.ops.iter().any(|op| op.paints()))
        .collect();
    assert!(!vectors.is_empty());
    assert!(
        vectors.iter().all(|drawing| drawing.slots.is_empty()),
        "binding the camera to a vector Canvas replays the whole board on every frame"
    );
    let count = records.len();
    drop(records);
    app.drag("label:二维画布", point(px(20.), px(10.))).unwrap();
    app.press("label:放大二维画布").unwrap();
    app.wait_for("text:125%").unwrap();
    assert_eq!(drawings.lock().unwrap().len(), count);
    for _ in 0..12 {
        app.press("label:放大二维画布").unwrap();
    }
    app.wait(std::time::Duration::from_millis(250)).unwrap();
    assert_eq!(
        drawings.lock().unwrap().len(),
        count + 1,
        "idle zoom creates one detail raster"
    );
    app.drag("label:二维画布", point(px(5.), px(5.))).unwrap();
    app.wait(std::time::Duration::from_millis(250)).unwrap();
    assert_eq!(
        drawings.lock().unwrap().len(),
        count + 1,
        "covered motion reuses the detail raster"
    );
}

#[test]
fn interrupted_drag_refines_the_final_camera_when_the_viewer_becomes_active_again() {
    let _guard = GUI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let drawings = Arc::new(Mutex::new(0));
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(480.), px(640.)),
        ..EngineConfig::new(Root::of::<ViewerFixture>())
    })
    .unwrap();
    let records = Arc::clone(&drawings);
    app.observe(move |_, batch| {
        for item in &batch.items {
            if matches!(item, MountItem::Canvas { .. }) {
                *records.lock().unwrap() += 1;
            }
        }
    });
    app.first_render().unwrap();
    let initial = *drawings.lock().unwrap();
    for _ in 0..12 {
        app.press("label:放大二维画布").unwrap();
    }
    app.drag_and_hold("label:二维画布", point(px(5.), px(5.)))
        .unwrap();
    app.press("label:切换查看器").unwrap();
    app.drop_held().unwrap();
    app.wait(std::time::Duration::from_millis(250)).unwrap();
    assert_eq!(*drawings.lock().unwrap(), initial);
    app.press("label:切换查看器").unwrap();
    app.wait(std::time::Duration::from_millis(250)).unwrap();
    assert_eq!(
        *drawings.lock().unwrap(),
        initial + 1,
        "resuming a cancelled gesture must refine its final camera without another input"
    );
}

#[test]
fn native_wheel_dispatches_only_active_matching_scenes_and_unregisters_on_disposal() {
    let _guard = GUI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    #[component]
    fn WheelFixture() -> impl IntoElement {
        set_tokens(oshwhub_nsh::app_theme().into_tokens());
        let active = signal(true);
        view()
            .flex_1()
            .child(
                view()
                    .min_h(px(44.))
                    .label("滚轮")
                    .on_press(|| {
                        two_dimensional::native_wheel("viewer-fixture", -100., 100., 100.);
                    })
                    .child(text("滚轮")),
            )
            .child(
                view()
                    .min_h(px(44.))
                    .label("其他场景滚轮")
                    .on_press(|| {
                        assert!(!two_dimensional::native_wheel("missing", -100., 100., 100.));
                    })
                    .child(text("其他场景")),
            )
            .child(
                view()
                    .min_h(px(44.))
                    .label("禁用交互")
                    .on_press(move || active.set(!active.get_untracked()))
                    .child(text("禁用")),
            )
            .child(two_dimensional::two_dimensional(Arc::new(scene()), active))
    }
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(480.), px(640.)),
        ..EngineConfig::new(Root::of::<WheelFixture>())
    })
    .unwrap();
    app.first_render().unwrap();
    app.press("label:滚轮").unwrap();
    app.wait_for("text:135%").unwrap();
    app.press("label:其他场景滚轮").unwrap();
    app.wait_for("text:135%").unwrap();
    app.press("label:禁用交互").unwrap();
    app.press("label:滚轮").unwrap();
    app.wait_for("text:135%").unwrap();
    drop(app);
    assert!(!two_dimensional::native_wheel(
        "viewer-fixture",
        -100.,
        100.,
        100.
    ));
}

#[test]
fn keyboard_interrupts_an_inflight_touch_gesture() {
    let mut camera = camera();
    camera.touch(GesturePhase::Start, &[pointer(1, 100., 100., true)]);
    camera.key("right");
    camera.touch(GesturePhase::Move, &[pointer(1, 110., 100., true)]);
    assert_eq!(
        camera.view.x, 54.,
        "a stale touch baseline must not overwrite keyboard movement"
    );
    assert!(
        camera
            .touch(GesturePhase::End, &[pointer(1, 110., 100., true)])
            .is_none()
    );
}

#[test]
fn invalid_embedded_image_is_reported_while_another_image_is_still_loading() {
    let _guard = GUI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    #[component]
    fn ImageFixture() -> impl IntoElement {
        set_tokens(oshwhub_nsh::app_theme().into_tokens());
        let mut scene = scene();
        for source in [
            "https://example.invalid/not-embedded.png",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aF1kAAAAASUVORK5CYII=",
        ] {
            scene.images.push(oshwhub_core::pcb::SceneImage {
                layer: 1,
                content: source.into(),
                width: 1.,
                height: 1.,
                transform: [1., 0., 0., 1., 0., 0.],
            });
        }
        two_dimensional::two_dimensional(Arc::new(scene), signal(true))
    }
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(480.), px(640.)),
        ..EngineConfig::new(Root::of::<ImageFixture>())
    })
    .unwrap();
    app.first_render().unwrap();
    app.wait_for("text:嵌入图片无效，请重新打开文档").unwrap();
    assert!(!app.screen().shows("正在读取嵌入图片…"));
}

#[test]
fn successful_pick_does_not_hide_the_last_preparation_failure() {
    let _guard = GUI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    #[component]
    fn FailedDrawingFixture() -> impl IntoElement {
        set_tokens(oshwhub_nsh::app_theme().into_tokens());
        let mut scene = oshwhub_core::pcb::demo();
        scene.batches[0].path = "broken path".into();
        two_dimensional::two_dimensional(Arc::new(scene), signal(true))
    }
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(480.), px(640.)),
        ..EngineConfig::new(Root::of::<FailedDrawingFixture>())
    })
    .unwrap();
    app.first_render().unwrap();
    let error = app
        .screen()
        .texts()
        .into_iter()
        .find(|text| text.contains("无法绘制的路径"))
        .expect("preparation error must be visible")
        .to_owned();
    app.press_at("label:二维画布", point(px(50.), px(50.)))
        .unwrap();
    assert!(
        app.screen().shows(&error),
        "a successful core pick must not clear an unresolved drawing failure"
    );
}

#[test]
fn layer_and_property_panels_switch_without_overlapping_on_small_screens() {
    let _guard = GUI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(320.), px(640.)),
        ..EngineConfig::new(Root::of::<ViewerFixture>())
    })
    .unwrap();
    app.first_render().unwrap();
    app.press("label:二维属性").unwrap();
    app.wait_for("text:点选图形查看属性").unwrap();
    app.press("label:二维图层").unwrap();
    app.wait_for("label:切换图层 顶层").unwrap();
    assert!(!app.screen().shows("点选图形查看属性"));
    app.press("label:二维属性").unwrap();
    app.wait_for("text:点选图形查看属性").unwrap();
    assert!(!app.screen().matches("label:切换图层 顶层").unwrap());
}
