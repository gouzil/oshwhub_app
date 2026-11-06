use std::{
    collections::{BTreeMap, HashMap},
    rc::{Rc, Weak as RcWeak},
    sync::{Arc, Weak},
};

use oshwhub_core::pcb::{Bounds3, Mesh, RawModel3d, RawScene3d};
use wgpu::util::DeviceExt;

use super::{
    camera3d::{Camera3d, eye_position, projection},
    mesh::{MeshCounts, validate_instances, validate_mesh},
};

pub const MAX_GPU_BYTES: u64 = 128 * 1024 * 1024;
pub(crate) const CAMERA_BYTES: u64 = 80;
pub(crate) const VERTEX_STRIDE: u64 = 20;
pub(crate) const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UploadStats {
    pub mesh_uploads: u64,
    pub instance_uploads: u64,
    pub camera_writes: u64,
    pub uploaded_bytes: u64,
    pub resident_bytes: u64,
}

struct Geometry {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    bytes: u64,
    bounds: Bounds3,
}

struct Part {
    geometry: Rc<Geometry>,
    instances: wgpu::Buffer,
    matrices: Vec<[f32; 16]>,
    mesh_key: String,
    bounds: Bounds3,
}

struct Depth {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: [u32; 2],
    bytes: u64,
}

pub struct Render3d {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_group: wgpu::BindGroup,
    camera_ready: bool,
    srgb: bool,
    board: Option<Part>,
    board_source: Weak<Mesh>,
    models: BTreeMap<String, Part>,
    geometries: HashMap<String, RcWeak<Geometry>>,
    depth: Option<Depth>,
    components_visible: bool,
    bounds: Option<Bounds3>,
    stats: UploadStats,
}

impl Render3d {
    pub async fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Result<Self, String> {
        let limits = device.limits();
        if format.is_depth_stencil_format()
            || format.is_compressed()
            || limits.max_vertex_buffers < 2
            || limits.max_vertex_attributes < 7
            || limits.max_vertex_buffer_array_stride < 64
            || limits.max_uniform_buffer_binding_size < CAMERA_BYTES
            || limits.max_buffer_size < CAMERA_BYTES
        {
            return Err("当前 GPU 或表面格式不支持三维绘制".into());
        }
        let (pipeline, camera_buffer, camera_group) = checked(device, || {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("OSHWHub board lighting"),
                source: wgpu::ShaderSource::Wgsl(include_str!("board.wgsl").into()),
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("OSHWHub indexed 3D"), layout: None,
                vertex: wgpu::VertexState {
                    module: &shader, entry_point: Some("vertex_main"), compilation_options: Default::default(),
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: VERTEX_STRIDE, step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Snorm8x4, 2 => Unorm8x4],
                        },
                        wgpu::VertexBufferLayout {
                            array_stride: 64, step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4],
                        },
                    ],
                },
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth24Plus,
                    depth_write_enabled: Some(true), depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(), bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader, entry_point: Some("fragment_main"), compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None, cache: None,
            });
            let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("OSHWHub 3D camera"), size: CAMERA_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let camera_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("OSHWHub 3D camera"), layout: &pipeline.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: camera_buffer.as_entire_binding() }],
            });
            (pipeline, camera_buffer, camera_group)
        }).await?;
        Ok(Self {
            pipeline,
            camera_buffer,
            camera_group,
            camera_ready: false,
            srgb: format.is_srgb(),
            board: None,
            board_source: Weak::new(),
            models: BTreeMap::new(),
            geometries: HashMap::new(),
            depth: None,
            components_visible: true,
            bounds: None,
            stats: UploadStats {
                resident_bytes: CAMERA_BYTES,
                ..Default::default()
            },
        })
    }

    pub async fn set_board(
        &mut self,
        device: &wgpu::Device,
        scene: &RawScene3d,
    ) -> Result<(), String> {
        if self.board_source.ptr_eq(&Arc::downgrade(&scene.board)) {
            return Ok(());
        }
        let counts = mesh_counts(device, &scene.board)?;
        let bounds = instance_bounds(&scene.board.bounds, &[IDENTITY])?;
        let uploaded = u64::from(counts.vertices) * VERTEX_STRIDE + counts.index_bytes + 64;
        let resident = checked_budget(
            CAMERA_BYTES + self.depth.as_ref().map_or(0, |depth| depth.bytes),
            0,
            uploaded,
        )?;
        let part = checked(device, || Part {
            geometry: Rc::new(upload_geometry(device, &scene.board, counts)),
            instances: upload_instances(device, &[IDENTITY]),
            matrices: vec![IDENTITY],
            mesh_key: String::new(),
            bounds,
        })
        .await?;
        self.board = Some(part);
        self.board_source = Arc::downgrade(&scene.board);
        self.models.clear();
        self.geometries.clear();
        self.bounds = Some(scene.board.bounds.clone());
        self.stats.mesh_uploads += 1;
        self.stats.instance_uploads += 1;
        self.stats.uploaded_bytes += uploaded;
        self.stats.resident_bytes = resident;
        Ok(())
    }

    pub async fn add_model(
        &mut self,
        device: &wgpu::Device,
        model: &RawModel3d,
    ) -> Result<(), String> {
        if self.board.is_none() {
            return Err("请先加载三维板体".into());
        }
        let counts = mesh_counts(device, &model.mesh)?;
        let matrices: Vec<_> = model
            .instances
            .iter()
            .map(|instance| instance.matrix)
            .collect();
        let count = validate_instances(&matrices, device.limits().max_buffer_size)
            .map_err(str::to_owned)?;
        if count == 0 || count > 2000 || matrices.iter().any(|matrix| !valid_transform(matrix)) {
            return Err("器件装配或变换参数无效".into());
        }
        let previous = self.models.get(&model.key);
        if previous.is_some_and(|part| part.mesh_key == model.mesh_key && part.matrices == matrices)
        {
            return Ok(());
        }
        let cached = self
            .geometries
            .get(&model.mesh_key)
            .and_then(RcWeak::upgrade);
        let geometry_bytes = if cached.is_some() {
            0
        } else {
            u64::from(counts.vertices) * VERTEX_STRIDE + counts.index_bytes
        };
        let replaced = previous.map_or(0, |part| {
            part.matrices.len() as u64 * 64
                + if part.mesh_key != model.mesh_key && Rc::strong_count(&part.geometry) == 1 {
                    part.geometry.bytes
                } else {
                    0
                }
        });
        let instance_bytes = u64::from(count) * 64;
        let resident = checked_budget(
            self.stats.resident_bytes,
            replaced,
            geometry_bytes + instance_bytes,
        )?;
        let bounds = instance_bounds(
            cached
                .as_ref()
                .map_or(&model.mesh.bounds, |geometry| &geometry.bounds),
            &matrices,
        )?;
        let part = checked(device, || Part {
            geometry: cached
                .unwrap_or_else(|| Rc::new(upload_geometry(device, &model.mesh, counts))),
            instances: upload_instances(device, &matrices),
            matrices,
            mesh_key: model.mesh_key.clone(),
            bounds,
        })
        .await?;
        self.geometries
            .insert(model.mesh_key.clone(), Rc::downgrade(&part.geometry));
        self.models.insert(model.key.clone(), part);
        self.geometries
            .retain(|_, geometry| geometry.strong_count() > 0);
        self.recompute_bounds();
        self.stats.mesh_uploads += u64::from(geometry_bytes != 0);
        self.stats.instance_uploads += 1;
        self.stats.uploaded_bytes += geometry_bytes + instance_bytes;
        self.stats.resident_bytes = resident;
        Ok(())
    }

    pub fn write_camera(
        &mut self,
        queue: &wgpu::Queue,
        camera: &Camera3d,
        aspect: f64,
    ) -> Result<(), String> {
        let bytes = camera_bytes(camera, aspect, self.srgb)?;
        queue.write_buffer(&self.camera_buffer, 0, &bytes);
        self.camera_ready = true;
        self.stats.camera_writes += 1;
        self.stats.uploaded_bytes += CAMERA_BYTES;
        Ok(())
    }

    pub async fn resize_depth(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> Result<bool, String> {
        if self
            .depth
            .as_ref()
            .is_some_and(|depth| depth.size == [width, height])
        {
            return Ok(false);
        }
        let limit = device.limits().max_texture_dimension_2d;
        if width > limit || height > limit {
            return Err("三维深度缓冲尺寸超过 GPU 上限".into());
        }
        let replaced = self.depth.as_ref().map_or(0, |depth| depth.bytes);
        if width == 0 || height == 0 {
            self.depth = None;
            self.stats.resident_bytes -= replaced;
            return Ok(replaced != 0);
        }
        let bytes = u64::from(width) * u64::from(height) * 4;
        let resident = checked_budget(self.stats.resident_bytes, replaced, bytes)?;
        let depth = checked(device, || {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("OSHWHub 3D depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth24Plus,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            Depth {
                view: texture.create_view(&Default::default()),
                _texture: texture,
                size: [width, height],
                bytes,
            }
        })
        .await?;
        self.depth = Some(depth);
        self.stats.resident_bytes = resident;
        Ok(true)
    }

    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
    ) -> Result<(), String> {
        let depth = self.depth.as_ref().ok_or("三维绘图尺寸尚未就绪")?;
        if self.board.is_some() && !self.camera_ready {
            return Err("三维相机尚未就绪".into());
        }
        let color = |value| {
            if self.srgb {
                srgb_to_linear(value)
            } else {
                value
            }
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("OSHWHub 3D board and components"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: color(0.045),
                        g: color(0.071),
                        b: color(0.102),
                        a: 1.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_group, &[]);
        for part in self
            .board
            .iter()
            .chain(self.models.values().filter(|_| self.components_visible))
        {
            pass.set_vertex_buffer(0, part.geometry.vertices.slice(..));
            pass.set_vertex_buffer(1, part.instances.slice(..));
            pass.set_index_buffer(part.geometry.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(
                0..part.geometry.index_count,
                0,
                0..part.matrices.len() as u32,
            );
        }
        Ok(())
    }

    pub fn set_components_visible(&mut self, visible: bool) {
        self.components_visible = visible;
    }

    pub fn stats(&self) -> UploadStats {
        self.stats
    }

    pub fn bounds(&self) -> Option<&Bounds3> {
        self.bounds.as_ref()
    }

    fn recompute_bounds(&mut self) {
        let mut bounds = self.board.as_ref().map(|part| part.bounds.clone());
        for part in self.models.values() {
            let combined = bounds.get_or_insert_with(|| part.bounds.clone());
            for i in 0..3 {
                combined.min[i] = combined.min[i].min(part.bounds.min[i]);
                combined.max[i] = combined.max[i].max(part.bounds.max[i]);
            }
        }
        self.bounds = bounds;
    }
}

fn mesh_counts(device: &wgpu::Device, mesh: &Mesh) -> Result<MeshCounts, String> {
    let counts = validate_mesh(mesh, u64::MAX).map_err(str::to_owned)?;
    let limit = device.limits().max_buffer_size;
    if u64::from(counts.vertices) * VERTEX_STRIDE > limit || counts.index_bytes > limit {
        return Err("三维网格超过当前 GPU 的缓冲区限制".into());
    }
    Ok(counts)
}

fn upload_geometry(device: &wgpu::Device, mesh: &Mesh, counts: MeshCounts) -> Geometry {
    let vertices = pack_vertices(&mesh.vertices);
    let indices: Vec<_> = mesh
        .indices
        .iter()
        .flat_map(|index| index.to_le_bytes())
        .collect();
    Geometry {
        vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("OSHWHub 20-byte vertices"),
            contents: &vertices,
            usage: wgpu::BufferUsages::VERTEX,
        }),
        indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("OSHWHub 3D indices"),
            contents: &indices,
            usage: wgpu::BufferUsages::INDEX,
        }),
        index_count: counts.indices,
        bytes: vertices.len() as u64 + indices.len() as u64,
        bounds: mesh.bounds.clone(),
    }
}

fn upload_instances(device: &wgpu::Device, matrices: &[[f32; 16]]) -> wgpu::Buffer {
    let bytes: Vec<_> = matrices
        .iter()
        .flatten()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("OSHWHub 3D instances"),
        contents: &bytes,
        usage: wgpu::BufferUsages::VERTEX,
    })
}

pub(crate) fn pack_vertices(vertices: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vertices.len() / 9 * VERTEX_STRIDE as usize);
    for vertex in vertices.as_chunks::<9>().0 {
        for value in &vertex[..3] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        // Match JS Math.round's ties toward +infinity, including negative normals.
        bytes.extend(vertex[3..6].iter().map(|value| {
            ((f64::from(*value) * 127. + 0.5).floor().clamp(-127., 127.) as i8) as u8
        }));
        bytes.push(0);
        bytes.extend(
            vertex[6..9]
                .iter()
                .map(|value| (f64::from(*value) * 255. + 0.5).floor().clamp(0., 255.) as u8),
        );
        bytes.push(255);
    }
    bytes
}

pub(crate) fn valid_transform(m: &[f32; 16]) -> bool {
    let m = m.map(f64::from);
    let determinant = m[0] * (m[5] * m[10] - m[6] * m[9]) - m[4] * (m[1] * m[10] - m[2] * m[9])
        + m[8] * (m[1] * m[6] - m[2] * m[5]);
    m.iter()
        .all(|value| value.is_finite() && value.abs() <= 1e8)
        && determinant.is_finite()
        && determinant.abs() >= 1e-15
        && [m[3], m[7], m[11], m[15]] == [0., 0., 0., 1.]
}

pub(crate) fn instance_bounds(bounds: &Bounds3, matrices: &[[f32; 16]]) -> Result<Bounds3, String> {
    let mut result = Bounds3 {
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };
    for matrix in matrices {
        for x in [bounds.min[0], bounds.max[0]] {
            for y in [bounds.min[1], bounds.max[1]] {
                for z in [bounds.min[2], bounds.max[2]] {
                    for i in 0..3 {
                        let value = f64::from(matrix[i]) * x
                            + f64::from(matrix[4 + i]) * y
                            + f64::from(matrix[8 + i]) * z
                            + f64::from(matrix[12 + i]);
                        if !(value as f32).is_finite() {
                            return Err("三维器件装配超出绘图精度范围".into());
                        }
                        result.min[i] = result.min[i].min(value);
                        result.max[i] = result.max[i].max(value);
                    }
                }
            }
        }
    }
    Ok(result)
}

pub(crate) fn checked_budget(current: u64, replaced: u64, additional: u64) -> Result<u64, String> {
    current
        .checked_sub(replaced)
        .and_then(|bytes| bytes.checked_add(additional))
        .filter(|bytes| *bytes <= MAX_GPU_BYTES)
        .ok_or_else(|| "三维缓存已达到 128 MiB 上限，部分内容未加载".into())
}

pub(crate) fn camera_bytes(
    camera: &Camera3d,
    aspect: f64,
    srgb: bool,
) -> Result<[u8; CAMERA_BYTES as usize], String> {
    if ![
        camera.yaw,
        camera.pitch,
        camera.distance,
        camera.radius,
        aspect,
    ]
    .into_iter()
    .chain(camera.target)
    .all(f64::is_finite)
        || camera.distance <= 0.
        || camera.radius <= 0.
        || aspect <= 0.
    {
        return Err("三维相机参数无效".into());
    }
    let eye = eye_position(camera).map(|value| value as f32);
    let values = projection(camera, aspect)
        .into_iter()
        .chain(eye)
        .chain([f32::from(u8::from(srgb))]);
    let mut bytes = [0_u8; CAMERA_BYTES as usize];
    for (chunk, value) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(values) {
        if !value.is_finite() {
            return Err("三维相机超出绘图精度范围".into());
        }
        chunk.copy_from_slice(&value.to_le_bytes());
    }
    Ok(bytes)
}

async fn checked<T>(device: &wgpu::Device, build: impl FnOnce() -> T) -> Result<T, String> {
    let oom = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let value = build();
    let validation = validation.pop();
    let internal = internal.pop();
    let oom = oom.pop();
    let validation = validation.await;
    let internal = internal.await;
    let oom = oom.await;
    if let Some(error) = validation.or(internal).or(oom) {
        Err(format!("三维 GPU 资源创建失败：{error}"))
    } else {
        Ok(value)
    }
}

fn srgb_to_linear(value: f64) -> f64 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_vertices_keep_gl_positions_signed_normals_and_colors_in_twenty_bytes() {
        let packed = pack_vertices(&[1.25, -2., 3., 1., -1., -0.5, 1., 0.5, -0.25]);
        assert_eq!(
            packed,
            [
                0, 0, 160, 63, 0, 0, 0, 192, 0, 0, 64, 64, 127, 129, 193, 0, 255, 128, 0, 255,
            ]
        );
    }

    #[test]
    fn cumulative_budget_counts_instances_replacements_and_integer_overflow() {
        assert_eq!(checked_budget(134_217_664, 0, 64).unwrap(), 134_217_728);
        assert!(checked_budget(134_217_664, 0, 65).is_err());
        assert_eq!(checked_budget(134_217_728, 128, 128).unwrap(), 134_217_728);
        assert!(checked_budget(134_217_728, 0, u64::MAX).is_err());
        assert!(checked_budget(0, 1, 0).is_err());
    }
}
