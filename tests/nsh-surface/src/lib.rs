//! Test-only NSH layout, focus, clipping and disposal around the product surface.

use nsh_core::prelude::*;
use oshwhub_nsh::viewer::surface::{SurfaceStatus, surface};

#[native_module("oshwhub/surface-probe")]
fn native(registry: &mut Registry) {
    registry.register::<SurfaceProbe>();
}

#[native_components(macos)]
fn components(components: &nsh_platform_macos::Components) {
    oshwhub_nsh::viewer::surface::register_macos(components);
}

#[native_components(ios)]
fn components_ios(components: &nsh_platform_ios::Components) {
    oshwhub_nsh::viewer::surface::register_ios(components);
}

#[native_components(windows)]
fn components_windows(components: &nsh_platform_windows::Components) {
    oshwhub_nsh::__nsh_native_components_windows(components);
}

#[component]
pub fn SurfaceProbe() -> impl IntoElement {
    let retained = signal(false);
    let drawing_shown = signal(false);
    view()
        .flex_1()
        .style_when(move || !drawing_shown.get(), |style| style.pt_6())
        .child(when(
            move || !drawing_shown.get(),
            move || {
                nsh_base::button()
                    .accessibility_label("Retained GPU")
                    .min_h(px(44.))
                    .on_press(move || retained.update(|value| *value = !*value))
                    .child(text("Retained GPU"))
            },
        ))
        .child(when(
            move || !retained.get(),
            move || basic_surface_probe(drawing_shown),
        ))
        .child(when(move || retained.get(), retained_surface_probe))
}

fn retained_surface_probe() -> impl IntoElement {
    use oshwhub_core::pcb::{Bounds3, Mesh, RawScene3d};
    use oshwhub_nsh::viewer::{
        camera3d::fit_camera,
        surface::{SurfaceModel, surface_with_model},
    };
    use std::sync::Arc;
    let board = Arc::new(RawScene3d {
        board: Arc::new(Mesh {
            vertices: [[-1., -1., 0.], [1., -1., 0.], [0., 1., 0.]]
                .into_iter()
                .flat_map(|p| p.into_iter().chain([0., 0., 1.]).chain([0.8, 0., 0.]))
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
    let load_model = Arc::clone(&model);
    let visible = signal(true);
    let tick = signal(0);
    let status = signal(SurfaceStatus::Pending);
    view()
        .flex_1()
        .p_6()
        .gap_4()
        .bg(rgb(0xffffff))
        .child(
            nsh_base::button()
                .min_h(px(44.))
                .accessibility_label(move || {
                    Some(
                        if visible.get() {
                            "Hide retained GPU"
                        } else {
                            "Show retained GPU"
                        }
                        .into(),
                    )
                })
                .on_press(move || visible.update(|value| *value = !*value))
                .child(text(move || {
                    if visible.get() {
                        "Hide retained GPU"
                    } else {
                        "Show retained GPU"
                    }
                })),
        )
        .child(
            nsh_base::button()
                .accessibility_label("Load retained board")
                .min_h(px(44.))
                .on_press(move || {
                    load_model.set_board(Arc::clone(&board));
                    tick.set(load_model.tick());
                })
                .child(text("Load retained board")),
        )
        .child(text(move || format!("Retained {:?}", status.get())))
        .child(
            view()
                .flex_1()
                .label("Retained GPU pixels")
                .style_when(move || !visible.get(), |style| style.invisible())
                .child(surface_with_model(status, model, tick)),
        )
}

fn basic_surface_probe(drawing_shown: Signal<bool>) -> impl IntoElement {
    let shown = signal(true);
    let pcb_shown = signal(false);
    let wide = signal(false);
    let status = signal(SurfaceStatus::Pending);
    view()
        .flex_1()
        .p_6()
        .gap_4()
        .bg(rgb(0xffffff))
        .child(
            nsh_base::button()
                .accessibility_label("Show drawing")
                .min_h(px(44.0))
                .on_press(move || drawing_shown.update(|value| *value = !*value))
                .child(text("Show drawing")),
        )
        .child(when(
            move || !drawing_shown.get(),
            move || {
                view()
                    .gap_4()
                    .child(
                        nsh_base::button()
                            .accessibility_label("Toggle GPU")
                            .min_h(px(44.0))
                            .on_press(move || shown.update(|value| *value = !*value))
                            .child(text("Toggle GPU")),
                    )
                    .child(
                        nsh_base::button()
                            .accessibility_label("Resize GPU")
                            .min_h(px(44.0))
                            .on_press(move || wide.update(|value| *value = !*value))
                            .child(text("Resize GPU")),
                    )
                    .child(text(move || format!("{:?}", status.get())).test_id("gpu-status"))
                    .child(when(
                        move || shown.get() && !pcb_shown.get() && !drawing_shown.get(),
                        move || {
                            view()
                                .relative()
                                .w(px(320.0))
                                .h(px(240.0))
                                .style_when(move || wide.get(), |style| style.w(px(480.0)))
                                .rounded(px(24.0))
                                .overflow_hidden()
                                .test_id("gpu-host")
                                .child(surface(status))
                                .child(
                                    view()
                                        .absolute()
                                        .top_0()
                                        .right_0()
                                        .w(px(64.0))
                                        .h(px(40.0))
                                        .bg(rgb(0xff0000))
                                        .test_id("gpu-overlay")
                                        .child(text("Overlay")),
                                )
                        },
                    ))
                    .child(
                        nsh_base::button()
                            .accessibility_label("After GPU")
                            .min_h(px(44.0))
                            .child(text("After GPU")),
                    )
                    .child(
                        nsh_base::button()
                            .accessibility_label("Show PCB")
                            .min_h(px(44.0))
                            .on_press(move || pcb_shown.update(|value| *value = !*value))
                            .child(text("Show PCB")),
                    )
            },
        ))
        .child(when(
            move || pcb_shown.get() && !drawing_shown.get(),
            || {
                oshwhub_nsh::viewer::three_dimensional::three_dimensional(
                    std::sync::Arc::new(oshwhub_core::pcb::demo()),
                    signal(true),
                )
            },
        ))
        .child(when(
            move || drawing_shown.get(),
            || {
                oshwhub_nsh::viewer::two_dimensional::two_dimensional(
                    std::sync::Arc::new(drawing_fixture()),
                    signal(true),
                )
            },
        ))
}

fn drawing_fixture() -> oshwhub_core::pcb::Scene {
    use oshwhub_core::pcb::{Bounds, DrawBatch, SceneImage, TextLabel};
    let mut scene = oshwhub_core::pcb::demo();
    scene.bounds = Bounds {
        min_x: 0.,
        min_y: 0.,
        max_x: 120.,
        max_y: 80.,
    };
    scene.batches = vec![DrawBatch {
        layer: 1,
        path: "M10 35H45V65H10Z M20 45V55H35V45Z".into(),
        fill: true,
        width: 0.,
        bounds: scene.bounds,
        color: Some("#ff6600".into()),
    }];
    scene.texts = vec![TextLabel {
        layer: 1,
        x: 10.,
        y: 20.,
        text: "中文原理图 ABC".into(),
        size: 6.,
        rotation: 0.,
        align: "left".into(),
        vertical_align: "".into(),
        color: Some("#ffffff".into()),
        bold: true,
    }];
    // Authored SVG: green background, white text above a white embedded PNG.
    scene.images = vec![SceneImage {
        layer: 1, width: 48., height: 36., transform: [1., 0., 0., 1., 60., 35.],
        content: concat!("data:image/svg+xml;base64,", "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSI2NCIgaGVpZ2h0PSI0OCI+PHJlY3Qgd2lkdGg9IjY0IiBoZWlnaHQ9IjQ4IiBmaWxsPSIjMDBmZjAwIi8+PHRleHQgeD0iNCIgeT0iMjAiIGZvbnQtc2l6ZT0iMTIiIGZpbGw9IndoaXRlIj5TVkfkuK3mloc8L3RleHQ+PGltYWdlIGhyZWY9ImRhdGE6aW1hZ2UvcG5nO2Jhc2U2NCxpVkJPUncwS0dnb0FBQUFOU1VoRVVnQUFBQUVBQUFBQkNBUUFBQUMxSEF3Q0FBQUFDMGxFUVZSNDJtUDgveDhBQXdNQ0FPK3lNYzhBQUFBQVNVVk9SSzVDWUlJPSIgeD0iNCIgeT0iMjgiIHdpZHRoPSIxNiIgaGVpZ2h0PSIxNiIvPjwvc3ZnPg==").into(),
    }];
    scene
}
