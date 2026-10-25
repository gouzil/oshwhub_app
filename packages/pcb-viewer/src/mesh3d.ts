import type { PackedMesh } from "@oshwhub/bridge";

function decodeBase64(data: string, bytes: number): Uint8Array<ArrayBuffer> {
  if (data.length !== Math.ceil(bytes / 3) * 4) throw new Error("三维网格数据不完整。");
  const binary = atob(data);
  if (binary.length !== bytes) throw new Error("三维网格数据不完整。");
  const result = new Uint8Array(bytes);
  for (let i = 0; i < bytes; i++) result[i] = binary.charCodeAt(i);
  return result;
}

export function decodeMesh(mesh: PackedMesh): Float32Array {
  const count = mesh.vertexCount;
  if (
    !Number.isInteger(count) ||
    count <= 0 ||
    (mesh.indices === undefined && count % 3 !== 0) ||
    count > 3_000_000 ||
    mesh.data.length !== count * 48 ||
    mesh.bounds.min.length !== 3 ||
    mesh.bounds.max.length !== 3 ||
    mesh.bounds.min.some(
      (v, i) =>
        !Number.isFinite(v) || !Number.isFinite(mesh.bounds.max[i]) || v > mesh.bounds.max[i],
    )
  )
    throw new Error("三维网格大小无效。");
  const bytes = decodeBase64(mesh.data, count * 36);
  const view = new DataView(bytes.buffer);
  const vertices = new Float32Array(bytes.buffer);
  for (let i = 0; i < vertices.length; i++) {
    const value = view.getFloat32(i * 4, true);
    if (!Number.isFinite(value)) throw new Error("三维网格包含无效坐标。");
    vertices[i] = value;
  }
  return vertices;
}

export function decodeIndices(mesh: PackedMesh): Uint32Array<ArrayBuffer> | null {
  if (mesh.indices === undefined && mesh.indexCount === undefined) return null;
  const count = mesh.indexCount;
  if (
    typeof mesh.indices !== "string" ||
    count === undefined ||
    !Number.isInteger(count) ||
    count <= 0 ||
    count % 3 !== 0 ||
    count > 8_000_000
  )
    throw new Error("三维网格索引无效。");
  const bytes = decodeBase64(mesh.indices, count * 4);
  const view = new DataView(bytes.buffer);
  const indices = new Uint32Array(bytes.buffer);
  for (let i = 0; i < count; i++) {
    const value = view.getUint32(i * 4, true);
    if (value >= mesh.vertexCount) throw new Error("三维网格索引超出范围。");
    indices[i] = value;
  }
  return indices;
}
