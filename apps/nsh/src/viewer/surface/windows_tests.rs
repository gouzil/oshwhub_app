use super::*;
use crate::viewer::camera3d::fit_camera;
use oshwhub_core::pcb::{Bounds3, Instance, Mesh, RawModel3d, RawScene3d};
use std::sync::Arc;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;

fn test_device() -> ID3D11Device {
    unsafe {
        let mut device = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            Default::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
        .or_else(|_| {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )
        })
        .unwrap();
        let device = device.unwrap();
        let desc = device
            .cast::<IDXGIDevice>()
            .unwrap()
            .GetAdapter()
            .unwrap()
            .GetDesc()
            .unwrap();
        let end = desc
            .Description
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(desc.Description.len());
        eprintln!(
            "D3D11 adapter: {}; feature level: 0x{:x}",
            String::from_utf16_lossy(&desc.Description[..end]),
            device.GetFeatureLevel().0
        );
        device
    }
}

#[test]
#[ignore = "requires a Windows D3D11/D2D adapter"]
fn d3d11_gpu_pixels_preserve_clipping_overlay_resize_and_disposal() {
    let _ = env_logger::try_init();
    unsafe {
        let device = test_device();
        let immediate = device.GetImmediateContext().unwrap();
        let dxgi: IDXGIDevice = device.cast().unwrap();
        let d2d = D2D1CreateDevice(&dxgi, None).unwrap();
        let dc = d2d
            .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
            .unwrap();
        let board = Arc::new(RawScene3d {
            board: Arc::new(Mesh {
                vertices: [[-1., -1., 0.], [1., -1., 0.], [0., 1., 0.]]
                    .into_iter()
                    .flat_map(|p| p.into_iter().chain([0., 0., 1.]).chain([0.8, 0., 0.]))
                    .collect(),
                indices: vec![0, 1, 2],
                bounds: Bounds3 {
                    min: [-1., -1., 0.],
                    max: [1., 1., 0.],
                },
            }),
            model_keys: vec![],
            component_count: 0,
            bound_count: 0,
            thickness: 1.6,
            warnings: vec![],
        });
        for cycle in 0..20 {
            let model = SurfaceModel::new(fit_camera(&board.board.bounds, 1.));
            let handle = model.handle();
            model.set_board(Arc::clone(&board));
            if cycle == 0 {
                let mut vertices = board.board.vertices.clone();
                for vertex in vertices.as_chunks_mut::<9>().0 {
                    vertex[6..9].copy_from_slice(&[0., 0.8, 0.]);
                }
                let geometry = Arc::new(Mesh {
                    vertices,
                    indices: board.board.indices.clone(),
                    bounds: board.board.bounds.clone(),
                });
                for (key, x) in [("left", -0.4), ("right", 0.4)] {
                    let mut matrix = crate::viewer::render3d::IDENTITY;
                    matrix[0] = 0.2;
                    matrix[5] = 0.2;
                    matrix[12] = x;
                    matrix[14] = 0.1;
                    model.add_model(Arc::new(RawModel3d {
                        key: key.into(),
                        mesh_key: "shared-green".into(),
                        mesh: Arc::clone(&geometry),
                        instances: (0..if key == "left" { 2 } else { 1 })
                            .map(|row| {
                                let mut matrix = matrix;
                                matrix[13] = row as f32 * 0.6;
                                Instance {
                                    title: format!("{key}-{row}"),
                                    matrix,
                                }
                            })
                            .collect(),
                    }));
                }
            }
            let mut renderer = Renderer::new(&dc).unwrap();
            for (width, height) in [(320, 240), (480, 300), (320, 240)] {
                let desc = D3D11_TEXTURE2D_DESC {
                    Width: width,
                    Height: height,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
                    ..Default::default()
                };
                let mut target = None;
                device
                    .CreateTexture2D(&desc, None, Some(&mut target))
                    .unwrap();
                let target = target.unwrap();
                let bitmap = dc
                    .CreateBitmapFromDxgiSurface(
                        &target.cast::<IDXGISurface>().unwrap(),
                        Some(&D2D1_BITMAP_PROPERTIES1 {
                            pixelFormat: D2D1_PIXEL_FORMAT {
                                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                                alphaMode: D2D1_ALPHA_MODE_IGNORE,
                            },
                            dpiX: 96.,
                            dpiY: 96.,
                            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                            colorContext: Default::default(),
                        }),
                    )
                    .unwrap();
                dc.SetTarget(&bitmap);
                dc.BeginDraw();
                dc.Clear(Some(&D2D1_COLOR_F {
                    r: 0.,
                    g: 0.,
                    b: 1.,
                    a: 1.,
                }));
                dc.PushAxisAlignedClip(
                    &D2D_RECT_F {
                        left: 8.,
                        top: 8.,
                        right: width as f32 - 8.,
                        bottom: height as f32 - 8.,
                    },
                    D2D1_ANTIALIAS_MODE_ALIASED,
                );
                renderer
                    .draw(&dc, (width as f32, height as f32), Some(&model))
                    .unwrap();
                if cycle == 0 {
                    // Reuse one texture without a CPU wait; include unchanged-tick NSH redraws.
                    let before = renderer.scene.as_ref().unwrap().stats;
                    assert_eq!(
                        before.mesh_uploads, 2,
                        "two models share one geometry upload"
                    );
                    assert_eq!(before.instance_uploads, 3);
                    let mut invalid = crate::viewer::render3d::IDENTITY;
                    invalid[0] = f32::NAN;
                    let invalid = Arc::new(RawModel3d {
                        key: "left".into(),
                        mesh_key: "shared-green".into(),
                        mesh: Arc::clone(&board.board),
                        instances: vec![Instance {
                            title: "invalid".into(),
                            matrix: invalid,
                        }],
                    });
                    let scene = renderer.scene.as_mut().unwrap();
                    assert!(
                        scene
                            .add_model(&device, &invalid, u64::from(width) * u64::from(height) * 8)
                            .is_err()
                    );
                    assert_eq!(
                        scene.stats, before,
                        "a rejected replacement keeps the old GPU resources"
                    );
                    for frame in 0..64 {
                        let mut camera = model.frame().camera;
                        camera.yaw += if frame % 2 == 0 { 0.01 } else { -0.01 };
                        model.set_camera(camera);
                        renderer
                            .draw(&dc, (width as f32, height as f32), Some(&model))
                            .unwrap();
                        renderer
                            .draw(&dc, (width as f32, height as f32), Some(&model))
                            .unwrap();
                    }
                    let after = renderer.scene.as_ref().unwrap().stats;
                    assert_eq!(after.mesh_uploads, before.mesh_uploads);
                    assert_eq!(after.instance_uploads, before.instance_uploads);
                    assert_eq!(after.camera_writes, before.camera_writes + 64);
                    assert_eq!(
                        after.uploaded_bytes,
                        before.uploaded_bytes + 64 * CAMERA_BYTES
                    );
                    model.set_active(false);
                    assert_eq!(
                        renderer
                            .draw(&dc, (width as f32, height as f32), Some(&model))
                            .unwrap(),
                        None
                    );
                    assert_eq!(renderer.scene.as_ref().unwrap().stats, after);
                    model.set_active(true);
                }
                dc.PopAxisAlignedClip();
                let green = dc
                    .CreateSolidColorBrush(
                        &D2D1_COLOR_F {
                            r: 0.,
                            g: 1.,
                            b: 0.,
                            a: 1.,
                        },
                        None,
                    )
                    .unwrap();
                dc.FillRectangle(
                    &D2D_RECT_F {
                        left: 12.,
                        top: 12.,
                        right: 40.,
                        bottom: 40.,
                    },
                    &green,
                );
                dc.EndDraw(None, None).unwrap();
                let mut staging = None;
                device
                    .CreateTexture2D(
                        &D3D11_TEXTURE2D_DESC {
                            Usage: D3D11_USAGE_STAGING,
                            BindFlags: 0,
                            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                            ..desc
                        },
                        None,
                        Some(&mut staging),
                    )
                    .unwrap();
                let staging = staging.unwrap();
                immediate.CopyResource(&staging, &target);
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                immediate
                    .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                    .unwrap();
                let pixel = |x: u32, y: u32| {
                    let at = mapped
                        .pData
                        .cast::<u8>()
                        .add((y * mapped.RowPitch + x * 4) as usize);
                    [*at.add(2), *at.add(1), *at]
                };
                let mut red = 0;
                let mut green_models = 0;
                for y in 8..height - 8 {
                    for x in 8..width - 8 {
                        let [r, g, b] = pixel(x, y);
                        if r > 70 && g < 40 && b < 40 {
                            red += 1;
                        }
                        if y > 40 && g > 70 && r < 40 && b < 40 {
                            green_models += 1;
                        }
                    }
                }
                assert!(red > 100, "GPU triangle absent in cycle {cycle}");
                if cycle == 0 {
                    assert!(green_models > 100, "GPU models absent");
                    let camera = model.frame().camera;
                    let matrix = crate::viewer::camera3d::projection(
                        &camera,
                        f64::from(width) / f64::from(height),
                    );
                    for world in [
                        [-0.4, -0.0667, 0.1, 1.],
                        [-0.4, 0.5333, 0.1, 1.],
                        [0.4, -0.0667, 0.1, 1.],
                    ] {
                        let clip: [f32; 4] = std::array::from_fn(|row| {
                            (0..4).map(|col| matrix[col * 4 + row] * world[col]).sum()
                        });
                        let x = ((clip[0] / clip[3] * 0.5 + 0.5) * width as f32) as u32;
                        let y = ((0.5 - clip[1] / clip[3] * 0.5) * height as f32) as u32;
                        let [r, g, b] = pixel(x, y);
                        assert!(
                            g > 70 && r < 40 && b < 40,
                            "instance missing at {world:?}: {x},{y} = {r},{g},{b}"
                        );
                    }
                }
                assert_eq!(pixel(0, 0), [0, 0, 255], "NSH clip must remain in force");
                assert_eq!(
                    pixel(20, 20),
                    [0, 255, 0],
                    "NSH content must cover the GPU bitmap"
                );
                immediate.Unmap(&staging, 0);
            }
            drop(renderer);
            drop(model);
            assert!(SurfaceModel::resolve(handle).is_none());
        }
    }
}

#[test]
#[ignore = "requires a Windows D3D11/D2D adapter"]
fn d3d11_gpu_upload_race_and_smaller_replacement_with_larger_target() {
    unsafe {
        let device = test_device();
        let dxgi: IDXGIDevice = device.cast().unwrap();
        let d2d = D2D1CreateDevice(&dxgi, None).unwrap();
        let dc = d2d
            .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
            .unwrap();
        let bitmap = dc
            .CreateBitmap(
                D2D_SIZE_U {
                    width: 64,
                    height: 64,
                },
                None,
                0,
                &D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_IGNORE,
                    },
                    dpiX: 96.,
                    dpiY: 96.,
                    bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                    colorContext: Default::default(),
                },
            )
            .unwrap();
        dc.SetTarget(&bitmap);
        let make_board = |repeat| {
            Arc::new(RawScene3d {
                board: Arc::new(Mesh {
                    // Many unused vertices exercise real allocation without overdraw from a million triangles.
                    vertices: [
                        -1., -1., 0., 0., 0., 1., 0.8, 0., 0., 1., -1., 0., 0., 0., 1., 0.8, 0.,
                        0., 0., 1., 0., 0., 0., 1., 0.8, 0., 0.,
                    ]
                    .repeat(repeat),
                    indices: vec![0, 1, 2],
                    bounds: Bounds3 {
                        min: [-1., -1., 0.],
                        max: [1., 1., 0.],
                    },
                }),
                model_keys: vec![],
                component_count: 0,
                bound_count: 0,
                thickness: 1.6,
                warnings: vec![],
            })
        };
        let dense = make_board(1_200_000);
        let model = SurfaceModel::new(fit_camera(&dense.board.bounds, 1.));
        model.set_board(Arc::clone(&dense));
        let watch = Arc::clone(&dense);
        let baseline_refs = Arc::strong_count(&dense);
        let updating = Arc::clone(&model);
        let update = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while Arc::strong_count(&watch) <= baseline_refs {
                assert!(
                    std::time::Instant::now() < deadline,
                    "upload did not acquire resources"
                );
                std::thread::yield_now();
            }
            let mut camera = updating.frame().camera;
            camera.yaw += 0.25;
            updating.set_camera(camera);
        });
        let mut renderer = Renderer::new(&dc).unwrap();
        dc.BeginDraw();
        renderer.draw(&dc, (64., 64.), Some(&model)).unwrap();
        update.join().unwrap();
        let cached = renderer.last_model;
        let before = renderer.scene.as_ref().unwrap().stats;
        assert!(
            cached != Some((model.handle(), model.tick())),
            "the older camera frame must not be cached as the update made during upload"
        );
        renderer.draw(&dc, (64., 64.), Some(&model)).unwrap();
        assert_eq!(renderer.last_model, Some((model.handle(), model.tick())));
        assert_eq!(
            renderer.scene.as_ref().unwrap().stats.camera_writes,
            before.camera_writes + 1
        );
        dc.EndDraw(None, None).unwrap();
        let small = make_board(1);
        model.set_board(Arc::clone(&small));
        dc.BeginDraw();
        let result = renderer.draw(&dc, (8192., 1024.), Some(&model));
        dc.EndDraw(None, None).unwrap();
        assert!(
            result.is_ok(),
            "a valid small replacement must not count the outgoing mesh budget: {result:?}"
        );
        assert!(
            renderer
                .scene
                .as_ref()
                .unwrap()
                .board_source
                .ptr_eq(&Arc::downgrade(&small))
        );
    }
}
