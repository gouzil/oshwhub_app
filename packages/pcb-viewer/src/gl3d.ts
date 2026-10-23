import type { Bounds3, PackedMesh } from "@oshwhub/bridge";
import { type Camera3d, eyePosition, projection } from "./camera3d";
import { decodeIndices, decodeMesh } from "./mesh3d";

export const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
type Part = {
  buffer: WebGLBuffer;
  indices: WebGLBuffer | null;
  count: number;
  instances: { matrix: Float32Array; normal: Float32Array }[];
  component: boolean;
};
const vertexShader = `
attribute vec3 aPosition;
attribute vec3 aNormal;
attribute vec3 aColor;
uniform mat4 uCamera;
uniform mat4 uModel;
uniform mat3 uNormal;
varying vec3 vNormal;
varying vec3 vColor;
varying vec3 vPosition;
void main() {
  vec4 world = uModel * vec4(aPosition, 1.0);
  vPosition = world.xyz;
  vNormal = normalize(uNormal * aNormal);
  vColor = aColor;
  gl_Position = uCamera * world;
}`;
const fragmentShader = `
precision mediump float;
uniform vec3 uEye;
varying vec3 vNormal;
varying vec3 vColor;
varying vec3 vPosition;
void main() {
  vec3 n = normalize(vNormal);
  if (!gl_FrontFacing) n = -n;
  vec3 light = normalize(vec3(-0.4, -0.5, 1.0));
  float diffuse = max(dot(n, light), 0.0) * 0.65;
  float fill = max(dot(n, normalize(vec3(0.6, 0.5, 0.2))), 0.0) * 0.18;
  vec3 halfway = normalize(light + normalize(uEye - vPosition));
  float specular = pow(max(dot(n, halfway), 0.0), 40.0) * 0.16;
  gl_FragColor = vec4(vColor * (0.32 + diffuse + fill) + vec3(specular), 1.0);
}`;

function normalMatrix(m: number[]): Float32Array {
  const cross = (a: number[], b: number[]) => [
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  ];
  const a = m.slice(0, 3),
    b = m.slice(4, 7),
    c = m.slice(8, 11);
  const x = cross(b, c),
    y = cross(c, a),
    z = cross(a, b);
  const determinant = a.reduce((sum, v, i) => sum + v * x[i], 0);
  if (!Number.isFinite(determinant) || Math.abs(determinant) < 1e-15)
    throw new Error("器件变换无效。");
  return Float32Array.from([...x, ...y, ...z], (v) => v / determinant);
}

export class Renderer3d {
  private readonly program: WebGLProgram;
  private readonly parts: Part[] = [];
  private readonly cameraLocation: WebGLUniformLocation;
  private readonly modelLocation: WebGLUniformLocation;
  private readonly normalLocation: WebGLUniformLocation;
  private readonly eyeLocation: WebGLUniformLocation;
  private readonly attributes: number[];
  bytes = 0;
  bounds: Bounds3 | null = null;

  // shortcut: partial GPU initialization relies on context teardown; add cleanup if driver failures recur.
  constructor(private readonly gl: WebGLRenderingContext) {
    const shader = (type: number, source: string) => {
      const shader = gl.createShader(type);
      if (!shader) throw new Error("无法创建三维着色器。");
      gl.shaderSource(shader, source);
      gl.compileShader(shader);
      if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
        gl.deleteShader(shader);
        throw new Error("当前设备无法编译三维着色器。");
      }
      return shader;
    };
    const vs = shader(gl.VERTEX_SHADER, vertexShader),
      fs = shader(gl.FRAGMENT_SHADER, fragmentShader);
    const program = gl.createProgram();
    if (!program) throw new Error("无法创建三维绘制程序。");
    gl.attachShader(program, vs);
    gl.attachShader(program, fs);
    gl.linkProgram(program);
    gl.deleteShader(vs);
    gl.deleteShader(fs);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      gl.deleteProgram(program);
      throw new Error("当前设备无法初始化三维绘制。");
    }
    this.program = program;
    const location = (name: string) => {
      const result = gl.getUniformLocation(program, name);
      if (result === null) throw new Error("三维绘制参数缺失。");
      return result;
    };
    this.cameraLocation = location("uCamera");
    this.modelLocation = location("uModel");
    this.normalLocation = location("uNormal");
    this.eyeLocation = location("uEye");
    this.attributes = ["aPosition", "aNormal", "aColor"].map((name) =>
      gl.getAttribLocation(program, name),
    );
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    gl.clearColor(0.045, 0.071, 0.102, 1);
  }

  add(mesh: PackedMesh, instances: number[][], component = false) {
    if (
      !instances.length ||
      instances.length > 2000 ||
      instances.some(
        (m) => m.length !== 16 || m.some((v) => !Number.isFinite(v) || Math.abs(v) > 1e8),
      )
    )
      throw new Error("器件装配参数无效。");
    const poses = instances.map((matrix) => ({
      matrix: Float32Array.from(matrix),
      normal: normalMatrix(matrix),
    }));
    let data = decodeMesh(mesh);
    let indices = decodeIndices(mesh);
    const gl = this.gl;
    const uintIndices =
      (gl as WebGLRenderingContext & { supportsWebGL2?: boolean }).supportsWebGL2 ||
      String(gl.getParameter(gl.VERSION)).startsWith("WebGL 2") ||
      gl.getExtension("OES_element_index_uint");
    if (indices && !uintIndices) {
      const expanded = new Float32Array(indices.length * 9);
      for (let i = 0; i < indices.length; i++)
        expanded.set(data.subarray(indices[i] * 9, indices[i] * 9 + 9), i * 9);
      data = expanded;
      indices = null;
    }
    const bytes = data.byteLength + (indices?.byteLength ?? 0);
    if (this.bytes + bytes > 64 * 1024 * 1024)
      throw new Error("三维缓存已达到 64 MiB 上限，部分器件未加载。");
    const buffer = gl.createBuffer();
    const indexBuffer = indices ? gl.createBuffer() : null;
    if (!buffer || (indices && !indexBuffer)) {
      gl.deleteBuffer(buffer);
      gl.deleteBuffer(indexBuffer);
      throw new Error("三维显存分配失败。");
    }
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
    if (indices) {
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indexBuffer);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, indices, gl.STATIC_DRAW);
    }
    if (gl.getError() !== gl.NO_ERROR) {
      gl.deleteBuffer(buffer);
      gl.deleteBuffer(indexBuffer);
      throw new Error("三维网格无法上传到显卡。");
    }
    this.parts.push({
      buffer,
      indices: indexBuffer,
      count: indices?.length ?? data.length / 9,
      instances: poses,
      component,
    });
    this.bytes += bytes;
    const min = this.bounds?.min.slice() ?? [Infinity, Infinity, Infinity];
    const max = this.bounds?.max.slice() ?? [-Infinity, -Infinity, -Infinity];
    for (const m of instances)
      for (const x of [mesh.bounds.min[0], mesh.bounds.max[0]])
        for (const y of [mesh.bounds.min[1], mesh.bounds.max[1]])
          for (const z of [mesh.bounds.min[2], mesh.bounds.max[2]]) {
            for (let i = 0; i < 3; i++) {
              const v = m[i] * x + m[4 + i] * y + m[8 + i] * z + m[12 + i];
              min[i] = Math.min(min[i], v);
              max[i] = Math.max(max[i], v);
            }
          }
    this.bounds = { min, max };
  }

  draw(camera: Camera3d, components: boolean, pixelSize?: { width: number; height: number }) {
    const gl = this.gl;
    // Expo GL's JS buffer dimensions stay at their initial value after native resize.
    const width = pixelSize?.width ?? gl.drawingBufferWidth;
    const height = pixelSize?.height ?? gl.drawingBufferHeight;
    gl.viewport(0, 0, width, height);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.useProgram(this.program);
    gl.uniformMatrix4fv(
      this.cameraLocation,
      false,
      projection(camera, width / Math.max(1, height)),
    );
    gl.uniform3fv(this.eyeLocation, Float32Array.from(eyePosition(camera)));
    for (const part of this.parts) {
      if (part.component && !components) continue;
      gl.bindBuffer(gl.ARRAY_BUFFER, part.buffer);
      if (part.indices) gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, part.indices);
      this.attributes.forEach((attribute, i) => {
        gl.enableVertexAttribArray(attribute);
        gl.vertexAttribPointer(attribute, 3, gl.FLOAT, false, 36, i * 12);
      });
      for (const pose of part.instances) {
        gl.uniformMatrix4fv(this.modelLocation, false, pose.matrix);
        gl.uniformMatrix3fv(this.normalLocation, false, pose.normal);
        if (part.indices) gl.drawElements(gl.TRIANGLES, part.count, gl.UNSIGNED_INT, 0);
        else gl.drawArrays(gl.TRIANGLES, 0, part.count);
      }
    }
    gl.flush();
    (gl as WebGLRenderingContext & { endFrameEXP?: () => void }).endFrameEXP?.();
  }

  dispose() {
    for (const part of this.parts) {
      this.gl.deleteBuffer(part.buffer);
      this.gl.deleteBuffer(part.indices);
    }
    this.parts.length = 0;
    this.gl.deleteProgram(this.program);
    this.bytes = 0;
  }
}
