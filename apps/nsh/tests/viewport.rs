#[path = "../src/viewer/raster.rs"]
mod raster;
#[path = "../src/viewer/viewport.rs"]
mod viewport;

use oshwhub_core::pcb::Bounds;
use raster::{covers_view, detail_bounds, plan_raster};
use viewport::{Point, Viewport, fit_viewport, gesture_viewport, is_tap, world_point, zoom_at};

const START: Viewport = Viewport {
    scale: 2.,
    x: 10.,
    y: 20.,
};

fn board_bounds() -> Bounds {
    Bounds {
        min_x: 10.,
        min_y: 20.,
        max_x: 110.,
        max_y: 70.,
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn fits_non_origin_bounds_with_equal_border() {
    assert_eq!(
        fit_viewport(&board_bounds(), 600., 400., 50.),
        Viewport {
            scale: 5.,
            x: 0.,
            y: -25.,
        }
    );
}

#[test]
fn small_canvases_keep_the_board_legible_and_centered() {
    for (width, height, scale, x, y) in [
        (320., 48., 0.672, 119.68, -6.24),
        (844., 70., 0.98, 363.2, -9.1),
        (64., 600., 0.448, 5.12, 279.84),
    ] {
        let fit = fit_viewport(&board_bounds(), width, height, 24.);
        assert_close(fit.scale, scale);
        assert_close(fit.x, x);
        assert_close(fit.y, y);
        let center = world_point(
            fit,
            Point {
                x: width / 2.,
                y: height / 2.,
            },
        );
        assert_close(center.x, 60.);
        assert_close(center.y, 45.);
    }
}

#[test]
fn zero_sized_geometry_and_canvas_still_produce_a_finite_fit() {
    assert_eq!(
        fit_viewport(
            &Bounds {
                min_x: 4.,
                min_y: 4.,
                max_x: 4.,
                max_y: 4.,
            },
            0.,
            0.,
            24.,
        ),
        Viewport {
            scale: 1000.,
            x: -4000.5,
            y: -4000.5,
        }
    );
}

#[test]
fn zoom_clamps_both_limits_without_moving_the_anchor() {
    let anchor = Point { x: 110., y: 70. };
    assert_eq!(
        zoom_at(START, 4., anchor, 1., 3.),
        Viewport {
            scale: 3.,
            x: -40.,
            y: -5.,
        }
    );
    assert_eq!(
        zoom_at(START, 0.1, anchor, 1., 3.),
        Viewport {
            scale: 1.,
            x: 60.,
            y: 45.,
        }
    );
}

#[test]
fn screen_points_convert_back_to_world_coordinates() {
    assert_eq!(
        world_point(
            Viewport {
                scale: 4.,
                x: -90.,
                y: -30.,
            },
            Point { x: 110., y: 70. },
        ),
        Point { x: 50., y: 25. }
    );
}

#[test]
fn one_pointer_pans_without_scaling() {
    assert_eq!(
        gesture_viewport(
            START,
            &[Point { x: 100., y: 100. }],
            &[Point { x: 130., y: 90. }],
            1.,
            10.,
        ),
        Viewport {
            scale: 2.,
            x: 40.,
            y: 10.,
        }
    );
}

#[test]
fn pinch_scales_around_the_moving_center_of_the_first_two_pointers() {
    assert_eq!(
        gesture_viewport(
            START,
            &[
                Point { x: 100., y: 100. },
                Point { x: 200., y: 100. },
                Point { x: 1000., y: 1000. },
            ],
            &[
                Point { x: 110., y: 120. },
                Point { x: 310., y: 120. },
                Point { x: -500., y: -500. },
            ],
            1.,
            10.,
        ),
        Viewport {
            scale: 4.,
            x: -70.,
            y: -40.,
        }
    );
}

#[test]
fn pinch_clamps_scale_and_keeps_the_center_anchored() {
    let start = [Point { x: 100., y: 100. }, Point { x: 200., y: 100. }];
    for (current, expected) in [
        (
            [Point { x: 130., y: 120. }, Point { x: 130., y: 120. }],
            Viewport {
                scale: 1.,
                x: 60.,
                y: 80.,
            },
        ),
        (
            [Point { x: 0., y: 0. }, Point { x: 1000., y: 0. }],
            Viewport {
                scale: 10.,
                x: -200.,
                y: -400.,
            },
        ),
    ] {
        assert_eq!(gesture_viewport(START, &start, &current, 1., 10.), expected);
    }
}

#[test]
fn empty_gestures_leave_the_view_unchanged() {
    let points = [Point { x: 100., y: 100. }];
    assert_eq!(gesture_viewport(START, &[], &points, 1., 10.), START);
    assert_eq!(gesture_viewport(START, &points, &[], 1., 10.), START);
}

#[test]
fn coincident_pinch_start_pans_without_dividing_by_zero() {
    assert_eq!(
        gesture_viewport(
            START,
            &[Point { x: 100., y: 100. }, Point { x: 100., y: 100. }],
            &[Point { x: 130., y: 120. }, Point { x: 170., y: 120. }],
            1.,
            10.,
        ),
        Viewport {
            scale: 2.,
            x: 60.,
            y: 40.,
        }
    );
}

#[test]
fn taps_require_one_pointer_and_at_most_five_pixels_of_motion() {
    let start = Point { x: 10., y: 10. };
    assert!(is_tap(start, Point { x: 12., y: 13. }, 1));
    assert!(is_tap(start, Point { x: 13., y: 14. }, 1));
    assert!(!is_tap(start, Point { x: 13.001, y: 14. }, 1));
    assert!(!is_tap(start, Point { x: 20., y: 10. }, 1));
    assert!(!is_tap(start, start, 0));
    assert!(!is_tap(start, start, 2));
}

#[test]
fn raster_caps_retina_and_thin_regions_without_zero_sized_images() {
    for (width, height, pixel_width, pixel_height) in [
        (1000., 1000., 2048, 2048),
        (1e8, 0.001, 4096, 1),
        (0.001, 1e8, 1, 4096),
        (0., 0., 1, 1),
    ] {
        let plan = plan_raster(
            &Bounds {
                min_x: 10.,
                min_y: -20.,
                max_x: width + 10.,
                max_y: height - 20.,
            },
            100.,
        );
        assert_eq!((plan.x, plan.y), (10., -20.));
        assert_eq!(
            (plan.pixel_width, plan.pixel_height),
            (pixel_width, pixel_height)
        );
        assert!(plan.pixel_width.max(plan.pixel_height) <= 4096);
        assert!(plan.pixel_width * plan.pixel_height <= 4_194_304);
        assert!(plan.width > 0. && plan.height > 0.);
    }
}

#[test]
fn raster_respects_lower_density_and_rounds_dimensions_down() {
    let plan = plan_raster(
        &Bounds {
            min_x: 10.,
            min_y: -20.,
            max_x: 20.,
            max_y: 0.,
        },
        0.25,
    );
    assert_eq!((plan.width, plan.height), (10., 20.));
    assert_eq!((plan.pixel_width, plan.pixel_height), (2, 5));
}

#[test]
fn detail_bounds_add_motion_room_around_a_panned_and_zoomed_view() {
    let view = Viewport {
        scale: 2.,
        x: -100.,
        y: 40.,
    };
    let bounds = detail_bounds(view, 800., 400.);
    assert_eq!((bounds.min_x, bounds.min_y), (0., -45.));
    assert_eq!((bounds.max_x, bounds.max_y), (500., 205.));
    let plan = plan_raster(&bounds, 4.);
    assert_eq!((plan.pixel_width, plan.pixel_height), (2000, 1000));
    for (x, covered) in [(-100., true), (-180., true), (-400., false)] {
        assert_eq!(
            covers_view(plan, Viewport { x, ..view }, 800., 400.),
            covered
        );
    }
}

#[test]
fn coverage_includes_exact_edges_and_rejects_each_uncovered_edge() {
    let plan = plan_raster(
        &Bounds {
            min_x: 0.,
            min_y: -45.,
            max_x: 500.,
            max_y: 205.,
        },
        4.,
    );
    for (x, y, covered) in [
        (0., 90., true),
        (1., 90., false),
        (0., 91., false),
        (-200., -10., true),
        (-200.1, -10., false),
        (-200., -10.1, false),
    ] {
        assert_eq!(
            covers_view(plan, Viewport { scale: 2., x, y }, 800., 400.),
            covered
        );
    }
}
