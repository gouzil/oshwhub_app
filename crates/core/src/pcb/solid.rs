use super::{Bounds, geometry::Point};
use crate::error::{AppError, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use i_overlay::{
    core::{fill_rule::FillRule as OverlayFill, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator,
    StrokeVertex, VertexBuffers,
    math::point,
    path::{FillRule, LineCap, LineJoin, Path},
};
use serde::Serialize;

pub const MM: f64 = 0.0254;
pub const MAX_VERTICES: usize = 1_500_000;

pub fn invalid() -> AppError {
    AppError::new("pcb_3d_format", "三维几何或模型数据无效")
}
fn limit() -> AppError {
    AppError::new("pcb_3d_limit", "三维场景超过安全大小限制")
}

#[derive(Clone, Debug, Serialize)]
pub struct Bounds3 {
    pub min: [f64; 3],
    pub max: [f64; 3],
}
impl Bounds3 {
    fn empty() -> Self {
        Self {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        }
    }
    fn include(&mut self, p: [f32; 3]) {
        for (i, v) in p.iter().enumerate() {
            self.min[i] = self.min[i].min(*v as f64);
            self.max[i] = self.max[i].max(*v as f64);
        }
    }
}

pub struct Mesh {
    pub vertices: Vec<f32>,
    pub bounds: Bounds3,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackedMesh {
    pub data: String,
    pub vertex_count: usize,
    pub bounds: Bounds3,
}
impl Mesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            bounds: Bounds3::empty(),
        }
    }
    pub fn triangle(
        &mut self,
        points: [[f32; 3]; 3],
        normal: Option<[f32; 3]>,
        color: [f32; 3],
    ) -> Result<()> {
        if self.vertices.len() / 9 + 3 > MAX_VERTICES {
            return Err(limit());
        }
        if !points
            .iter()
            .flatten()
            .chain(color.iter())
            .all(|v| v.is_finite() && v.abs() < 1e6)
        {
            return Err(invalid());
        }
        let a = std::array::from_fn::<_, 3, _>(|i| points[1][i] - points[0][i]);
        let b = std::array::from_fn::<_, 3, _>(|i| points[2][i] - points[0][i]);
        let n = normal.unwrap_or([
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]);
        let length = n.iter().map(|v| v * v).sum::<f32>().sqrt();
        if !length.is_finite() {
            return Err(invalid());
        }
        if length < 1e-12 {
            return Ok(());
        }
        let n = n.map(|v| v / length);
        for p in points {
            self.bounds.include(p);
            self.vertices.extend(p);
            self.vertices.extend(n);
            self.vertices.extend(color);
        }
        Ok(())
    }
    pub fn packed(&self) -> PackedMesh {
        let bytes: Vec<u8> = self.vertices.iter().flat_map(|v| v.to_le_bytes()).collect();
        PackedMesh {
            data: STANDARD.encode(bytes),
            vertex_count: self.vertices.len() / 9,
            bounds: self.bounds.clone(),
        }
    }
}

fn near(a: Point, b: Point) -> bool {
    // Independently rounded line/arc endpoints may differ by up to 5 micrometres.
    (a.0 - b.0).hypot(a.1 - b.1) <= 0.2
}
pub fn close_outlines(contours: &[Vec<Point>]) -> Vec<Vec<Point>> {
    let mut pending: Vec<_> = contours.iter().filter(|p| p.len() > 1).cloned().collect();
    let mut result = Vec::new();
    while let Some(mut path) = pending.pop() {
        while !near(path[0], *path.last().unwrap()) {
            let last = *path.last().unwrap();
            let Some(i) = pending
                .iter()
                .position(|p| near(last, p[0]) || near(last, *p.last().unwrap()))
            else {
                break;
            };
            let mut next = pending.swap_remove(i);
            if !near(last, next[0]) {
                next.reverse();
            }
            path.extend(next.into_iter().skip(1));
        }
        if path.len() > 3 && near(path[0], *path.last().unwrap()) {
            *path.last_mut().unwrap() = path[0];
            result.push(path);
        }
    }
    result
}

type Contours = Vec<Vec<[f64; 2]>>;
fn arrays(contours: &[Vec<Point>]) -> Contours {
    contours
        .iter()
        .map(|p| p.iter().map(|p| [p.0, p.1]).collect())
        .collect()
}
fn signed_area(contour: &[Point]) -> f64 {
    contour
        .iter()
        .zip(contour.iter().cycle().skip(1))
        .take(contour.len())
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum::<f64>()
        / 2.
}

fn board_shape(outlines: &[Vec<Point>], holes: &[Vec<Point>]) -> Result<Vec<Contours>> {
    if outlines.len() > 4096
        || holes.len() > 50_000
        || outlines.iter().chain(holes).map(Vec::len).sum::<usize>() > 500_000
    {
        return Err(limit());
    }
    if !outlines
        .iter()
        .chain(holes)
        .flatten()
        .all(|p| p.0.is_finite() && p.1.is_finite() && p.0.abs() < 1e8 && p.1.abs() < 1e8)
    {
        return Err(invalid());
    }
    let outline: Contours = arrays(outlines)
        .overlay(
            &Vec::<Vec<[f64; 2]>>::new(),
            OverlayRule::Subject,
            OverlayFill::EvenOdd,
        )
        .into_iter()
        .flatten()
        .collect();
    // Drill contours overlap; even-odd subtraction would close their intersections again.
    let holes: Vec<_> = holes
        .iter()
        .map(|h| {
            let mut h = h.clone();
            if signed_area(&h) < 0. {
                h.reverse();
            }
            h
        })
        .collect();
    Ok(outline.overlay(
        &arrays(&holes),
        OverlayRule::Difference,
        OverlayFill::NonZero,
    ))
}

fn tessellate(
    contours: &[Vec<Point>],
    fill: bool,
    width: f64,
) -> Result<VertexBuffers<[f32; 2], u32>> {
    let count = contours.iter().map(Vec::len).sum::<usize>();
    if count > 500_000 {
        return Err(limit());
    }
    let mut builder = Path::builder();
    for contour in contours.iter().filter(|p| p.len() > 1) {
        let convert = |p: Point| point((p.0 * MM) as f32, (-p.1 * MM) as f32);
        builder.begin(convert(contour[0]));
        let closed = fill || near(contour[0], *contour.last().unwrap());
        for p in &contour[1..] {
            builder.line_to(convert(*p));
        }
        builder.end(closed);
    }
    let path = builder.build();
    let mut result = VertexBuffers::new();
    if fill {
        FillTessellator::new()
            .tessellate_path(
                &path,
                &FillOptions::default().with_fill_rule(FillRule::NonZero),
                &mut BuffersBuilder::new(&mut result, |v: FillVertex| v.position().to_array()),
            )
            .map_err(|_| invalid())?;
    } else {
        StrokeTessellator::new()
            .tessellate_path(
                &path,
                &StrokeOptions::default()
                    .with_line_width((width * MM) as f32)
                    .with_line_cap(LineCap::Round)
                    .with_line_join(LineJoin::Round),
                &mut BuffersBuilder::new(&mut result, |v: StrokeVertex| v.position().to_array()),
            )
            .map_err(|_| invalid())?;
    }
    if result.indices.len() > MAX_VERTICES {
        return Err(limit());
    }
    Ok(result)
}

fn face(
    mesh: &mut Mesh,
    contours: &[Vec<Point>],
    fill: bool,
    width: f64,
    z: f64,
    bottom: bool,
    color: [f32; 3],
) -> Result<()> {
    let triangles = tessellate(contours, fill, width)?;
    for tri in triangles.indices.as_chunks::<3>().0 {
        let mut points = std::array::from_fn(|i| {
            let p = triangles.vertices[tri[i] as usize];
            [p[0], p[1], z as f32]
        });
        let cross = (points[1][0] - points[0][0]) * (points[2][1] - points[0][1])
            - (points[1][1] - points[0][1]) * (points[2][0] - points[0][0]);
        if (cross > 0.) == bottom {
            points.swap(1, 2);
        }
        mesh.triangle(points, Some([0., 0., if bottom { -1. } else { 1. }]), color)?;
    }
    Ok(())
}

pub fn board_mesh(outlines: &[Vec<Point>], holes: &[Vec<Point>], thickness: f64) -> Result<Mesh> {
    if !thickness.is_finite() || !(0.1..=20.).contains(&thickness) {
        return Err(invalid());
    }
    let shapes = board_shape(outlines, holes)?;
    if shapes.is_empty() {
        return Err(invalid());
    }
    let mut mesh = Mesh::new();
    for shape in shapes {
        let mut contours: Vec<Vec<Point>> = shape
            .iter()
            .map(|p| p.iter().map(|p| (p[0], p[1])).collect())
            .collect();
        for (index, contour) in contours.iter_mut().enumerate() {
            if (signed_area(contour) > 0.) == (index == 0) {
                contour.reverse();
            }
        }
        face(
            &mut mesh,
            &contours,
            true,
            0.,
            thickness,
            false,
            [0.055, 0.31, 0.22],
        )?;
        face(
            &mut mesh,
            &contours,
            true,
            0.,
            0.,
            true,
            [0.055, 0.31, 0.22],
        )?;
        for contour in &contours {
            for i in 0..contour.len() {
                let a = contour[i];
                let b = contour[(i + 1) % contour.len()];
                let xy = |p: Point, z: f64| [(p.0 * MM) as f32, (-p.1 * MM) as f32, z as f32];
                let (a0, b0, a1, b1) = (xy(a, 0.), xy(b, 0.), xy(a, thickness), xy(b, thickness));
                mesh.triangle([a0, b0, b1], None, [0.28, 0.33, 0.20])?;
                mesh.triangle([a0, b1, a1], None, [0.28, 0.33, 0.20])?;
            }
        }
    }
    Ok(mesh)
}

#[derive(Clone, Default)]
pub struct Seed {
    pub outlines: Vec<Vec<Point>>,
    pub holes: Vec<Vec<Point>>,
    pub bindings: Vec<super::models::Binding>,
    pub component_count: usize,
    pub thickness: Option<f64>,
    pub warnings: Vec<String>,
}

pub fn stack_thickness(doc: &super::history::Document) -> Option<f64> {
    let layers: Vec<_> = doc.active().filter(|r| r.kind == "LAYER_PHYS").collect();
    let z_index = |id: i64| {
        layers
            .iter()
            .find(|r| r.id.get(1).and_then(serde_json::Value::as_i64) == Some(id))?
            .data
            .as_ref()?["zIndex"]
            .as_f64()
    };
    let top = z_index(1)?;
    let bottom = z_index(2)?;
    if !top.is_finite() || !bottom.is_finite() || top >= bottom {
        return None;
    }
    let mut mils = 0.;
    for r in layers {
        let d = r.data.as_ref()?;
        let z = d["zIndex"].as_f64()?;
        if !z.is_finite() {
            return None;
        }
        if (top..=bottom).contains(&z) {
            let value = d["thickness"].as_f64()?;
            if !value.is_finite() || value < 0. {
                return None;
            }
            mils += value;
        }
    }
    let mm = mils * MM;
    (mm.is_finite() && (0.1..=20.).contains(&mm)).then_some(mm)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene3d {
    pub board: PackedMesh,
    pub model_keys: Vec<String>,
    pub component_count: usize,
    pub bound_count: usize,
    pub thickness: f64,
    pub warnings: Vec<String>,
}

pub fn prepare(
    seed: &Seed,
    surfaces: impl Iterator<Item = (i32, bool, f64, Vec<Vec<Point>>, String)>,
    bounds: Bounds,
) -> Result<Scene3d> {
    let mut warnings = seed.warnings.clone();
    if seed.outlines.len() > 4096 {
        return Err(limit());
    }
    let mut outlines = close_outlines(&seed.outlines);
    if outlines.is_empty() {
        outlines.push(vec![
            (bounds.min_x, bounds.min_y),
            (bounds.max_x, bounds.min_y),
            (bounds.max_x, bounds.max_y),
            (bounds.min_x, bounds.max_y),
            (bounds.min_x, bounds.min_y),
        ]);
        warnings.push("板框未形成闭环，当前按绘图范围预览板体".into());
    }
    let thickness = seed.thickness.unwrap_or(1.6);
    if seed.thickness.is_none() {
        warnings.push("叠层厚度缺失或无效，当前按 1.6 mm 预览".into());
    }
    let shape = board_shape(&outlines, &seed.holes)?;
    let clip: Contours = shape.into_iter().flatten().collect();
    let mut mesh = board_mesh(&outlines, &seed.holes, thickness)?;
    // Union paint triangles by material before clipping, so drills also cut through tracks and pads.
    let mut groups = std::collections::BTreeMap::<(i32, u8), Contours>::new();
    let mut vertices = 0;
    for (layer, fill, width, contours, kind) in surfaces {
        if !matches!(layer, 1..=4 | 12) || kind == "COMPONENT" {
            continue;
        }
        let sides: &[i32] = if layer == 12 {
            &[1, 2]
        } else {
            std::slice::from_ref(&layer)
        };
        let triangles = tessellate(&contours, fill, width)?;
        vertices += triangles.indices.len();
        if vertices > MAX_VERTICES {
            return Err(limit());
        }
        for side in sides {
            let material = if *side == 3 || *side == 4 {
                2
            } else if kind == "PAD" || kind == "VIA" {
                1
            } else {
                0
            };
            let group = groups.entry((*side, material)).or_default();
            for tri in triangles.indices.as_chunks::<3>().0 {
                let mut points: Vec<_> = tri
                    .iter()
                    .map(|i| {
                        let p = triangles.vertices[*i as usize];
                        [p[0] as f64 / MM, -p[1] as f64 / MM]
                    })
                    .collect();
                if (points[1][0] - points[0][0]) * (points[2][1] - points[0][1])
                    - (points[1][1] - points[0][1]) * (points[2][0] - points[0][0])
                    < 0.
                {
                    points.reverse();
                }
                group.push(points);
            }
        }
    }
    for ((side, material), triangles) in groups {
        let clipped = triangles.overlay(&clip, OverlayRule::Intersect, OverlayFill::NonZero);
        let bottom = side == 2 || side == 4;
        let offset = 0.005 + material as f64 * 0.005;
        let z = if bottom { -offset } else { thickness + offset };
        let color = match material {
            2 => [0.9, 0.94, 0.88],
            1 => [0.82, 0.68, 0.36],
            _ => [0.075, 0.37, 0.26],
        };
        for shape in clipped {
            let contours: Vec<_> = shape
                .iter()
                .map(|p| p.iter().map(|p| (p[0], p[1])).collect())
                .collect();
            face(&mut mesh, &contours, true, 0., z, bottom, color)?;
        }
    }
    let model_keys = seed
        .bindings
        .iter()
        .map(|b| b.key.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(Scene3d {
        board: mesh.packed(),
        model_keys,
        component_count: seed.component_count,
        bound_count: seed.bindings.len(),
        thickness,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_faces_leave_the_drilled_hole_open() {
        let outline = vec![vec![
            (0., 0.),
            (400., 0.),
            (400., 240.),
            (0., 240.),
            (0., 0.),
        ]];
        let holes = vec![vec![
            (100., 80.),
            (140., 80.),
            (140., 120.),
            (100., 120.),
            (100., 80.),
        ]];
        let mesh = board_mesh(&outline, &holes, 1.6).unwrap();
        let mut top_area: f64 = 0.;
        for triangle in mesh.vertices.as_chunks::<27>().0.iter() {
            assert!(triangle.iter().all(|v| v.is_finite()));
            if triangle[5] > 0.9 {
                let p = |i: usize| (triangle[i] as f64, triangle[i + 1] as f64);
                let (a, b, c) = (p(0), p(9), p(18));
                top_area += ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).abs() / 2.;
            }
        }
        assert!((top_area - 60.903104).abs() < 0.001, "{top_area}");
        assert!((mesh.bounds.max[2] - 1.6).abs() < 0.0001);
    }

    #[test]
    fn unordered_outline_segments_join_without_bridging_open_edges() {
        let contours = vec![
            vec![(10., 10.), (0., 10.)],
            vec![(0., 0.), (10., 0.)],
            vec![(0., 10.), (0., 0.)],
            vec![(10., 0.), (10., 10.)],
        ];
        let closed = close_outlines(&contours);
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].first(), closed[0].last());
        assert!(close_outlines(&contours[..2]).is_empty());
    }

    #[test]
    fn independently_rounded_arc_endpoints_can_close_without_bridging_real_gaps() {
        let mut paths = vec![
            vec![(0., 0.), (100., 0.)],
            vec![(100.1, 0.), (100., 100.)],
            vec![(100., 100.), (0., 100.)],
            vec![(0., 100.), (0., 0.)],
        ];
        assert_eq!(close_outlines(&paths).len(), 1);
        paths[1][0] = (101., 0.);
        assert!(close_outlines(&paths).is_empty());
    }

    #[test]
    fn stack_thickness_sums_between_copper_faces_and_rejects_invalid_layers() {
        let log = r#"{"type":"DOCHEAD"}||{"uuid":"pcb","docType":"PCB"}|
{"type":"LAYER_PHYS","id":["LAYER_PHYS",3]}||{"zIndex":1,"thickness":10}|
{"type":"LAYER_PHYS","id":["LAYER_PHYS",1]}||{"zIndex":1000,"thickness":1.4}|
{"type":"LAYER_PHYS","id":["LAYER_PHYS",361]}||{"zIndex":1001,"thickness":60}|
{"type":"LAYER_PHYS","id":["LAYER_PHYS",2]}||{"zIndex":9000,"thickness":1.4}|"#;
        let mut project = super::super::history::Project::default();
        super::super::history::replay(&mut project, log).unwrap();
        let doc = project.docs.get("pcb").unwrap();
        assert!((stack_thickness(doc).unwrap() - 1.59512).abs() < 1e-8);
        let mut doc = doc.clone();
        let r = doc.records.values_mut().find(|r| r.id[1] == 361).unwrap();
        r.data.as_mut().unwrap()["thickness"] = serde_json::json!(-1.);
        assert!(stack_thickness(&doc).is_none());
    }

    fn top_area(mesh: &Mesh) -> f64 {
        mesh.vertices
            .as_chunks::<27>()
            .0
            .iter()
            .filter(|t| t[5] > 0.9)
            .map(|t| {
                (((t[9] - t[0]) as f64 * (t[19] - t[1]) as f64)
                    - ((t[10] - t[1]) as f64 * (t[18] - t[0]) as f64))
                    .abs()
                    / 2.
            })
            .sum()
    }

    #[test]
    fn overlapping_and_reversed_drills_stay_open_and_edge_slots_cut_the_outline() {
        let outline = vec![vec![
            (0., 0.),
            (100., 0.),
            (100., 100.),
            (0., 100.),
            (0., 0.),
        ]];
        let hole = vec![(20., 20.), (40., 20.), (40., 40.), (20., 40.), (20., 20.)];
        let reversed = hole.iter().rev().copied().collect();
        let adjacent = vec![(30., 20.), (50., 20.), (50., 40.), (30., 40.), (30., 20.)];
        let slot = vec![(90., 40.), (110., 40.), (110., 60.), (90., 60.), (90., 40.)];
        let mesh = board_mesh(&outline, &[hole, reversed, adjacent, slot], 1.6).unwrap();
        assert!((top_area(&mesh) - 9200. * MM * MM).abs() < 0.0001);
    }

    #[test]
    fn board_winding_and_normals_face_outward() {
        let mesh = board_mesh(
            &[vec![
                (0., 0.),
                (100., 0.),
                (100., 100.),
                (0., 100.),
                (0., 0.),
            ]],
            &[],
            1.6,
        )
        .unwrap();
        let center = [50. * MM, -50. * MM, 0.8];
        for t in mesh.vertices.as_chunks::<27>().0.iter() {
            let a = [t[9] - t[0], t[10] - t[1], t[11] - t[2]];
            let b = [t[18] - t[0], t[19] - t[1], t[20] - t[2]];
            let cross = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            assert!(
                cross
                    .iter()
                    .enumerate()
                    .map(|(i, v)| v * t[3 + i])
                    .sum::<f32>()
                    > 0.
            );
            assert!(
                (0..3)
                    .map(|i| ((t[i] + t[9 + i] + t[18 + i]) as f64 / 3. - center[i])
                        * t[3 + i] as f64)
                    .sum::<f64>()
                    > 0.
            );
        }
    }
}
