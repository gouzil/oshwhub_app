#![cfg(not(target_arch = "wasm32"))]

#[allow(dead_code)]
#[path = "../src/viewer/camera3d.rs"]
mod camera3d;
#[path = "../src/viewer/mesh.rs"]
mod mesh;
#[path = "../src/viewer/render3d.rs"]
mod render3d;

use std::{
    sync::{Arc, OnceLock, mpsc},
    time::Duration,
};

use camera3d::{Camera3d, orbit_camera};
use oshwhub_core::pcb::{Bounds3, Instance, Mesh, RawModel3d, RawScene3d};
use pollster::block_on;
use render3d::Render3d;

const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

struct Headless {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn gpu() -> &'static Headless {
    static GPU: OnceLock<Headless> = OnceLock::new();
    GPU.get_or_init(|| {
        block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::PRIMARY,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    ..Default::default()
                })
                .await
                .expect("a headless GPU adapter is required for the renderer tests");
            eprintln!("Headless renderer GPU: {:?}", adapter.get_info());
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("3D renderer readback tests"),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    ..Default::default()
                })
                .await
                .unwrap();
            Headless { device, queue }
        })
    })
}

fn triangle(z: f32, color: [f32; 3], normal: [f32; 3]) -> Arc<Mesh> {
    let vertices = [[-1., -1., z], [1., -1., z], [0., 1., z]]
        .into_iter()
        .flat_map(|position| position.into_iter().chain(normal).chain(color))
        .collect();
    Arc::new(Mesh {
        vertices,
        indices: vec![0, 1, 2],
        bounds: Bounds3 {
            min: [-1., -1., f64::from(z)],
            max: [1., 1., f64::from(z)],
        },
    })
}

fn board(mesh: Arc<Mesh>) -> RawScene3d {
    RawScene3d {
        board: mesh,
        model_keys: Vec::new(),
        component_count: 0,
        bound_count: 0,
        thickness: 1.6,
        warnings: Vec::new(),
    }
}

fn model(key: &str, mesh_key: &str, mesh: Arc<Mesh>, matrices: Vec<[f32; 16]>) -> RawModel3d {
    RawModel3d {
        key: key.into(),
        mesh_key: mesh_key.into(),
        mesh,
        instances: matrices
            .into_iter()
            .enumerate()
            .map(|(i, matrix)| Instance {
                title: i.to_string(),
                matrix,
            })
            .collect(),
    }
}

fn camera() -> Camera3d {
    Camera3d {
        yaw: 0.,
        pitch: std::f64::consts::FRAC_PI_2,
        distance: 5.,
        radius: 2.,
        target: [0.; 3],
    }
}

fn readback(renderer: &Render3d, format: wgpu::TextureFormat) -> Vec<u8> {
    let gpu = gpu();
    let size = wgpu::Extent3d {
        width: 64,
        height: 64,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("3D readback target"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let output = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("3D readback bytes"),
        size: 64 * 256,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    renderer
        .encode(&mut encoder, &target.create_view(&Default::default()))
        .unwrap();
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &output,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(64),
            },
        },
        size,
    );
    let submission = gpu.queue.submit([encoder.finish()]);
    let (send, receive) = mpsc::channel();
    output
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            send.send(result).unwrap();
        });
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(10)),
        })
        .unwrap();
    receive
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .unwrap();
    let bytes = output.slice(..).get_mapped_range().to_vec();
    output.unmap();
    bytes
}

fn pixel_near(pixels: &[u8], x: usize, y: usize, expected: [u8; 3], tolerance: u8) {
    let pixel = &pixels[(y * 64 + x) * 4..][..4];
    for (actual, expected) in pixel[..3].iter().zip(expected) {
        assert!(
            actual.abs_diff(expected) <= tolerance,
            "pixel ({x}, {y}) = {pixel:?}, expected {expected}"
        );
    }
    assert_eq!(pixel[3], 255);
}

#[test]
fn headless_mesh_sharing_updates_instances_and_keeps_camera_writes_small() {
    let gpu = gpu();
    block_on(async {
        let mut renderer = Render3d::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm)
            .await
            .unwrap();
        let scene = board(triangle(0., [0.5, 0.3, 0.2], [0., 0., 1.]));
        let weak_cpu_mesh = Arc::downgrade(&scene.board);
        renderer.set_board(&gpu.device, &scene).await.unwrap();
        let initial = renderer.stats();
        assert_eq!(initial.mesh_uploads, 1);
        assert_eq!(initial.instance_uploads, 1);
        assert_eq!(initial.uploaded_bytes, 3 * 20 + 3 * 4 + 64);
        assert_eq!(initial.resident_bytes, 80 + 3 * 20 + 3 * 4 + 64);
        renderer.set_board(&gpu.device, &scene).await.unwrap();
        assert_eq!(renderer.stats(), initial);
        let first = model("a", "shared", Arc::clone(&scene.board), vec![IDENTITY]);
        let mut second = model("b", "shared", Arc::clone(&scene.board), vec![IDENTITY]);
        renderer.add_model(&gpu.device, &first).await.unwrap();
        renderer.add_model(&gpu.device, &second).await.unwrap();
        let shared = renderer.stats();
        assert_eq!(shared.mesh_uploads, 2);
        assert_eq!(shared.instance_uploads, 3);
        assert_eq!(shared.resident_bytes, initial.resident_bytes + 72 + 2 * 64);
        renderer.add_model(&gpu.device, &second).await.unwrap();
        assert_eq!(renderer.stats(), shared);
        second.instances[0].matrix[12] = 2.5;
        renderer.add_model(&gpu.device, &second).await.unwrap();
        assert_eq!(renderer.stats().mesh_uploads, 2);
        assert_eq!(renderer.stats().instance_uploads, 4);
        assert_eq!(renderer.stats().resident_bytes, shared.resident_bytes);
        assert_eq!(renderer.bounds().unwrap().max[0], 3.5);
        renderer.write_camera(&gpu.queue, &camera(), 1.).unwrap();
        renderer
            .write_camera(&gpu.queue, &orbit_camera(camera(), 10., -20.), 1.)
            .unwrap();
        assert_eq!(renderer.stats().mesh_uploads, 2);
        assert_eq!(renderer.stats().instance_uploads, 4);
        assert_eq!(renderer.stats().camera_writes, 2);
        assert_eq!(
            renderer.stats().uploaded_bytes,
            shared.uploaded_bytes + 64 + 2 * 80
        );
        drop(first);
        drop(second);
        drop(scene);
        assert!(
            weak_cpu_mesh.upgrade().is_none(),
            "GPU ownership must not retain raw CPU mesh vectors"
        );
        renderer
            .set_board(
                &gpu.device,
                &board(triangle(0., [1., 0., 0.], [0., 0., 1.])),
            )
            .await
            .unwrap();
        assert_eq!(renderer.stats().resident_bytes, initial.resident_bytes);
        assert_eq!(renderer.stats().mesh_uploads, 3);
    });
}

#[test]
fn headless_depth_lighting_mirrored_instances_and_srgb_match_the_gl_image() {
    let gpu = gpu();
    block_on(async {
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ] {
            let mut renderer = Render3d::new(&gpu.device, format).await.unwrap();
            renderer
                .set_board(
                    &gpu.device,
                    &board(triangle(1., [0.5, 0., 0.], [0., 0., 1.])),
                )
                .await
                .unwrap();
            assert!(renderer.resize_depth(&gpu.device, 64, 64).await.unwrap());
            assert!(!renderer.resize_depth(&gpu.device, 64, 64).await.unwrap());
            renderer
                .add_model(
                    &gpu.device,
                    &model(
                        "back",
                        "back",
                        triangle(0., [0., 0.5, 0.], [0., 0., 1.]),
                        vec![IDENTITY],
                    ),
                )
                .await
                .unwrap();
            renderer.write_camera(&gpu.queue, &camera(), 1.).unwrap();
            let pixels = readback(&renderer, format);
            pixel_near(&pixels, 0, 0, [11, 18, 26], 1);
            // Independent GL light calculation for +Z, quantized red 128/255.
            pixel_near(&pixels, 32, 32, [132, 8, 8], 3);
            let mut bottom = IDENTITY;
            bottom[0] = 2.;
            bottom[5] = -0.5;
            bottom[10] = -1.;
            bottom[14] = 2.;
            renderer
                .add_model(
                    &gpu.device,
                    &model(
                        "front",
                        "front",
                        triangle(
                            0.,
                            [0., 0., 0.5],
                            [
                                std::f32::consts::FRAC_1_SQRT_2,
                                0.,
                                std::f32::consts::FRAC_1_SQRT_2,
                            ],
                        ),
                        vec![bottom],
                    ),
                )
                .await
                .unwrap();
            let pixels = readback(&renderer, format);
            // Nonuniform inverse-transpose plus bottom Y/Z mirroring yields
            // the visible normal [-1/sqrt(5), 0, 2/sqrt(5)].
            pixel_near(&pixels, 32, 32, [3, 3, 127], 3);
            let below = Camera3d {
                pitch: -std::f64::consts::FRAC_PI_2,
                ..camera()
            };
            renderer.write_camera(&gpu.queue, &below, 1.).unwrap();
            pixel_near(&readback(&renderer, format), 32, 32, [0, 86, 0], 3);
            let uploads = renderer.stats().mesh_uploads;
            renderer.set_components_visible(false);
            pixel_near(&readback(&renderer, format), 32, 32, [86, 0, 0], 3);
            assert_eq!(renderer.stats().mesh_uploads, uploads);
        }
    });
}

#[test]
fn headless_draws_both_instances_using_the_index_buffer_winding() {
    let gpu = gpu();
    block_on(async {
        let mut renderer = Render3d::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm)
            .await
            .unwrap();
        renderer
            .set_board(
                &gpu.device,
                &board(triangle(0., [0.5, 0., 0.], [0., 0., 1.])),
            )
            .await
            .unwrap();
        renderer.resize_depth(&gpu.device, 64, 64).await.unwrap();
        let mut mesh = triangle(0., [0., 0.5, 0.], [0., 0., -1.]);
        Arc::get_mut(&mut mesh).unwrap().indices.reverse();
        let matrices = [-0.6, 0.6].map(|x| {
            let mut matrix = IDENTITY;
            matrix[0] = 0.3;
            matrix[5] = 0.3;
            matrix[12] = x;
            matrix[14] = 1.;
            matrix
        });
        renderer
            .add_model(
                &gpu.device,
                &model("two", "indexed", mesh, matrices.to_vec()),
            )
            .await
            .unwrap();
        let top = Camera3d {
            yaw: -std::f64::consts::FRAC_PI_2,
            ..camera()
        };
        renderer.write_camera(&gpu.queue, &top, 1.).unwrap();
        let pixels = readback(&renderer, wgpu::TextureFormat::Rgba8Unorm);
        for x in [20, 44] {
            let pixel = &pixels[(32 * 64 + x) * 4..][..4];
            assert!(
                pixel[1] > 100 && pixel[0] < 30 && pixel[2] < 30,
                "instance at x={x}: {pixel:?}"
            );
        }
        assert_eq!(renderer.stats().mesh_uploads, 2);
        assert_eq!(renderer.stats().instance_uploads, 2);
    });
}

#[test]
fn headless_invalid_updates_preserve_the_successful_board_and_depth() {
    let gpu = gpu();
    block_on(async {
        assert!(
            Render3d::new(&gpu.device, wgpu::TextureFormat::Depth32Float)
                .await
                .is_err()
        );
        assert!(
            Render3d::new(&gpu.device, wgpu::TextureFormat::Rgba8Uint)
                .await
                .is_err()
        );
        let mut renderer = Render3d::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm)
            .await
            .unwrap();
        renderer
            .set_board(
                &gpu.device,
                &board(triangle(1., [0.5, 0., 0.], [0., 0., 1.])),
            )
            .await
            .unwrap();
        renderer.resize_depth(&gpu.device, 64, 64).await.unwrap();
        renderer.write_camera(&gpu.queue, &camera(), 1.).unwrap();
        let successful = renderer.stats();
        let mut singular = IDENTITY;
        singular[0] = 0.;
        assert!(
            renderer
                .add_model(
                    &gpu.device,
                    &model(
                        "bad",
                        "bad",
                        triangle(0., [0.; 3], [0., 0., 1.]),
                        vec![singular]
                    )
                )
                .await
                .is_err()
        );
        assert!(
            renderer
                .resize_depth(
                    &gpu.device,
                    gpu.device.limits().max_texture_dimension_2d + 1,
                    64
                )
                .await
                .is_err()
        );
        let invalid = Mesh {
            vertices: vec![f32::NAN; 27],
            indices: vec![0, 1, 2],
            bounds: Bounds3 {
                min: [0.; 3],
                max: [1.; 3],
            },
        };
        assert!(
            renderer
                .set_board(&gpu.device, &board(Arc::new(invalid)))
                .await
                .is_err()
        );
        let invalid = Mesh {
            vertices: vec![0.; 27],
            indices: vec![0, 1, 3],
            bounds: Bounds3 {
                min: [0.; 3],
                max: [1.; 3],
            },
        };
        assert!(
            renderer
                .add_model(
                    &gpu.device,
                    &model("bad-index", "bad-index", Arc::new(invalid), vec![IDENTITY])
                )
                .await
                .is_err()
        );
        assert!(
            renderer
                .write_camera(
                    &gpu.queue,
                    &Camera3d {
                        distance: f64::NAN,
                        ..camera()
                    },
                    1.
                )
                .is_err()
        );
        assert_eq!(renderer.stats(), successful);
        pixel_near(
            &readback(&renderer, wgpu::TextureFormat::Rgba8Unorm),
            32,
            32,
            [132, 8, 8],
            3,
        );
    });
}
