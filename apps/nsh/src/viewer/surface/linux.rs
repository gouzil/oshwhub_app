//! GTK composites the GPU framebuffer with NSH's clipping and overlays.

use std::{cell::RefCell, ffi::CString, num::NonZeroU32, rc::Rc};

use gtk::prelude::*;
use nsh_core::JsonValue;
use nsh_platform_linux::{ComponentContext, Components, NativeComponent, gtk4 as gtk};

use super::{SurfaceModel, SurfaceStatus, physical_size};
use crate::viewer::gpu::Gpu;

#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetProcAddress(name: *const std::ffi::c_char) -> *const std::ffi::c_void;
}
#[link(name = "GLESv2")]
unsafe extern "C" {
    fn glGetIntegerv(name: u32, value: *mut i32);
}

struct State {
    target: Option<(wgpu::Texture, [u32; 2], u32)>,
    gpu: Option<Gpu>,
    handle: Option<u64>,
    context: ComponentContext,
    status: Option<SurfaceStatus>,
}

impl State {
    fn release(&mut self) {
        self.target.take();
        self.gpu.take();
    }

    fn publish(&mut self, result: Result<Option<SurfaceStatus>, String>) {
        let next = match result {
            Ok(Some(status)) => status,
            Err(error) => SurfaceStatus::Error(error),
            Ok(None) => return,
        };
        if self.status.as_ref() == Some(&next) {
            return;
        }
        match &next {
            SurfaceStatus::Ready {
                backend,
                width,
                height,
            } => self.context.emit(
                "ready",
                serde_json::json!({ "backend": backend, "width": width, "height": height }),
            ),
            SurfaceStatus::Error(error) => {
                self.context.emit("error", JsonValue::String(error.clone()))
            }
            SurfaceStatus::Pending => {}
        }
        self.status = Some(next);
    }

    fn render(&mut self, area: &gtk::GLArea) -> Result<Option<SurfaceStatus>, String> {
        if let Some(error) = area.error() {
            return Err(format!("无法创建 GTK GPU 上下文：{error}"));
        }
        let model = self.handle.and_then(SurfaceModel::resolve);
        if self.handle.is_some() && model.is_none() {
            self.release();
            return Ok(None);
        }
        if model.as_ref().is_some_and(|model| !model.frame().active) {
            return Ok(None);
        }
        // GTK binds its framebuffer before render; initialization may change that binding.
        let mut framebuffer = 0;
        // SAFETY: GLArea's render callback has made its context current on the GTK thread.
        unsafe { glGetIntegerv(glow::DRAW_FRAMEBUFFER_BINDING, &mut framebuffer) };
        let framebuffer = NonZeroU32::new(framebuffer as u32).ok_or("GTK 绘图目标尚未就绪")?;
        if self.gpu.is_none() {
            // SAFETY: all use and disposal of this external adapter stays in GLArea's context.
            let adapter = unsafe {
                wgpu::hal::gles::Adapter::new_external(
                    |name| eglGetProcAddress(CString::new(name).unwrap().as_ptr()),
                    wgpu::GlBackendOptions::default(),
                )
            }
            .ok_or("当前 GTK 上下文不支持三维 GPU 绘制")?;
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::GL,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            // SAFETY: both the instance and external adapter use the GLES backend.
            let adapter =
                unsafe { instance.create_adapter_from_hal::<wgpu::hal::api::Gles>(adapter) };
            self.gpu = Some(pollster::block_on(Gpu::offscreen(
                adapter,
                wgpu::TextureFormat::Rgba8Unorm,
            ))?);
        }
        let gpu = self.gpu.as_mut().unwrap();
        let Some(size) = physical_size(
            f64::from(area.width()),
            f64::from(area.height()),
            f64::from(area.scale_factor()),
            gpu.device.limits().max_texture_dimension_2d,
        )
        .map_err(str::to_owned)?
        else {
            return Ok(None);
        };
        if gpu.needs_prepare(model.as_deref(), size) {
            pollster::block_on(gpu.prepare(model.as_deref(), size))?;
            gpu.configure(size);
        }
        if self
            .target
            .as_ref()
            .is_none_or(|(_, old_size, old_fb)| *old_size != size || *old_fb != framebuffer.get())
        {
            let format = wgpu::TextureFormat::Rgba8Unorm;
            let mut texture = wgpu::hal::gles::Texture::default_framebuffer(format);
            texture.inner = wgpu::hal::gles::TextureInner::ExternalNativeFramebuffer {
                inner: glow::NativeFramebuffer(framebuffer),
            };
            // SAFETY: GTK owns the matching RGBA framebuffer for this allocation; this
            // non-owning wrapper is replaced on resize and dropped before the context.
            let texture = unsafe {
                gpu.device.create_texture_from_hal::<wgpu::hal::api::Gles>(
                    texture,
                    &wgpu::TextureDescriptor {
                        label: Some("GTK framebuffer"),
                        size: wgpu::Extent3d {
                            width: size[0],
                            height: size[1],
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    },
                )
            };
            self.target = Some((texture, size, framebuffer.get()));
        }
        let target = self
            .target
            .as_ref()
            .unwrap()
            .0
            .create_view(&Default::default());
        Ok(gpu
            .draw_into(model.as_deref(), &target)?
            .then(|| SurfaceStatus::Ready {
                backend: gpu.backend.clone(),
                width: size[0],
                height: size[1],
            }))
    }
}

struct GtkSurface {
    area: gtk::GLArea,
    state: Rc<RefCell<State>>,
}

impl NativeComponent for GtkSurface {
    fn widget(&self) -> gtk::Widget {
        self.area.clone().upcast()
    }

    fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
        if name == "model" {
            let handle = value
                .map(|value| value["handle"].as_u64().ok_or("三维绘图句柄无效"))
                .transpose();
            match handle {
                Ok(handle) => self.state.borrow_mut().handle = handle,
                Err(error) => self.state.borrow_mut().publish(Err(error.into())),
            }
        }
        self.area.queue_render();
    }
}

impl Drop for GtkSurface {
    fn drop(&mut self) {
        if self.area.is_realized() {
            self.area.make_current();
        }
        self.state.borrow_mut().release();
    }
}

pub fn register(components: &Components) {
    components.register(
        "oshwhub.gpu",
        Rc::new(|context| {
            let area = gtk::GLArea::new();
            area.set_allowed_apis(gtk::gdk::GLAPI::GLES);
            area.set_required_version(3, 0);
            area.set_auto_render(false);
            area.set_can_target(false);
            let state = Rc::new(RefCell::new(State {
                target: None,
                gpu: None,
                handle: None,
                context,
                status: None,
            }));
            let render_state = Rc::clone(&state);
            area.connect_render(move |area, _| {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    render_state.borrow_mut().render(area)
                }))
                .unwrap_or_else(|_| Err("GTK GPU 绘图失败，请重新打开查看器".into()));
                render_state.borrow_mut().publish(result);
                gtk::glib::Propagation::Stop
            });
            let unrealize_state = Rc::clone(&state);
            area.connect_unrealize(move |area| {
                area.make_current();
                unrealize_state.borrow_mut().release();
            });
            let realize_state = Rc::clone(&state);
            area.connect_realize(move |area| {
                area.make_current();
                if let Some(error) = area.error() {
                    realize_state
                        .borrow_mut()
                        .publish(Err(format!("无法创建 GTK GPU 上下文：{error}")));
                }
            });
            Box::new(GtkSurface { area, state })
        }),
    );
}
