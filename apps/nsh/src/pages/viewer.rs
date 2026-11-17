use super::ViewerRoute;
use crate::viewer::{three_dimensional::three_dimensional, two_dimensional::two_dimensional};
use nsh_base::{button, theme};
use nsh_core::{
    AccessibilityRole,
    element::{AnyElement, Owner, SignalEquals},
    prelude::*,
};
use oshwhub_core::{
    error::{AppError, Result},
    pcb::Scene,
};
use std::{cell::Cell, future::Future, pin::Pin, rc::Rc, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Pcb2d,
    Schematic,
    ThreeD,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Pcb,
    Schematic,
}

fn initial_mode(route: &ViewerRoute) -> Mode {
    match route {
        ViewerRoute::Demo => Mode::Pcb2d,
        ViewerRoute::Project {
            pcb_document_id,
            schematic_document_id,
            initial_schematic,
            ..
        } => {
            if *initial_schematic && schematic_document_id.is_some() {
                Mode::Schematic
            } else if pcb_document_id.is_some() {
                Mode::Pcb2d
            } else {
                Mode::Schematic
            }
        }
    }
}

fn source_for(route: &ViewerRoute, mode: Mode) -> Option<Source> {
    match route {
        ViewerRoute::Demo => (mode != Mode::Schematic).then_some(Source::Pcb),
        ViewerRoute::Project {
            pcb_document_id,
            schematic_document_id,
            ..
        } => match mode {
            Mode::Schematic => schematic_document_id.as_ref().map(|_| Source::Schematic),
            Mode::Pcb2d | Mode::ThreeD => pcb_document_id.as_ref().map(|_| Source::Pcb),
        },
    }
}

fn load_scene(route: ViewerRoute, source: Source) -> Pin<Box<dyn Future<Output = Result<Scene>>>> {
    Box::pin(async move {
        match route {
            ViewerRoute::Demo => Ok(oshwhub_core::pcb::demo()),
            ViewerRoute::Project {
                project_id,
                pcb_document_id,
                schematic_document_id,
                ..
            } => {
                let document_id = match source {
                    Source::Pcb => pcb_document_id,
                    Source::Schematic => schematic_document_id,
                }
                .ok_or_else(|| AppError::new("missing_document", "此模式没有公开的设计文件"))?;
                let client = crate::services::client()?;
                crate::services::run(async move {
                    match source {
                        Source::Pcb => {
                            oshwhub_core::pcb::load(&client, &project_id, &document_id).await
                        }
                        Source::Schematic => {
                            oshwhub_core::schematic::load(&client, &project_id, &document_id).await
                        }
                    }
                })
                .await
            }
        }
    })
}

fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Pcb2d => "PCB 2D",
        Mode::Schematic => "原理图",
        Mode::ThreeD => "PCB 3D",
    }
}

fn mode_button(
    mode: Mode,
    current: Signal<Mode>,
    request: Rc<dyn Fn(Mode)>,
    enabled: bool,
) -> AnyElement {
    let label = mode_label(mode);
    let request_mode = Rc::clone(&request);
    button()
        .accessibility_label(format!("切换到 {label}"))
        .role(AccessibilityRole::Tab)
        .selected(move || current.get() == mode)
        .disabled(!enabled)
        .min_h(px(44.0))
        .px_3()
        .py_2()
        .rounded(px(9.0))
        .bg(move || {
            if current.get() == mode {
                theme().accent
            } else {
                theme().muted
            }
        })
        .on_press(move || request_mode(mode))
        .child(text(label))
        .into_element()
}

/// The complete Rust viewer route. Scene data stays in an `Arc` and never crosses
/// the UI bridge during camera gestures or GPU uploads.
pub fn viewer(route: ViewerRoute, on_back: Rc<dyn Fn()>) -> AnyElement {
    viewer_with_loader(route, on_back, Rc::new(load_scene))
}

type SceneLoader = Rc<dyn Fn(ViewerRoute, Source) -> Pin<Box<dyn Future<Output = Result<Scene>>>>>;

fn viewer_with_loader(route: ViewerRoute, on_back: Rc<dyn Fn()>, load: SceneLoader) -> AnyElement {
    let mode = signal(initial_mode(&route));
    let pcb_scene = signal_with(None::<Arc<Scene>>, SignalEquals::Never);
    let schematic_scene = signal_with(None::<Arc<Scene>>, SignalEquals::Never);
    let three_started = signal(false);
    let loading = signal(true);
    let error = signal::<Option<String>>(None);
    let generation = Rc::new(Cell::new(0_u64));
    let task = stored::<Option<Spawned<()>>>(None);
    let owner = Owner::current();

    let request: Rc<dyn Fn(Mode)> = Rc::new({
        let route = route.clone();
        let generation = Rc::clone(&generation);
        move |next_mode| {
            owner.run(|| {
                mode.set(next_mode);
                if next_mode == Mode::ThreeD {
                    three_started.set(true);
                }
                generation.set(generation.get().wrapping_add(1));
                let ticket = generation.get();
                task.with(|task| {
                    if let Some(previous) = task.take() {
                        previous.cancel();
                    }
                });
                let Some(source) = source_for(&route, next_mode) else {
                    loading.set(false);
                    error.set(Some("此项目没有公开对应的设计文件".into()));
                    return;
                };
                let scene = match source {
                    Source::Pcb => pcb_scene,
                    Source::Schematic => schematic_scene,
                };
                error.set(None);
                if scene.get_untracked().is_some() {
                    loading.set(false);
                    return;
                }
                loading.set(true);
                let route = route.clone();
                let generation = Rc::clone(&generation);
                let load = Rc::clone(&load);
                let request = spawn(async move {
                    let result = load(route, source).await;
                    owner.run(|| {
                        if generation.get() != ticket {
                            return;
                        }
                        match result {
                            Ok(next) => {
                                scene.set(Some(Arc::new(next)));
                                error.set(None);
                            }
                            Err(failure) => {
                                error.set(Some(failure.message));
                            }
                        }
                        loading.set(false);
                    });
                });
                task.with(|task| *task = Some(request));
            })
        }
    });
    request(initial_mode(&route));

    let pcb_active = signal(false);
    let schematic_active = signal(false);
    let three_active = signal(false);
    effect(move || {
        let ready = !loading.get() && error.get().is_none();
        pcb_active.set(ready && mode.get() == Mode::Pcb2d);
        schematic_active.set(ready && mode.get() == Mode::Schematic);
        three_active.set(ready && mode.get() == Mode::ThreeD);
    });

    let route_for_modes = route.clone();
    let request_for_modes = Rc::clone(&request);
    let title = match &route {
        ViewerRoute::Demo => "PCB 示例板".to_owned(),
        ViewerRoute::Project { title, .. } => title.clone(),
    };
    view()
        .flex_1()
        .min_h(px(0.0))
        .bg(move || theme().background)
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .p_2()
                .border_b_1()
                .border_color(move || theme().border)
                .child(
                    button()
                        .accessibility_label("返回上一页")
                        .min_h(px(44.0))
                        .px_3()
                        .on_press(move || on_back())
                        .child(text("← 返回")),
                )
                .child(
                    text(title)
                        .role(AccessibilityRole::Header)
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .child(view().flex_row().flex_wrap().gap_1().children([
                    mode_button(
                        Mode::Pcb2d,
                        mode,
                        Rc::clone(&request_for_modes),
                        source_for(&route_for_modes, Mode::Pcb2d).is_some(),
                    ),
                    mode_button(
                        Mode::Schematic,
                        mode,
                        Rc::clone(&request_for_modes),
                        source_for(&route_for_modes, Mode::Schematic).is_some(),
                    ),
                    mode_button(
                        Mode::ThreeD,
                        mode,
                        request_for_modes,
                        source_for(&route_for_modes, Mode::ThreeD).is_some(),
                    ),
                ])),
        )
        .child(dynamic(move || {
            if loading.get() {
                return view()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(text(format!("正在准备 {}…", mode_label(mode.get()))))
                    .into_element();
            }
            if let Some(message) = error.get() {
                let retry = Rc::clone(&request);
                return view()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .p_5()
                    .child(text("查看器暂时无法加载").font_weight(FontWeight::SEMIBOLD))
                    .child(text(message).role(AccessibilityRole::Alert))
                    .child(
                        button()
                            .accessibility_label("重试加载设计文件")
                            .min_h(px(44.0))
                            .on_press(move || retry(mode.get()))
                            .child(text("重试")),
                    )
                    .into_element();
            }
            view().hidden().into_element()
        }))
        .children(
            [
                (Mode::Pcb2d, pcb_scene, pcb_active),
                (Mode::Schematic, schematic_scene, schematic_active),
                (Mode::ThreeD, pcb_scene, three_active),
            ]
            .map(|(target, scene, active)| {
                view()
                    .flex_1()
                    .min_h(px(0.))
                    .style_when(move || !active.get(), |s| s.hidden().invisible())
                    .accessibility_elements_hidden(move || !active.get())
                    .child(dynamic(move || {
                        if target == Mode::ThreeD && !three_started.get() {
                            return view().into_element();
                        }
                        let Some(scene) = scene.get() else {
                            return view().into_element();
                        };
                        match target {
                            Mode::Pcb2d => two_dimensional(scene, active, oshwhub_core::pcb::pick),
                            Mode::Schematic => {
                                two_dimensional(scene, active, oshwhub_core::schematic::pick)
                            }
                            Mode::ThreeD => three_dimensional(scene, active),
                        }
                    }))
            }),
        )
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsh_core::{Size, element::Root};
    use nsh_engine::EngineConfig;
    use nsh_testing::Headless;

    #[component]
    fn RetainedModes() -> impl IntoElement {
        viewer_with_loader(
            project(true, true, false),
            Rc::new(|| {}),
            Rc::new(|_, _| {
                Box::pin(async {
                    let mut scene = oshwhub_core::pcb::demo();
                    scene
                        .layers
                        .iter_mut()
                        .find(|layer| layer.id == 1)
                        .unwrap()
                        .name = "顶层".into();
                    Ok(scene)
                })
            }),
        )
    }

    #[test]
    fn switching_modes_retains_each_camera_and_layer_state() {
        let mut app = Headless::start(EngineConfig {
            viewport: Size::new(px(640.), px(800.)),
            ..EngineConfig::new(
                Root::of::<RetainedModes>().tokens(crate::app_theme().into_tokens()),
            )
        })
        .unwrap();
        app.first_render().unwrap();
        app.wait_for("label:二维画布").unwrap();
        app.press("label:放大二维画布").unwrap();
        app.wait_for("text:125%").unwrap();
        app.press("label:二维图层").unwrap();
        app.press("label:切换图层 顶层").unwrap();
        app.wait_for("text:☐ 顶层").unwrap();
        app.press("label:切换到 PCB 3D").unwrap();
        app.wait_for("label:三维画布").unwrap();
        app.press("label:切换到 PCB 2D").unwrap();
        app.wait_for("text:125%").unwrap();
        app.wait_for("text:☐ 顶层").unwrap();
        app.press("label:切换到 原理图").unwrap();
        app.wait_for("text:100%").unwrap();
        app.press("label:缩小二维画布").unwrap();
        app.wait_for("text:80%").unwrap();
        app.press("label:切换到 PCB 2D").unwrap();
        app.wait_for("text:125%").unwrap();
        app.press("label:切换到 原理图").unwrap();
        app.wait_for("text:80%").unwrap();
    }

    #[test]
    fn schematic_taps_query_the_schematic_cache() {
        let mut app = Headless::start(EngineConfig {
            viewport: Size::new(px(640.), px(800.)),
            ..EngineConfig::new(
                Root::of::<RetainedModes>().tokens(crate::app_theme().into_tokens()),
            )
        })
        .unwrap();
        app.first_render().unwrap();
        app.wait_for("label:二维画布").unwrap();
        app.press("label:切换到 原理图").unwrap();
        app.wait_for("label:二维画布").unwrap();
        // The fixture only caches PCB data, so the schematic picker must report its own miss.
        app.press_at(
            "label:二维画布",
            nsh_core::geometry::point(px(100.), px(100.)),
        )
        .unwrap();
        app.wait_for("text:场景已过期，请重新打开原理图").unwrap();
    }

    fn project(pcb: bool, schematic: bool, initial_schematic: bool) -> ViewerRoute {
        ViewerRoute::Project {
            project_id: "project".into(),
            pcb_document_id: pcb.then(|| "pcb".into()),
            schematic_document_id: schematic.then(|| "schematic".into()),
            title: "fixture".into(),
            initial_schematic,
        }
    }

    #[component]
    fn DemoModes() -> impl IntoElement {
        viewer(ViewerRoute::Demo, Rc::new(|| {}))
    }

    #[test]
    fn demo_keeps_the_existing_three_dimensional_mode() {
        let mut app = Headless::start(EngineConfig {
            viewport: Size::new(px(640.), px(800.)),
            ..EngineConfig::new(Root::of::<DemoModes>().tokens(crate::app_theme().into_tokens()))
        })
        .unwrap();
        app.first_render().unwrap();
        app.wait_for("label:二维画布").unwrap();
        app.press("label:放大二维画布").unwrap();
        app.press("label:切换到 PCB 3D").unwrap();
        app.wait_for("label:三维画布").unwrap();
        app.press("label:切换到 PCB 2D").unwrap();
        app.wait_for("text:125%").unwrap();
    }

    #[test]
    fn mode_selection_respects_published_documents() {
        assert_eq!(initial_mode(&ViewerRoute::Demo), Mode::Pcb2d);
        assert_eq!(initial_mode(&project(true, true, true)), Mode::Schematic);
        assert_eq!(initial_mode(&project(true, true, false)), Mode::Pcb2d);
        assert_eq!(initial_mode(&project(false, true, false)), Mode::Schematic);
        assert_eq!(
            source_for(&project(true, false, false), Mode::ThreeD),
            Some(Source::Pcb)
        );
        assert_eq!(source_for(&project(false, true, true), Mode::Pcb2d), None);
        assert_eq!(
            source_for(&ViewerRoute::Demo, Mode::ThreeD),
            Some(Source::Pcb)
        );
        assert_eq!(source_for(&ViewerRoute::Demo, Mode::Schematic), None);
    }
}
