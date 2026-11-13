use oshwhub_core::pcb::Bounds;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub scale: f64,
    pub x: f64,
    pub y: f64,
}

pub fn fit_viewport(bounds: &Bounds, width: f64, height: f64, padding: f64) -> Viewport {
    let world_width = (bounds.max_x - bounds.min_x).max(0.001);
    let world_height = (bounds.max_y - bounds.min_y).max(0.001);
    let inset = padding.min(width * 0.15).min(height * 0.15);
    let scale = ((width - inset * 2.).max(1.) / world_width)
        .min((height - inset * 2.).max(1.) / world_height);
    Viewport {
        scale,
        x: (width - world_width * scale) / 2. - bounds.min_x * scale,
        y: (height - world_height * scale) / 2. - bounds.min_y * scale,
    }
}

pub fn zoom_at(view: Viewport, next_scale: f64, anchor: Point, min: f64, max: f64) -> Viewport {
    let scale = next_scale.min(max).max(min);
    let world = world_point(view, anchor);
    Viewport {
        scale,
        x: anchor.x - world.x * scale,
        y: anchor.y - world.y * scale,
    }
}

pub fn world_point(view: Viewport, point: Point) -> Point {
    Point {
        x: (point.x - view.x) / view.scale,
        y: (point.y - view.y) / view.scale,
    }
}

fn center(points: &[Point]) -> Point {
    if points.len() > 1 {
        Point {
            x: (points[0].x + points[1].x) / 2.,
            y: (points[0].y + points[1].y) / 2.,
        }
    } else {
        points[0]
    }
}

pub fn gesture_viewport(
    view: Viewport,
    start: &[Point],
    current: &[Point],
    min: f64,
    max: f64,
) -> Viewport {
    if start.is_empty() || current.is_empty() {
        return view;
    }
    let from = center(start);
    let to = center(current);
    let mut scale = view.scale;
    if start.len() > 1 && current.len() > 1 {
        let before = (start[1].x - start[0].x).hypot(start[1].y - start[0].y);
        let after = (current[1].x - current[0].x).hypot(current[1].y - current[0].y);
        if before > 0. {
            scale = (view.scale * after / before).min(max).max(min);
        }
    }
    let world = world_point(view, from);
    Viewport {
        scale,
        x: to.x - world.x * scale,
        y: to.y - world.y * scale,
    }
}

pub fn is_tap(start: Point, end: Point, pointer_count: usize) -> bool {
    pointer_count == 1 && (end.x - start.x).hypot(end.y - start.y) <= 5.
}
