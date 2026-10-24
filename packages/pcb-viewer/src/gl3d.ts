import type { Bounds3, PackedMesh } from "@oshwhub/bridge";
import { type Camera3d, eyePosition, projection } from "./camera3d";
import { decodeIndices, decodeMesh } from "./mesh3d";

export const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
type Part = {
  buffer: WebGLBuffer;
  indices: WebGLBuffer | null;
  instanceBuffer: WebGLBuffer | null;
  count: number;
  instances: Float32Array[];
  component: boolean;
  meshKey?: string;
};
type InstancedApi = {
  drawArraysInstancedANGLE: (
    mode: number,
    first: number,
    count: number,
    instanceCount: number,
  ) => void;
  drawElementsInstancedANGLE: (
    mode: number,
    count: number,
    type: number,
    offset: number,
    instanceCount: number,
  ) => void;
  vertexAttribDivisorANGLE: (index: number, divisor: number) => void;
};
const VERTEX_STRIDE = 20;
const vertexShader = `
precision highp float;
attribute vec3 aPosition;
attribute vec4 aNormal;
attribute vec4 aColor;
attribute vec4 aModel0;
attribute vec4 aModel1;
attribute vec4 aModel2;
attribute vec4 aModel3;
uniform mat4 uCamera;
varying vec3 vNormal;
varying vec3 vColor;
varying vec3 vPosition;
mat3 normalMatrix(mat4 model) {
  mat3 a = mat3(model);
  vec3 x = cross(a[1], a[2]);
  vec3 y = cross(a[2], a[0]);
  vec3 z = cross(a[0], a[1]);
  return mat3(x, y, z) / dot(a[0], x);
}
void main() {
  mat4 model = mat4(aModel0, aModel1, aModel2, aModel3);
  vec4 world = model * vec4(aPosition, 1.0);
  vPosition = world.xyz;
  vNormal = normalize(normalMatrix(model) * aNormal.xyz);
  vColor = aColor.xyz;
  gl_Position = uCamera * world;
}`;
export const fragmentShader = `
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
  float backDiffuse = max(dot(n, -light), 0.0) * 0.35;
  float fill = max(dot(n, normalize(vec3(0.6, 0.5, 0.2))), 0.0) * 0.18;
  vec3 halfway = normalize(light + normalize(uEye - vPosition));
  float specular = pow(max(dot(n, halfway), 0.0), 40.0) * 0.16;
  gl_FragColor = vec4(vColor * (0.38 + diffuse + backDiffuse + fill) + vec3(specular), 1.0);
}`;

function packVertices(data: Float32Array): Uint8Array {
  const result = new Uint8Array((data.length / 9) * VERTEX_STRIDE);
  const positions = new Float32Array(result.buffer);
  const normals = new Int8Array(result.buffer);
  const colors = result;
  for (let i = 0; i < data.length / 9; i++) {
    const source = i * 9;
    const target = i * VERTEX_STRIDE;
    positions.set(data.subarray(source, source + 3), target / 4);
    for (let component = 0; component < 3; component++) {
      normals[target + 12 + component] = Math.max(
        -127,
        Math.min(127, Math.round(data[source + 3 + component] * 127)),
      );
      colors[target + 16 + component] = Math.max(
        0,
        Math.min(255, Math.round(data[source + 6 + component] * 255)),
      );
    }
    colors[target + 19] = 255;
  }
  return result;
}

export class Renderer3d {
  private readonly program: WebGLProgram;
  private readonly parts: Part[] = [];
  private readonly cameraLocation: WebGLUniformLocation;
  private readonly eyeLocation: WebGLUniformLocation;
  private readonly attributes: number[];
  private readonly modelAttributes: number[];
  private instanced: InstancedApi | null | undefined;
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
    this.eyeLocation = location("uEye");
    this.attributes = ["aPosition", "aNormal", "aColor"].map((name) =>
      gl.getAttribLocation(program, name),
    );
    this.modelAttributes = ["aModel0", "aModel1", "aModel2", "aModel3"].map((name) =>
      gl.getAttribLocation(program, name),
    );
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    gl.clearColor(0.045, 0.071, 0.102, 1);
  }

  add(mesh: PackedMesh, instances: number[][], component = false, meshKey?: string) {
    if (
      !instances.length ||
      instances.length > 2000 ||
      instances.some(
        (m) => m.length !== 16 || m.some((v) => !Number.isFinite(v) || Math.abs(v) > 1e8),
      )
    )
      throw new Error("器件装配参数无效。");
    const poses = instances.map((matrix) => {
      const determinant =
        matrix[0] * (matrix[5] * matrix[10] - matrix[6] * matrix[9]) -
        matrix[4] * (matrix[1] * matrix[10] - matrix[2] * matrix[9]) +
        matrix[8] * (matrix[1] * matrix[6] - matrix[2] * matrix[5]);
      if (!Number.isFinite(determinant) || Math.abs(determinant) < 1e-15)
        throw new Error("器件变换无效。");
      return Float32Array.from(matrix);
    });
    const gl = this.gl;
    let instanced = this.instanced;
    if (instanced === undefined) {
      instanced = this.getInstancedApi();
      this.instanced = instanced;
    }
    const existing = meshKey
      ? this.parts.find((part) => part.component === component && part.meshKey === meshKey)
      : undefined;
    if (existing) {
      const instanceBytes = instanced ? instances.length * 16 * 4 : 0;
      if (this.bytes + instanceBytes > 64 * 1024 * 1024)
        throw new Error("三维缓存已达到 64 MiB 上限，部分器件未加载。");
      existing.instances.push(...poses);
      if (existing.instanceBuffer) this.uploadInstances(existing);
      this.bytes += instanceBytes;
      this.includeBounds(mesh.bounds, instances);
      return;
    }
    let data = packVertices(decodeMesh(mesh));
    let indices = decodeIndices(mesh);
    const instanceBytes = instanced ? instances.length * 16 * 4 : 0;
    const uintIndices =
      (gl as WebGLRenderingContext & { supportsWebGL2?: boolean }).supportsWebGL2 ||
      String(gl.getParameter(gl.VERSION)).startsWith("WebGL 2") ||
      gl.getExtension("OES_element_index_uint");
    if (indices && !uintIndices) {
      if (this.bytes + indices.length * VERTEX_STRIDE + instanceBytes > 64 * 1024 * 1024)
        throw new Error("当前设备展开三维网格需要超过 64 MiB，请使用支持索引绘制的设备。");
      const expanded = new Uint8Array(indices.length * VERTEX_STRIDE);
      for (let i = 0; i < indices.length; i++)
        expanded.set(
          data.subarray(indices[i] * VERTEX_STRIDE, indices[i] * VERTEX_STRIDE + VERTEX_STRIDE),
          i * VERTEX_STRIDE,
        );
      data = expanded;
      indices = null;
    }
    const bytes = data.byteLength + (indices?.byteLength ?? 0) + instanceBytes;
    if (this.bytes + bytes > 64 * 1024 * 1024)
      throw new Error("三维缓存已达到 64 MiB 上限，部分器件未加载。");

    const buffer = gl.createBuffer();
    const indexBuffer = indices ? gl.createBuffer() : null;
    const instanceBuffer = instanced ? gl.createBuffer() : null;
    if (!buffer || (indices && !indexBuffer) || (instanced && !instanceBuffer)) {
      gl.deleteBuffer(buffer);
      gl.deleteBuffer(indexBuffer);
      gl.deleteBuffer(instanceBuffer);
      throw new Error("三维显存分配失败。");
    }
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
    if (indices) {
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indexBuffer);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, indices, gl.STATIC_DRAW);
    }
    if (instanceBuffer && instanced) {
      gl.bindBuffer(gl.ARRAY_BUFFER, instanceBuffer);
      gl.bufferData(gl.ARRAY_BUFFER, this.instanceData(poses), gl.STATIC_DRAW);
    }
    if (gl.getError() !== gl.NO_ERROR) {
      gl.deleteBuffer(buffer);
      gl.deleteBuffer(indexBuffer);
      throw new Error("三维网格无法上传到显卡。");
    }
    this.parts.push({
      buffer,
      indices: indexBuffer,
      instanceBuffer,
      count: indices?.length ?? data.byteLength / VERTEX_STRIDE,
      instances: poses,
      component,
      meshKey,
    });
    this.bytes += bytes;
    this.includeBounds(mesh.bounds, instances);
  }

  private getInstancedApi(): InstancedApi | null {
    const value = this.gl.getExtension("ANGLE_instanced_arrays") as Partial<InstancedApi> | null;
    return value &&
      typeof value.drawArraysInstancedANGLE === "function" &&
      typeof value.drawElementsInstancedANGLE === "function" &&
      typeof value.vertexAttribDivisorANGLE === "function"
      ? (value as InstancedApi)
      : null;
  }

  private instanceData(instances: Float32Array[]) {
    const data = new Float32Array(instances.length * 16);
    instances.forEach((matrix, index) => {
      data.set(matrix, index * 16);
    });
    return data;
  }

  private uploadInstances(part: Part) {
    if (!part.instanceBuffer) return;
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, part.instanceBuffer);
    this.gl.bufferData(
      this.gl.ARRAY_BUFFER,
      this.instanceData(part.instances),
      this.gl.STATIC_DRAW,
    );
  }

  private includeBounds(bounds: Bounds3, instances: number[][]) {
    const min = this.bounds?.min.slice() ?? [Infinity, Infinity, Infinity];
    const max = this.bounds?.max.slice() ?? [-Infinity, -Infinity, -Infinity];
    for (const m of instances)
      for (const x of [bounds.min[0], bounds.max[0]])
        for (const y of [bounds.min[1], bounds.max[1]])
          for (const z of [bounds.min[2], bounds.max[2]]) {
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
        gl.vertexAttribPointer(
          attribute,
          i === 0 ? 3 : 4,
          i === 1 ? gl.BYTE : i === 2 ? gl.UNSIGNED_BYTE : gl.FLOAT,
          i > 0,
          VERTEX_STRIDE,
          i === 0 ? 0 : i === 1 ? 12 : i === 2 ? 16 : 0,
        );
      });
      if (part.instanceBuffer && this.instanced) {
        gl.bindBuffer(gl.ARRAY_BUFFER, part.instanceBuffer);
        this.modelAttributes.forEach((attribute, i) => {
          gl.enableVertexAttribArray(attribute);
          gl.vertexAttribPointer(attribute, 4, gl.FLOAT, false, 64, i * 16);
          this.instanced?.vertexAttribDivisorANGLE(attribute, 1);
        });
        if (part.indices)
          this.instanced.drawElementsInstancedANGLE(
            gl.TRIANGLES,
            part.count,
            gl.UNSIGNED_INT,
            0,
            part.instances.length,
          );
        else
          this.instanced.drawArraysInstancedANGLE(
            gl.TRIANGLES,
            0,
            part.count,
            part.instances.length,
          );
      } else {
        this.modelAttributes.forEach((attribute) => {
          gl.disableVertexAttribArray(attribute);
        });
        for (const matrix of part.instances) {
          this.modelAttributes.forEach((attribute, i) => {
            gl.vertexAttrib4fv(attribute, matrix.subarray(i * 4, i * 4 + 4));
          });
          if (part.indices) gl.drawElements(gl.TRIANGLES, part.count, gl.UNSIGNED_INT, 0);
          else gl.drawArrays(gl.TRIANGLES, 0, part.count);
        }
      }
    }
    gl.flush();
    (gl as WebGLRenderingContext & { endFrameEXP?: () => void }).endFrameEXP?.();
  }

  dispose() {
    for (const part of this.parts) {
      this.gl.deleteBuffer(part.buffer);
      this.gl.deleteBuffer(part.indices);
      this.gl.deleteBuffer(part.instanceBuffer);
    }
    this.parts.length = 0;
    this.gl.deleteProgram(this.program);
    this.bytes = 0;
  }
}
