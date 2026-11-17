//! JNI surface ownership only; pages, meshes and camera state remain in Rust.

// Android's thread_local expansion triggers this lint despite the explicit const initializer.
#![allow(clippy::missing_const_for_thread_local)]

use std::{
    cell::RefCell,
    collections::BTreeMap,
    ffi::c_void,
    ptr::NonNull,
    sync::atomic::{AtomicU64, Ordering},
};

use nsh::jni::{
    JNIEnv,
    objects::{JClass, JObject},
    sys::{jint, jlong, jstring},
};
use wgpu::rwh::{AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle};

use super::{SurfaceModel, physical_size};
use crate::viewer::gpu::Gpu;

#[link(name = "android")]
unsafe extern "C" {
    fn ANativeWindow_fromSurface(env: *mut c_void, surface: *mut c_void) -> *mut c_void;
    fn ANativeWindow_release(window: *mut c_void);
}

struct Window(NonNull<c_void>);
impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: fromSurface acquired exactly one reference, held until after Gpu drops.
        unsafe { ANativeWindow_release(self.0.as_ptr()) };
    }
}

struct Host {
    gpu: Gpu,
    _window: Window,
}

thread_local! {
    // TextureView invokes all callbacks on its UI thread; GLES contexts stay on that thread.
    static HOSTS: RefCell<BTreeMap<u64, Host>> = const { RefCell::new(BTreeMap::new()) };
}

impl Host {
    fn new(
        env: &JNIEnv<'_>,
        surface: JObject<'_>,
        width: i32,
        height: i32,
    ) -> Result<Self, String> {
        // SAFETY: Java supplies a live Surface for this synchronous call.
        let window = NonNull::new(unsafe {
            ANativeWindow_fromSurface(env.get_raw().cast(), surface.as_raw().cast())
        })
        .map(Window)
        .ok_or("无法取得 Android 绘图窗口")?;
        let mut last_error = String::new();
        // A TextureView has one producer; initialize Vulkan and GLES separately.
        for backend in [wgpu::Backends::VULKAN, wgpu::Backends::GL] {
            let result = guarded(|| {
                let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: backend,
                    ..wgpu::InstanceDescriptor::new_without_display_handle()
                });
                // SAFETY: Host releases its GPU before the acquired ANativeWindow reference.
                let surface = unsafe {
                    instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: Some(RawDisplayHandle::Android(
                            AndroidDisplayHandle::new(),
                        )),
                        raw_window_handle: RawWindowHandle::AndroidNdk(
                            AndroidNdkWindowHandle::new(window.0),
                        ),
                    })
                }
                .map_err(|error| format!("无法创建 Android GPU 表面：{error}"))?;
                let mut gpu = pollster::block_on(Gpu::new(instance, surface))?;
                let size = physical_size(
                    f64::from(width),
                    f64::from(height),
                    1.,
                    gpu.device.limits().max_texture_dimension_2d,
                )
                .map_err(str::to_owned)?
                .ok_or("Android 绘图区域尚未布局")?;
                pollster::block_on(gpu.prepare(None, size))?;
                gpu.configure(size);
                Ok(gpu)
            });
            match result {
                Ok(gpu) => {
                    return Ok(Self {
                        gpu,
                        _window: window,
                    });
                }
                Err(error) => last_error = error,
            }
        }
        Err(last_error)
    }

    fn draw(
        &mut self,
        model: u64,
        width: i32,
        height: i32,
    ) -> Result<Option<serde_json::Value>, String> {
        let model_state = (model != 0).then(|| SurfaceModel::resolve(model)).flatten();
        if (model != 0 && model_state.is_none())
            || model_state
                .as_ref()
                .is_some_and(|model| !model.frame().active)
        {
            return Ok(None);
        }
        let Some(size) = physical_size(
            f64::from(width),
            f64::from(height),
            1.,
            self.gpu.device.limits().max_texture_dimension_2d,
        )
        .map_err(str::to_owned)?
        else {
            return Ok(None);
        };
        if self.gpu.needs_prepare(model_state.as_deref(), size) {
            pollster::block_on(self.gpu.prepare(model_state.as_deref(), size))?;
            self.gpu.configure(size);
        }
        Ok(Some(if self.gpu.draw(model_state.as_deref())? {
            serde_json::json!({"backend": self.gpu.backend, "width": size[0], "height": size[1]})
        } else {
            serde_json::json!({"pending": true})
        }))
    }
}

fn guarded<T>(job: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
        .unwrap_or_else(|_| Err("Android GPU 绘图失败，请重新打开查看器".into()))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_gouzil_oshwhub_GpuComponent_create(
    mut env: JNIEnv<'_>,
    _: JClass<'_>,
    surface: JObject<'_>,
    width: jint,
    height: jint,
) -> jlong {
    let result = guarded(|| {
        HOSTS.with(|hosts| {
            let mut hosts = hosts.borrow_mut();
            if hosts.len() >= 8 {
                return Err("同时打开的绘图表面过多".into());
            }
            let host = Host::new(&env, surface, width, height)?;
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let handle = NEXT.fetch_add(1, Ordering::Relaxed);
            hosts.insert(handle, host);
            Ok(handle as jlong)
        })
    });
    match result {
        Ok(handle) => handle,
        Err(error) => {
            let _ = env.throw_new("java/lang/IllegalStateException", error);
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_gouzil_oshwhub_GpuComponent_draw(
    env: JNIEnv<'_>,
    _: JClass<'_>,
    host: jlong,
    model: jlong,
    width: jint,
    height: jint,
) -> jstring {
    let result = guarded(|| {
        HOSTS.with(|hosts| {
            hosts
                .borrow_mut()
                .get_mut(&(host as u64))
                .ok_or("Android 绘图表面已释放")?
                .draw(model as u64, width, height)
        })
    });
    let json = match result {
        Ok(Some(json)) => json,
        Ok(None) => return std::ptr::null_mut(),
        Err(error) => serde_json::json!({"error": error}),
    };
    env.new_string(json.to_string())
        .map_or(std::ptr::null_mut(), |value| value.into_raw())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_gouzil_oshwhub_GpuComponent_destroy(
    _: JNIEnv<'_>,
    _: JClass<'_>,
    host: jlong,
) {
    let _ = guarded(|| {
        HOSTS.with(|hosts| hosts.borrow_mut().remove(&(host as u64)));
        Ok(())
    });
}
