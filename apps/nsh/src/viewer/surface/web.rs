use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use nsh_core::{element::AnyElement, prelude::*};
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

use super::{SurfaceModel, SurfaceStatus, physical_size};
use crate::viewer::gpu::Gpu;

#[derive(Default)]
struct State {
    gpu: Option<Gpu>,
    canvas: Option<HtmlCanvasElement>,
    model: Option<Arc<SurfaceModel>>,
    size: [f64; 2],
    preparing: bool,
    disposed: bool,
    initialization_key: Option<([f64; 2], f64, u64)>,
}

fn release(canvas: &HtmlCanvasElement) {
    if let Ok(Some(context)) = canvas.get_context("webgl2")
        && let Ok(context) = context.dyn_into::<web_sys::WebGl2RenderingContext>()
        && let Ok(Some(extension)) = context.get_extension("WEBGL_lose_context")
        && let Ok(extension) = extension.dyn_into::<web_sys::WebglLoseContext>()
    {
        extension.lose_context();
    }
    canvas.remove();
}

fn publish(status: Signal<SurfaceStatus>, result: Result<Option<SurfaceStatus>, String>) {
    let next = match result {
        Ok(Some(ready)) => ready,
        Err(error) => SurfaceStatus::Error(error),
        _ => return,
    };
    if status.get_untracked() != next {
        status.set(next);
    }
}

fn ready(gpu: &Gpu) -> SurfaceStatus {
    let [width, height] = gpu.size();
    SurfaceStatus::Ready {
        backend: gpu.backend.clone(),
        width,
        height,
    }
}

fn scale() -> Result<f64, String> {
    Ok(web_sys::window()
        .ok_or("无法取得浏览器窗口")?
        .device_pixel_ratio())
}

fn active(state: &State) -> bool {
    !state.disposed
        && !state.size.contains(&0.0)
        && state
            .model
            .as_ref()
            .is_none_or(|model| model.frame().active)
}

// Own the GPU across awaits, so callbacks never borrow an in-flight GPU.
async fn prepare(state: Rc<RefCell<State>>, status: Signal<SurfaceStatus>, mut gpu: Gpu) {
    let mut success = false;
    loop {
        let request = {
            let state = state.borrow();
            if !active(&state) {
                None
            } else {
                Some((state.size, state.model.clone()))
            }
        };
        let Some((logical, model)) = request else {
            break;
        };
        let size = match scale().and_then(|scale| {
            physical_size(
                logical[0],
                logical[1],
                scale,
                gpu.device.limits().max_texture_dimension_2d,
            )
            .map_err(str::to_owned)
        }) {
            Ok(Some(size)) => size,
            Ok(None) => break,
            Err(error) => {
                if !state.borrow().disposed {
                    publish(status, Err(error));
                }
                break;
            }
        };
        let revision = model.as_ref().map_or(0, |model| model.resource_revision());
        let result = gpu.prepare(model.as_deref(), size).await;
        {
            let state = state.borrow();
            if !active(&state) {
                break;
            }
            let latest_size = scale().and_then(|scale| {
                physical_size(
                    state.size[0],
                    state.size[1],
                    scale,
                    gpu.device.limits().max_texture_dimension_2d,
                )
                .map_err(str::to_owned)
            });
            let latest_revision = state
                .model
                .as_ref()
                .map_or(0, |model| model.resource_revision());
            if latest_revision != revision || latest_size.as_ref().ok() != Some(&Some(size)) {
                // A single task coalesces changes arriving during allocation.
                continue;
            }
        }
        let result = result.and_then(|()| {
            let state = state.borrow();
            let canvas = state.canvas.as_ref().ok_or("绘图表面已释放")?;
            if canvas.width() != size[0] {
                canvas.set_width(size[0]);
            }
            if canvas.height() != size[1] {
                canvas.set_height(size[1]);
            }
            gpu.configure(size);
            Ok(gpu.draw(model.as_deref())?.then(|| ready(&gpu)))
        });
        success = result.is_ok();
        if !state.borrow().disposed {
            publish(status, result);
        }
        break;
    }
    let mut state = state.borrow_mut();
    state.preparing = false;
    if !success {
        state.initialization_key = None;
    }
    if !state.disposed {
        state.gpu = Some(gpu);
    }
}

async fn initialize(state: Rc<RefCell<State>>, status: Signal<SurfaceStatus>, selector: String) {
    let result = async {
        if state.borrow().disposed {
            return Ok(None);
        }
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or("无法取得浏览器文档")?;
        let container = document
            .query_selector(&selector)
            .map_err(|_| "无法定位绘图表面")?
            .ok_or("绘图表面已经卸载")?;
        let mut last_error = "浏览器没有可用的 GPU 后端".to_owned();
        for backend in [wgpu::Backends::BROWSER_WEBGPU, wgpu::Backends::GL] {
            let canvas: HtmlCanvasElement = document
                .create_element("canvas")
                .map_err(|_| "无法创建绘图表面")?
                .dyn_into()
                .map_err(|_| "无法创建 Canvas")?;
            canvas
                .set_attribute("aria-hidden", "true")
                .map_err(|_| "无法配置绘图表面")?;
            canvas
                .set_attribute(
                    "style",
                    "position:absolute;inset:0;width:100%;height:100%;pointer-events:none",
                )
                .map_err(|_| "无法配置绘图表面")?;
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: backend,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let surface = match instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            {
                Ok(surface) => surface,
                Err(error) => {
                    last_error = error.to_string();
                    release(&canvas);
                    continue;
                }
            };
            match Gpu::new(instance, surface).await {
                Ok(gpu) => {
                    if state.borrow().disposed {
                        drop(gpu);
                        release(&canvas);
                        return Ok(None);
                    }
                    if container.append_child(&canvas).is_err() {
                        drop(gpu);
                        release(&canvas);
                        return Err("无法挂载绘图表面".into());
                    }
                    state.borrow_mut().canvas = Some(canvas);
                    return Ok(Some(gpu));
                }
                Err(error) => {
                    last_error = error;
                    release(&canvas);
                }
            }
        }
        Err(last_error)
    }
    .await;
    match result {
        Ok(Some(gpu)) => prepare(Rc::clone(&state), status, gpu).await,
        result => {
            let mut current = state.borrow_mut();
            current.preparing = false;
            current.initialization_key = None;
            drop(current);
            if !state.borrow().disposed {
                publish(status, result.map(|_| None));
            }
        }
    }
}

fn refresh(
    state: &Rc<RefCell<State>>,
    status: Signal<SurfaceStatus>,
    selector: &str,
    allow_prepare: bool,
) {
    let mut current = state.borrow_mut();
    if !active(&current) || current.preparing {
        return;
    }
    let scale = match scale() {
        Ok(scale) => scale,
        Err(error) => {
            drop(current);
            publish(status, Err(error));
            return;
        }
    };
    if current.gpu.is_some() {
        let result = (|| {
            let size = physical_size(
                current.size[0],
                current.size[1],
                scale,
                current
                    .gpu
                    .as_ref()
                    .unwrap()
                    .device
                    .limits()
                    .max_texture_dimension_2d,
            )
            .map_err(str::to_owned)?;
            let Some(size) = size else {
                return Ok(None);
            };
            if allow_prepare
                && current
                    .gpu
                    .as_ref()
                    .unwrap()
                    .needs_prepare(current.model.as_deref(), size)
            {
                return Ok(Some(size));
            }
            let model = current.model.clone();
            let gpu = current.gpu.as_mut().unwrap();
            let drawn = gpu.draw(model.as_deref())?;
            // Camera frames do not spawn tasks, resize Canvas, or configure the surface.
            publish(status, Ok(drawn.then(|| ready(gpu))));
            Ok(None)
        })();
        match result {
            Ok(Some(_)) => {
                let gpu = current.gpu.take().unwrap();
                current.preparing = true;
                drop(current);
                let state = Rc::clone(state);
                spawn(async move {
                    prepare(state, status, gpu).await;
                });
            }
            Err(error) => {
                drop(current);
                publish(status, Err(error));
            }
            _ => {}
        }
    } else {
        let key = (
            current.size,
            scale,
            current
                .model
                .as_ref()
                .map_or(0, |model| model.resource_revision()),
        );
        if current.initialization_key == Some(key) {
            return;
        }
        current.initialization_key = Some(key);
        current.preparing = true;
        drop(current);
        let state = Rc::clone(state);
        let selector = selector.to_owned();
        // NSH sends layout before its DOM mount batch has been flushed.
        request_animation_frame(move |_| {
            spawn(async move {
                initialize(state, status, selector).await;
            });
        });
    }
}

pub(super) fn surface(
    status: Signal<SurfaceStatus>,
    model: Option<(Arc<SurfaceModel>, Signal<u64>)>,
) -> AnyElement {
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    let id = format!("oshwhub-gpu-{}", NEXT.fetch_add(1, Ordering::Relaxed));
    let state = Rc::new(RefCell::new(State {
        model: model.as_ref().map(|(model, _)| Arc::clone(model)),
        ..State::default()
    }));
    let owned = Rc::clone(&state);
    cleanup(move || {
        let (gpu, canvas) = {
            let mut state = owned.borrow_mut();
            state.disposed = true;
            state.model.take();
            (state.gpu.take(), state.canvas.take())
        };
        drop(gpu);
        if let Some(canvas) = canvas {
            release(&canvas);
        }
    });
    let selector = format!("[data-testid=\"{id}\"]");
    if let Some((_, tick)) = model {
        let state = Rc::clone(&state);
        let selector = selector.clone();
        effect(move || {
            tick.get();
            let allow_prepare = {
                let current = state.borrow();
                current
                    .gpu
                    .as_ref()
                    .is_none_or(|gpu| gpu.needs_resource_prepare(current.model.as_deref()))
            };
            refresh(&state, status, &selector, allow_prepare);
        });
    }
    view()
        .flex_1()
        .relative()
        .overflow_hidden()
        .focusable(true)
        .label("三维绘图区域")
        .test_id(&id)
        .on_layout(move |bounds: &nsh_core::Bounds<nsh_core::Points>| {
            state.borrow_mut().size = [
                f64::from(bounds.size.width.0),
                f64::from(bounds.size.height.0),
            ];
            refresh(&state, status, &selector, true);
        })
        .into_element()
}
