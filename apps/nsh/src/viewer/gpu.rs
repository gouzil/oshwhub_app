//! A demand-driven surface shared by NSH's platform hosts.

use super::{render3d::Render3d, surface::SurfaceModel};
use oshwhub_core::pcb::{RawModel3d, RawScene3d};
use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};

pub(super) struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,
    pub backend: String,
    renderer: Option<Render3d>,
    board_source: Weak<RawScene3d>,
    model_sources: BTreeMap<String, Weak<RawModel3d>>,
    model_handle: Option<u64>,
    resource_revision: u64,
    prepared_size: Option<[u32; 2]>,
    configured: bool,
    outdated: bool,
}

impl Gpu {
    #[cfg(any(target_os = "linux", test))]
    pub async fn offscreen(
        adapter: wgpu::Adapter,
        format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        Self::from_adapter(
            adapter,
            None,
            wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: 1,
                height: 1,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                view_formats: vec![],
            },
        )
        .await
    }
    #[cfg(not(target_os = "linux"))]
    pub async fn new(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
    ) -> Result<Self, String> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::LowPower,
                ..Default::default()
            })
            .await
            .map_err(|error| format!("无法使用 GPU：{error}"))?;
        let mut config = surface
            .get_default_config(&adapter, 1, 1)
            .ok_or_else(|| "GPU 不支持当前绘图表面".to_owned())?;
        let capabilities = surface.get_capabilities(&adapter);
        if let Some(format) = capabilities.formats.iter().find(|format| !format.is_srgb()) {
            config.format = *format;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        config.alpha_mode = capabilities
            .alpha_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
            .unwrap_or(config.alpha_mode);
        Self::from_adapter(adapter, Some(surface), config).await
    }

    async fn from_adapter(
        adapter: wgpu::Adapter,
        surface: Option<wgpu::Surface<'static>>,
        config: wgpu::SurfaceConfiguration,
    ) -> Result<Self, String> {
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("OSHWHub viewer"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|error| format!("无法创建 GPU 设备：{error}"))?;
        let backend = format!("{:?}", adapter.get_info().backend);
        Ok(Self {
            device,
            queue,
            surface,
            config,
            backend,
            renderer: None,
            board_source: Weak::new(),
            model_sources: BTreeMap::new(),
            model_handle: None,
            resource_revision: 0,
            prepared_size: None,
            configured: false,
            outdated: false,
        })
    }

    pub fn needs_prepare(&self, model: Option<&SurfaceModel>, size: [u32; 2]) -> bool {
        !self.configured
            || self.outdated
            || self.prepared_size != Some(size)
            || self.needs_resource_prepare(model)
    }

    /// Resource identity/revision check used to keep camera-only frames synchronous.
    pub fn needs_resource_prepare(&self, model: Option<&SurfaceModel>) -> bool {
        self.model_handle != model.map(SurfaceModel::handle)
            || self.resource_revision != model.map_or(0, SurfaceModel::resource_revision)
    }

    /// Call only for resource or physical-size changes, with the host's RefCell borrow released.
    pub async fn prepare(
        &mut self,
        model: Option<&SurfaceModel>,
        size: [u32; 2],
    ) -> Result<(), String> {
        let changed_owner = self.model_handle != model.map(SurfaceModel::handle);
        let next_revision = model.map_or(0, SurfaceModel::resource_revision);
        let resource_changed = changed_owner || self.resource_revision != next_revision;
        if resource_changed {
            let resources = model
                .and_then(|model| {
                    model.resources_since(if changed_owner {
                        u64::MAX
                    } else {
                        self.resource_revision
                    })
                })
                .ok_or("三维资源状态已过期，请重试")?;
            let Some(board) = resources.board else {
                self.renderer = None;
                self.board_source = Weak::new();
                self.model_sources.clear();
                self.model_handle = model.map(SurfaceModel::handle);
                self.resource_revision = next_revision;
                self.prepared_size = Some(size);
                return Ok(());
            };
            if !changed_owner && self.board_source.ptr_eq(&Arc::downgrade(&board)) {
                let renderer = self.renderer.as_mut().ok_or("三维绘图状态已释放")?;
                for model in &resources.models {
                    if self
                        .model_sources
                        .get(&model.key)
                        .is_some_and(|source| source.ptr_eq(&Arc::downgrade(model)))
                    {
                        continue;
                    }
                    // Each addition validates and allocates before replacing its previous part.
                    renderer.add_model(&self.device, model).await?;
                    self.model_sources
                        .insert(model.key.clone(), Arc::downgrade(model));
                }
                renderer
                    .resize_depth(&self.device, size[0], size[1])
                    .await?;
            } else {
                // A new board commits only after its complete replacement is ready.
                let mut next = Render3d::new(&self.device, self.config.format).await?;
                next.set_board(&self.device, &board).await?;
                for model in &resources.models {
                    next.add_model(&self.device, model).await?;
                }
                next.resize_depth(&self.device, size[0], size[1]).await?;
                self.renderer = Some(next);
                self.board_source = Arc::downgrade(&board);
                self.model_sources = resources
                    .models
                    .iter()
                    .map(|model| (model.key.clone(), Arc::downgrade(model)))
                    .collect();
            }
        } else if let Some(renderer) = self.renderer.as_mut() {
            renderer
                .resize_depth(&self.device, size[0], size[1])
                .await?;
        }
        self.model_handle = model.map(SurfaceModel::handle);
        self.resource_revision = next_revision;
        self.prepared_size = Some(size);
        Ok(())
    }

    /// Canvas dimensions and the surface are committed only after depth allocation succeeds.
    pub fn configure(&mut self, size: [u32; 2]) {
        if !self.configured || self.outdated || self.size() != size {
            self.config.width = size[0];
            self.config.height = size[1];
            if let Some(surface) = &self.surface {
                surface.configure(&self.device, &self.config);
            }
            self.configured = true;
            self.outdated = false;
        }
    }

    pub fn size(&self) -> [u32; 2] {
        [self.config.width, self.config.height]
    }

    #[cfg(not(target_os = "linux"))]
    pub fn draw(&mut self, model: Option<&SurfaceModel>) -> Result<bool, String> {
        if !self.configured || model.is_some_and(|model| !model.frame().active) {
            return Ok(false);
        }
        let frame = match self
            .surface
            .as_ref()
            .ok_or("当前 GPU 宿主需要外部绘图目标")?
            .get_current_texture()
        {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.outdated = true;
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                return Err("GPU 表面已丢失，请重新打开查看器".into());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("GPU 绘图配置无效，请重新打开查看器".into());
            }
        };
        let target = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        if !self.draw_into(model, &target)? {
            return Ok(false);
        }
        frame.present();
        Ok(true)
    }

    pub fn draw_into(
        &mut self,
        model: Option<&SurfaceModel>,
        target: &wgpu::TextureView,
    ) -> Result<bool, String> {
        if !self.configured || model.is_some_and(|model| !model.frame().active) {
            return Ok(false);
        }
        if let (Some(renderer), Some(model)) = (self.renderer.as_mut(), model) {
            let frame = model.frame();
            renderer.set_components_visible(frame.components_visible);
            renderer.write_camera(
                &self.queue,
                &frame.camera,
                f64::from(self.config.width) / f64::from(self.config.height),
            )?;
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("OSHWHub frame"),
            });
        if let Some(renderer) = &self.renderer {
            renderer.encode(&mut encoder, target)?;
        } else {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("OSHWHub background"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 11.0 / 255.0,
                            g: 18.0 / 255.0,
                            b: 26.0 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        self.queue.submit([encoder.finish()]);
        Ok(true)
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        self.device.destroy();
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::viewer::camera3d::fit_camera;
    use nsh_platform_macos::objc2::rc::Retained;
    use objc2_quartz_core::CAMetalLayer;
    use oshwhub_core::pcb::{Bounds3, Instance, Mesh, RawModel3d, RawScene3d};
    use std::sync::Arc;

    #[test]
    fn offscreen_host_draws_into_the_supplied_texture() {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::METAL,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = instance.request_adapter(&Default::default()).await.unwrap();
            let mut gpu = Gpu::offscreen(adapter, wgpu::TextureFormat::Rgba8Unorm)
                .await
                .unwrap();
            gpu.prepare(None, [64, 64]).await.unwrap();
            gpu.configure([64, 64]);
            let size = wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            };
            let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            assert!(
                gpu.draw_into(None, &target.create_view(&Default::default()))
                    .unwrap()
            );
            let output = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 64 * 256,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                target.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &output,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(64),
                    },
                },
                size,
            );
            let submission = gpu.queue.submit([encoder.finish()]);
            let (send, receive) = std::sync::mpsc::channel();
            output
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    send.send(result).unwrap()
                });
            gpu.device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: Some(std::time::Duration::from_secs(10)),
                })
                .unwrap();
            receive
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap()
                .unwrap();
            assert_eq!(
                &output.slice(..).get_mapped_range()[..4],
                &[11, 18, 26, 255]
            );
            output.unmap();
        });
    }

    fn mesh() -> Arc<Mesh> {
        Arc::new(Mesh {
            vertices: vec![
                0., 0., 0., 0., 0., 1., 1., 0., 0., 1., 0., 0., 0., 0., 1., 1., 0., 0., 0., 1., 0.,
                0., 0., 1., 1., 0., 0.,
            ],
            indices: vec![0, 1, 2],
            bounds: Bounds3 {
                min: [0.; 3],
                max: [1.; 3],
            },
        })
    }

    #[test]
    fn progressive_prepare_retains_the_renderer_and_uploads_each_mesh_once() {
        let layer = CAMetalLayer::new();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        // SAFETY: the layer outlives the GPU and its surface below.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                Retained::as_ptr(&layer).cast_mut().cast(),
            ))
        }
        .unwrap();
        pollster::block_on(async {
            let mut gpu = Gpu::new(instance, surface).await.unwrap();
            let board = Arc::new(RawScene3d {
                board: mesh(),
                model_keys: vec![],
                component_count: 0,
                bound_count: 0,
                thickness: 1.6,
                warnings: vec![],
            });
            let camera = fit_camera(&board.board.bounds, 1.);
            let model = SurfaceModel::new(camera);
            model.set_board(board);
            gpu.prepare(Some(&model), [32, 32]).await.unwrap();
            gpu.renderer
                .as_mut()
                .unwrap()
                .write_camera(&gpu.queue, &camera, 1.)
                .unwrap();
            for index in 0..3 {
                model.add_model(Arc::new(RawModel3d {
                    key: format!("part-{index}"),
                    mesh_key: format!("mesh-{index}"),
                    mesh: mesh(),
                    instances: vec![Instance {
                        title: "part".into(),
                        matrix: [
                            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
                        ],
                    }],
                }));
                gpu.prepare(Some(&model), [32, 32]).await.unwrap();
                let stats = gpu.renderer.as_ref().unwrap().stats();
                assert_eq!(
                    stats.camera_writes, 1,
                    "model arrivals must retain the renderer and its camera buffer"
                );
                assert_eq!(stats.mesh_uploads, index + 2);
                assert_eq!(stats.instance_uploads, index + 2);
            }
            let stats = gpu.renderer.as_ref().unwrap().stats();
            model.add_model(Arc::new(RawModel3d {
                key: "invalid".into(),
                mesh_key: "invalid".into(),
                mesh: mesh(),
                instances: vec![],
            }));
            assert!(gpu.prepare(Some(&model), [32, 32]).await.is_err());
            assert_eq!(
                gpu.renderer.as_ref().unwrap().stats(),
                stats,
                "a failed addition must preserve uploaded geometry"
            );
        });
    }
}
