use super::*;
use crate::viewer::{
    mesh::{MeshCounts, validate_instances, validate_mesh},
    render3d::{
        IDENTITY, MAX_GPU_BYTES, UploadStats, VERTEX_STRIDE, instance_bounds, pack_vertices,
        valid_transform,
    },
};
use oshwhub_core::pcb::{Bounds3, Mesh, RawModel3d, RawScene3d};
use std::{
    collections::{BTreeMap, HashMap},
    rc::Weak as RcWeak,
    sync::{Arc, Weak},
};

pub(super) fn buffer(
    device: &ID3D11Device,
    bytes: &[u8],
    bind: D3D11_BIND_FLAG,
) -> windows::core::Result<ID3D11Buffer> {
    let length = u32::try_from(bytes.len()).map_err(|_| failure("三维缓冲区过大"))?;
    if length == 0 || u64::from(length) > MAX_GPU_BYTES {
        return Err(failure("三维缓冲区大小无效"));
    }
    let mut result = None;
    unsafe {
        device.CreateBuffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: length,
                Usage: if bind == D3D11_BIND_CONSTANT_BUFFER {
                    D3D11_USAGE_DEFAULT
                } else {
                    D3D11_USAGE_IMMUTABLE
                },
                BindFlags: bind.0 as u32,
                ..Default::default()
            },
            Some(&D3D11_SUBRESOURCE_DATA {
                pSysMem: bytes.as_ptr().cast(),
                ..Default::default()
            }),
            Some(&mut result),
        )?;
    }
    result.ok_or_else(|| failure("无法创建三维 GPU 缓冲区"))
}

struct Geometry {
    vertices: ID3D11Buffer,
    indices: ID3D11Buffer,
    count: u32,
    bytes: u64,
    bounds: Bounds3,
}
impl Geometry {
    fn new(device: &ID3D11Device, mesh: &Mesh, counts: MeshCounts) -> windows::core::Result<Self> {
        let vertices = pack_vertices(&mesh.vertices);
        let indices: Vec<_> = mesh
            .indices
            .iter()
            .flat_map(|index| index.to_le_bytes())
            .collect();
        Ok(Self {
            vertices: buffer(device, &vertices, D3D11_BIND_VERTEX_BUFFER)?,
            indices: buffer(device, &indices, D3D11_BIND_INDEX_BUFFER)?,
            count: counts.indices,
            bytes: vertices.len() as u64 + indices.len() as u64,
            bounds: mesh.bounds.clone(),
        })
    }
}
struct Part {
    geometry: Rc<Geometry>,
    instances: ID3D11Buffer,
    matrices: Vec<[f32; 16]>,
    mesh_key: String,
    source: Weak<RawModel3d>,
}
fn instances(device: &ID3D11Device, matrices: &[[f32; 16]]) -> windows::core::Result<ID3D11Buffer> {
    let bytes: Vec<_> = matrices
        .iter()
        .flatten()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    buffer(device, &bytes, D3D11_BIND_VERTEX_BUFFER)
}

pub(super) struct Scene {
    board: Part,
    pub board_source: Weak<RawScene3d>,
    models: BTreeMap<String, Part>,
    geometries: HashMap<String, RcWeak<Geometry>>,
    // Target color/depth bytes are accounted at allocation; retained mesh stats exclude them.
    pub stats: UploadStats,
}
impl Scene {
    pub fn new(
        device: &ID3D11Device,
        scene: &Arc<RawScene3d>,
        target_bytes: u64,
    ) -> windows::core::Result<Self> {
        let counts = validate_mesh(&scene.board, MAX_GPU_BYTES).map_err(failure)?;
        instance_bounds(&scene.board.bounds, &[IDENTITY]).map_err(failure)?;
        let uploaded = u64::from(counts.vertices) * VERTEX_STRIDE + counts.index_bytes + 64;
        checked_budget(CAMERA_BYTES, 0, uploaded + target_bytes).map_err(failure)?;
        Ok(Self {
            board: Part {
                geometry: Rc::new(Geometry::new(device, &scene.board, counts)?),
                instances: instances(device, &[IDENTITY])?,
                matrices: vec![IDENTITY],
                mesh_key: String::new(),
                source: Weak::new(),
            },
            board_source: Arc::downgrade(scene),
            models: BTreeMap::new(),
            geometries: HashMap::new(),
            stats: UploadStats {
                mesh_uploads: 1,
                instance_uploads: 1,
                uploaded_bytes: uploaded,
                resident_bytes: CAMERA_BYTES + uploaded,
                ..Default::default()
            },
        })
    }

    pub fn add_model(
        &mut self,
        device: &ID3D11Device,
        model: &Arc<RawModel3d>,
        target_bytes: u64,
    ) -> windows::core::Result<()> {
        let previous = self.models.get(&model.key);
        if previous.is_some_and(|part| part.source.ptr_eq(&Arc::downgrade(model))) {
            return Ok(());
        }
        let counts = validate_mesh(&model.mesh, MAX_GPU_BYTES).map_err(failure)?;
        let matrices: Vec<_> = model
            .instances
            .iter()
            .map(|instance| instance.matrix)
            .collect();
        let count = validate_instances(&matrices, MAX_GPU_BYTES).map_err(failure)?;
        if count == 0 || count > 2000 || matrices.iter().any(|matrix| !valid_transform(matrix)) {
            return Err(failure("器件装配或变换参数无效"));
        }
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
        let additional = geometry_bytes + u64::from(count) * 64;
        let resident = checked_budget(
            self.stats.resident_bytes,
            replaced,
            additional + target_bytes,
        )
        .map_err(failure)?
            - target_bytes;
        instance_bounds(
            cached
                .as_ref()
                .map_or(&model.mesh.bounds, |geometry| &geometry.bounds),
            &matrices,
        )
        .map_err(failure)?;
        let part = Part {
            geometry: if let Some(geometry) = cached {
                geometry
            } else {
                Rc::new(Geometry::new(device, &model.mesh, counts)?)
            },
            instances: instances(device, &matrices)?,
            matrices,
            mesh_key: model.mesh_key.clone(),
            source: Arc::downgrade(model),
        };
        self.geometries
            .insert(model.mesh_key.clone(), Rc::downgrade(&part.geometry));
        self.models.insert(model.key.clone(), part);
        self.geometries
            .retain(|_, geometry| geometry.strong_count() > 0);
        self.stats.mesh_uploads += u64::from(geometry_bytes != 0);
        self.stats.instance_uploads += 1;
        self.stats.uploaded_bytes += additional;
        self.stats.resident_bytes = resident;
        Ok(())
    }

    pub fn draw(&self, dc: &ID3D11DeviceContext, components_visible: bool) {
        for part in
            std::iter::once(&self.board).chain(self.models.values().filter(|_| components_visible))
        {
            unsafe {
                dc.IASetVertexBuffers(
                    0,
                    2,
                    Some(
                        [
                            Some(part.geometry.vertices.clone()),
                            Some(part.instances.clone()),
                        ]
                        .as_ptr(),
                    ),
                    Some([VERTEX_STRIDE as u32, 64].as_ptr()),
                    Some([0, 0].as_ptr()),
                );
                dc.IASetIndexBuffer(&part.geometry.indices, DXGI_FORMAT_R32_UINT, 0);
                dc.DrawIndexedInstanced(part.geometry.count, part.matrices.len() as u32, 0, 0, 0);
            }
        }
    }
}
