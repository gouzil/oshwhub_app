use oshwhub_core::pcb::Mesh;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshCounts {
    pub vertices: u32,
    pub indices: u32,
    pub vertex_bytes: u64,
    pub index_bytes: u64,
}

pub fn validate_mesh(mesh: &Mesh, max_buffer_size: u64) -> Result<MeshCounts, &'static str> {
    if mesh.vertices.is_empty()
        || !mesh.vertices.len().is_multiple_of(9)
        || mesh.indices.is_empty()
        || !mesh.indices.len().is_multiple_of(3)
    {
        return Err("三维网格的顶点或三角形数量无效");
    }
    let vertices =
        u32::try_from(mesh.vertices.len() / 9).map_err(|_| "三维顶点数量超过绘图上限")?;
    let indices = u32::try_from(mesh.indices.len()).map_err(|_| "三维索引数量超过绘图上限")?;
    let vertex_bytes = std::mem::size_of_val(mesh.vertices.as_slice()) as u64;
    let index_bytes = std::mem::size_of_val(mesh.indices.as_slice()) as u64;
    if vertex_bytes > max_buffer_size || index_bytes > max_buffer_size {
        return Err("三维网格超过当前 GPU 的缓冲区限制");
    }
    if !mesh.vertices.iter().all(|value| value.is_finite())
        || mesh.indices.iter().any(|index| *index >= vertices)
        || mesh
            .bounds
            .min
            .iter()
            .zip(mesh.bounds.max)
            .any(|(min, max)| !min.is_finite() || !max.is_finite() || *min > max)
    {
        return Err("三维网格的坐标、索引或边界无效");
    }
    Ok(MeshCounts {
        vertices,
        indices,
        vertex_bytes,
        index_bytes,
    })
}

pub fn validate_instances<'a>(
    matrices: impl IntoIterator<Item = &'a [f32; 16]>,
    max_buffer_size: u64,
) -> Result<u32, &'static str> {
    let mut count = 0_u32;
    for matrix in matrices {
        if !matrix.iter().all(|value| value.is_finite()) {
            return Err("三维器件的变换矩阵无效");
        }
        count = count.checked_add(1).ok_or("三维器件数量超过绘图上限")?;
        if u64::from(count) * 64 > max_buffer_size {
            return Err("三维器件超过当前 GPU 的缓冲区限制");
        }
    }
    Ok(count)
}
