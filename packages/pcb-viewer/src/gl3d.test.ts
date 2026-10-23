import { expect, it, vi } from "vitest";
import { fitCamera, projection } from "./camera3d";
import { identity, Renderer3d } from "./gl3d";

function renderer(version = "WebGL 1.0", extension = false, nativeWebGL2 = false) {
  const gl = {
    drawingBufferWidth: 640,
    drawingBufferHeight: 480,
    supportsWebGL2: nativeWebGL2,
    NO_ERROR: 0,
    VERSION: 7938,
    getParameter: () => version,
    getExtension: () => (extension ? {} : null),
    createBuffer: () => ({}),
    bindBuffer: vi.fn(),
    bufferData: vi.fn(),
    getError: () => 0,
    viewport: vi.fn(),
    clear: vi.fn(),
    useProgram: vi.fn(),
    uniformMatrix4fv: vi.fn(),
    uniformMatrix3fv: vi.fn(),
    uniform3fv: vi.fn(),
    drawArrays: vi.fn(),
    drawElements: vi.fn(),
    flush: vi.fn(),
  };
  const value = Object.assign(Object.create(Renderer3d.prototype), {
    gl,
    parts: [],
    attributes: [],
    bytes: 0,
    bounds: null,
  }) as Renderer3d;
  return { gl, value };
}

it("keeps uint indices on WebGL2, native GLES3 and WebGL1 with the extension", () => {
  const mesh = {
    data: Buffer.alloc(4 * 36).toString("base64"),
    vertexCount: 4,
    indices: Buffer.from(new Uint32Array([0, 1, 2, 0, 2, 3]).buffer).toString("base64"),
    indexCount: 6,
    bounds: { min: [0, 0, 0], max: [1, 1, 0] },
  };
  for (const [version, extension, native, indexed] of [
    ["WebGL 1.0", false, false, false],
    ["WebGL 1.0", true, false, true],
    ["WebGL 2.0", false, false, true],
    ["OpenGL ES 3.0", false, true, true],
  ] as const) {
    const { gl, value } = renderer(version, extension, native);
    value.add(mesh, [identity]);
    expect(value.bytes).toBe(indexed ? 168 : 216);
    value.draw(fitCamera(mesh.bounds, 1), true);
    expect(gl.drawElements).toHaveBeenCalledTimes(indexed ? 1 : 0);
    expect(gl.drawArrays).toHaveBeenCalledTimes(indexed ? 0 : 1);
  }
});

it("uses resized native pixel dimensions even when Expo's initial buffer properties stay stale", () => {
  const { gl, value } = renderer();
  const camera = fitCamera({ min: [-30, -20, 0], max: [30, 20, 2] }, 640 / 480);
  const original = structuredClone(camera);
  value.draw(camera, true, { width: 900, height: 1200 });
  expect(gl.viewport).toHaveBeenLastCalledWith(0, 0, 900, 1200);
  expect(gl.uniformMatrix4fv).toHaveBeenCalledWith(undefined, false, projection(camera, 0.75));
  expect(camera).toEqual(original);
});
