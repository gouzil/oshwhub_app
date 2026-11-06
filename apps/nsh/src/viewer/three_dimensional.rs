//! Retained three-dimensional PCB viewer.  CPU meshes stay in the typed surface owner.

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
    sync::Arc,
};

use nsh_base::{button, theme};
use nsh_core::{
    TouchEvent, TouchPhase,
    element::{AnyElement, Owner, TouchClaim},
    geometry::Points,
    prelude::*,
};
use oshwhub_core::error::Result as CoreResult;
use oshwhub_core::pcb::{Bounds3, RawModel3d, RawScene3d, Scene};

use super::{
    camera3d::{Camera3d, fit_camera, orbit_camera, pan_camera, zoom_camera},
    surface::{SurfaceModel, SurfaceStatus, surface_with_model},
};
use crate::{components::action, services};

#[derive(Clone, Copy)]
struct Pointer {
    x: f64,
    y: f64,
    changed: bool,
}
struct Gesture {
    base: Camera3d,
    points: Vec<Pointer>,
}

type BoardLoader = Rc<dyn Fn(String) -> Pin<Box<dyn Future<Output = CoreResult<RawScene3d>>>>>;
type ModelLoader =
    Rc<dyn Fn(String, String) -> Pin<Box<dyn Future<Output = CoreResult<RawModel3d>>>>>;

struct ViewerState {
    owner: Owner,
    load_board: BoardLoader,
    load_model: ModelLoader,
    scene: Arc<Scene>,
    active: Signal<bool>,
    model: Arc<SurfaceModel>,
    tick: Signal<u64>,
    camera: RefCell<Camera3d>,
    gesture: RefCell<Option<Gesture>>,
    aspect: Cell<f64>,
    height: Cell<f64>,
    interacted: Cell<bool>,
    components_visible: Signal<bool>,
    status: Signal<SurfaceStatus>,
    loaded: Signal<usize>,
    total: Signal<usize>,
    error: Signal<Option<String>>,
    load_epoch: Cell<u64>,
}

impl ViewerState {
    fn publish(&self) {
        self.model.set_camera(*self.camera.borrow());
        self.model
            .set_components_visible(self.components_visible.get());
        self.tick.set(self.model.tick());
    }
    fn set_camera(&self, camera: Camera3d, manual: bool) {
        if manual {
            self.interacted.set(true);
        }
        *self.camera.borrow_mut() = camera;
        self.publish();
    }
    fn wheel(&self, delta: f64, x: f64, y: f64) -> bool {
        if !self.active.get_untracked() || ![delta, x, y].into_iter().all(f64::is_finite) {
            return false;
        }
        let factor = (-delta.clamp(-200., 200.) * 0.003).exp();
        self.zoom(factor);
        true
    }
    fn zoom(&self, factor: f64) {
        let camera = *self.camera.borrow();
        self.set_camera(zoom_camera(camera, factor), true);
    }
    fn touch(&self, event: &TouchEvent) {
        if !self.active.get_untracked() {
            return;
        }
        let points: Vec<_> = event
            .points
            .iter()
            .map(|p| Pointer {
                x: f64::from(p.location.x.0),
                y: f64::from(p.location.y.0),
                changed: p.changed,
            })
            .collect();
        if event.phase == TouchPhase::Cancel {
            self.gesture.borrow_mut().take();
            return;
        }
        if event.phase == TouchPhase::Start {
            self.gesture.replace(Some(Gesture {
                base: *self.camera.borrow(),
                points,
            }));
            return;
        }
        let mut gesture_ref = self.gesture.borrow_mut();
        let Some(gesture) = gesture_ref.as_mut() else {
            return;
        };
        let current: Vec<_> = points
            .iter()
            .copied()
            .filter(|p| event.phase != TouchPhase::End || !p.changed)
            .collect();
        if current.is_empty() {
            gesture_ref.take();
            return;
        }
        let next = if gesture.points.len() >= 2 && current.len() >= 2 {
            let a = gesture.points[0];
            let b = gesture.points[1];
            let c = current[0];
            let d = current[1];
            let old_mid = ((a.x + b.x) / 2., (a.y + b.y) / 2.);
            let new_mid = ((c.x + d.x) / 2., (c.y + d.y) / 2.);
            let old_distance = (a.x - b.x).hypot(a.y - b.y).max(1.);
            let new_distance = (c.x - d.x).hypot(c.y - d.y).max(1.);
            let mut camera = pan_camera(
                gesture.base,
                new_mid.0 - old_mid.0,
                new_mid.1 - old_mid.1,
                self.height.get(),
            );
            camera = zoom_camera(camera, new_distance / old_distance);
            camera
        } else if let (Some(start), Some(end)) = (gesture.points.first(), current.first()) {
            // A single finger orbits; two fingers pan and pinch above.
            orbit_camera(gesture.base, end.x - start.x, end.y - start.y)
        } else {
            gesture.base
        };
        self.set_camera(next, true);
    }
    fn layout(&self, width: f64, height: f64) {
        if width.is_finite() && height.is_finite() && width > 0. && height > 0. {
            self.aspect.set(width / height);
            self.height.set(height);
        }
    }
    fn fit(&self) {
        if let Some(bounds) = self
            .model
            .resources_since(0)
            .and_then(|resources| resources.board)
            .map(|board| board.board.bounds.clone())
        {
            self.interacted.set(false);
            self.set_camera(fit_camera(&bounds, self.aspect.get()), false);
        }
    }

    fn auto_fit_loaded(&self) {
        if self.interacted.get() {
            return;
        }
        let Some(resources) = self.model.resources_since(0) else {
            return;
        };
        let Some(board) = resources.board else {
            return;
        };
        let mut bounds = board.board.bounds.clone();
        for model in &resources.models {
            include_model(&mut bounds, model);
        }
        self.set_camera(fit_camera(&bounds, self.aspect.get()), false);
    }
}

fn include_model(bounds: &mut Bounds3, model: &RawModel3d) {
    for instance in &model.instances {
        let m = instance.matrix;
        for x in [model.mesh.bounds.min[0], model.mesh.bounds.max[0]] {
            for y in [model.mesh.bounds.min[1], model.mesh.bounds.max[1]] {
                for z in [model.mesh.bounds.min[2], model.mesh.bounds.max[2]] {
                    let point = [
                        f64::from(m[0]) * x
                            + f64::from(m[4]) * y
                            + f64::from(m[8]) * z
                            + f64::from(m[12]),
                        f64::from(m[1]) * x
                            + f64::from(m[5]) * y
                            + f64::from(m[9]) * z
                            + f64::from(m[13]),
                        f64::from(m[2]) * x
                            + f64::from(m[6]) * y
                            + f64::from(m[10]) * z
                            + f64::from(m[14]),
                    ];
                    for (axis, value) in point.into_iter().enumerate() {
                        bounds.min[axis] = bounds.min[axis].min(value);
                        bounds.max[axis] = bounds.max[axis].max(value);
                    }
                }
            }
        }
    }
}

fn retry_count() -> usize {
    3
}

fn start_loading(state: Rc<ViewerState>) {
    let owner = state.owner;
    owner.run(|| {
        let retrying = state.load_epoch.get() != 0;
        let epoch = state.load_epoch.get().wrapping_add(1);
        state.load_epoch.set(epoch);
        if retrying {
            // Wake the surface even when the previous board/model data is retained.
            state.model.retry_resources();
            state.tick.set(state.model.tick());
            state.status.set(SurfaceStatus::Pending);
        }
        state.error.set(None);
        state.loaded.set(0);
        state.total.set(0);
        let scene_id = state.scene.id.clone();
        let board_state = Rc::clone(&state);
        spawn(async move {
            let mut board_result = None;
            let mut failure = None;
            for _ in 0..retry_count() {
                if board_state.load_epoch.get() != epoch {
                    return;
                }
                let result = (board_state.load_board)(scene_id.clone()).await;
                if board_state.load_epoch.get() != epoch {
                    return;
                }
                match result {
                    Ok(board) => {
                        board_result = Some(board);
                        break;
                    }
                    Err(error) => failure = Some(error.message),
                }
            }
            let Some(board) = board_result else {
                board_state.error.set(failure);
                return;
            };
            let board = Arc::new(board);
            let keys = board.model_keys.clone();
            owner.run(|| {
                if board_state.load_epoch.get() != epoch {
                    return;
                }
                board_state.total.set(board.model_keys.len());
                board_state.model.set_board(Arc::clone(&board));
                board_state.tick.set(board_state.model.tick());
                if !board_state.interacted.get() {
                    board_state.set_camera(
                        fit_camera(&board.board.bounds, board_state.aspect.get()),
                        false,
                    );
                }
                board_state.status.set(SurfaceStatus::Pending);
            });
            let queue = Rc::new(RefCell::new(VecDeque::from(keys)));
            let workers = queue.borrow().len().min(4);
            for _ in 0..workers {
                let queue = Rc::clone(&queue);
                let state = Rc::clone(&board_state);
                let scene_id = scene_id.clone();
                spawn(async move {
                    loop {
                        if state.load_epoch.get() != epoch {
                            break;
                        }
                        let key = queue.borrow_mut().pop_front();
                        let Some(key) = key else {
                            break;
                        };
                        let mut result = None;
                        let mut failure = None;
                        for _ in 0..retry_count() {
                            if state.load_epoch.get() != epoch {
                                return;
                            }
                            let loaded = (state.load_model)(scene_id.clone(), key.clone()).await;
                            if state.load_epoch.get() != epoch {
                                return;
                            }
                            match loaded {
                                Ok(model) => {
                                    result = Some(model);
                                    break;
                                }
                                Err(error) => failure = Some(error.message),
                            }
                        }
                        if let Some(model) = result {
                            if state.load_epoch.get() != epoch {
                                return;
                            }
                            let model = Arc::new(model);
                            state.model.add_model(Arc::clone(&model));
                            state.auto_fit_loaded();
                            state.loaded.update(|loaded| *loaded += 1);
                            state.tick.set(state.model.tick());
                        } else {
                            state.error.set(failure);
                        }
                    }
                });
            }
        });
    });
}

type WheelHandler = dyn Fn(f64, f64, f64) -> bool;

thread_local! {
    static WHEEL_HANDLERS: RefCell<Vec<(String, Weak<WheelHandler>)>> = RefCell::new(Vec::new());
}

/// Native hosts can route a wheel event to the active scene without serializing its meshes.
pub fn native_wheel(scene_id: &str, delta: f64, x: f64, y: f64) -> bool {
    WHEEL_HANDLERS.with(|handlers| {
        handlers
            .borrow()
            .iter()
            .rev()
            .filter(|(id, _)| id == scene_id)
            .filter_map(|(_, handler)| handler.upgrade())
            .any(|handler| handler(delta, x, y))
    })
}

fn register_native_wheel(state: &Rc<ViewerState>) {
    let scene_id = state.scene.id.clone();
    let owner = Owner::current();
    let state = Rc::clone(state);
    let handler: Rc<WheelHandler> =
        Rc::new(move |delta, x, y| owner.run(|| state.wheel(delta, x, y)));
    WHEEL_HANDLERS.with(|handlers| {
        handlers
            .borrow_mut()
            .push((scene_id, Rc::downgrade(&handler)))
    });
    cleanup(move || {
        WHEEL_HANDLERS.with(|handlers| {
            handlers
                .borrow_mut()
                .retain(|(_, weak)| !weak.ptr_eq(&Rc::downgrade(&handler)))
        });
        drop(handler);
    });
}

#[cfg(target_arch = "wasm32")]
fn register_web_wheel(state: &Rc<ViewerState>) -> Result<(), String> {
    use js_sys::{Function, Object, Reflect};
    use std::{
        future::poll_fn,
        task::{Poll, Waker},
    };
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("浏览器文档尚未就绪")?;
    let queue = Rc::new(RefCell::new(VecDeque::<(f64, f64, f64)>::new()));
    let wake = Rc::new(RefCell::new(None::<Waker>));
    let enabled = Rc::new(Cell::new(false));
    {
        let enabled = Rc::clone(&enabled);
        let active = state.active;
        effect(move || enabled.set(active.get()));
    }
    {
        let queue = Rc::clone(&queue);
        let wake = Rc::clone(&wake);
        let state = Rc::clone(state);
        // DOM callbacks enqueue input; the owned task enters the reactive runtime.
        spawn(async move {
            loop {
                let (delta, x, y) = poll_fn(|cx| {
                    if let Some(input) = queue.borrow_mut().pop_front() {
                        Poll::Ready(input)
                    } else {
                        *wake.borrow_mut() = Some(cx.waker().clone());
                        Poll::Pending
                    }
                })
                .await;
                state.wheel(delta, x, y);
            }
        });
    }
    let listener = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
        if !enabled.get() {
            return;
        }
        let target = match Reflect::get(&event, &JsValue::from_str("target")) {
            Ok(target) => target,
            Err(_) => return,
        };
        let Ok(closest) = Reflect::get(&target, &JsValue::from_str("closest")) else {
            return;
        };
        let Ok(closest) = closest.dyn_into::<Function>() else {
            return;
        };
        let Ok(canvas) = closest.call1(
            &target,
            &JsValue::from_str("[aria-label=\"三维画布\"], [aria-label=\"三维绘图区域\"]"),
        ) else {
            return;
        };
        if canvas.is_null() || canvas.is_undefined() {
            return;
        }
        let n = |name: &str| {
            Reflect::get(&event, &JsValue::from_str(name))
                .ok()
                .and_then(|v| v.as_f64())
        };
        let unit = match n("deltaMode") {
            Some(1.) => 16.,
            Some(2.) => Reflect::get(&canvas, &JsValue::from_str("clientHeight"))
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(1.),
            _ => 1.,
        };
        let delta = n("deltaY").unwrap_or(0.) * unit;
        let x = n("clientX").unwrap_or(0.);
        let y = n("clientY").unwrap_or(0.);
        queue.borrow_mut().push_back((delta, x, y));
        if let Some(wake) = wake.borrow_mut().take() {
            wake.wake();
        }
        if let Ok(prevent) = Reflect::get(&event, &JsValue::from_str("preventDefault"))
            && let Ok(prevent) = prevent.dyn_into::<Function>()
        {
            let _ = prevent.call0(&event);
        }
    });
    let target: JsValue = document.into();
    let add = js_sys::Reflect::get(&target, &JsValue::from_str("addEventListener"))
        .map_err(|_| "无法注册浏览器滚轮")?;
    let add: Function = add.dyn_into().map_err(|_| "无法注册浏览器滚轮")?;
    let options = Object::new();
    Reflect::set(&options, &JsValue::from_str("capture"), &JsValue::TRUE)
        .map_err(|_| "无法配置浏览器滚轮")?;
    Reflect::set(&options, &JsValue::from_str("passive"), &JsValue::FALSE)
        .map_err(|_| "无法配置浏览器滚轮")?;
    add.call3(
        &target,
        &JsValue::from_str("wheel"),
        listener.as_ref(),
        &options,
    )
    .map_err(|_| "无法注册浏览器滚轮")?;
    cleanup(move || {
        if let Ok(remove) = js_sys::Reflect::get(&target, &JsValue::from_str("removeEventListener"))
            && let Ok(remove) = remove.dyn_into::<Function>()
        {
            let _ = remove.call3(
                &target,
                &JsValue::from_str("wheel"),
                listener.as_ref(),
                &JsValue::TRUE,
            );
        }
        drop(listener);
    });
    Ok(())
}

pub fn three_dimensional(scene: Arc<Scene>, active: Signal<bool>) -> AnyElement {
    three_dimensional_with_loaders(
        scene,
        active,
        Rc::new(|scene_id| Box::pin(services::run(oshwhub_core::pcb::prepare_3d_raw(scene_id)))),
        Rc::new(|scene_id, key| {
            Box::pin(async move {
                let client = services::client()?;
                services::run(async move {
                    oshwhub_core::pcb::model_3d_raw(&client, &scene_id, &key).await
                })
                .await
            })
        }),
    )
}

fn three_dimensional_with_loaders(
    scene: Arc<Scene>,
    active: Signal<bool>,
    load_board: BoardLoader,
    load_model: ModelLoader,
) -> AnyElement {
    let camera = fit_camera(
        &oshwhub_core::pcb::Bounds3 {
            min: [0.; 3],
            max: [1.; 3],
        },
        1.,
    );
    let model = SurfaceModel::new(camera);
    let tick = signal(model.tick());
    let state = Rc::new(ViewerState {
        owner: Owner::current(),
        load_board,
        load_model,
        scene,
        active,
        model: Arc::clone(&model),
        tick,
        camera: RefCell::new(camera),
        gesture: RefCell::new(None),
        aspect: Cell::new(1.),
        height: Cell::new(1.),
        interacted: Cell::new(false),
        components_visible: signal(true),
        status: signal(SurfaceStatus::Pending),
        loaded: signal(0),
        total: signal(0),
        error: signal(None),
        load_epoch: Cell::new(0),
    });
    register_native_wheel(&state);
    #[cfg(target_arch = "wasm32")]
    if let Err(error) = register_web_wheel(&state) {
        state.error.set(Some(error));
    }
    {
        let state = Rc::clone(&state);
        effect(move || state.model.set_active(active.get()));
    }
    {
        let status = state.status;
        let error = state.error;
        effect(move || {
            if let SurfaceStatus::Error(message) = status.get() {
                error.set(Some(message));
            }
        });
    }
    start_loading(Rc::clone(&state));
    let touch_state = Rc::clone(&state);
    let layout_state = Rc::clone(&state);
    let canvas_ref = element_ref();
    let mut toolbar = view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .p_2()
        .bg(move || theme().background);
    let fit_state = Rc::clone(&state);
    toolbar = toolbar.child(action(
        "适应",
        "适应三维场景",
        move || fit_state.fit(),
        || false,
    ));
    let zoom_in = Rc::clone(&state);
    toolbar = toolbar.child(action(
        "＋",
        "放大三维场景",
        move || zoom_in.zoom(1.25),
        || false,
    ));
    let zoom_out = Rc::clone(&state);
    toolbar = toolbar.child(action(
        "−",
        "缩小三维场景",
        move || zoom_out.zoom(0.8),
        || false,
    ));
    let components = Rc::clone(&state);
    let components_selected = components.components_visible;
    toolbar = toolbar.child(action(
        "器件",
        "显示三维器件",
        move || {
            let next = !components.components_visible.get();
            components.components_visible.set(next);
            components.publish();
        },
        move || components_selected.get(),
    ));
    let status = state.status;
    let loaded = state.loaded;
    let total = state.total;
    let error = state.error;
    let retry = Rc::clone(&state);
    let drawing = surface_with_model(status, Arc::clone(&model), tick);
    let canvas = view()
        .flex_1()
        .min_h(px(0.))
        .relative()
        .overflow_hidden()
        .element_ref(&canvas_ref)
        .focusable(true)
        .label("三维画布")
        .hint("单指旋转，双指平移和缩放；滚轮缩放")
        .claim_touches(TouchClaim::on_start())
        .keep_touches(true)
        .on_press(|| {})
        .on_touch(move |event: &TouchEvent| {
            canvas_ref.focus();
            touch_state.touch(event);
        })
        .on_layout(move |bounds: &nsh_core::Bounds<Points>| {
            layout_state.layout(
                f64::from(bounds.size.width.0),
                f64::from(bounds.size.height.0),
            )
        })
        .child(drawing);
    view()
        .flex_1()
        .min_h(px(0.))
        .child(toolbar)
        .child(canvas)
        .child(
            text(move || format!("{} / {} 个器件", loaded.get(), total.get()))
                .text_color(move || theme().muted_foreground),
        )
        .child(when(
            move || error.get().is_some(),
            move || {
                let retry_state = Rc::clone(&retry);
                view()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        text(error.get().unwrap_or_default())
                            .text_color(move || theme().destructive),
                    )
                    .child(
                        button()
                            .accessibility_label("重试三维模型")
                            .min_h(px(44.))
                            .on_press(move || start_loading(retry_state.clone()))
                            .child(text("重试")),
                    )
            },
        ))
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsh_core::{Size, element::Root};
    use nsh_engine::EngineConfig;
    use nsh_testing::Headless;

    fn board(keys: &[&str]) -> RawScene3d {
        RawScene3d {
            board: Arc::new(oshwhub_core::pcb::Mesh {
                vertices: vec![
                    0., 0., 0., 0., 0., 1., 1., 0., 0., 1., 0., 0., 0., 0., 1., 1., 0., 0., 0., 1.,
                    0., 0., 0., 1., 1., 0., 0.,
                ],
                indices: vec![0, 1, 2],
                bounds: Bounds3 {
                    min: [0.; 3],
                    max: [1.; 3],
                },
            }),
            model_keys: keys.iter().map(|key| (*key).into()).collect(),
            component_count: keys.len(),
            bound_count: keys.len(),
            thickness: 1.6,
            warnings: vec![],
        }
    }

    fn app<C: nsh_core::element::Component<Props = ()>>() -> Headless {
        let mut app = Headless::start(EngineConfig {
            viewport: Size::new(px(640.), px(800.)),
            ..EngineConfig::new(Root::of::<C>().tokens(crate::app_theme().into_tokens()))
        })
        .unwrap();
        app.first_render().unwrap();
        app
    }

    #[component]
    fn RetryFixture() -> impl IntoElement {
        let attempts = signal(0usize);
        view()
            .flex_1()
            .child(text(move || format!("板体请求 {}", attempts.get())))
            .child(three_dimensional_with_loaders(
                Arc::new(oshwhub_core::pcb::demo()),
                signal(true),
                Rc::new(move |_| {
                    let attempt = attempts.get_untracked() + 1;
                    attempts.set(attempt);
                    Box::pin(async move {
                        if attempt <= 3 {
                            Err(oshwhub_core::error::AppError::new("offline", "板体离线"))
                        } else {
                            services::run(async {
                                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                                Ok(board(&[]))
                            })
                            .await
                        }
                    })
                }),
                Rc::new(|_, _| unreachable!()),
            ))
    }

    #[test]
    fn retry_survives_removal_of_its_error_panel() {
        let mut app = app::<RetryFixture>();
        app.wait_for("text:板体离线").unwrap();
        app.wait_for("text:板体请求 3").unwrap();
        app.press("label:重试三维模型").unwrap();
        app.wait_for("text:板体请求 4").unwrap();
        assert!(!app.shows("板体离线"));
    }

    #[component]
    fn PartialFailureFixture() -> impl IntoElement {
        three_dimensional_with_loaders(
            Arc::new(oshwhub_core::pcb::demo()),
            signal(true),
            Rc::new(|_| Box::pin(async { Ok(board(&["failed", "ok"])) })),
            Rc::new(|_, key| {
                Box::pin(async move {
                    if key == "failed" {
                        Err(oshwhub_core::error::AppError::new("offline", "器件离线"))
                    } else {
                        Ok(RawModel3d {
                            key,
                            mesh_key: "mesh".into(),
                            mesh: board(&[]).board,
                            instances: vec![],
                        })
                    }
                })
            }),
        )
    }

    #[test]
    fn a_successful_model_does_not_hide_an_unresolved_failure() {
        let mut app = app::<PartialFailureFixture>();
        app.wait_for("text:1 / 2 个器件").unwrap();
        app.wait_for("text:器件离线").unwrap();
        assert!(app.screen().matches("label:重试三维模型").unwrap());
    }

    static LATE_MODEL: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>> =
        std::sync::Mutex::new(None);

    #[component]
    fn StaleFailureFixture() -> impl IntoElement {
        let attempts = Rc::new(Cell::new(0));
        let completed = signal(false);
        view()
            .flex_1()
            .child(text(move || {
                if completed.get() {
                    "迟到请求已结束"
                } else {
                    "等待迟到请求"
                }
            }))
            .child(three_dimensional_with_loaders(
                Arc::new(oshwhub_core::pcb::demo()),
                signal(true),
                Rc::new(move |_| {
                    attempts.set(attempts.get() + 1);
                    let first = attempts.get() == 1;
                    Box::pin(async move {
                        Ok(if first {
                            board(&["failed", "late"])
                        } else {
                            board(&[])
                        })
                    })
                }),
                Rc::new(move |_, key| {
                    let receiver = if key == "late" {
                        let (sender, receiver) = tokio::sync::oneshot::channel();
                        *LATE_MODEL.lock().unwrap() = Some(sender);
                        Some(receiver)
                    } else {
                        None
                    };
                    Box::pin(async move {
                        if let Some(receiver) = receiver {
                            receiver.await.unwrap();
                            completed.set(true);
                            Err(oshwhub_core::error::AppError::new("offline", "迟到失败"))
                        } else {
                            Err(oshwhub_core::error::AppError::new("offline", "可重试失败"))
                        }
                    })
                }),
            ))
    }

    #[test]
    fn a_late_model_failure_cannot_replace_a_successful_retry() {
        let mut app = app::<StaleFailureFixture>();
        app.wait_for("text:可重试失败").unwrap();
        app.press("label:重试三维模型").unwrap();
        app.wait_for("text:0 / 0 个器件").unwrap();
        LATE_MODEL.lock().unwrap().take().unwrap().send(()).unwrap();
        app.wait_for("text:迟到请求已结束").unwrap();
        assert!(!app.shows("迟到失败"));
        assert!(!app.screen().matches("label:重试三维模型").unwrap());
    }

    #[test]
    fn final_finger_release_clears_the_gesture() {
        let runtime = nsh_core::element::Runtime::default();
        let _entered = runtime.enter();
        let scene = Arc::new(oshwhub_core::pcb::demo());
        let camera = fit_camera(
            &Bounds3 {
                min: [0.; 3],
                max: [1.; 3],
            },
            1.,
        );
        let state = ViewerState {
            owner: Owner::current(),
            load_board: Rc::new(|_| unreachable!()),
            load_model: Rc::new(|_, _| unreachable!()),
            scene,
            active: signal(true),
            model: SurfaceModel::new(camera),
            tick: signal(0),
            camera: RefCell::new(camera),
            gesture: RefCell::new(Some(Gesture {
                base: camera,
                points: vec![Pointer {
                    x: 0.,
                    y: 0.,
                    changed: true,
                }],
            })),
            aspect: Cell::new(1.),
            height: Cell::new(1.),
            interacted: Cell::new(false),
            components_visible: signal(true),
            status: signal(SurfaceStatus::Pending),
            loaded: signal(0),
            total: signal(0),
            error: signal(None),
            load_epoch: Cell::new(0),
        };
        let event = TouchEvent {
            phase: TouchPhase::End,
            timestamp: 0.,
            points: vec![],
            activated: None,
        };
        state.touch(&event);
        assert!(state.gesture.borrow().is_none());
    }
}
