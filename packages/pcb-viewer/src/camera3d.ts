import type { Bounds3 } from "@oshwhub/bridge";

export type Camera3d = {
  yaw: number;
  pitch: number;
  distance: number;
  radius: number;
  target: number[];
};
const halfFov = Math.PI / 8;
const clamp = (v: number, min: number, max: number) => Math.max(min, Math.min(max, v));

export function fitCamera(bounds: Bounds3, aspect: number): Camera3d {
  const target = bounds.min.map((v, i) => (v + bounds.max[i]) / 2);
  const radius = Math.max(0.1, Math.hypot(...bounds.max.map((v, i) => v - bounds.min[i])) / 2);
  const angle = Math.atan(Math.tan(halfFov) * Math.min(1, Math.max(0.05, aspect)));
  return {
    target,
    radius,
    yaw: -Math.PI / 2,
    pitch: Math.PI / 3,
    distance: (radius / Math.sin(angle)) * 1.12,
  };
}

export function orbitCamera(camera: Camera3d, dx: number, dy: number): Camera3d {
  return {
    ...camera,
    yaw: (camera.yaw - dx * 0.008) % (2 * Math.PI),
    pitch: clamp(camera.pitch + dy * 0.008, -Math.PI / 2, Math.PI / 2),
  };
}

export function zoomCamera(camera: Camera3d, factor: number): Camera3d {
  return {
    ...camera,
    distance: clamp(camera.distance / factor, camera.radius * 0.12, camera.radius * 200),
  };
}

function axes(camera: Camera3d) {
  const c = Math.cos(camera.yaw),
    s = Math.sin(camera.yaw);
  const cp = Math.cos(camera.pitch),
    sp = Math.sin(camera.pitch);
  return { right: [-s, c, 0], up: [-c * sp, -s * sp, cp], outward: [c * cp, s * cp, sp] };
}

export function eyePosition(camera: Camera3d) {
  return axes(camera).outward.map((v, i) => camera.target[i] + v * camera.distance);
}

export function panCamera(camera: Camera3d, dx: number, dy: number, height: number): Camera3d {
  const { right, up } = axes(camera);
  const scale = (2 * camera.distance * Math.tan(halfFov)) / Math.max(height, 1);
  return {
    ...camera,
    target: camera.target.map((v, i) => v - (dx * right[i] - dy * up[i]) * scale),
  };
}

export function projection(camera: Camera3d, aspect: number): Float32Array {
  const { right, up, outward } = axes(camera);
  const eye = eyePosition(camera);
  const dot = (a: number[], b: number[]) => a.reduce((sum, v, i) => sum + v * b[i], 0);
  const view = [
    right[0],
    up[0],
    outward[0],
    0,
    right[1],
    up[1],
    outward[1],
    0,
    right[2],
    up[2],
    outward[2],
    0,
    -dot(right, eye),
    -dot(up, eye),
    -dot(outward, eye),
    1,
  ];
  // Keep close PCB surface planes distinct on native 16-bit depth buffers.
  const near = Math.max(0.001, camera.distance * 0.05, camera.distance - camera.radius * 1.2);
  const far = Math.max(camera.distance + camera.radius * 4, near + 1);
  const f = 1 / Math.tan(halfFov);
  const p = [
    f / Math.max(0.05, aspect),
    0,
    0,
    0,
    0,
    f,
    0,
    0,
    0,
    0,
    (far + near) / (near - far),
    -1,
    0,
    0,
    (2 * far * near) / (near - far),
    0,
  ];
  return Float32Array.from({ length: 16 }, (_, i) => {
    const row = i % 4,
      col = Math.floor(i / 4);
    return [0, 1, 2, 3].reduce((sum, k) => sum + p[k * 4 + row] * view[col * 4 + k], 0);
  });
}
