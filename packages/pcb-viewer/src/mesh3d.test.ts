import { expect, it } from "vitest";
import { decodeIndices, decodeMesh } from "./mesh3d";

it("decodes packed little-endian GPU vertices and rejects corrupt or oversized meshes", () => {
  const data = Buffer.alloc(27 * 4);
  data.writeFloatLE(1.5, 0);
  const packed = {
    data: data.toString("base64"),
    vertexCount: 3,
    bounds: { min: [0, 0, 0], max: [2, 2, 2] },
  };
  expect(decodeMesh(packed)[0]).toBe(1.5);
  expect(() => decodeMesh({ ...packed, vertexCount: 6 })).toThrow();
  expect(() => decodeMesh({ ...packed, vertexCount: 1_500_003 })).toThrow();
  expect(() => decodeMesh({ ...packed, data: "corrupt" })).toThrow();
  expect(() => decodeMesh({ ...packed, bounds: { min: [NaN, 0, 0], max: [2, 2, 2] } })).toThrow();
  expect(() => decodeMesh({ ...packed, bounds: { min: [3, 0, 0], max: [2, 2, 2] } })).toThrow();
  data.writeFloatLE(Number.NaN, 4);
  expect(() => decodeMesh({ ...packed, data: data.toString("base64") })).toThrow();
});

it("decodes indexed meshes and rejects out-of-range or incomplete indices", () => {
  const packed = {
    data: Buffer.alloc(4 * 36).toString("base64"),
    vertexCount: 4,
    indices: Buffer.from(new Uint32Array([0, 1, 2, 0, 2, 3]).buffer).toString("base64"),
    indexCount: 6,
    bounds: { min: [0, 0, 0], max: [1, 1, 0] },
  };
  expect(decodeMesh(packed)).toHaveLength(36);
  expect([...(decodeIndices(packed) ?? [])]).toEqual([0, 1, 2, 0, 2, 3]);
  expect(() => decodeIndices({ ...packed, vertexCount: 3 })).toThrow();
  expect(() => decodeIndices({ ...packed, indexCount: 9 })).toThrow();
  expect(() => decodeIndices({ ...packed, indices: undefined })).toThrow();
});

it("accepts shared vertices with more indices than the old expanded-vertex limit", () => {
  const count = 1_500_003;
  const packed = {
    data: Buffer.alloc(3 * 36).toString("base64"),
    vertexCount: 3,
    indices: Buffer.alloc(count * 4).toString("base64"),
    indexCount: count,
    bounds: { min: [0, 0, 0], max: [1, 1, 0] },
  };
  expect(decodeIndices(packed)).toHaveLength(count);
  expect(() => decodeIndices({ ...packed, indexCount: 6_000_003 })).toThrow();
});
