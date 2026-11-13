#![cfg(target_os = "linux")]

use gtk::prelude::*;
use nsh_core::prelude::*;
use nsh_platform_linux::{AppOptions, AppView, Root, gtk4 as gtk};
use oshwhub_core::pcb::{Bounds3, Mesh, RawScene3d};
use oshwhub_nsh::viewer::{
    camera3d::fit_camera,
    surface::{SurfaceModel, SurfaceStatus, surface_with_model},
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

static STATUSES: Mutex<Vec<SurfaceStatus>> = Mutex::new(Vec::new());
static MODEL: Mutex<u64> = Mutex::new(0);

#[component]
fn Probe() -> impl IntoElement {
    let status = signal(SurfaceStatus::Pending);
    effect(move || STATUSES.lock().unwrap().push(status.get()));
    let board = Arc::new(RawScene3d {
        board: Arc::new(Mesh {
            vertices: [[-1., -1., 0.], [1., -1., 0.], [0., 1., 0.]]
                .into_iter()
                .flat_map(|p| p.into_iter().chain([0., 0., 1.]).chain([0.5, 0., 0.]))
                .collect(),
            indices: vec![0, 1, 2],
            bounds: Bounds3 {
                min: [-1., -1., 0.],
                max: [1., 1., 0.],
            },
        }),
        model_keys: vec![],
        component_count: 0,
        bound_count: 0,
        thickness: 1.6,
        warnings: vec![],
    });
    let model = SurfaceModel::new(fit_camera(&board.board.bounds, 1.));
    *MODEL.lock().unwrap() = model.handle();
    model.set_board(board);
    view().flex_1().bg(rgb(0xffffff)).child(
        view()
            .flex_1()
            .relative()
            .rounded(px(24.))
            .overflow_hidden()
            .child(surface_with_model(status, model, signal(0)))
            .child(
                view()
                    .absolute()
                    .top_0()
                    .right_0()
                    .w(px(64.))
                    .h(px(40.))
                    .bg(rgb(0xff0000)),
            ),
    )
}

fn wait(mut predicate: impl FnMut() -> bool) {
    let context = gtk::glib::MainContext::default();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "native surface timed out: {:?}",
            STATUSES.lock().unwrap()
        );
        if !context.iteration(false) {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

fn ready_size() -> Option<(String, u32, u32)> {
    STATUSES
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find_map(|status| match status {
            SurfaceStatus::Ready {
                backend,
                width,
                height,
            } => Some((backend.clone(), *width, *height)),
            _ => None,
        })
}

#[test]
#[ignore = "requires GTK and a display: xvfb-run -a cargo test -p oshwhub-nsh --test linux_surface -- --ignored"]
fn gtk_surface_draws_geometry_with_clipping_overlay_resize_and_disposal() {
    gtk::init().unwrap();
    nsh_platform_linux::set_ui_sets(vec![nsh_base::registry()]);
    let components = nsh_platform_linux::Components::default();
    oshwhub_nsh::__nsh_native_components_linux(&components);
    let app = AppView::new(AppOptions {
        components,
        ..Default::default()
    });
    let window = gtk::Window::new();
    window.set_default_size(400, 300);
    window.set_child(Some(&app.widget()));
    window.present();
    for cycle in 0..20 {
        STATUSES.lock().unwrap().clear();
        app.load(nsh_engine::App::Rust(Root::of::<Probe>()));
        wait(|| ready_size().is_some());
        let (backend, width, height) = ready_size().unwrap();
        assert_eq!(backend, "Gl", "Linux must use the GTK GPU context");
        assert!(width > 0 && height > 0);
        if cycle == 0 {
            window.set_default_size(480, 320);
            wait(|| ready_size().is_some_and(|(_, next, _)| next > width));
            let context = gtk::glib::MainContext::default();
            for _ in 0..50 {
                context.iteration(false);
                std::thread::sleep(Duration::from_millis(2));
            }
            let snapshot = gtk::Snapshot::new();
            let widget = app.widget();
            let paintable = gtk::WidgetPaintable::new(Some(&widget));
            paintable.snapshot(
                &snapshot,
                f64::from(widget.width()),
                f64::from(widget.height()),
            );
            let node = snapshot.to_node().unwrap();
            let renderer = gtk::gsk::Renderer::for_surface(&window.surface().unwrap()).unwrap();
            let texture = renderer.render_texture(&node, None);
            texture
                .save_to_png(std::env::temp_dir().join("oshwhub-nsh-linux-surface.png"))
                .unwrap();
            let stride = texture.width() as usize * 4;
            let mut pixels = vec![0; stride * texture.height() as usize];
            texture.download(&mut pixels, stride);
            renderer.unrealize();
            let pixel = |x: usize, y: usize| &pixels[y * stride + x * 4..][..4];
            assert_eq!(
                pixel(2, 2),
                &[255, 255, 255, 255],
                "rounded clip must show the parent"
            );
            assert_eq!(
                pixel(texture.width() as usize - 32, 20),
                &[0, 0, 255, 255],
                "NSH overlay must cover the GPU texture"
            );
            assert!(
                pixel(170, 230)[2] > 40 && pixel(170, 95)[2] < 30,
                "world +Y must point up, with the triangle's wide base below its apex"
            );
            assert!(
                pixels[50 * stride..]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[2] > 40 && p[0] < 30 && p[1] < 30)
                    .count()
                    > 200,
                "the red mesh must reach the GTK framebuffer below the overlay"
            );
        }
        let handle = *MODEL.lock().unwrap();
        app.stop();
        wait(|| SurfaceModel::resolve(handle).is_none());
    }
    window.close();
}
