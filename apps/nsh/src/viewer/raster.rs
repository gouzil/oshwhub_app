use super::viewport::Viewport;
use oshwhub_core::pcb::Bounds;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterPlan {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

pub fn plan_raster(bounds: &Bounds, density: f64) -> RasterPlan {
    let width = (bounds.max_x - bounds.min_x).max(0.001);
    let height = (bounds.max_y - bounds.min_y).max(0.001);
    let scale = density
        .min(4096. / width.max(height))
        .min((4_194_304. / width / height).sqrt());
    RasterPlan {
        x: bounds.min_x,
        y: bounds.min_y,
        width,
        height,
        pixel_width: (width * scale).floor().max(1.) as u32,
        pixel_height: (height * scale).floor().max(1.) as u32,
    }
}

pub fn detail_bounds(view: Viewport, width: f64, height: f64) -> Bounds {
    let margin_x = width * 0.125;
    let margin_y = height * 0.125;
    Bounds {
        min_x: (-view.x - margin_x) / view.scale,
        min_y: (-view.y - margin_y) / view.scale,
        max_x: (-view.x + width + margin_x) / view.scale,
        max_y: (-view.y + height + margin_y) / view.scale,
    }
}

pub fn covers_view(plan: RasterPlan, view: Viewport, width: f64, height: f64) -> bool {
    plan.x * view.scale + view.x <= 0.
        && plan.y * view.scale + view.y <= 0.
        && (plan.x + plan.width) * view.scale + view.x >= width
        && (plan.y + plan.height) * view.scale + view.y >= height
}
