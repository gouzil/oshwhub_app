use oshwhub_core::pcb::{Bounds3, Mesh, RawModel3d, RawScene3d};
use oshwhub_nsh::viewer::surface::physical_size;
use oshwhub_nsh::viewer::{
    camera3d::{fit_camera, orbit_camera},
    surface::SurfaceModel,
};
use std::sync::Arc;

fn board() -> Arc<RawScene3d> {
    Arc::new(RawScene3d {
        board: Arc::new(Mesh {
            vertices: vec![0.; 27],
            indices: vec![0, 1, 2],
            bounds: Bounds3 {
                min: [0.; 3],
                max: [1.; 3],
            },
        }),
        model_keys: vec!["first".into()],
        component_count: 1,
        bound_count: 1,
        thickness: 1.6,
        warnings: vec![],
    })
}

// A camera frame must never clone/traverse the typed resource collection.
#[test]
fn camera_and_visibility_updates_leave_resources_untouched() {
    let board = board();
    let camera = fit_camera(&board.board.bounds, 1.);
    let state = SurfaceModel::new(camera);
    state.set_board(Arc::clone(&board));
    let model = Arc::new(RawModel3d {
        key: "first".into(),
        mesh_key: "shared".into(),
        mesh: Arc::clone(&board.board),
        instances: vec![],
    });
    state.add_model(Arc::clone(&model));
    let revision = state.resource_revision();
    let count = Arc::strong_count(&model);
    let tick = state.tick();
    state.set_camera(orbit_camera(camera, 50., 10.));
    state.set_active(false);
    state.set_components_visible(false);
    assert!(state.tick() > tick);
    assert_eq!(state.resource_revision(), revision);
    assert!(state.resources_since(revision).is_none());
    assert_eq!(Arc::strong_count(&model), count);
    assert!(!state.frame().active);
    assert!(!state.frame().components_visible);
    assert_ne!(state.frame().camera, camera);
}

#[test]
fn typed_registry_and_disposal_release_cpu_meshes() {
    let raw = board();
    let mesh = Arc::downgrade(&raw.board);
    let state = SurfaceModel::new(fit_camera(&raw.board.bounds, 1.));
    let handle = state.handle();
    state.set_board(raw);
    assert!(Arc::ptr_eq(&state, &SurfaceModel::resolve(handle).unwrap()));
    assert!(mesh.upgrade().is_some());
    state.dispose();
    assert!(SurfaceModel::resolve(handle).is_none());
    assert!(mesh.upgrade().is_none());
    assert!(!state.frame().active);
    let board = board();
    let weak = Arc::downgrade(&board);
    state.set_board(board);
    assert!(
        weak.upgrade().is_none(),
        "late async work cannot restore disposed resources"
    );
}

#[test]
fn native_host_resolves_engine_thread_models_and_observes_disposal() {
    let state = SurfaceModel::new(fit_camera(&board().board.bounds, 1.));
    state.set_board(board());
    let handle = state.handle();
    let (ready, observed) = std::sync::mpsc::channel();
    let (proceed, disposed) = std::sync::mpsc::channel();
    let host = std::thread::spawn(move || {
        let model = SurfaceModel::resolve(handle)
            .expect("native components run on a different thread from the Rust root");
        assert!(model.resources_since(u64::MAX).unwrap().board.is_some());
        ready.send(()).unwrap();
        disposed.recv().unwrap();
        assert!(!model.frame().active);
        assert!(model.resources_since(u64::MAX).is_none());
        assert!(SurfaceModel::resolve(handle).is_none());
    });
    observed
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    state.dispose();
    proceed.send(()).unwrap();
    host.join().unwrap();
}

#[test]
fn identical_resources_do_not_trigger_preparation_and_drop_unregisters() {
    let board1 = board();
    let state = SurfaceModel::new(fit_camera(&board1.board.bounds, 1.));
    let handle = state.handle();
    state.set_board(Arc::clone(&board1));
    let revision = state.resource_revision();
    let snapshot = state.resources_since(0).unwrap();
    assert_eq!(snapshot.revision, revision);
    assert!(Arc::ptr_eq(snapshot.board.as_ref().unwrap(), &board1));
    drop(snapshot);
    state.set_board(Arc::clone(&board1));
    assert_eq!(state.resource_revision(), revision);
    let board2 = board();
    state.set_board(Arc::clone(&board2));
    let model = Arc::new(RawModel3d {
        key: "same".into(),
        mesh_key: "mesh".into(),
        mesh: Arc::clone(&board2.board),
        instances: vec![],
    });
    state.add_model(Arc::clone(&model));
    let revision = state.resource_revision();
    state.add_model(Arc::new(RawModel3d {
        key: "same".into(),
        mesh_key: "mesh".into(),
        mesh: Arc::new(Mesh {
            vertices: vec![0.; 27],
            indices: vec![0, 1, 2],
            bounds: Bounds3 {
                min: [0.; 3],
                max: [1.; 3],
            },
        }),
        instances: vec![],
    }));
    assert_eq!(state.resource_revision(), revision);
    drop(state);
    assert!(SurfaceModel::resolve(handle).is_none());
}

#[cfg(target_os = "macos")]
#[test]
fn native_camera_props_carry_only_handle_and_tick_and_unmount_unregisters() {
    use nsh_core::{MountItem, element::Root, geometry::Size, prelude::*};
    use nsh_engine::EngineConfig;
    use nsh_testing::Headless;
    use oshwhub_nsh::viewer::surface::{SurfaceStatus, surface_with_model};
    use std::sync::Mutex;

    #[component]
    fn Fixture() -> impl IntoElement {
        let model = SurfaceModel::new(fit_camera(&board().board.bounds, 1.));
        model.set_board(board());
        let tick = signal(model.tick());
        let status = signal(SurfaceStatus::Pending);
        let shown = signal(true);
        let result = signal("registered");
        let handle = model.handle();
        let camera = Arc::clone(&model);
        view()
            .flex_1()
            .child(
                nsh_base::button()
                    .accessibility_label("camera")
                    .on_press(move || {
                        camera.set_camera(orbit_camera(camera.frame().camera, 12., 4.));
                        tick.set(camera.tick());
                    })
                    .child(text("camera")),
            )
            .child(
                nsh_base::button()
                    .accessibility_label("remove")
                    .on_press(move || shown.set(false))
                    .child(text("remove")),
            )
            .child(
                nsh_base::button()
                    .accessibility_label("inspect")
                    .on_press(move || {
                        result.set(if SurfaceModel::resolve(handle).is_none() {
                            "released"
                        } else {
                            "registered"
                        });
                    })
                    .child(text("inspect")),
            )
            .child(text(move || result.get()))
            .child(when(
                move || shown.get(),
                move || surface_with_model(status, Arc::clone(&model), tick),
            ))
    }
    let props = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&props);
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(480.), px(640.)),
        ..EngineConfig::new(Root::of::<Fixture>())
    })
    .unwrap();
    app.observe(move |_, batch| {
        for item in &batch.items {
            if let MountItem::ComponentProps { props, .. } = item {
                observed.lock().unwrap().extend(
                    props
                        .iter()
                        .map(|(name, value)| (name.to_string(), value.clone())),
                );
            }
        }
    });
    app.first_render().unwrap();
    let first = props
        .lock()
        .unwrap()
        .iter()
        .find_map(|(name, value)| (name == "model").then_some(value.clone()).flatten())
        .unwrap();
    assert_eq!(first.as_object().unwrap().len(), 2);
    let handle = first["handle"].as_u64().unwrap();
    props.lock().unwrap().clear();
    app.press("label:camera").unwrap();
    let props = props.lock().unwrap();
    assert_eq!(
        props.len(),
        1,
        "camera changes do not resend dimensions or resource data"
    );
    assert_eq!(props[0].0, "model");
    let frame = props[0].1.as_ref().unwrap();
    assert_eq!(frame["handle"], handle);
    assert!(frame["tick"].as_u64().unwrap() > first["tick"].as_u64().unwrap());
    assert!(frame.to_string().len() < 80);
    drop(props);
    app.press("label:remove").unwrap();
    app.press("label:inspect").unwrap();
    app.wait_for("text:released").unwrap();
}

#[test]
fn surface_uses_physical_pixels_and_suspends_zero_area() {
    assert_eq!(physical_size(100.25, 50.0, 2.0, 4096), Ok(Some([201, 100])));
    assert_eq!(physical_size(0.0, 50.0, 2.0, 4096), Ok(None));
}

#[test]
fn untrusted_or_unsupported_dimensions_never_reach_gpu_allocation() {
    for (width, height, scale) in [
        (f64::NAN, 50.0, 2.0),
        (100.0, f64::INFINITY, 1.0),
        (-1.0, 50.0, 1.0),
        (100.0, 50.0, 0.0),
        (100.0, 50.0, f64::INFINITY),
        (4097.0, 1.0, 1.0),
        (f64::MAX, 1.0, f64::MAX),
    ] {
        assert!(physical_size(width, height, scale, 4096).is_err());
    }
}
