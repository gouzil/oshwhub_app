use oshwhub_core::pcb::Bounds3;

const HALF_FOV: f64 = std::f64::consts::FRAC_PI_8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera3d {
    pub yaw: f64,
    pub pitch: f64,
    pub distance: f64,
    pub radius: f64,
    pub target: [f64; 3],
}

pub fn fit_camera(bounds: &Bounds3, aspect: f64) -> Camera3d {
    let target = std::array::from_fn(|i| (bounds.min[i] + bounds.max[i]) / 2.);
    let radius = ((bounds.max[0] - bounds.min[0])
        .hypot(bounds.max[1] - bounds.min[1])
        .hypot(bounds.max[2] - bounds.min[2])
        / 2.)
        .max(0.1);
    let angle = (HALF_FOV.tan() * aspect.clamp(0.05, 1.)).atan();
    Camera3d {
        target,
        radius,
        yaw: -std::f64::consts::FRAC_PI_2,
        pitch: std::f64::consts::FRAC_PI_3,
        distance: radius / angle.sin() * 1.12,
    }
}

pub fn orbit_camera(camera: Camera3d, dx: f64, dy: f64) -> Camera3d {
    Camera3d {
        yaw: camera.yaw + dx * 0.008,
        pitch: camera.pitch + dy * 0.008,
        ..camera
    }
}

pub fn zoom_camera(camera: Camera3d, factor: f64) -> Camera3d {
    Camera3d {
        distance: (camera.distance / factor).clamp(camera.radius * 0.12, camera.radius * 200.),
        ..camera
    }
}

fn axes(camera: &Camera3d) -> [[f64; 3]; 3] {
    let (s, c) = camera.yaw.sin_cos();
    let (sp, cp) = camera.pitch.sin_cos();
    [[-s, c, 0.], [-c * sp, -s * sp, cp], [c * cp, s * cp, sp]]
}

pub fn eye_position(camera: &Camera3d) -> [f64; 3] {
    let [_, _, outward] = axes(camera);
    std::array::from_fn(|i| camera.target[i] + outward[i] * camera.distance)
}

pub fn pan_camera(camera: Camera3d, dx: f64, dy: f64, height: f64) -> Camera3d {
    let [right, up, _] = axes(&camera);
    let scale = 2. * camera.distance * HALF_FOV.tan() / height.max(1.);
    Camera3d {
        target: std::array::from_fn(|i| camera.target[i] - (dx * right[i] - dy * up[i]) * scale),
        ..camera
    }
}

/// Column-major world-to-clip matrix with wgpu's 0..1 depth range.
pub fn projection(camera: &Camera3d, aspect: f64) -> [f32; 16] {
    let [right, up, outward] = axes(camera);
    let eye = eye_position(camera);
    let dot = |axis: [f64; 3]| axis.into_iter().zip(eye).map(|(a, b)| a * b).sum::<f64>();
    let view = [
        right[0],
        up[0],
        outward[0],
        0.,
        right[1],
        up[1],
        outward[1],
        0.,
        right[2],
        up[2],
        outward[2],
        0.,
        -dot(right),
        -dot(up),
        -dot(outward),
        1.,
    ];
    let near = 0.001_f64
        .max(camera.distance * 0.05)
        .max(camera.distance - camera.radius * 1.2);
    let far = (camera.distance + camera.radius * 4.).max(near + 1.);
    let f = 1. / HALF_FOV.tan();
    // WebGPU clip z = (OpenGL clip z + clip w) / 2, so near/far map to 0/1.
    let perspective = [
        f / aspect.max(0.05),
        0.,
        0.,
        0.,
        0.,
        f,
        0.,
        0.,
        0.,
        0.,
        far / (near - far),
        -1.,
        0.,
        0.,
        far * near / (near - far),
        0.,
    ];
    std::array::from_fn(|i| {
        let row = i % 4;
        let col = i / 4;
        (0..4)
            .map(|k| perspective[k * 4 + row] * view[col * 4 + k])
            .sum::<f64>() as f32
    })
}
