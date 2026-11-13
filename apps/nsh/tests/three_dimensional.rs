#![cfg(any(target_os = "macos", target_os = "ios"))]

use nsh_core::{MountItem, Size, element::Root, prelude::*};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;
use oshwhub_nsh::viewer::surface::SurfaceModel;
use oshwhub_nsh::viewer::three_dimensional::{native_wheel, three_dimensional};
use std::sync::{Arc, Mutex};

static GUI_LOCK: Mutex<()> = Mutex::new(());

#[component]
fn ViewerFixture() -> impl IntoElement {
    let scene = Arc::new(oshwhub_core::pcb::demo());
    let id = scene.id.clone();
    view()
        .flex_1()
        .child(
            view()
                .min_h(px(44.))
                .label("三维滚轮")
                .on_press(move || assert!(native_wheel(&id, -100., 100., 100.)))
                .child(text("滚轮")),
        )
        .child(three_dimensional(scene, signal(true)))
}

fn app() -> (Headless, Arc<SurfaceModel>) {
    let handle = Arc::new(Mutex::new(None));
    let observed = Arc::clone(&handle);
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(480.), px(640.)),
        ..EngineConfig::new(
            Root::of::<ViewerFixture>().tokens(oshwhub_nsh::app_theme().into_tokens()),
        )
    })
    .unwrap();
    app.observe(move |_, batch| {
        for item in &batch.items {
            if let MountItem::ComponentProps { props, .. } = item {
                for (name, value) in props {
                    if name.as_ref() == "model" {
                        *observed.lock().unwrap() =
                            value.as_ref().and_then(|v| v["handle"].as_u64());
                    }
                }
            }
        }
    });
    app.first_render().unwrap();
    let model = SurfaceModel::resolve(handle.lock().unwrap().unwrap()).unwrap();
    (app, model)
}

#[test]
fn wheel_zoom_keeps_the_viewer_interactive() {
    let _guard = GUI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let (mut app, model) = app();
    let before = model.frame().camera;
    app.press("label:三维滚轮").unwrap();
    assert!(model.frame().camera.distance < before.distance);
}

#[test]
fn toolbar_zoom_keeps_the_viewer_interactive() {
    let _guard = GUI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let (mut app, model) = app();
    let before = model.frame().camera;
    app.press("label:放大三维场景").unwrap();
    assert!(model.frame().camera.distance < before.distance);
    let zoomed = model.frame().camera;
    app.press("label:缩小三维场景").unwrap();
    assert!(model.frame().camera.distance > zoomed.distance);
}

#[test]
fn toolbar_zoom_out_changes_the_camera() {
    let _guard = GUI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let (mut app, model) = app();
    let before = model.frame().camera;
    app.press("label:缩小三维场景").unwrap();
    assert!(model.frame().camera.distance > before.distance);
}

#[cfg(target_os = "macos")]
#[test]
fn macos_canvas_wheel_event_updates_the_gpu_camera() {
    use nsh_core::{EventKind, EventPayload};
    let _guard = GUI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let handles = Arc::new(Mutex::new((None, None)));
    let observed = Arc::clone(&handles);
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(480.), px(640.)),
        ..EngineConfig::new(
            Root::of::<ViewerFixture>().tokens(oshwhub_nsh::app_theme().into_tokens()),
        )
    })
    .unwrap();
    app.observe(move |_, batch| {
        let mut handles = observed.lock().unwrap();
        for item in &batch.items {
            match item {
                MountItem::Component { id, name } if name.as_ref() == "oshwhub.wheel" => {
                    handles.0 = Some(*id)
                }
                MountItem::ComponentProps { props, .. } => {
                    for (name, value) in props {
                        if name.as_ref() == "model" {
                            handles.1 = value.as_ref().and_then(|v| v["handle"].as_u64());
                        }
                    }
                }
                _ => {}
            }
        }
    });
    app.first_render().unwrap();
    let (target, handle) = *handles.lock().unwrap();
    let target = target.expect("Mac 3D canvas must host its wheel input target");
    let model = SurfaceModel::resolve(handle.unwrap()).unwrap();
    let before = model.frame().camera;
    app.engine().control().dispatch_event(
        target,
        EventKind::NativeEvent,
        EventPayload::Native {
            name: "wheel".into(),
            data: serde_json::json!({"delta":-100, "x":100, "y":100}),
        },
    );
    app.wait(std::time::Duration::from_millis(40)).unwrap();
    assert!(model.frame().camera.distance < before.distance);
}
