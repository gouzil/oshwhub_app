use super::camera3d::Camera3d;
use nsh_core::{element::AnyElement, prelude::*};
use oshwhub_core::pcb::{RawModel3d, RawScene3d};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

fn models() -> &'static Mutex<BTreeMap<u64, Weak<SurfaceModel>>> {
    static MODELS: OnceLock<Mutex<BTreeMap<u64, Weak<SurfaceModel>>>> = OnceLock::new();
    MODELS.get_or_init(Mutex::default)
}

/// Meshes stay in Rust; hosts receive only this owner's registry handle and a small tick.
pub struct SurfaceModel {
    handle: u64,
    state: Mutex<ModelState>,
}
struct ModelState {
    tick: u64,
    resource_revision: u64,
    frame: SurfaceFrame,
    board: Option<Arc<RawScene3d>>,
    models: BTreeMap<String, Arc<RawModel3d>>,
    disposed: bool,
}
impl ModelState {
    fn changed(&mut self, resources: bool) {
        if resources {
            self.resource_revision = self.resource_revision.wrapping_add(1);
        }
        self.tick = self.tick.wrapping_add(1);
    }
}
pub struct SurfaceResources {
    pub revision: u64,
    pub board: Option<Arc<RawScene3d>>,
    pub models: Vec<Arc<RawModel3d>>,
}
#[derive(Clone, Copy)]
pub struct SurfaceFrame {
    pub camera: Camera3d,
    pub active: bool,
    pub components_visible: bool,
}
impl SurfaceModel {
    pub fn new(camera: Camera3d) -> Arc<Self> {
        static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
        let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        let model = Arc::new(Self {
            handle,
            state: Mutex::new(ModelState {
                tick: 0,
                resource_revision: 0,
                frame: SurfaceFrame {
                    camera,
                    active: true,
                    components_visible: true,
                },
                board: None,
                models: BTreeMap::new(),
                disposed: false,
            }),
        });
        models()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(handle, Arc::downgrade(&model));
        model
    }
    fn state(&self) -> MutexGuard<'_, ModelState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
    pub fn handle(&self) -> u64 {
        self.handle
    }
    pub fn tick(&self) -> u64 {
        self.state().tick
    }
    pub fn resource_revision(&self) -> u64 {
        self.state().resource_revision
    }
    pub fn frame(&self) -> SurfaceFrame {
        self.state().frame
    }
    pub fn set_camera(&self, camera: Camera3d) {
        let mut state = self.state();
        if !state.disposed && state.frame.camera != camera {
            state.frame.camera = camera;
            state.changed(false);
        }
    }
    pub fn set_active(&self, active: bool) {
        let mut state = self.state();
        if !state.disposed && state.frame.active != active {
            state.frame.active = active;
            state.changed(false);
        }
    }
    pub fn set_components_visible(&self, components_visible: bool) {
        let mut state = self.state();
        if !state.disposed && state.frame.components_visible != components_visible {
            state.frame.components_visible = components_visible;
            state.changed(false);
        }
    }
    pub fn set_board(&self, board: Arc<RawScene3d>) {
        let mut state = self.state();
        if state.disposed
            || state
                .board
                .as_ref()
                .is_some_and(|old| Arc::ptr_eq(old, &board))
        {
            return;
        }
        state.board = Some(board);
        state.models.clear();
        state.changed(true);
    }
    pub fn add_model(&self, model: Arc<RawModel3d>) {
        let mut state = self.state();
        if state.disposed
            || state
                .models
                .get(&model.key)
                .is_some_and(|old| same_model(old, &model))
        {
            return;
        }
        state.models.insert(model.key.clone(), model);
        state.changed(true);
    }
    pub fn retry_resources(&self) {
        let mut state = self.state();
        if !state.disposed {
            state.changed(true);
        }
    }
    pub fn resources_since(&self, revision: u64) -> Option<SurfaceResources> {
        let state = self.state();
        if state.disposed || (revision != u64::MAX && revision == state.resource_revision) {
            return None;
        }
        Some(SurfaceResources {
            revision: state.resource_revision,
            board: state.board.clone(),
            models: state.models.values().map(Arc::clone).collect(),
        })
    }
    pub fn dispose(&self) {
        {
            let mut state = self.state();
            if state.disposed {
                return;
            }
            state.disposed = true;
            state.frame.active = false;
            state.board.take();
            state.models.clear();
        }
        models()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.handle);
    }
    pub fn resolve(handle: u64) -> Option<Arc<Self>> {
        models()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&handle)
            .and_then(Weak::upgrade)
    }
}

fn same_model(old: &RawModel3d, next: &RawModel3d) -> bool {
    old.key == next.key
        && old.mesh_key == next.mesh_key
        && old.instances.len() == next.instances.len()
        && old
            .instances
            .iter()
            .zip(&next.instances)
            .all(|(a, b)| a.title == b.title && a.matrix == b.matrix)
}

impl Drop for SurfaceModel {
    fn drop(&mut self) {
        self.dispose();
    }
}

#[cfg(target_os = "android")]
mod android;
#[cfg(any(
    test,
    not(any(
        target_arch = "wasm32",
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
        target_os = "linux"
    ))
))]
#[cfg_attr(test, allow(dead_code))]
mod fallback;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod metal;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_os = "macos")]
pub use metal::register as register_macos;
#[cfg(target_os = "ios")]
pub use metal::register as register_ios;

#[cfg(target_os = "linux")]
pub use linux::register as register_linux;

#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceStatus {
    Pending,
    Ready {
        backend: String,
        width: u32,
        height: u32,
    },
    Error(String),
}

pub fn surface_with_model(
    status: Signal<SurfaceStatus>,
    model: Arc<SurfaceModel>,
    tick: Signal<u64>,
) -> AnyElement {
    let owned = Arc::clone(&model);
    cleanup(move || owned.dispose());
    surface_inner(status, Some((model, tick)))
}

pub fn surface(status: Signal<SurfaceStatus>) -> AnyElement {
    surface_inner(status, None)
}

fn surface_inner(
    status: Signal<SurfaceStatus>,
    model: Option<(Arc<SurfaceModel>, Signal<u64>)>,
) -> AnyElement {
    status.set(SurfaceStatus::Pending);
    #[cfg(target_arch = "wasm32")]
    {
        web::surface(status, model)
    }
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
        target_os = "linux"
    ))]
    {
        let size = signal([0.0_f64; 2]);
        let mut native = native("oshwhub.gpu")
            .flex_1()
            .prop("size", move || serde_json::json!(size.get()))
            .on_native_event(move |event: &NativeEvent| {
                let next = if &*event.name == "ready" {
                    SurfaceStatus::Ready {
                        backend: event.data["backend"].as_str().unwrap_or_default().into(),
                        width: event.data["width"].as_u64().unwrap_or(0) as u32,
                        height: event.data["height"].as_u64().unwrap_or(0) as u32,
                    }
                } else if &*event.name == "error" {
                    SurfaceStatus::Error(event.data.as_str().unwrap_or("GPU 绘图失败").into())
                } else {
                    return;
                };
                if status.get_untracked() != next {
                    status.set(next);
                }
            });
        if let Some((model, tick)) = model {
            let handle = model.handle();
            native = native.prop(
                "model",
                move || serde_json::json!({"handle":handle,"tick":tick.get()}),
            );
        }
        // GTK's GLArea texture origin is opposite to wgpu's render-target origin.
        #[cfg(target_os = "linux")]
        let native = native.scale_y(-1.);
        view()
            .flex_1()
            .focusable(true)
            .label("三维绘图区域")
            .on_layout(move |bounds: &nsh_core::Bounds<nsh_core::Points>| {
                size.set([
                    f64::from(bounds.size.width.0),
                    f64::from(bounds.size.height.0),
                ]);
            })
            .child(native)
            .into_element()
    }
    #[cfg(not(any(
        target_arch = "wasm32",
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
        target_os = "linux"
    )))]
    {
        fallback::surface(status, model)
    }
}

pub fn physical_size(
    width: f64,
    height: f64,
    scale: f64,
    limit: u32,
) -> Result<Option<[u32; 2]>, &'static str> {
    if ![width, height, scale].iter().all(|value| value.is_finite())
        || width < 0.0
        || height < 0.0
        || scale <= 0.0
    {
        return Err("绘图尺寸或显示缩放无效");
    }
    let size = [(width * scale).ceil(), (height * scale).ceil()];
    if size
        .iter()
        .any(|value| !value.is_finite() || *value > f64::from(limit))
    {
        return Err("绘图尺寸超过当前 GPU 的纹理上限");
    }
    if size.contains(&0.0) {
        return Ok(None);
    }
    Ok(Some([size[0] as u32, size[1] as u32]))
}
