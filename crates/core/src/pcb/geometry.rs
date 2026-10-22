use super::Bounds;
use crate::error::{AppError, Result};
use serde_json::Value;
use std::fmt::Write;

pub type Point = (f64, f64);
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}
impl Transform {
    pub fn identity() -> Self {
        Self {
            a: 1.,
            b: 0.,
            c: 0.,
            d: 1.,
            tx: 0.,
            ty: 0.,
        }
    }
    pub fn board(ox: f64, oy: f64) -> Self {
        Self {
            a: 1.,
            b: 0.,
            c: 0.,
            d: -1.,
            tx: -ox,
            ty: oy,
        }
    }
    pub fn component(self, x: f64, y: f64, angle: f64, bottom: bool) -> Self {
        let (s, c) = angle.to_radians().sin_cos();
        let m = if bottom { -1. } else { 1. };
        Self {
            a: self.a * c + self.c * s,
            b: self.b * c + self.d * s,
            c: (-self.a * s + self.c * c) * m,
            d: (-self.b * s + self.d * c) * m,
            tx: self.a * x + self.c * y + self.tx,
            ty: self.b * x + self.d * y + self.ty,
        }
    }
    pub fn point(self, p: Point) -> Point {
        (
            self.a * p.0 + self.c * p.1 + self.tx,
            self.b * p.0 + self.d * p.1 + self.ty,
        )
    }
    pub fn affine(self, a: f64, b: f64, c: f64, d: f64, tx: f64, ty: f64) -> Self {
        Self {
            a: self.a * a + self.c * b,
            b: self.b * a + self.d * b,
            c: self.a * c + self.c * d,
            d: self.b * c + self.d * d,
            tx: self.a * tx + self.c * ty + self.tx,
            ty: self.b * tx + self.d * ty + self.ty,
        }
    }
    fn determinant(self) -> f64 {
        self.a * self.d - self.b * self.c
    }
}
#[derive(Clone, Debug, Default)]
pub struct Geometry {
    pub path: String,
    pub contours: Vec<Vec<Point>>,
    pub bounds: Bounds,
}
impl Bounds {
    pub fn empty() -> Self {
        Self {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        }
    }
    pub fn include(&mut self, p: Point) {
        self.min_x = self.min_x.min(p.0);
        self.min_y = self.min_y.min(p.1);
        self.max_x = self.max_x.max(p.0);
        self.max_y = self.max_y.max(p.1);
    }
    pub fn union(&mut self, b: Self) {
        self.include((b.min_x, b.min_y));
        self.include((b.max_x, b.max_y));
    }
    pub fn expand(self, r: f64) -> Self {
        Self {
            min_x: self.min_x - r,
            min_y: self.min_y - r,
            max_x: self.max_x + r,
            max_y: self.max_y + r,
        }
    }
    pub fn contains(self, p: Point, tolerance: f64) -> bool {
        p.0 >= self.min_x - tolerance
            && p.0 <= self.max_x + tolerance
            && p.1 >= self.min_y - tolerance
            && p.1 <= self.max_y + tolerance
    }
    pub fn finite(self) -> bool {
        [self.min_x, self.min_y, self.max_x, self.max_y]
            .iter()
            .all(|x| x.is_finite())
    }
}

#[derive(Clone)]
enum Segment {
    Line(Point, Point),
    Arc(Point, Point, f64),
    Cubic(Point, Point, Point, Point),
    Quad(Point, Point, Point),
}
impl Segment {
    fn start(&self) -> Point {
        match self {
            Self::Line(a, _)
            | Self::Arc(a, _, _)
            | Self::Cubic(a, _, _, _)
            | Self::Quad(a, _, _) => *a,
        }
    }
    fn end(&self) -> Point {
        match self {
            Self::Line(_, b)
            | Self::Arc(_, b, _)
            | Self::Cubic(_, _, _, b)
            | Self::Quad(_, _, b) => *b,
        }
    }
    fn reverse(self) -> Self {
        match self {
            Self::Line(a, b) => Self::Line(b, a),
            Self::Arc(a, b, t) => Self::Arc(b, a, -t),
            Self::Cubic(a, b, c, d) => Self::Cubic(d, c, b, a),
            Self::Quad(a, b, c) => Self::Quad(c, b, a),
        }
    }
    fn transform(self, t: Transform) -> Self {
        match self {
            Self::Line(a, b) => Self::Line(t.point(a), t.point(b)),
            Self::Arc(a, b, r) => Self::Arc(
                t.point(a),
                t.point(b),
                if t.determinant() < 0. { -r } else { r },
            ),
            Self::Cubic(a, b, c, d) => Self::Cubic(t.point(a), t.point(b), t.point(c), t.point(d)),
            Self::Quad(a, b, c) => Self::Quad(t.point(a), t.point(b), t.point(c)),
        }
    }
}

fn error() -> AppError {
    AppError::new("pcb_geometry", "Unsupported or malformed PCB geometry")
}
fn number(v: &Value) -> Result<f64> {
    v.as_f64()
        .filter(|n| n.is_finite() && n.abs() <= 1e8)
        .ok_or_else(error)
}
fn pt(v: &[Value], i: usize, k: f64) -> Result<Point> {
    Ok((
        number(v.get(i).ok_or_else(error)?)? * k,
        number(v.get(i + 1).ok_or_else(error)?)? * k,
    ))
}
fn f(v: f64) -> String {
    let s = format!("{:.5}", if v.abs() < 0.000005 { 0. } else { v });
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}
fn p(v: Point) -> String {
    format!("{} {}", f(v.0), f(v.1))
}

fn arc_center(a: Point, b: Point, deg: f64) -> Option<(Point, f64)> {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let chord = dx.hypot(dy);
    let theta = deg.to_radians();
    if chord < 1e-9 || theta.abs() < 1e-8 || theta.abs() >= std::f64::consts::TAU - 1e-8 {
        return None;
    }
    let h = 0.5 / (theta / 2.).tan();
    let center = ((a.0 + b.0) / 2. - dy * h, (a.1 + b.1) / 2. + dx * h);
    let radius = (a.0 - center.0).hypot(a.1 - center.1);
    if radius.is_finite() {
        Some((center, radius))
    } else {
        None
    }
}

fn flatten(s: &Segment, points: &mut Vec<Point>) -> Result<()> {
    match *s {
        Segment::Line(_, b) => points.push(b),
        Segment::Arc(a, b, deg) => {
            if let Some((c, r)) = arc_center(a, b, deg) {
                let angle = deg.to_radians();
                let start = (a.1 - c.1).atan2(a.0 - c.0);
                let step = if r > 0.05 {
                    2. * (1. - 0.05 / r).clamp(-1., 1.).acos()
                } else {
                    0.2
                };
                let count = (angle.abs() / step.max(0.001)).ceil().clamp(2., 2048.) as usize;
                for i in 1..=count {
                    let t = start + angle * i as f64 / count as f64;
                    points.push((c.0 + r * t.cos(), c.1 + r * t.sin()));
                }
            } else {
                points.push(b)
            }
        }
        Segment::Cubic(a, b, c, d) => {
            for i in 1..=32 {
                let t = i as f64 / 32.;
                let u = 1. - t;
                points.push((
                    u * u * u * a.0 + 3. * u * u * t * b.0 + 3. * u * t * t * c.0 + t * t * t * d.0,
                    u * u * u * a.1 + 3. * u * u * t * b.1 + 3. * u * t * t * c.1 + t * t * t * d.1,
                ));
            }
        }
        Segment::Quad(a, b, c) => {
            for i in 1..=24 {
                let t = i as f64 / 24.;
                let u = 1. - t;
                points.push((
                    u * u * a.0 + 2. * u * t * b.0 + t * t * c.0,
                    u * u * a.1 + 2. * u * t * b.1 + t * t * c.1,
                ));
            }
        }
    }
    if points.len() > 1_000_000 {
        return Err(AppError::new("pcb_limit", "Too many geometry vertices"));
    }
    Ok(())
}

fn make(contours: Vec<Vec<Segment>>, tf: Transform, closed: bool) -> Result<Geometry> {
    let mut out = Geometry {
        bounds: Bounds::empty(),
        ..Geometry::default()
    };
    if contours.len() > 4096 {
        return Err(AppError::new("pcb_limit", "Too many geometry contours"));
    }
    let mut prepared = Vec::new();
    let mut vertices = 0usize;
    // General affine image transforms turn circles into ellipses. Their arcs are
    // flattened; board/component transforms retain the native SVG arc commands.
    let circular = (tf.a * tf.a + tf.b * tf.b - tf.c * tf.c - tf.d * tf.d).abs() < 1e-8
        && (tf.a * tf.c + tf.b * tf.d).abs() < 1e-8;
    for segments in contours {
        if segments.is_empty() {
            continue;
        }
        let mut transformed = Vec::new();
        for segment in segments {
            if !circular && matches!(segment, Segment::Arc(..)) {
                let mut points = vec![segment.start()];
                flatten(&segment, &mut points)?;
                transformed.extend(
                    points
                        .windows(2)
                        .map(|p| Segment::Line(tf.point(p[0]), tf.point(p[1]))),
                );
            } else {
                transformed.push(segment.transform(tf));
            }
        }
        let mut segments = transformed;
        if closed && segments.last().unwrap().end() != segments[0].start() {
            segments.push(Segment::Line(
                segments.last().unwrap().end(),
                segments[0].start(),
            ));
        }
        let mut points = vec![segments[0].start()];
        for s in &segments {
            flatten(s, &mut points)?;
        }
        vertices += points.len();
        if vertices > 1_000_000 {
            return Err(AppError::new("pcb_limit", "Too many geometry vertices"));
        }
        let mut bounds = Bounds::empty();
        for p in &points {
            bounds.include(*p);
        }
        let area: f64 = points
            .windows(2)
            .map(|w| w[0].0 * w[1].1 - w[1].0 * w[0].1)
            .sum();
        prepared.push((segments, points, bounds, area));
    }
    let depths: Vec<_> = prepared
        .iter()
        .map(|(_, points, bounds, area)| {
            if !closed {
                return 0;
            }
            prepared
                .iter()
                .filter(|(_, outer, b, a)| {
                    a.abs() > area.abs()
                        && b.contains((bounds.min_x, bounds.min_y), 0.)
                        && b.contains((bounds.max_x, bounds.max_y), 0.)
                        && inside_ring(outer, points[0])
                })
                .count()
        })
        .collect();
    for (index, (mut segments, mut points, _, area)) in prepared.into_iter().enumerate() {
        if closed && (area > 0.) != (depths[index] % 2 == 0) {
            segments = segments.into_iter().rev().map(Segment::reverse).collect();
            points.reverse();
        }
        write!(out.path, "M {} ", p(segments[0].start())).unwrap();
        for s in &segments {
            match *s {
                Segment::Line(_, b) => write!(out.path, "L {} ", p(b)).unwrap(),
                Segment::Arc(a, b, t) => {
                    if let Some((_, r)) = arc_center(a, b, t) {
                        write!(
                            out.path,
                            "A {} {} 0 {} {} {} ",
                            f(r),
                            f(r),
                            i32::from(t.abs() > 180.),
                            i32::from(t > 0.),
                            p(b)
                        )
                        .unwrap()
                    } else {
                        write!(out.path, "L {} ", p(b)).unwrap()
                    }
                }
                Segment::Cubic(_, b, c, d) => {
                    out.bounds.include(b);
                    out.bounds.include(c);
                    write!(out.path, "C {} {} {} ", p(b), p(c), p(d)).unwrap()
                }
                Segment::Quad(_, b, c) => {
                    out.bounds.include(b);
                    write!(out.path, "Q {} {} ", p(b), p(c)).unwrap()
                }
            }
        }
        if closed {
            out.path.push_str("Z ");
        }
        for point in &points {
            out.bounds.include(*point);
        }
        out.contours.push(points);
    }
    if !out.bounds.finite() {
        return Err(error());
    }
    out.bounds = out.bounds.expand(0.05);
    Ok(out)
}

fn circle_segments(cx: f64, cy: f64, r: f64) -> Vec<Segment> {
    let ps = [
        (cx + r, cy),
        (cx, cy + r),
        (cx - r, cy),
        (cx, cy - r),
        (cx + r, cy),
    ];
    ps.windows(2)
        .map(|w| Segment::Arc(w[0], w[1], 90.))
        .collect()
}
fn rounded(w: f64, h: f64, r: f64) -> Vec<Segment> {
    let x = w / 2.;
    let y = h / 2.;
    let r = r.max(0.).min(x.min(y));
    if r < 1e-8 {
        return vec![
            Segment::Line((-x, -y), (x, -y)),
            Segment::Line((x, -y), (x, y)),
            Segment::Line((x, y), (-x, y)),
            Segment::Line((-x, y), (-x, -y)),
        ];
    }
    vec![
        Segment::Line((-x + r, -y), (x - r, -y)),
        Segment::Arc((x - r, -y), (x, -y + r), 90.),
        Segment::Line((x, -y + r), (x, y - r)),
        Segment::Arc((x, y - r), (x - r, y), 90.),
        Segment::Line((x - r, y), (-x + r, y)),
        Segment::Arc((-x + r, y), (-x, y - r), 90.),
        Segment::Line((-x, y - r), (-x, -y + r)),
        Segment::Arc((-x, -y + r), (-x + r, -y), 90.),
    ]
}

pub fn path(v: &Value, tf: Transform, scale: f64, closed: bool) -> Result<Geometry> {
    let array = v.as_array().filter(|a| !a.is_empty()).ok_or_else(error)?;
    let items: Vec<&[Value]> = if array[0].is_array() {
        array
            .iter()
            .map(|v| v.as_array().map(Vec::as_slice).ok_or_else(error))
            .collect::<Result<_>>()?
    } else {
        vec![array.as_slice()]
    };
    let mut contours = Vec::new();
    for item in items {
        if item.len() > 1_000_000 {
            return Err(AppError::new("pcb_limit", "PCB path too long"));
        }
        if item.first().and_then(Value::as_str) == Some("CIRCLE") {
            let c = pt(item, 1, scale)?;
            let r = number(item.get(3).ok_or_else(error)?)? * scale;
            if r <= 0. {
                return Err(error());
            }
            contours.push(circle_segments(c.0, c.1, r));
            continue;
        }
        if item.first().and_then(Value::as_str) == Some("R") {
            let c = pt(item, 1, scale)?;
            let w = number(item.get(3).ok_or_else(error)?)? * scale;
            let h = number(item.get(4).ok_or_else(error)?)? * scale;
            if w <= 0. || h <= 0. {
                return Err(error());
            }
            let angle = item.get(5).map(number).transpose()?.unwrap_or(0.);
            let radius = item.get(7).map(number).transpose()?.unwrap_or(0.) * scale;
            let local = Transform {
                a: 1.,
                b: 0.,
                c: 0.,
                d: 1.,
                tx: 0.,
                ty: 0.,
            }
            .component(c.0, c.1, angle, false)
            .component(w / 2., -h / 2., 0., false);
            contours.push(
                rounded(w, h, radius)
                    .into_iter()
                    .map(|s| s.transform(local))
                    .collect(),
            );
            continue;
        }
        let mut current = pt(item, 0, scale)?;
        let mut i = 2;
        let mut segments = Vec::new();
        while i < item.len() {
            if let Some(token) = item[i].as_str() {
                i += 1;
                match token {
                    "L" => {}
                    "ARC" | "CARC" => {
                        let angle = number(item.get(i).ok_or_else(error)?)?;
                        if angle.abs() >= 360. {
                            return Err(error());
                        }
                        let end = pt(item, i + 1, scale)?;
                        segments.push(Segment::Arc(current, end, angle));
                        current = end;
                        i += 3;
                    }
                    "C" => {
                        let a = pt(item, i, scale)?;
                        let b = pt(item, i + 2, scale)?;
                        let c = pt(item, i + 4, scale)?;
                        segments.push(Segment::Cubic(current, a, b, c));
                        current = c;
                        i += 6;
                    }
                    "Q" => {
                        let a = pt(item, i, scale)?;
                        let b = pt(item, i + 2, scale)?;
                        segments.push(Segment::Quad(current, a, b));
                        current = b;
                        i += 4;
                    }
                    _ => return Err(error()),
                }
            } else {
                let end = pt(item, i, scale)?;
                segments.push(Segment::Line(current, end));
                current = end;
                i += 2;
            }
        }
        contours.push(segments);
    }
    make(contours, tf, closed)
}

pub fn rect(
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
    radius: f64,
    angle: f64,
    tf: Transform,
) -> Result<Geometry> {
    if ![cx, cy, w, h, radius, angle]
        .iter()
        .all(|v| v.is_finite() && v.abs() < 1e8)
        || w <= 0.
        || h <= 0.
    {
        return Err(error());
    }
    make(
        vec![rounded(w, h, radius)],
        tf.component(cx, cy, angle, false),
        true,
    )
}
pub fn circle(cx: f64, cy: f64, r: f64, tf: Transform) -> Result<Geometry> {
    if ![cx, cy, r].iter().all(|v| v.is_finite() && v.abs() < 1e8) || r <= 0. {
        return Err(error());
    }
    make(vec![circle_segments(cx, cy, r)], tf, true)
}

pub fn subtract(geometry: &mut Geometry, hole: &Geometry) {
    for ring in &hole.contours {
        let reversed: Vec<_> = ring.iter().rev().copied().collect();
        if let Some(first) = reversed.first() {
            write!(geometry.path, "M {} ", p(*first)).unwrap();
            for point in reversed.iter().skip(1) {
                write!(geometry.path, "L {} ", p(*point)).unwrap();
            }
            geometry.path.push_str("Z ");
        }
        geometry.contours.push(reversed);
    }
}

pub fn distance(a: Point, b: Point, p: Point) -> f64 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let n = dx * dx + dy * dy;
    let t = if n < 1e-16 {
        0.
    } else {
        ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / n
    }
    .clamp(0., 1.);
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}
fn inside_ring(ring: &[Point], p: Point) -> bool {
    let mut inside = false;
    for edge in ring.windows(2) {
        let (a, b) = (edge[0], edge[1]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            inside = !inside;
        }
    }
    inside
}
pub fn hit(contours: &[Vec<Point>], p: Point, tolerance: f64, fill: bool, width: f64) -> bool {
    let mut winding = 0i32;
    for ring in contours {
        for edge in ring.windows(2) {
            let (a, b) = (edge[0], edge[1]);
            if distance(a, b, p) <= tolerance + width / 2. {
                return true;
            }
            if fill {
                let cross = (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1);
                if a.1 <= p.1 && b.1 > p.1 && cross > 0. {
                    winding += 1;
                } else if a.1 > p.1 && b.1 <= p.1 && cross < 0. {
                    winding -= 1;
                }
            }
        }
    }
    fill && winding != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn pcb_transform_flips_y_once_and_bottom_component_mirrors_before_rotation() {
        let t = Transform::board(0., 0.).component(100., 200., 90., true);
        let p = t.point((10., 20.));
        assert!((p.0 - 120.).abs() < 1e-8);
        assert!((p.1 + 210.).abs() < 1e-8);
    }
    #[test]
    fn path_keeps_arcs_and_bounds_include_curve_bulge() {
        let g = path(
            &json!([0, 0, "L", 10, 0, "ARC", 90, 10, 10]),
            Transform::board(0., 0.),
            1.,
            false,
        )
        .unwrap();
        assert!(g.path.contains('A'));
        assert!(g.bounds.max_x > 12.);
        assert!(g.bounds.min_y <= -10.);
        assert!(g.bounds.max_y < 0.2);
        assert!(
            path(
                &json!([0, 0, "UNKNOWN", 1, 2]),
                Transform::board(0., 0.),
                1.,
                false
            )
            .is_err()
        );
    }
    #[test]
    fn poured_scale_changes_coordinates_but_preserves_arc_angle() {
        let g = path(
            &json!([[0, 0, "ARC", 90, 1, 1, "L", 0, 0]]),
            Transform::board(0., 0.),
            10.,
            true,
        )
        .unwrap();
        assert!(g.path.contains("10 -10"));
        assert!(g.bounds.max_x < 10.2);
        assert!(g.bounds.min_y <= -10.);
    }
    #[test]
    fn rounded_pad_rotates_about_its_center() {
        let g = rect(40., 20., 20., 10., 5., 90., Transform::board(0., 0.)).unwrap();
        assert!((g.bounds.min_x - 35.).abs() < 0.2);
        assert!((g.bounds.max_x - 45.).abs() < 0.2);
        assert!((g.bounds.min_y + 30.).abs() < 0.2);
        assert!((g.bounds.max_y + 10.).abs() < 0.2);
    }

    #[test]
    fn closed_paths_normalize_disjoint_islands_and_nested_holes_for_nonzero_fill() {
        let g = path(
            &json!([
                [0, 0, "L", 10, 0, 10, 10, 0, 10, 0, 0],
                [2, 2, "L", 8, 2, 8, 8, 2, 8, 2, 2],
                [4, 4, "L", 6, 4, 6, 6, 4, 6, 4, 4],
                [20, 0, "L", 30, 0, 30, 10, 20, 10, 20, 0]
            ]),
            Transform::board(0., 0.),
            1.,
            true,
        )
        .unwrap();
        let signs: Vec<_> = g
            .contours
            .iter()
            .map(|ring| {
                ring.windows(2)
                    .map(|e| e[0].0 * e[1].1 - e[1].0 * e[0].1)
                    .sum::<f64>()
                    > 0.
            })
            .collect();
        assert_eq!(signs, vec![true, false, true, true]);
        assert!(!hit(&g.contours, (3., -3.), 0., true, 0.));
        assert!(hit(&g.contours, (5., -5.), 0., true, 0.));
        assert!(hit(&g.contours, (25., -5.), 0., true, 0.));
        let rings = vec![
            vec![(0., 0.), (10., 0.), (10., 10.), (0., 10.), (0., 0.)],
            vec![(5., 0.), (15., 0.), (15., 10.), (5., 10.), (5., 0.)],
        ];
        assert!(hit(&rings, (7., 5.), 0., true, 0.));
    }
}
