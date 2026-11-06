use oshwhub_core::pcb::Bounds;

use super::viewport::{
    Point, Viewport, fit_viewport, gesture_viewport, is_tap, world_point, zoom_at,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GesturePhase {
    Start,
    Move,
    End,
    Cancel,
}

#[derive(Clone, Copy)]
pub struct Pointer {
    pub id: u32,
    pub point: Point,
    pub changed: bool,
}

struct Gesture {
    base: Viewport,
    points: Vec<Pointer>,
    origin: Pointer,
    max_pointers: usize,
    moved: bool,
}

pub struct Camera {
    pub view: Viewport,
    bounds: Bounds,
    fitted: Viewport,
    size: Option<Point>,
    gesture: Option<Gesture>,
}

impl Camera {
    pub fn new(bounds: Bounds) -> Self {
        let view = Viewport {
            scale: 1.,
            x: 0.,
            y: 0.,
        };
        Self {
            view,
            bounds,
            fitted: view,
            size: None,
            gesture: None,
        }
    }

    pub fn layout(&mut self, width: f64, height: f64) {
        if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
            return;
        }
        let fitted = fit_viewport(&self.bounds, width, height, 24.);
        if let Some(size) = self.size {
            if size
                == (Point {
                    x: width,
                    y: height,
                })
            {
                return;
            }
            let center = world_point(
                self.view,
                Point {
                    x: size.x / 2.,
                    y: size.y / 2.,
                },
            );
            let scale = fitted.scale * self.view.scale / self.fitted.scale;
            self.view = Viewport {
                scale,
                x: width / 2. - center.x * scale,
                y: height / 2. - center.y * scale,
            };
        } else {
            self.view = fitted;
        }
        self.fitted = fitted;
        self.size = Some(Point {
            x: width,
            y: height,
        });
        self.gesture = None;
    }

    fn zoom(&mut self, factor: f64, at: Point) {
        self.view = zoom_at(
            self.view,
            self.view.scale * factor,
            at,
            self.fitted.scale / 20.,
            self.fitted.scale * 128.,
        );
        self.gesture = None;
    }

    pub fn wheel(&mut self, delta_y: f64, at: Point) {
        if delta_y.is_finite() && at.x.is_finite() && at.y.is_finite() {
            self.zoom((-delta_y.clamp(-200., 200.) * 0.003).exp(), at);
        }
    }

    pub fn key(&mut self, key: &str) {
        if !matches!(
            key,
            "+" | "=" | "-" | "0" | "left" | "right" | "up" | "down"
        ) {
            return;
        }
        self.cancel();
        let center = self
            .size
            .map(|size| Point {
                x: size.x / 2.,
                y: size.y / 2.,
            })
            .unwrap_or(Point { x: 0., y: 0. });
        match key {
            "+" | "=" => self.zoom(1.25, center),
            "-" => self.zoom(0.8, center),
            "0" => {
                self.view = self.fitted;
                self.gesture = None;
            }
            "left" => self.view.x -= 30.,
            "right" => self.view.x += 30.,
            "up" => self.view.y -= 30.,
            "down" => self.view.y += 30.,
            _ => {}
        }
    }

    pub fn cancel(&mut self) {
        self.gesture = None;
    }

    pub fn touch(&mut self, phase: GesturePhase, points: &[Pointer]) -> Option<Point> {
        if phase == GesturePhase::Cancel {
            self.cancel();
            return None;
        }
        let mut current: Vec<_> = points
            .iter()
            .copied()
            .filter(|point| phase != GesturePhase::End || !point.changed)
            .collect();
        current.sort_by_key(|point| point.id);
        if self.gesture.is_none() {
            if phase == GesturePhase::Start
                && let Some(&origin) = current.first()
            {
                self.gesture = Some(Gesture {
                    base: self.view,
                    points: current.clone(),
                    origin,
                    max_pointers: current.len(),
                    moved: false,
                });
            }
            return None;
        }
        let gesture = self.gesture.as_mut().unwrap();
        gesture.max_pointers = gesture.max_pointers.max(current.len());
        if let Some(point) = points.iter().find(|point| point.id == gesture.origin.id) {
            gesture.moved |= !is_tap(gesture.origin.point, point.point, 1);
        }
        if current.is_empty() {
            let gesture = self.gesture.take().unwrap();
            let end = points
                .iter()
                .find(|point| point.id == gesture.origin.id)?
                .point;
            return (phase == GesturePhase::End
                && !gesture.moved
                && is_tap(gesture.origin.point, end, gesture.max_pointers))
            .then_some(end);
        }
        if phase == GesturePhase::Move
            && current
                .iter()
                .map(|point| point.id)
                .eq(gesture.points.iter().map(|point| point.id))
        {
            let start: Vec<_> = gesture.points.iter().map(|point| point.point).collect();
            let end: Vec<_> = current.iter().map(|point| point.point).collect();
            self.view = gesture_viewport(
                gesture.base,
                &start,
                &end,
                self.fitted.scale / 20.,
                self.fitted.scale * 128.,
            );
        } else {
            gesture.base = self.view;
            gesture.points = current;
        }
        None
    }
}

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    sync::Arc,
};

use nsh_base::{button, theme};
use nsh_core::{
    TouchEvent, TouchPhase,
    canvas::{CanvasImage, CanvasOp, Drawing, Num, Paint, PathOp},
    element::{
        AnimatedValue, AnyElement, Owner, Painter, Signal, SignalEquals, Stored, TouchClaim,
        canvas, signal_with, text_metrics,
    },
    geometry::{DevicePixels, Points, Size},
    prelude::*,
};
use oshwhub_core::pcb::{PickResult, Scene, SceneImage};

use super::{
    cache::SceneCache,
    drawing::prepare_scene,
    raster::{RasterPlan, plan_raster, refine_raster},
};
use crate::components::action;

#[derive(Clone)]
struct ImageSource {
    uri: Arc<str>,
    #[cfg(not(target_arch = "wasm32"))]
    svg: Option<Arc<super::embedded_image::Svg>>,
}

struct ViewerState {
    scene: Arc<Scene>,
    active: Signal<bool>,
    camera: RefCell<Camera>,
    x: AnimatedValue,
    y: AnimatedValue,
    scale: AnimatedValue,
    zoom: Signal<f64>,
    overview: RasterPlan,
    raster_view: Signal<Option<(Viewport, Point)>>,
    detail: Signal<Option<RasterPlan>>,
    cache: Stored<SceneCache>,
    drawing: Signal<Arc<Drawing>>,
    image_sources: Signal<Option<Result<Arc<[ImageSource]>, String>>>,
    selected: Signal<Option<Arc<PickResult>>>,
    error: Signal<Option<String>>,
    layers_open: Signal<bool>,
    properties_open: Signal<bool>,
}

impl ViewerState {
    fn publish_camera(&self) {
        let camera = self.camera.borrow();
        self.x.set(camera.view.x as f32);
        self.y.set(camera.view.y as f32);
        self.scale.set(camera.view.scale as f32);
        self.zoom.set(camera.view.scale / camera.fitted.scale);
        self.raster_view.set(
            camera
                .size
                .filter(|_| camera.gesture.is_none())
                .map(|size| (camera.view, size)),
        );
    }

    fn key(&self, key: &str) {
        if self.active.get_untracked() {
            self.camera.borrow_mut().key(key);
            self.publish_camera();
        }
    }

    fn touch(&self, event: &TouchEvent) {
        if !self.active.get_untracked() {
            return;
        }
        let phase = match event.phase {
            TouchPhase::Start => GesturePhase::Start,
            TouchPhase::Move => GesturePhase::Move,
            TouchPhase::End => GesturePhase::End,
            TouchPhase::Cancel => GesturePhase::Cancel,
        };
        let points: Vec<_> = event
            .points
            .iter()
            .map(|point| Pointer {
                id: point.id,
                point: Point {
                    x: f64::from(point.location.x.0),
                    y: f64::from(point.location.y.0),
                },
                changed: point.changed,
            })
            .collect();
        let tap = self.camera.borrow_mut().touch(phase, &points);
        self.publish_camera();
        if let Some(at) = tap {
            let camera = self.camera.borrow();
            let world = world_point(camera.view, at);
            let visible = self
                .cache
                .with(|cache| cache.visible.iter().copied().collect::<Vec<_>>());
            match oshwhub_core::pcb::pick(
                &self.scene.id,
                world.x,
                world.y,
                (8. / camera.view.scale).min(1e6),
                Some(&visible),
            ) {
                Ok(result) => {
                    if result.is_some() {
                        self.layers_open.set(false);
                    }
                    self.properties_open.set(result.is_some());
                    self.selected.set(result.map(Arc::new));
                    self.error.set(self.cache.with(|cache| cache.error.clone()));
                }
                Err(error) => self.error.set(Some(error.message)),
            }
        }
    }

    fn layer(&self, id: i32) {
        if !self.active.get_untracked() {
            return;
        }
        let result = self.cache.with(|cache| {
            cache.set_visible(id, !cache.visible.contains(&id))?;
            Ok::<_, String>(Arc::clone(&cache.drawing))
        });
        match result {
            Ok(drawing) => {
                self.drawing.set(drawing);
                if self.selected.with_untracked(|selected| {
                    selected
                        .as_ref()
                        .is_some_and(|selected| selected.layer == id)
                }) {
                    self.selected.set(None);
                }
            }
            Err(error) => self.error.set(Some(error)),
        }
    }
}

type WheelHandler = dyn Fn(f64, f64, f64) -> bool;
thread_local! {
    static WHEEL_HANDLERS: RefCell<Vec<(String, Weak<WheelHandler>)>> = RefCell::new(Vec::new());
}

/// Called by a host on the entered NSH engine thread; coordinates are canvas-local logical pixels.
pub fn native_wheel(scene_id: &str, delta_y: f64, x: f64, y: f64) -> bool {
    let handlers = WHEEL_HANDLERS.with(|handlers| {
        handlers
            .borrow()
            .iter()
            .rev()
            .filter(|(id, _)| id == scene_id)
            .filter_map(|(_, handler)| handler.upgrade())
            .collect::<Vec<_>>()
    });
    handlers.into_iter().any(|handler| handler(delta_y, x, y))
}

fn register_wheel(state: &Rc<ViewerState>) {
    let state = Rc::clone(state);
    let scene_id = state.scene.id.clone();
    let owner = Owner::current();
    let handler: Rc<WheelHandler> = Rc::new(move |delta, x, y| {
        owner.run(|| {
            if !state.active.get_untracked() || ![delta, x, y].into_iter().all(f64::is_finite) {
                return false;
            }
            state.camera.borrow_mut().wheel(delta, Point { x, y });
            state.publish_camera();
            true
        })
    });
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

fn selection_painter(state: &ViewerState) -> Painter {
    let Some(selected) = state.selected.get() else {
        return Painter::new();
    };
    let mut painter = Painter::new();
    let x = painter.num(&state.x);
    let y = painter.num(&state.y);
    let scale = painter.num(&state.scale);
    painter = painter
        .op(CanvasOp::Save)
        .op(CanvasOp::Translate(x, y))
        .op(CanvasOp::Scale(scale, scale));
    {
        let b = selected.bounds;
        let n = |value: f64| Num::Const(value as f32);
        let path = Arc::from([
            PathOp::MoveTo(n(b.min_x), n(b.min_y)),
            PathOp::LineTo(n(b.max_x), n(b.min_y)),
            PathOp::LineTo(n(b.max_x), n(b.max_y)),
            PathOp::LineTo(n(b.min_x), n(b.max_y)),
            PathOp::Close,
        ]);
        painter = painter
            .op(CanvasOp::StrokeStyle(Paint::Color(rgba(0xffc857ff).into())))
            .op(CanvasOp::LineWidth(Num::Const(1.)))
            .op(CanvasOp::Stroke(Some(path)));
    }
    painter.op(CanvasOp::Restore)
}

fn raster_layer(state: &Rc<ViewerState>, plan: RasterPlan, id: &'static str) -> AnyElement {
    #[cfg(target_arch = "wasm32")]
    {
        let drawing = state.drawing;
        raster_canvas(state, plan, id, move || drawing.get())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let prepared = signal(None::<Arc<Drawing>>);
        let job_state = Rc::clone(state);
        effect(move || {
            let drawing = job_state.drawing.get();
            let sources = job_state.image_sources.get();
            if !job_state.active.get() {
                return;
            }
            let Some(Ok(sources)) = sources else {
                return;
            };
            let images = sources
                .iter()
                .filter_map(|source| {
                    source
                        .svg
                        .as_ref()
                        .map(|svg| (Arc::clone(&source.uri), Arc::clone(svg)))
                })
                .collect::<Vec<_>>();
            if images.is_empty() {
                prepared.set(Some(drawing));
                return;
            }
            let error = job_state.error;
            // This effect only follows drawing/lifecycle changes, so disposal cancels its work.
            spawn(async move {
                let result = super::embedded_image::raster_drawing(&drawing, &images, plan).await;
                match result {
                    Ok(drawing) => prepared.set(Some(Arc::new(drawing))),
                    Err(message) => error.set(Some(message)),
                }
            });
        });
        let state = Rc::clone(state);
        dynamic(move || {
            let Some(drawing) = prepared.get() else {
                return view().into_element();
            };
            let images = drawing
                .ops
                .iter()
                .filter_map(|op| match op {
                    CanvasOp::DrawImage { image, .. } => Some(Arc::clone(&image.uri)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if images.is_empty() {
                return raster_canvas(&state, plan, id, move || Arc::clone(&drawing));
            }
            let loaded = signal(vec![false; images.len()]);
            let error = state.error;
            let nodes = images
                .into_iter()
                .enumerate()
                .map(|(index, uri)| {
                    image(nsh_core::element::SharedString::from(uri))
                        .w(px(1.))
                        .h(px(1.))
                        .opacity(0.)
                        .on_load(move |_: &Size<DevicePixels>| {
                            loaded.update(|ready| ready[index] = true)
                        })
                        .on_error(move |message: &str| error.set(Some(message.to_owned())))
                        .into_element()
                })
                .collect::<Vec<_>>();
            let state = Rc::clone(&state);
            view()
                .absolute()
                .left(px(0.))
                .top(px(0.))
                .right(px(0.))
                .bottom(px(0.))
                .pointer_events(nsh_core::PointerEvents::None)
                .child(view().absolute().w(px(1.)).h(px(1.)).children(nodes))
                .child(when(
                    move || loaded.with(|ready| ready.iter().all(|ready| *ready)),
                    move || {
                        let drawing = Arc::clone(&drawing);
                        raster_canvas(&state, plan, id, move || Arc::clone(&drawing))
                    },
                ))
                .into_element()
        })
        .into_element()
    }
}

fn raster_canvas(
    state: &Rc<ViewerState>,
    plan: RasterPlan,
    id: &'static str,
    drawing: impl Fn() -> Arc<Drawing> + 'static,
) -> AnyElement {
    let ratio = f64::from(pixel_ratio()).max(0.01);
    let width = f64::from(plan.pixel_width) / ratio;
    let height = f64::from(plan.pixel_height) / ratio;
    canvas(move || {
        let n = |value: f64| Num::Const(value as f32);
        let mut painter = Painter::new()
            .op(CanvasOp::Save)
            .op(CanvasOp::Scale(
                n(width / plan.width),
                n(height / plan.height),
            ))
            .op(CanvasOp::Translate(n(-plan.x), n(-plan.y)));
        for op in &drawing().ops {
            painter = painter.op(op.clone());
        }
        painter.op(CanvasOp::Restore)
    })
    .absolute()
    .left(px(0.))
    .top(px(0.))
    .w(px(width as f32))
    .h(px(height as f32))
    .origin_top_left()
    .bind_translate_x(&state.x.add(&state.scale.mul(plan.x as f32)))
    .bind_translate_y(&state.y.add(&state.scale.mul(plan.y as f32)))
    .bind_transform(
        nsh_core::TransformKind::ScaleX,
        &state.scale.mul((plan.width / width) as f32),
    )
    .bind_transform(
        nsh_core::TransformKind::ScaleY,
        &state.scale.mul((plan.height / height) as f32),
    )
    .bg(rgba(0x111827ff))
    .pointer_events(nsh_core::PointerEvents::None)
    .test_id(id)
    .into_element()
}

/// Camera transforms stay outside the bounded overview/detail vector drawings.
pub fn two_dimensional(scene: Arc<Scene>, active: Signal<bool>) -> AnyElement {
    let state = Rc::new(ViewerState {
        camera: RefCell::new(Camera::new(scene.bounds)),
        overview: plan_raster(&scene.bounds, f64::MAX),
        scene,
        active,
        x: animated_value(0.),
        y: animated_value(0.),
        scale: animated_value(1.),
        zoom: signal(1.),
        raster_view: signal(None),
        detail: signal(None),
        cache: stored(SceneCache::default()),
        drawing: signal(Arc::new(Drawing::default())),
        image_sources: signal_with(None, SignalEquals::Never),
        selected: signal_with(None, SignalEquals::Never),
        error: signal(None),
        layers_open: signal(false),
        properties_open: signal(false),
    });
    register_wheel(&state);
    #[cfg(target_arch = "wasm32")]
    if let Err(error) = install_web_wheel(&state) {
        state.error.set(Some(error));
    }
    let image_sources = state.image_sources;
    let image_sizes = signal(vec![
        None::<Result<[u32; 2], String>>;
        state.scene.images.len()
    ]);
    if state.scene.images.is_empty() {
        image_sources.set(Some(Ok(Arc::from([]))));
    } else {
        let scene = Arc::clone(&state.scene);
        spawn(async move {
            let sources: Result<Arc<[ImageSource]>, String> = async move {
                let mut sources = Vec::with_capacity(scene.images.len());
                for index in 0..scene.images.len() {
                    #[cfg(not(target_arch = "wasm32"))]
                    let source = {
                        let scene = Arc::clone(&scene);
                        crate::services::compute(move || {
                            let image = &scene.images[index];
                            validate_embedded_image(image)
                                .and_then(|_| super::embedded_image::Svg::parse(&image.content))
                                .map(|svg| ImageSource {
                                    uri: Arc::from(image.content.as_str()),
                                    svg,
                                })
                                .map_err(|error| {
                                    oshwhub_core::error::AppError::new("INVALID_IMAGE", error)
                                })
                        })
                        .await
                        .map_err(|error| error.message)?
                    };
                    #[cfg(target_arch = "wasm32")]
                    let source = {
                        let image = &scene.images[index];
                        validate_embedded_image(image)?;
                        ImageSource {
                            uri: Arc::from(image.content.as_str()),
                        }
                    };
                    sources.push(source);
                }
                Ok(Arc::from(sources))
            }
            .await;
            #[cfg(not(target_arch = "wasm32"))]
            if let Ok(sources) = &sources {
                image_sizes.update(|sizes| {
                    for (index, source) in sources.iter().enumerate() {
                        if let Some(svg) = &source.svg {
                            sizes[index] = Some(Ok(svg.size()));
                        }
                    }
                });
            }
            image_sources.set(Some(sources));
        });
    }
    {
        let state = Rc::clone(&state);
        effect(move || {
            let sources = match image_sources.get() {
                None => return,
                Some(Ok(sources)) => sources,
                Some(Err(error)) => {
                    state.cache.with(|cache| {
                        cache.replace(Err(error.clone()));
                    });
                    state.error.set(Some(error));
                    return;
                }
            };
            let sizes = image_sizes.get();
            if let Some(error) = sizes
                .iter()
                .find_map(|size| size.as_ref().and_then(|result| result.as_ref().err()))
            {
                state.cache.with(|cache| {
                    cache.replace(Err(error.clone()));
                });
                state.error.set(Some(error.clone()));
                return;
            }
            if sizes.iter().any(Option::is_none) {
                return;
            }
            let result = prepare_scene(
                &state.scene,
                |font| {
                    let css = format!("{} {}px sans-serif", font.weight, font.size.0);
                    let metrics = text_metrics(&css, "Mg中文");
                    if metrics.ascent.0 > 0. && metrics.descent.0 >= 0. {
                        Ok(metrics)
                    } else {
                        Err("系统字体尚未就绪".into())
                    }
                },
                |image| {
                    let index = state
                        .scene
                        .images
                        .iter()
                        .position(|candidate| std::ptr::eq(candidate, image))
                        .ok_or("图片不属于当前场景")?;
                    let size = sizes[index].as_ref().ok_or("图片尚未解码")?.clone()?;
                    Ok((
                        CanvasImage {
                            uri: Arc::clone(&sources[index].uri),
                        },
                        size,
                    ))
                },
            );
            let (success, drawing, error) = state.cache.with(|cache| {
                let success = cache.replace(result);
                (success, Arc::clone(&cache.drawing), cache.error.clone())
            });
            if success {
                state.drawing.set(drawing);
            }
            state.error.set(error);
        });
    }
    {
        let state = Rc::clone(&state);
        effect(move || {
            let view = state.raster_view.get();
            let ratio = f64::from(pixel_ratio()) * 1.5;
            if !active.get() {
                return;
            }
            let Some((view, size)) = view else {
                return;
            };
            let base = state.overview;
            let detail = state.detail;
            set_timeout(std::time::Duration::from_millis(180), move || {
                detail.set(refine_raster(
                    base,
                    detail.get_untracked(),
                    view,
                    size.x,
                    size.y,
                    ratio,
                ));
            });
        });
    }
    {
        let state = Rc::clone(&state);
        effect(move || {
            if !active.get() {
                state.camera.borrow_mut().cancel();
                state.publish_camera();
            }
        });
    }
    let mut toolbar = view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .p_2()
        .bg(move || theme().background);
    for (title, label, key) in [
        ("适应", "适应二维画布", "0"),
        ("＋", "放大二维画布", "+"),
        ("−", "缩小二维画布", "-"),
    ] {
        let state = Rc::clone(&state);
        toolbar = toolbar.child(action(title, label, move || state.key(key), || false));
    }
    let layers_open = state.layers_open;
    let properties_open = state.properties_open;
    let zoom = state.zoom;
    toolbar = toolbar
        .child(action(
            "图层",
            "二维图层",
            move || {
                if active.get_untracked() {
                    properties_open.set(false);
                    layers_open.set(!layers_open.get_untracked());
                }
            },
            move || layers_open.get(),
        ))
        .child(action(
            "属性",
            "二维属性",
            move || {
                if active.get_untracked() {
                    layers_open.set(false);
                    properties_open.set(!properties_open.get_untracked());
                }
            },
            move || properties_open.get(),
        ))
        .child(
            text(move || format!("{:.0}%", zoom.get() * 100.))
                .text_size(px(12.))
                .text_color(move || theme().muted_foreground),
        );
    let painter_state = Rc::clone(&state);
    let overview_state = Rc::clone(&state);
    let detail_state = Rc::clone(&state);
    let touch_state = Rc::clone(&state);
    let layout_state = Rc::clone(&state);
    let canvas_ref = element_ref();
    let drawing = view()
        .flex_1()
        .min_h(px(0.))
        .relative()
        .overflow_hidden()
        .child(dynamic(move || {
            raster_layer(&overview_state, overview_state.overview, "pcb-overview")
        }))
        .child(dynamic(move || match detail_state.detail.get() {
            Some(plan) => raster_layer(&detail_state, plan, "pcb-detail"),
            None => view().into_element(),
        }))
        .child(
            canvas(move || selection_painter(&painter_state))
                .absolute()
                .left(px(0.))
                .right(px(0.))
                .top(px(0.))
                .bottom(px(0.))
                .pointer_events(nsh_core::PointerEvents::None),
        )
        .element_ref(&canvas_ref)
        .label("二维画布")
        .hint("单指拖动平移，双指缩放；方向键平移，0 适应，减号和等号缩放")
        .test_id("pcb-canvas")
        .focusable(true)
        .claim_touches(TouchClaim::on_start())
        .keep_touches(true)
        .on_press(|| {})
        .on_touch(move |event: &TouchEvent| {
            canvas_ref.focus();
            touch_state.touch(event);
        })
        .on_layout(move |bounds: &nsh_core::Bounds<Points>| {
            layout_state.camera.borrow_mut().layout(
                f64::from(bounds.size.width.0),
                f64::from(bounds.size.height.0),
            );
            layout_state.publish_camera();
        });
    let keyboard_state = Rc::clone(&state);
    let error = state.error;
    let loading_count = state.scene.images.len();
    let layers_state = Rc::clone(&state);
    let properties_state = Rc::clone(&state);
    let image_nodes = state
        .scene
        .images
        .iter()
        .enumerate()
        .map(|(index, _)| {
            dynamic(move || {
                let Some(Ok(sources)) = image_sources.get() else {
                    return view().into_element();
                };
                #[cfg(not(target_arch = "wasm32"))]
                if sources[index].svg.is_some() {
                    return view().into_element();
                }
                image(nsh_core::element::SharedString::from(Arc::clone(
                    &sources[index].uri,
                )))
                .w(px(1.))
                .h(px(1.))
                .opacity(0.)
                .on_load(move |size: &Size<DevicePixels>| {
                    image_sizes.update(|sizes| {
                        sizes[index] = Some(Ok([
                            u32::try_from(size.width.0).unwrap_or(0),
                            u32::try_from(size.height.0).unwrap_or(0),
                        ]))
                    });
                })
                .on_error(move |message: &str| {
                    image_sizes.update(|sizes| sizes[index] = Some(Err(message.to_owned())));
                })
                .into_element()
            })
            .into_element()
        })
        .collect::<Vec<_>>();
    view()
        .flex_1()
        .min_h(px(0.))
        .child(toolbar)
        .child(
            view()
                .flex_1()
                .min_h(px(0.))
                .relative()
                .overflow_hidden()
                .bg(rgba(0x111827ff))
                .child(drawing)
                .child(
                    view()
                        .absolute()
                        .left(px(0.))
                        .top(px(0.))
                        .w(px(1.))
                        .h(px(1.))
                        .children(image_nodes),
                )
                .child(when(
                    move || layers_open.get(),
                    move || layer_panel(&layers_state),
                ))
                .child(when(
                    move || properties_open.get(),
                    move || property_panel(&properties_state),
                ))
                .child(when(
                    move || error.get().is_some(),
                    move || {
                        view()
                            .absolute()
                            .left(px(8.))
                            .right(px(8.))
                            .bottom(px(8.))
                            .p_3()
                            .bg(move || theme().background)
                            .child(
                                text(move || error.get().unwrap_or_default())
                                    .text_color(move || theme().destructive),
                            )
                    },
                ))
                .child(when(
                    move || {
                        loading_count > 0
                            && !image_sources.with(|sources| matches!(sources, Some(Err(_))))
                            && image_sizes.with(|sizes| {
                                sizes.iter().any(Option::is_none)
                                    && !sizes.iter().any(|size| matches!(size, Some(Err(_))))
                            })
                    },
                    || {
                        view()
                            .absolute()
                            .left(px(8.))
                            .bottom(px(8.))
                            .p_2()
                            .bg(move || theme().background)
                            .child(text("正在读取嵌入图片…"))
                    },
                )),
        )
        .child(when(
            move || active.get(),
            move || {
                for (stroke, key) in [
                    ("=", "+"),
                    ("shift+=", "+"),
                    ("-", "-"),
                    ("0", "0"),
                    ("left", "left"),
                    ("right", "right"),
                    ("up", "up"),
                    ("down", "down"),
                ] {
                    let state = Rc::clone(&keyboard_state);
                    on_key(stroke, move |_| state.key(key));
                }
                view().w(px(0.)).h(px(0.))
            },
        ))
        .into_element()
}

fn layer_panel(state: &Rc<ViewerState>) -> AnyElement {
    let mut list = view()
        .gap_1()
        .p_2()
        .child(text("图层").font_weight(FontWeight::SEMIBOLD));
    for layer in &state.scene.layers {
        let id = layer.id;
        let name = layer.name.clone();
        let label = format!("切换图层 {name}");
        let state = Rc::clone(state);
        let checked_state = Rc::clone(&state);
        list = list.child(
            button()
                .accessibility_label(label)
                .min_h(px(44.))
                .px_3()
                .py_2()
                .rounded(px(8.))
                .bg(move || theme().muted)
                .on_press(move || state.layer(id))
                .child(
                    text(move || {
                        checked_state.drawing.get();
                        let visible = checked_state
                            .cache
                            .with(|cache| cache.visible.contains(&id));
                        format!("{} {name}", if visible { "☑" } else { "☐" })
                    })
                    .text_color(move || theme().foreground),
                ),
        );
    }
    view()
        .absolute()
        .left(px(8.))
        .top(px(8.))
        .w(px(248.))
        .max_h(px(320.))
        .rounded(px(12.))
        .bg(move || theme().background)
        .child(scroll().max_h(px(320.)).child(list))
        .into_element()
}

fn property_panel(state: &Rc<ViewerState>) -> AnyElement {
    let selected = state.selected;
    let scene = Arc::clone(&state.scene);
    view()
        .absolute()
        .right(px(8.))
        .top(px(8.))
        .w(px(264.))
        .max_h(px(320.))
        .rounded(px(12.))
        .bg(move || theme().background)
        .child(scroll().max_h(px(320.)).child(dynamic(move || {
            let mut properties = view()
                .gap_2()
                .p_3()
                .child(text("属性").font_weight(FontWeight::SEMIBOLD));
            if let Some(selected) = selected.get() {
                for (label, value) in [
                    ("类型", selected.kind.as_str()),
                    ("ID", selected.id.as_str()),
                ] {
                    properties = properties.child(text(format!("{label}：{value}")));
                }
                let layer = scene
                    .layers
                    .iter()
                    .find(|layer| layer.id == selected.layer)
                    .map(|layer| layer.name.as_str())
                    .unwrap_or("未知图层");
                properties = properties.child(text(format!("图层：{layer}")));
                if let Some(net) = &selected.net {
                    properties = properties.child(text(format!("网络：{net}")));
                }
                for (key, value) in &selected.properties {
                    properties = properties.child(text(format!("{key}：{value}")));
                }
            } else {
                properties = properties.child(text("点选图形查看属性"));
            }
            for warning in &scene.warnings {
                properties = properties
                    .child(text(warning.clone()).text_color(move || theme().muted_foreground));
            }
            properties.into_element()
        })))
        .into_element()
}

fn validate_embedded_image(image: &SceneImage) -> Result<(), String> {
    let invalid = || "嵌入图片无效，请重新打开文档".to_owned();
    let (header, _) = image.content.split_once(',').ok_or_else(invalid)?;
    if image.content.len() > 16 * 1024 * 1024
        || !matches!(
            header,
            "data:image/png;base64"
                | "data:image/jpeg;base64"
                | "data:image/jpg;base64"
                | "data:image/webp;base64"
                | "data:image/svg+xml;base64"
        )
        || !image.width.is_finite()
        || !image.height.is_finite()
        || image.width <= 0.
        || image.height <= 0.
        || !image.transform.into_iter().all(f64::is_finite)
    {
        return Err(invalid());
    }
    let (_, bytes) = nsh_core::image::decode_data_uri(&image.content).map_err(|_| invalid())?;
    if bytes.is_empty() {
        return Err(invalid());
    }
    // Bound raster headers before the platform allocates a decoded bitmap; on_load still proves decoding.
    if header != "data:image/svg+xml;base64" {
        let size = nsh_core::picker::image_size(&bytes).ok_or_else(invalid)?;
        if size.width.0 <= 0
            || size.height.0 <= 0
            || i64::from(size.width.0) * i64::from(size.height.0) > 16_777_216
        {
            return Err(invalid());
        }
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn install_web_wheel(state: &Rc<ViewerState>) -> Result<(), String> {
    use js_sys::{Function, Object, Reflect};
    use std::{
        cell::Cell,
        collections::VecDeque,
        future::poll_fn,
        task::{Poll, Waker},
    };
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};

    fn method(value: &JsValue, name: &str) -> Option<Function> {
        Reflect::get(value, &JsValue::from_str(name))
            .ok()?
            .dyn_into()
            .ok()
    }
    fn number(value: &JsValue, name: &str) -> Option<f64> {
        Reflect::get(value, &JsValue::from_str(name))
            .ok()?
            .as_f64()
            .filter(|number| number.is_finite())
    }

    let document: JsValue = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("浏览器文档尚未就绪")?
        .into();
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
        let scene_id = state.scene.id.clone();
        // DOM callbacks run outside the reactive runtime; its task enters it before touching signals.
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
                native_wheel(&scene_id, delta, x, y);
            }
        });
    }
    let listener = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
        if !enabled.get() {
            return;
        }
        let Some(target) = Reflect::get(&event, &JsValue::from_str("target")).ok() else {
            return;
        };
        let Some(closest) = method(&target, "closest") else {
            return;
        };
        let Ok(canvas) = closest.call1(
            &target,
            &JsValue::from_str("[data-testid=\"pcb-canvas\"], [aria-label=\"二维画布\"]"),
        ) else {
            return;
        };
        if canvas.is_null() || canvas.is_undefined() {
            return;
        }
        let Some(bounds) =
            method(&canvas, "getBoundingClientRect").and_then(|method| method.call0(&canvas).ok())
        else {
            return;
        };
        let (Some(delta), Some(client_x), Some(client_y), Some(left), Some(top)) = (
            number(&event, "deltaY"),
            number(&event, "clientX"),
            number(&event, "clientY"),
            number(&bounds, "left"),
            number(&bounds, "top"),
        ) else {
            return;
        };
        let unit = match number(&event, "deltaMode") {
            Some(1.) => 16.,
            Some(2.) => number(&bounds, "height").unwrap_or(1.),
            _ => 1.,
        };
        queue
            .borrow_mut()
            .push_back((delta * unit, client_x - left, client_y - top));
        if let Some(wake) = wake.borrow_mut().take() {
            wake.wake();
        }
        if let Some(prevent_default) = method(&event, "preventDefault") {
            let _ = prevent_default.call0(&event);
        }
    });
    let options = Object::new();
    Reflect::set(&options, &JsValue::from_str("passive"), &JsValue::FALSE)
        .map_err(|_| "无法配置浏览器滚轮")?;
    Reflect::set(&options, &JsValue::from_str("capture"), &JsValue::TRUE)
        .map_err(|_| "无法配置浏览器滚轮")?;
    method(&document, "addEventListener")
        .ok_or("浏览器不支持滚轮监听")?
        .call3(
            &document,
            &JsValue::from_str("wheel"),
            listener.as_ref(),
            &options,
        )
        .map_err(|_| "无法注册浏览器滚轮")?;
    cleanup(move || {
        if let Some(remove) = method(&document, "removeEventListener") {
            let _ = remove.call3(
                &document,
                &JsValue::from_str("wheel"),
                listener.as_ref(),
                &JsValue::TRUE,
            );
        }
        drop(listener);
    });
    Ok(())
}
