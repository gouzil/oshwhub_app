//! Render on NSH's D3D11 device, then composite through its D2D clip/transform tree.
use super::{SurfaceModel, SurfaceStatus, physical_size};
use crate::viewer::render3d::{CAMERA_BYTES, camera_bytes, checked_budget};
use nsh_core::JsonValue;
use nsh_platform_windows::{ComponentContext, Components, NativeComponent};
use std::{cell::RefCell, rc::Rc};
use windows::{
    Win32::{
        Foundation::E_FAIL,
        Graphics::{
            Direct2D::{Common::*, *},
            Direct3D::*,
            Direct3D11::*,
            Dxgi::{Common::*, IDXGISurface},
        },
    },
    core::Interface,
};

#[path = "windows_mesh.rs"]
mod mesh;
#[path = "windows_shader.rs"]
mod shader;

fn failure(message: impl Into<String>) -> windows::core::Error {
    windows::core::Error::new(E_FAIL, message.into())
}

struct Target {
    bitmap: ID2D1Bitmap1,
    color: ID3D11RenderTargetView,
    depth: ID3D11DepthStencilView,
    size: [u32; 2],
}

struct Renderer {
    target: Option<Target>,
    scene: Option<mesh::Scene>,
    device: ID3D11Device1,
    immediate: ID3D11DeviceContext1,
    context_state: ID3DDeviceContextState,
    pipeline: shader::Pipeline,
    camera: ID3D11Buffer,
    model_handle: Option<u64>,
    resource_revision: u64,
    last_model: Option<(u64, u64)>,
    d2d_device: ID2D1Device,
}

impl Renderer {
    fn new(dc: &ID2D1DeviceContext) -> windows::core::Result<Self> {
        // SAFETY: NSH calls the component only on its drawing thread; all resources use its device.
        unsafe {
            let d2d_device = dc.GetDevice()?;
            let dxgi = d2d_device.cast::<ID2D1Device2>()?.GetDxgiDevice()?;
            let device: ID3D11Device1 = dxgi.cast()?;
            if device.GetFeatureLevel().0 < D3D_FEATURE_LEVEL_11_0.0 {
                return Err(failure("三维绘图需要 D3D11 feature level 11.0"));
            }
            let immediate = device.GetImmediateContext()?.cast()?;
            let mut context_state = None;
            device.CreateDeviceContextState(
                0,
                &[device.GetFeatureLevel()],
                D3D11_SDK_VERSION,
                &ID3D11Device::IID,
                None,
                Some(&mut context_state),
            )?;
            let pipeline = shader::Pipeline::new(&device)?;
            let camera = mesh::buffer(
                &device,
                &[0; CAMERA_BYTES as usize],
                D3D11_BIND_CONSTANT_BUFFER,
            )?;
            Ok(Self {
                target: None,
                scene: None,
                device,
                immediate,
                context_state: context_state.ok_or_else(|| failure("无法保存 GPU 绘图状态"))?,
                pipeline,
                camera,
                model_handle: None,
                resource_revision: 0,
                last_model: None,
                d2d_device,
            })
        }
    }

    fn resize(
        &mut self,
        dc: &ID2D1DeviceContext,
        size: [u32; 2],
        resident_bytes: u64,
    ) -> windows::core::Result<bool> {
        if self
            .target
            .as_ref()
            .is_some_and(|target| target.size == size)
        {
            return Ok(false);
        }
        let target_bytes = u64::from(size[0]) * u64::from(size[1]) * 8;
        checked_budget(resident_bytes, 0, target_bytes).map_err(failure)?;
        unsafe {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: size[0],
                Height: size[1],
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
                ..Default::default()
            };
            let mut color = None;
            self.device.CreateTexture2D(&desc, None, Some(&mut color))?;
            let color = color.ok_or_else(|| failure("无法创建三维颜色缓冲"))?;
            let bitmap = dc.CreateBitmapFromDxgiSurface(
                &color.cast::<IDXGISurface>()?,
                Some(&D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: desc.Format,
                        alphaMode: D2D1_ALPHA_MODE_IGNORE,
                    },
                    dpiX: 96.,
                    dpiY: 96.,
                    bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
                    colorContext: Default::default(),
                }),
            )?;
            let mut color_view = None;
            self.device
                .CreateRenderTargetView(&color, None, Some(&mut color_view))?;
            let mut depth = None;
            self.device.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Format: DXGI_FORMAT_D24_UNORM_S8_UINT,
                    BindFlags: D3D11_BIND_DEPTH_STENCIL.0 as u32,
                    ..desc
                },
                None,
                Some(&mut depth),
            )?;
            let mut depth_view = None;
            self.device.CreateDepthStencilView(
                &depth.ok_or_else(|| failure("无法创建三维深度缓冲"))?,
                None,
                Some(&mut depth_view),
            )?;
            self.target = Some(Target {
                bitmap,
                color: color_view.ok_or_else(|| failure("无法创建三维颜色视图"))?,
                depth: depth_view.ok_or_else(|| failure("无法创建三维深度视图"))?,
                size,
            });
        }
        self.last_model = None;
        Ok(true)
    }

    fn prepare(
        &mut self,
        dc: &ID2D1DeviceContext,
        model: Option<&SurfaceModel>,
        size: [u32; 2],
    ) -> windows::core::Result<bool> {
        let handle = model.map(SurfaceModel::handle);
        let revision = model.map_or(0, SurfaceModel::resource_revision);
        if handle == self.model_handle && revision == self.resource_revision {
            return self.resize(
                dc,
                size,
                self.scene
                    .as_ref()
                    .map_or(CAMERA_BYTES, |scene| scene.stats.resident_bytes),
            );
        }
        let target_bytes = u64::from(size[0]) * u64::from(size[1]) * 8;
        let resized = if let Some(resources) =
            model.and_then(|model| model.resources_since(u64::MAX))
            && let Some(board) = resources.board
        {
            if handle == self.model_handle
                && self.scene.as_ref().is_some_and(|scene| {
                    scene
                        .board_source
                        .ptr_eq(&std::sync::Arc::downgrade(&board))
                })
            {
                for model in &resources.models {
                    self.scene.as_mut().expect("scene checked").add_model(
                        &self.device,
                        model,
                        target_bytes,
                    )?;
                }
                self.resize(
                    dc,
                    size,
                    self.scene
                        .as_ref()
                        .expect("scene checked")
                        .stats
                        .resident_bytes,
                )?
            } else {
                // Keep the previous scene until the entire replacement validates and allocates.
                let mut next = mesh::Scene::new(&self.device, &board, target_bytes)?;
                for model in &resources.models {
                    next.add_model(&self.device, model, target_bytes)?;
                }
                let resized = self.resize(dc, size, next.stats.resident_bytes)?;
                self.scene = Some(next);
                resized
            }
        } else {
            let resized = self.resize(dc, size, CAMERA_BYTES)?;
            self.scene = None;
            resized
        };
        self.model_handle = handle;
        self.resource_revision = revision;
        Ok(resized)
    }

    fn draw(
        &mut self,
        dc: &ID2D1DeviceContext,
        size: (f32, f32),
        model: Option<&SurfaceModel>,
    ) -> windows::core::Result<Option<SurfaceStatus>> {
        let snapshot = model.map(SurfaceModel::frame_snapshot);
        let frame = snapshot.map(|(_, frame)| frame);
        if frame.is_some_and(|frame| !frame.active) {
            return Ok(None);
        }
        let (mut dpi, mut dpi_y) = (96., 96.);
        unsafe { dc.GetDpi(&mut dpi, &mut dpi_y) };
        let Some(physical) = physical_size(
            size.0.into(),
            size.1.into(),
            (dpi / 96.).into(),
            D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION,
        )
        .map_err(failure)?
        else {
            return Ok(None);
        };
        let resized = self.prepare(dc, model, physical)?;
        let next_model = model
            .zip(snapshot)
            .map(|(model, (tick, _))| (model.handle(), tick));
        let target = self.target.as_ref().expect("resize created target");
        if resized || self.last_model != next_model {
            let bytes = frame
                .map(|frame| {
                    camera_bytes(
                        &frame.camera,
                        f64::from(physical[0]) / f64::from(physical[1]),
                        false,
                    )
                })
                .transpose()
                .map_err(failure)?;
            // Finish queued D2D reads before writing the same texture, without waiting on the CPU.
            unsafe {
                dc.Flush(None, None)?;
                let mut previous = None;
                self.immediate
                    .SwapDeviceContextState(&self.context_state, Some(&mut previous));
                self.pipeline.bind(&self.immediate);
                self.immediate
                    .OMSetRenderTargets(Some(&[Some(target.color.clone())]), &target.depth);
                self.immediate.RSSetViewports(Some(&[D3D11_VIEWPORT {
                    Width: physical[0] as f32,
                    Height: physical[1] as f32,
                    MinDepth: 0.,
                    MaxDepth: 1.,
                    ..Default::default()
                }]));
                self.immediate
                    .ClearRenderTargetView(&target.color, &[0.045, 0.071, 0.102, 1.]);
                self.immediate
                    .ClearDepthStencilView(&target.depth, D3D11_CLEAR_DEPTH.0, 1., 0);
                if let Some(bytes) = bytes {
                    self.immediate.UpdateSubresource(
                        &self.camera,
                        0,
                        None,
                        bytes.as_ptr().cast(),
                        0,
                        0,
                    );
                    self.immediate
                        .VSSetConstantBuffers(0, Some(&[Some(self.camera.clone())]));
                    self.immediate
                        .PSSetConstantBuffers(0, Some(&[Some(self.camera.clone())]));
                    if let Some(scene) = &mut self.scene {
                        scene.draw(
                            &self.immediate,
                            frame.expect("camera has frame").components_visible,
                        );
                        scene.stats.camera_writes += 1;
                        scene.stats.uploaded_bytes += CAMERA_BYTES;
                    }
                }
                // Restore every NSH D3D state, including bindings and targets, before D2D resumes.
                self.immediate
                    .SwapDeviceContextState(previous.as_ref(), None);
                self.device.GetDeviceRemovedReason()?;
            }
            self.last_model = next_model;
        }
        unsafe {
            dc.DrawBitmap(
                &target.bitmap,
                Some(&D2D_RECT_F {
                    left: 0.,
                    top: 0.,
                    right: size.0,
                    bottom: size.1,
                }),
                1.,
                D2D1_INTERPOLATION_MODE_LINEAR,
                None,
                None,
            );
        }
        Ok(Some(SurfaceStatus::Ready {
            backend: "D3D11".into(),
            width: physical[0],
            height: physical[1],
        }))
    }
}

struct State {
    renderer: Option<Renderer>,
    model: Option<u64>,
    last_status: Option<SurfaceStatus>,
}
struct WindowsSurface {
    state: RefCell<State>,
    context: ComponentContext,
}
impl NativeComponent for WindowsSurface {
    fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
        if name == "model" {
            self.state.get_mut().model = value.and_then(|value| value["handle"].as_u64());
        }
        self.context.redraw();
    }
    fn draw(&self, dc: &ID2D1DeviceContext, size: (f32, f32)) {
        let changed = {
            let mut state = self.state.borrow_mut();
            let result = (|| {
                let model = state.model.and_then(SurfaceModel::resolve);
                if state.model.is_some() && model.is_none() {
                    state.renderer.take();
                    return Ok(None);
                }
                let device = unsafe { dc.GetDevice()? };
                if state
                    .renderer
                    .as_ref()
                    .is_none_or(|renderer| renderer.d2d_device != device)
                {
                    state.renderer = Some(Renderer::new(dc)?);
                }
                state.renderer.as_mut().expect("renderer initialized").draw(
                    dc,
                    size,
                    model.as_deref(),
                )
            })();
            let status = match result {
                Ok(Some(status)) => status,
                Ok(None) => return,
                Err(error) => SurfaceStatus::Error(format!("无法绘制三维场景：{error}")),
            };
            if state.last_status.as_ref() == Some(&status) {
                return;
            }
            state.last_status = Some(status.clone());
            status
        };
        match changed {
            SurfaceStatus::Ready {
                backend,
                width,
                height,
            } => self.context.emit(
                "ready",
                serde_json::json!({"backend":backend,"width":width,"height":height}),
            ),
            SurfaceStatus::Error(error) => self.context.emit("error", JsonValue::String(error)),
            SurfaceStatus::Pending => {}
        }
    }
}

pub fn register(components: &Components) {
    components.register(
        "oshwhub.gpu",
        Rc::new(|context| {
            Box::new(WindowsSurface {
                state: RefCell::new(State {
                    renderer: None,
                    model: None,
                    last_status: None,
                }),
                context,
            })
        }),
    );
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
