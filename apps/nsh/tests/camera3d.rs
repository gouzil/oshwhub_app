#[path = "../src/viewer/camera3d.rs"]
mod camera3d;

use camera3d::{
    Camera3d, eye_position, fit_camera, orbit_camera, pan_camera, projection, zoom_camera,
};
use oshwhub_core::pcb::Bounds3;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

fn camera() -> Camera3d {
    Camera3d {
        yaw: 0.,
        pitch: 0.,
        distance: 10.,
        radius: 2.,
        target: [1., 2., 3.],
    }
}

fn ndc(matrix: [f32; 16], point: [f64; 3]) -> [f64; 3] {
    let clip: [f64; 4] = std::array::from_fn(|row| {
        f64::from(matrix[row]) * point[0]
            + f64::from(matrix[4 + row]) * point[1]
            + f64::from(matrix[8 + row]) * point[2]
            + f64::from(matrix[12 + row])
    });
    assert!(clip[3] > 0., "point must be in front of the camera");
    std::array::from_fn(|row| clip[row] / clip[3])
}

#[test]
fn fit_centers_the_bounds_and_uses_their_bounding_sphere() {
    let bounds = Bounds3 {
        min: [2., -6., 8.],
        max: [8., 2., 8.],
    };
    let fitted = fit_camera(&bounds, 1.);
    assert_eq!(fitted.target, [5., -2., 8.]);
    close(fitted.radius, 5., 1e-12);
    close(fitted.distance, 14.63350520661542, 1e-12);
    assert_eq!(fit_camera(&bounds, 2.).distance, fitted.distance);
    assert!(fit_camera(&bounds, 0.5).distance > fitted.distance);
    assert_eq!(
        fit_camera(&bounds, 0.).distance,
        fit_camera(&bounds, 0.05).distance
    );
    let point = fit_camera(
        &Bounds3 {
            min: [1., 2., 3.],
            max: [1., 2., 3.],
        },
        1.,
    );
    close(point.radius, 0.1, 1e-12);
    assert!(point.distance > point.radius);
}

#[test]
fn fit_keeps_all_bounds_corners_inside_portrait_and_landscape_views() {
    let bounds = Bounds3 {
        min: [-10., -5., -1.],
        max: [10., 5., 1.],
    };
    for aspect in [0., 0.2, 1., 2.] {
        let fitted = fit_camera(&bounds, aspect);
        let matrix = projection(&fitted, aspect);
        for x in [bounds.min[0], bounds.max[0]] {
            for y in [bounds.min[1], bounds.max[1]] {
                for z in [bounds.min[2], bounds.max[2]] {
                    let point = ndc(matrix, [x, y, z]);
                    assert!(point[0].abs() < 1. && point[1].abs() < 1., "{point:?}");
                    assert!((0. ..=1.).contains(&point[2]), "{point:?}");
                }
            }
        }
    }
}

#[test]
fn orbit_is_unbounded_and_can_look_up_at_the_bottom() {
    let original = camera();
    let bottom = orbit_camera(original, 0., -FRAC_PI_2 / 0.008);
    close(bottom.pitch, -FRAC_PI_2, 1e-12);
    let eye = eye_position(&bottom);
    for (actual, expected) in eye.into_iter().zip([1., 2., -7.]) {
        close(actual, expected, 1e-12);
    }
    let rotated = orbit_camera(original, TAU / 0.008 * 2., PI / 0.008 * 3.);
    close(rotated.yaw, TAU * 2., 1e-12);
    close(rotated.pitch, PI * 3., 1e-12);
    assert_eq!(rotated.target, original.target);
    assert_eq!(rotated.distance, original.distance);
}

#[test]
fn pan_moves_in_screen_axes_and_scales_with_distance_and_height() {
    let original = camera();
    let panned = pan_camera(original, 100., 50., 1000.);
    for (actual, expected) in
        panned
            .target
            .into_iter()
            .zip([1., 1.17157287525381, 3.414213562373095])
    {
        close(actual, expected, 1e-12);
    }
    let twice_as_far = pan_camera(
        Camera3d {
            distance: 20.,
            ..original
        },
        100.,
        50.,
        2000.,
    );
    assert_eq!(panned.target, twice_as_far.target);
    let turned = pan_camera(
        Camera3d {
            yaw: FRAC_PI_2,
            ..original
        },
        1.,
        0.,
        0.,
    );
    close(turned.target[0], 9.284271247461902, 1e-12);
    close(turned.target[1], 2., 1e-12);
    assert_eq!(turned.distance, original.distance);
}

#[test]
fn zoom_is_constrained_to_the_scene_radius() {
    let original = camera();
    close(zoom_camera(original, 2.).distance, 5., 1e-12);
    close(zoom_camera(original, 1e20).distance, 0.24, 1e-12);
    close(zoom_camera(original, 1e-20).distance, 400., 1e-12);
    let zoomed = zoom_camera(original, 2.);
    assert_eq!(zoomed.target, original.target);
    assert_eq!(zoomed.yaw, original.yaw);
    assert_eq!(zoomed.pitch, original.pitch);
    assert_eq!(zoomed.radius, original.radius);
}

#[test]
fn projection_preserves_perspective_and_remaps_gl_depth_to_webgpu() {
    // This pose has eye [11, 2, 3], near 7.6 and far 18. The old GL
    // depths at these points are -1, -11/65 and 1; wgpu uses (z_gl + 1)/2.
    let matrix = projection(&camera(), 2.);
    for (point, depth) in [
        ([3.4, 2., 3.], 0.),
        ([1., 2., 3.], 27. / 65.),
        ([-7., 2., 3.], 1.),
    ] {
        let projected = ndc(matrix, point);
        close(projected[0], 0., 1e-6);
        close(projected[1], 0., 1e-6);
        close(projected[2], depth, 1e-6);
    }
    let offset = ndc(matrix, [1., 3., 4.]);
    close(offset[0], 0.12071067811865475, 1e-6);
    close(offset[1], 0.2414213562373095, 1e-6);
    let farther = ndc(matrix, [-5., 3., 4.]);
    close(farther[0] / offset[0], 0.625, 1e-6);
    close(farther[1] / offset[1], 0.625, 1e-6);
}

#[test]
fn projection_stays_finite_at_orbit_poles_and_zoom_limits() {
    for pitch in [-FRAC_PI_2, FRAC_PI_2, PI, TAU * 3.] {
        for factor in [1e-20, 1., 1e20] {
            let posed = zoom_camera(Camera3d { pitch, ..camera() }, factor);
            let matrix = projection(&posed, 0.);
            assert!(matrix.iter().all(|value| value.is_finite()));
            let target = ndc(matrix, posed.target);
            close(target[0], 0., 1e-3);
            close(target[1], 0., 1e-3);
            assert!((0. ..=1.).contains(&target[2]), "{target:?}");
        }
    }
}
