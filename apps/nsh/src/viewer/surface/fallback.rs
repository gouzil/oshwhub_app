//! Canvas fallback for native hosts without a registered GPU component.
//!
//! Linux, Windows and Android expose NSH's retained canvas through their
//! native renderers.  Keeping this path in Rust makes the viewer usable on
//! every NSH target while the platform-specific wgpu hosts are brought up.
// shortcut: non-GPU hosts cap the retained canvas at 100,000 triangles; replace this path with a native wgpu surface after same-device GPU captures pass.

use std::{cmp::Ordering, sync::Arc};

use nsh_core::{
    element::{AnyElement, Painter},
    prelude::*,
};
use oshwhub_core::pcb::Mesh;

use super::{SurfaceModel, SurfaceResources, SurfaceStatus};
use crate::viewer::camera3d::{eye_position, projection};

// A Canvas drawing is retained by each native host.  Keep pathological model
// data from turning one frame into millions of platform draw operations.
const MAX_TRIANGLES: usize = 100_000;
const CAPACITY_ERROR: &str = "三维场景超过当前设备的绘图容量，无法完整显示";
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

#[derive(Clone, Copy)]
struct Triangle {
    points: [[f32; 2]; 3],
    depth: f32,
    color: u32,
}

fn normalize(vector: [f64; 3]) -> Option<[f64; 3]> {
    let length = vector
        .into_iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    (length.is_finite() && length > 1e-12).then_some(vector.map(|value| value / length))
}

fn world_point(matrix: &[f32; 16], point: [f32; 3]) -> [f64; 3] {
    let p = point.map(f64::from);
    [
        f64::from(matrix[0]) * p[0]
            + f64::from(matrix[4]) * p[1]
            + f64::from(matrix[8]) * p[2]
            + f64::from(matrix[12]),
        f64::from(matrix[1]) * p[0]
            + f64::from(matrix[5]) * p[1]
            + f64::from(matrix[9]) * p[2]
            + f64::from(matrix[13]),
        f64::from(matrix[2]) * p[0]
            + f64::from(matrix[6]) * p[1]
            + f64::from(matrix[10]) * p[2]
            + f64::from(matrix[14]),
    ]
}

fn world_normal(matrix: &[f32; 16], normal: [f32; 3]) -> Option<[f64; 3]> {
    normalize([
        f64::from(matrix[0]) * f64::from(normal[0])
            + f64::from(matrix[4]) * f64::from(normal[1])
            + f64::from(matrix[8]) * f64::from(normal[2]),
        f64::from(matrix[1]) * f64::from(normal[0])
            + f64::from(matrix[5]) * f64::from(normal[1])
            + f64::from(matrix[9]) * f64::from(normal[2]),
        f64::from(matrix[2]) * f64::from(normal[0])
            + f64::from(matrix[6]) * f64::from(normal[1])
            + f64::from(matrix[10]) * f64::from(normal[2]),
    ])
}

fn clip_point(matrix: &[f32; 16], point: [f64; 3]) -> Option<([f32; 2], f32)> {
    let clip = [
        f64::from(matrix[0]) * point[0]
            + f64::from(matrix[4]) * point[1]
            + f64::from(matrix[8]) * point[2]
            + f64::from(matrix[12]),
        f64::from(matrix[1]) * point[0]
            + f64::from(matrix[5]) * point[1]
            + f64::from(matrix[9]) * point[2]
            + f64::from(matrix[13]),
        f64::from(matrix[2]) * point[0]
            + f64::from(matrix[6]) * point[1]
            + f64::from(matrix[10]) * point[2]
            + f64::from(matrix[14]),
        f64::from(matrix[3]) * point[0]
            + f64::from(matrix[7]) * point[1]
            + f64::from(matrix[11]) * point[2]
            + f64::from(matrix[15]),
    ];
    if !clip.iter().all(|value| value.is_finite()) || clip[3] <= 1e-6 {
        return None;
    }
    let x = clip[0] / clip[3];
    let y = clip[1] / clip[3];
    let z = clip[2] / clip[3];
    ((-1e4..=1e4).contains(&x) && (-1e4..=1e4).contains(&y) && z.is_finite())
        .then_some(([x as f32, y as f32], z as f32))
}

fn shade(color: [f32; 3], normal: [f64; 3], center: [f64; 3], eye: [f64; 3]) -> u32 {
    let light = normalize([-0.4, -0.5, 1.0]).unwrap_or([0., 0., 1.]);
    let fill = normalize([0.6, 0.5, 0.2]).unwrap_or([0., 0., 1.]);
    let view = normalize([eye[0] - center[0], eye[1] - center[1], eye[2] - center[2]])
        .unwrap_or([0., 0., 1.]);
    let dot = |a: [f64; 3], b: [f64; 3]| a.into_iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
    let diffuse = dot(normal, light).max(0.) * 0.65;
    let back_diffuse = dot(normal, [-light[0], -light[1], -light[2]]).max(0.) * 0.35;
    let fill_light = dot(normal, fill).max(0.) * 0.18;
    let halfway =
        normalize([light[0] + view[0], light[1] + view[1], light[2] + view[2]]).unwrap_or(light);
    let specular = dot(normal, halfway).max(0.).powi(40) * 0.16;
    let factor = 0.38 + diffuse + back_diffuse + fill_light;
    let channels = color.map(|value| (f64::from(value) * factor + specular).clamp(0., 1.));
    ((channels[0] * 255.).round() as u32) << 24
        | ((channels[1] * 255.).round() as u32) << 16
        | ((channels[2] * 255.).round() as u32) << 8
        | 0xff
}

fn add_mesh(
    triangles: &mut Vec<Triangle>,
    mesh: &Mesh,
    instances: &[[f32; 16]],
    camera: &crate::viewer::camera3d::Camera3d,
    width: f64,
    height: f64,
) {
    let camera_matrix = projection(camera, width / height);
    let eye = eye_position(camera);
    for matrix in instances {
        for indices in mesh.indices.as_chunks::<3>().0 {
            let mut world = [[0.; 3]; 3];
            let mut normals = [[0.; 3]; 3];
            let mut colors = [[0.; 3]; 3];
            let mut valid = true;
            for (slot, index) in indices.iter().enumerate() {
                let start = (*index as usize).saturating_mul(9);
                if start + 9 > mesh.vertices.len() {
                    valid = false;
                    break;
                }
                let vertex = &mesh.vertices[start..start + 9];
                world[slot] = world_point(matrix, [vertex[0], vertex[1], vertex[2]]);
                normals[slot] = [vertex[3], vertex[4], vertex[5]];
                colors[slot] = [vertex[6], vertex[7], vertex[8]];
            }
            if !valid {
                continue;
            }
            let Some(normal) = normalize(
                normals
                    .iter()
                    .filter_map(|normal| world_normal(matrix, *normal))
                    .fold([0.; 3], |sum, next| {
                        [sum[0] + next[0], sum[1] + next[1], sum[2] + next[2]]
                    }),
            ) else {
                continue;
            };
            let Some(projected) = world
                .map(|point| clip_point(&camera_matrix, point))
                .into_iter()
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let center = [
                (world[0][0] + world[1][0] + world[2][0]) / 3.,
                (world[0][1] + world[1][1] + world[2][1]) / 3.,
                (world[0][2] + world[1][2] + world[2][2]) / 3.,
            ];
            let points = [projected[0], projected[1], projected[2]].map(|(point, _)| {
                [
                    ((f64::from(point[0]) * 0.5 + 0.5) * width) as f32,
                    ((1. - (f64::from(point[1]) * 0.5 + 0.5)) * height) as f32,
                ]
            });
            if points.iter().flatten().any(|value| !value.is_finite()) {
                continue;
            }
            let color = colors
                .iter()
                .fold([0.; 3], |sum, next| {
                    [sum[0] + next[0], sum[1] + next[1], sum[2] + next[2]]
                })
                .map(|value| value / 3.);
            let depth = projected.iter().map(|(_, depth)| *depth).sum::<f32>() / 3.;
            triangles.push(Triangle {
                points,
                depth,
                color: shade(color, normal, center, eye),
            });
        }
    }
}

fn within_capacity(resources: &SurfaceResources, components_visible: bool) -> bool {
    let mut count = resources
        .board
        .as_ref()
        .map_or(0, |board| board.board.indices.len() / 3);
    if components_visible {
        for model in &resources.models {
            let Some(next) = (model.mesh.indices.len() / 3)
                .checked_mul(model.instances.len())
                .and_then(|triangles| count.checked_add(triangles))
            else {
                return false;
            };
            count = next;
            if count > MAX_TRIANGLES {
                return false;
            }
        }
    }
    count <= MAX_TRIANGLES
}

fn drawing(model: Option<&SurfaceModel>, width: f64, height: f64) -> Painter {
    let mut painter =
        Painter::new()
            .fill_color(rgba(0x0b121aff))
            .fill_rect(0., 0., width as f32, height as f32);
    let Some(model) = model else {
        return painter;
    };
    let frame = model.frame();
    if !frame.active {
        return painter;
    }
    let Some(resources) = model.resources_since(u64::MAX) else {
        return painter;
    };
    if !within_capacity(&resources, frame.components_visible) {
        return painter;
    }
    let mut triangles = Vec::new();
    if let Some(board) = resources.board {
        add_mesh(
            &mut triangles,
            &board.board,
            &[IDENTITY],
            &frame.camera,
            width,
            height,
        );
    }
    if frame.components_visible {
        for model in resources.models {
            add_mesh(
                &mut triangles,
                &model.mesh,
                &model
                    .instances
                    .iter()
                    .map(|instance| instance.matrix)
                    .collect::<Vec<_>>(),
                &frame.camera,
                width,
                height,
            );
        }
    }
    triangles.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(Ordering::Equal));
    for triangle in triangles {
        painter = painter
            .begin_path()
            .move_to(triangle.points[0][0], triangle.points[0][1])
            .line_to(triangle.points[1][0], triangle.points[1][1])
            .line_to(triangle.points[2][0], triangle.points[2][1])
            .close_path()
            .fill_color(rgba(triangle.color))
            .fill();
    }
    painter
}

pub(super) fn surface(
    status: Signal<SurfaceStatus>,
    model: Option<(Arc<SurfaceModel>, Signal<u64>)>,
) -> AnyElement {
    let size = signal([0.0_f64; 2]);
    let (model, tick) = model.map_or((None, signal(0)), |(model, tick)| (Some(model), tick));
    let status_for_layout = status;
    let tick_for_layout = tick;
    let model_for_status = model.clone();
    effect(move || {
        let _ = tick_for_layout.get();
        let [width, height] = size.get();
        let over_capacity = model_for_status.as_ref().is_some_and(|model| {
            model.resources_since(u64::MAX).is_some_and(|resources| {
                !within_capacity(&resources, model.frame().components_visible)
            })
        });
        let next = if over_capacity {
            SurfaceStatus::Error(CAPACITY_ERROR.into())
        } else if width.is_finite()
            && height.is_finite()
            && width > 0.
            && height > 0.
            && width <= f64::from(u32::MAX)
            && height <= f64::from(u32::MAX)
        {
            SurfaceStatus::Ready {
                backend: "NSH Canvas".into(),
                width: width.ceil() as u32,
                height: height.ceil() as u32,
            }
        } else if width == 0. || height == 0. {
            SurfaceStatus::Pending
        } else {
            SurfaceStatus::Error("绘图尺寸无效或超过 Canvas 上限".into())
        };
        if status_for_layout.get_untracked() != next {
            status_for_layout.set(next);
        }
    });
    let model_for_draw = model.clone();
    view()
        .flex_1()
        .on_layout(move |bounds: &nsh_core::Bounds<nsh_core::Points>| {
            size.set([
                f64::from(bounds.size.width.0),
                f64::from(bounds.size.height.0),
            ]);
        })
        .child(canvas(move || {
            let _ = tick.get();
            let [width, height] = size.get();
            if width.is_finite()
                && height.is_finite()
                && width > 0.
                && height > 0.
                && width <= f64::from(u32::MAX)
                && height <= f64::from(u32::MAX)
            {
                drawing(model_for_draw.as_deref(), width, height)
            } else {
                Painter::new()
            }
        }))
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsh_core::{Size, element::Root};
    use nsh_engine::EngineConfig;
    use nsh_testing::Headless;
    use oshwhub_core::pcb::{Bounds3, RawScene3d};

    #[component]
    fn OverCapacityFixture() -> impl IntoElement {
        let camera = crate::viewer::camera3d::fit_camera(
            &Bounds3 {
                min: [0.; 3],
                max: [1.; 3],
            },
            1.,
        );
        let model = SurfaceModel::new(camera);
        model.set_board(Arc::new(RawScene3d {
            board: Arc::new(Mesh {
                vertices: vec![0.; 27],
                indices: [0, 1, 2].repeat(MAX_TRIANGLES + 1),
                bounds: Bounds3 {
                    min: [0.; 3],
                    max: [1.; 3],
                },
            }),
            model_keys: vec![],
            component_count: 0,
            bound_count: 0,
            thickness: 1.6,
            warnings: vec![],
        }));
        let tick = signal(model.tick());
        let status = signal(SurfaceStatus::Pending);
        view()
            .flex_1()
            .child(surface(status, Some((model, tick))))
            .child(text(move || match status.get() {
                SurfaceStatus::Error(message) => message,
                SurfaceStatus::Ready { .. } => "已完整显示".into(),
                SurfaceStatus::Pending => "准备中".into(),
            }))
    }

    #[test]
    fn over_capacity_scene_reports_an_error_instead_of_ready_partial_geometry() {
        let mut app = Headless::start(EngineConfig {
            viewport: Size::new(px(320.), px(480.)),
            ..EngineConfig::new(Root::of::<OverCapacityFixture>())
        })
        .unwrap();
        app.first_render().unwrap();
        assert!(app.shows("三维场景超过当前设备的绘图容量，无法完整显示"));
        assert!(!app.shows("已完整显示"));
    }

    #[test]
    fn clip_point_rejects_points_behind_the_camera() {
        let perspective = [
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 1., 0., 0., 0., 0.,
        ];
        assert!(clip_point(&perspective, [0., 0., -1.]).is_none());
        assert!(clip_point(&perspective, [0., 0., 0.]).is_none());
        assert!(clip_point(&perspective, [0., 0., 1.]).is_some());
    }
}
