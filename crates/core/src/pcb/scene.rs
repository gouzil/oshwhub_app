use super::*;
use super::{
    geometry::{self as g, Geometry, Point, Transform},
    history::{Document, Project, Record},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    sync::{Mutex, OnceLock},
    time::Instant,
};

const HOLE_COLOR: &str = "#0b121a";
const CELL: f64 = 100.;
const MAX_OBJECTS: usize = 200_000;
struct Paint {
    result: PickResult,
    geometry: Geometry,
    fill: bool,
    width: f64,
    stage: i32,
    color: Option<String>,
    pick_layers: Vec<i32>,
    selectable: bool,
}
struct HitObject {
    result: PickResult,
    contours: Vec<Vec<Point>>,
    fill: bool,
    width: f64,
    priority: (bool, i32, usize),
    layers: Vec<i32>,
}
struct Stored {
    id: String,
    _project: Project,
    objects: Vec<HitObject>,
    grid: HashMap<(i32, i32), Vec<usize>>,
    broad: Vec<usize>,
    layers: Vec<SceneLayer>,
}
fn cache() -> &'static Mutex<VecDeque<Stored>> {
    static CACHE: OnceLock<Mutex<VecDeque<Stored>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(VecDeque::new()))
}
fn invalid() -> AppError {
    AppError::new("pcb_geometry", "图元坐标、尺寸或路径无效")
}
fn n(d: &Value, k: &str, default: f64) -> f64 {
    d[k].as_f64()
        .filter(|v| v.is_finite() && v.abs() < 1e8)
        .unwrap_or(default)
}
fn required(d: &Value, k: &str) -> Result<f64> {
    d[k].as_f64()
        .filter(|v| v.is_finite() && v.abs() < 1e8)
        .ok_or_else(invalid)
}
fn xy(d: &Value, x: &str, y: &str) -> Result<Point> {
    Ok((required(d, x)?, required(d, y)?))
}
fn position(d: &Value) -> Result<Point> {
    xy(
        d,
        if d.get("x").is_some() {
            "x"
        } else {
            "positionX"
        },
        if d.get("y").is_some() {
            "y"
        } else {
            "positionY"
        },
    )
}
fn truth(d: &Value) -> bool {
    d == true || d == 1
}
fn font_key(label: &str, family: &str, size: f64, stroke: f64, bold: bool, italic: bool) -> String {
    json!([
        label,
        family,
        size,
        stroke,
        i32::from(bold),
        i32::from(italic)
    ])
    .to_string()
}
fn text_bounds(label: &TextLabel) -> Bounds {
    let width = label.size * label.text.chars().count() as f64;
    let left = match label.align.as_str() {
        "right" => -width,
        "center" => -width / 2.,
        _ => 0.,
    };
    let tf = Transform::identity().component(label.x, label.y, label.rotation, false);
    let mut bounds = Bounds::empty();
    for point in [
        (left, -label.size),
        (left + width, -label.size),
        (left + width, label.size * 0.3),
        (left, label.size * 0.3),
    ] {
        bounds.include(tf.point(point));
    }
    bounds
}
fn text(d: &Value) -> String {
    d.as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| d.to_string())
}
fn rid(r: &Record) -> String {
    text(&r.id)
}
fn layer(d: &Value) -> i32 {
    d["layerId"]
        .as_i64()
        .filter(|v| (-1..=1024).contains(v))
        .unwrap_or(1) as i32
}
fn remap(id: i32, bottom: bool) -> i32 {
    if bottom && (1..=10).contains(&id) {
        if id % 2 == 1 { id + 1 } else { id - 1 }
    } else {
        id
    }
}
fn order(id: i32) -> i32 {
    match id {
        47 => 1000,
        11 => 900,
        3 => 800,
        4 => 700,
        12 | 0 => 650,
        1 => 600,
        2 => 400,
        15..=46 => 500 - id,
        _ => 100 + id,
    }
}
fn fallback(id: i32) -> (&'static str, &'static str) {
    match id {
        1 => ("Top Copper", "#ef5b5b"),
        2 => ("Bottom Copper", "#5a82ed"),
        3 => ("Top Silkscreen", "#f1e8c2"),
        4 => ("Bottom Silkscreen", "#d4bb55"),
        11 => ("Board Outline", "#ec9eec"),
        12 | 0 => ("Multi-Layer", "#d7bd67"),
        47 => ("Drills", HOLE_COLOR),
        5 | 6 => ("Solder Mask", "#8c53ae"),
        7 | 8 => ("Paste Mask", "#b9bdc4"),
        _ => ("Technical Layer", "#7788a4"),
    }
}
fn default_visible(id: i32) -> bool {
    matches!(id,0..=4|11|12|15..=47)
}
struct Builder<'a> {
    project: &'a Project,
    doc: &'a Document,
    tf: Transform,
    paints: Vec<Paint>,
    texts: Vec<TextLabel>,
    layers: BTreeMap<i32, SceneLayer>,
    warnings: BTreeMap<String, usize>,
    attrs: HashMap<String, BTreeMap<String, Value>>,
    pad_nets: HashMap<(String, String), String>,
    fonts: HashMap<String, &'a Value>,
    solder: Option<&'a Value>,
    paste: Option<&'a Value>,
    path_bytes: usize,
}
impl<'a> Builder<'a> {
    fn new(project: &'a Project, doc: &'a Document) -> Self {
        let canvas = doc.first("CANVAS").unwrap_or(&Value::Null);
        let mut layers = BTreeMap::new();
        let mut attrs: HashMap<String, BTreeMap<String, Value>> = HashMap::new();
        let mut pad_nets = HashMap::new();
        let mut solder = None;
        let mut paste = None;
        for r in doc.active() {
            let d = r.data.as_ref().unwrap();
            match r.kind.as_str() {
                "LAYER" => {
                    let id = layer(d);
                    let (name, color) = fallback(id);
                    let color = d["activeColor"]
                        .as_str()
                        .filter(|s| {
                            s.len() == 7
                                && s.starts_with('#')
                                && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
                        })
                        .unwrap_or(color);
                    layers.insert(
                        id,
                        SceneLayer {
                            id,
                            name: d["layerName"].as_str().unwrap_or(name).to_owned(),
                            color: color.to_owned(),
                            visible: default_visible(id) && d["show"] != false && d["show"] != 0,
                            order: order(id),
                        },
                    );
                }
                "ATTR" => {
                    if let (Some(parent), Some(key)) = (d["parentId"].as_str(), d["key"].as_str()) {
                        attrs
                            .entry(parent.to_owned())
                            .or_default()
                            .insert(key.to_owned(), d["value"].clone());
                    }
                }
                "PAD_NET" => {
                    if let Some(id) = r.id.as_array() {
                        if let (Some(c), Some(p)) = (
                            id.get(1).and_then(Value::as_str),
                            id.get(2).and_then(Value::as_str),
                        ) {
                            pad_nets.insert(
                                (c.to_owned(), p.to_owned()),
                                d["padNet"].as_str().unwrap_or("").to_owned(),
                            );
                        }
                    } else if let Some(p) = d["padNum"].as_str() {
                        pad_nets.insert(
                            (rid(r), p.to_owned()),
                            d["padNet"].as_str().unwrap_or("").to_owned(),
                        );
                    }
                }
                "RULE" => {
                    let typ =
                        r.id.as_array()
                            .and_then(|a| a.get(1))
                            .and_then(Value::as_str);
                    if d["ruleState"] == "DEFAULT" {
                        match typ {
                            Some("SOLDER") => solder = d.get("ruleContext"),
                            Some("PASTE") => paste = d.get("ruleContext"),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        let mut fonts = HashMap::new();
        for doc in project
            .docs
            .values()
            .filter(|d| d.head["docType"] == "FONT" && !d.deleted())
        {
            for r in doc.active().filter(|r| r.kind == "FONT") {
                if let Some(ids) = r.id.as_array().filter(|a| a.len() >= 3)
                    && let (Some(label), Some(family), Some(size)) =
                        (ids[0].as_str(), ids[1].as_str(), ids[2].as_f64())
                {
                    fonts.insert(
                        font_key(
                            label,
                            family,
                            size,
                            ids.get(3).and_then(Value::as_f64).unwrap_or(0.),
                            ids.get(4).is_some_and(truth),
                            ids.get(5).is_some_and(truth),
                        ),
                        r.data.as_ref().unwrap(),
                    );
                }
            }
        }
        Self {
            project,
            doc,
            tf: Transform::board(n(canvas, "originX", 0.), n(canvas, "originY", 0.)),
            paints: Vec::new(),
            texts: Vec::new(),
            layers,
            warnings: BTreeMap::new(),
            attrs,
            pad_nets,
            fonts,
            solder,
            paste,
            path_bytes: 0,
        }
    }
    fn warn(&mut self, message: impl Into<String>) {
        *self.warnings.entry(message.into()).or_default() += 1;
    }
    fn ensure_layer(&mut self, id: i32) {
        self.layers.entry(id).or_insert_with(|| {
            let (name, color) = fallback(id);
            SceneLayer {
                id,
                name: if (15..=46).contains(&id) {
                    format!("Inner {}", id - 14)
                } else {
                    name.into()
                },
                color: color.into(),
                visible: default_visible(id),
                order: order(id),
            }
        });
    }
    // A single sink keeps rendering and picking metadata consistent for every primitive.
    #[allow(clippy::too_many_arguments)]
    fn paint(
        &mut self,
        r: &Record,
        geometry: Geometry,
        lid: i32,
        fill: bool,
        width: f64,
        stage: i32,
        component: Option<&str>,
        net: Option<String>,
        selectable: bool,
        color: Option<String>,
    ) -> Result<()> {
        if self.paints.len() >= MAX_OBJECTS {
            return Err(AppError::new("pcb_limit", "场景对象超过限制"));
        }
        self.path_bytes = self.path_bytes.saturating_add(geometry.path.len());
        if self.path_bytes > 64 * 1024 * 1024 {
            return Err(AppError::new("pcb_limit", "绘图路径超过 64 MiB 限制"));
        }
        self.ensure_layer(lid);
        let id = component
            .map(|c| format!("{c}/{}", rid(r)))
            .unwrap_or_else(|| rid(r));
        let mut properties = BTreeMap::new();
        properties.insert("recordId".into(), rid(r));
        properties.insert("ticket".into(), r.header["ticket"].to_string());
        if let Some(c) = component {
            properties.insert("componentId".into(), c.to_owned());
            if let Some(attrs) = self.attrs.get(c) {
                for (k, v) in attrs.iter().take(24) {
                    properties.insert(k.clone(), text(v).chars().take(1024).collect());
                }
            }
        }
        let result = PickResult {
            id,
            kind: r.kind.clone(),
            layer: lid,
            net,
            properties,
            bounds: geometry.bounds.expand(width / 2.),
        };
        self.paints.push(Paint {
            result,
            geometry,
            fill,
            width,
            stage,
            color,
            pick_layers: vec![lid],
            selectable,
        });
        Ok(())
    }
    fn primitive(
        &mut self,
        r: &Record,
        tf: Transform,
        bottom: bool,
        component: Option<&str>,
    ) -> Result<()> {
        let d = r.data.as_ref().ok_or_else(invalid)?;
        let lid = remap(layer(d), bottom);
        let net = d["netName"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let color = d["specialColor"]
            .as_str()
            .filter(|s| {
                s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
            })
            .map(str::to_owned);
        match r.kind.as_str() {
            "LINE" | "ARC" | "CARC" => {
                let a = xy(d, "startX", "startY")?;
                let b = xy(d, "endX", "endY")?;
                let width = required(d, "width")?;
                if width <= 0. {
                    return Err(invalid());
                }
                let v = if r.kind == "LINE" {
                    json!([a.0, a.1, "L", b.0, b.1])
                } else {
                    json!([a.0, a.1, "ARC", required(d, "angle")?, b.0, b.1])
                };
                self.paint(
                    r,
                    g::path(&v, tf, 1., false)?,
                    lid,
                    false,
                    width,
                    2,
                    component,
                    net,
                    true,
                    color,
                )?;
            }
            "POLY" | "FILL" | "REGION" | "POUR" | "TEARDROP" | "FPC_FILL" => {
                let fill = matches!(r.kind.as_str(), "FILL" | "TEARDROP" | "FPC_FILL");
                let width = n(d, "width", 0.2).max(0.);
                if r.kind == "FILL"
                    && d["fillStyle"] != "SOLID"
                    && d["fillStyle"] != 0
                    && !d["fillStyle"].is_null()
                {
                    self.warn("网格 FILL 以轮廓显示");
                }
                let fill = fill
                    && (d["fillStyle"].is_null()
                        || d["fillStyle"] == "SOLID"
                        || d["fillStyle"] == 0);
                self.paint(
                    r,
                    g::path(&d["path"], tf, 1., fill || r.kind != "POLY")?,
                    lid,
                    fill,
                    width,
                    if fill { 1 } else { 2 },
                    component,
                    net,
                    true,
                    color,
                )?;
            }
            "PAD" => self.pad(r, tf, bottom, component)?,
            "VIA" => {
                let (x, y) = xy(d, "centerX", "centerY")?;
                let dia = required(d, "viaDiameter")?;
                let hole = n(d, "holeDiameter", 0.);
                let mut geometry = g::circle(x, y, dia / 2., tf)?;
                if hole > 0. {
                    let drill = g::circle(x, y, hole / 2., tf)?;
                    g::subtract(&mut geometry, &drill);
                    self.paint(
                        r,
                        drill,
                        47,
                        true,
                        0.,
                        4,
                        component,
                        net.clone(),
                        false,
                        Some(HOLE_COLOR.into()),
                    )?;
                }
                self.paint(
                    r,
                    geometry,
                    12,
                    true,
                    0.,
                    3,
                    component,
                    net.clone(),
                    true,
                    None,
                )?;
                for (face, field, key) in [
                    (5, "topSolderExpansion", "viaTopExpan"),
                    (6, "bottomSolderExpansion", "viaBotExpan"),
                ] {
                    let expansion = d[field]
                        .as_f64()
                        .or_else(|| self.solder.and_then(|v| v[key].as_f64()));
                    if let Some(e) = expansion.filter(|e| *e > -900. && dia + 2. * e > 0.) {
                        self.paint(
                            r,
                            g::circle(x, y, (dia + 2. * e) / 2., tf)?,
                            face,
                            true,
                            0.,
                            0,
                            component,
                            net.clone(),
                            false,
                            None,
                        )?;
                    }
                }
                if !d["ruleName"].as_str().unwrap_or("").is_empty() {
                    self.warn("盲埋孔按多层环显示，层跨度待校准");
                }
            }
            "POURED" => {
                let target = d["targetId"].as_str().or_else(|| {
                    r.id.as_array()
                        .and_then(|a| a.get(1))
                        .and_then(Value::as_str)
                });
                let pour = target
                    .and_then(|id| self.doc.active().find(|r| r.kind == "POUR" && r.id == id))
                    .and_then(|r| r.data.as_ref());
                let Some(pour) = pour else {
                    self.warn("POURED 缺少有效 POUR 引用，已跳过");
                    return Ok(());
                };
                let lid = layer(pour);
                let net = pour["netName"].as_str().map(str::to_owned);
                if let Some(items) = d["pourFill"].as_array() {
                    for item in items {
                        self.paint(
                            r,
                            g::path(&item["path"], tf, 10., truth(&item["fill"]))?,
                            lid,
                            truth(&item["fill"]),
                            n(item, "strokeWidth", 0.) * 10.,
                            0,
                            component,
                            net.clone(),
                            true,
                            None,
                        )?;
                    }
                } else {
                    self.paint(
                        r,
                        g::path(&d["path"], tf, 1., truth(&d["fill"]))?,
                        lid,
                        truth(&d["fill"]),
                        n(d, "strokeWidth", 0.),
                        0,
                        component,
                        net,
                        true,
                        None,
                    )?;
                }
            }
            "STRING" | "TEXT" => self.label(r, d, tf, lid, component)?,
            "ATTR" => {
                let key = d["key"].as_str().unwrap_or("");
                // Board-level attribute records already carry their final placement and
                // visibility. They replace the same key in the footprint master.
                if component.is_some_and(|c| self.attrs.get(c).is_some_and(|a| a.contains_key(key)))
                {
                    return Ok(());
                }
                let mut parts = Vec::new();
                if truth(&d["keyVisible"]) && !key.is_empty() {
                    parts.push(key.to_owned());
                }
                if truth(&d["valueVisible"]) && !d["value"].is_null() {
                    if d["value"].is_array() || d["value"].is_object() {
                        self.warn("可见 ATTR 使用复杂值，无法显示标签");
                        return Ok(());
                    }
                    let value = text(&d["value"]);
                    if !value.is_empty() {
                        parts.push(value);
                    }
                }
                if parts.is_empty() {
                    if truth(&d["keyVisible"]) || truth(&d["valueVisible"]) {
                        self.warn("可见 ATTR 缺少可显示文本，已跳过");
                    }
                    return Ok(());
                }
                let mut label = d.clone();
                label["text"] = Value::String(parts.join(": "));
                self.label(
                    r,
                    &label,
                    tf,
                    lid,
                    component.or_else(|| d["parentId"].as_str()),
                )?;
            }
            "IMAGE" => {
                if !d["path"].is_array() {
                    self.warn("未渲染依赖外部位图的 IMAGE");
                    return Ok(());
                }
                let base = g::path(&d["path"], Transform::identity(), 1., true)?;
                let mut b = Bounds::empty();
                for p in base.contours.iter().flatten() {
                    b.include(*p);
                }
                let w = required(d, "width")?;
                let h = required(d, "height")?;
                let sx = w / (b.max_x - b.min_x);
                let sy = h / (b.max_y - b.min_y);
                let mirror = truth(&d["mirror"]);
                let t = tf
                    .component(
                        required(d, "startX")?,
                        required(d, "startY")?,
                        n(d, "angle", 0.),
                        false,
                    )
                    .affine(
                        if mirror { -sx } else { sx },
                        0.,
                        0.,
                        sy,
                        if mirror { b.max_x * sx } else { -b.min_x * sx },
                        -b.max_y * sy,
                    );
                self.paint(
                    r,
                    g::path(&d["path"], t, 1., true)?,
                    lid,
                    true,
                    0.,
                    2,
                    component,
                    None,
                    true,
                    color,
                )?;
            }
            "DIMENSION" => {
                if let Some(coords) = d["coords"].as_array()
                    && coords.len() >= 4
                    && coords.len() % 2 == 0
                {
                    let mut points = coords.clone();
                    points.insert(2, Value::String("L".into()));
                    self.paint(
                        r,
                        g::path(&Value::Array(points), tf, 1., false)?,
                        lid,
                        false,
                        n(d, "strokeWidth", 1.),
                        2,
                        component,
                        None,
                        true,
                        color,
                    )?;
                }
                if d["text"].is_object() {
                    self.label(r, &d["text"], tf, lid, component)?;
                }
                self.warn("DIMENSION 标注线与文本已绘制，箭头及标注类型样式待支持");
            }
            "PAD_NET" | "META" | "META_CREATE" | "META_MODIFY" | "CANVAS" | "DELETE_DOC"
            | "LAYER" | "LAYER_PHYS" | "ACTIVE_LAYER" | "PRIMITIVE" | "NET" | "RULE"
            | "RULE_SELECTOR" | "RULE_TEMPLATE" | "PREFERENCE" | "GROUP" | "PANELIZE"
            | "SILK_OPTS" | "LAYER_FILL" | "REUSE_BLOCK" | "ELE_PLACEHOLDER" => {}
            _ => self.warn(format!("未渲染图元类型 {}，原始记录已保留", r.kind)),
        }
        Ok(())
    }
    fn pad(
        &mut self,
        r: &Record,
        tf: Transform,
        bottom: bool,
        component: Option<&str>,
    ) -> Result<()> {
        let d = r.data.as_ref().unwrap();
        let (cx, cy) = xy(d, "centerX", "centerY")?;
        let angle = n(d, "padAngle", 0.);
        let dp = &d["defaultPad"];
        let shape = dp["padType"].as_str().unwrap_or("RECT");
        let polygon = matches!(shape, "POLYGON" | "POLY");
        let width = n(dp, "width", 0.);
        let height = n(dp, "height", width);
        let radius = if matches!(shape, "OVAL" | "ROUND" | "ELLIPSE") {
            width.min(height) / 2.
        } else {
            n(dp, "radius", 0.)
        };
        let mut geometry = if polygon {
            g::path(&dp["path"], tf, 1., true)?
        } else if matches!(shape, "RECT" | "SQUARE" | "OVAL" | "ROUND" | "ELLIPSE") {
            g::rect(cx, cy, width, height, radius, angle, tf)?
        } else {
            self.warn(format!("未支持 PAD 形状 {shape}"));
            return Ok(());
        };
        let num = d["num"].as_str().unwrap_or("");
        let net = component
            .and_then(|c| self.pad_nets.get(&(c.to_owned(), num.to_owned())).cloned())
            .or_else(|| {
                d["netName"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            });
        let hw = n(&d["hole"], "width", 0.);
        let hh = n(&d["hole"], "height", hw);
        let through = hw > 0.;
        if through {
            let a = angle.to_radians();
            let ox = n(d, "padOffsetX", 0.);
            let oy = n(d, "padOffsetY", 0.);
            let hx = cx + ox * a.cos() - oy * a.sin();
            let hy = cy + ox * a.sin() + oy * a.cos();
            let round = matches!(
                d["hole"]["holeType"].as_str().unwrap_or("ROUND"),
                "ROUND" | "SLOT" | "OVAL"
            );
            let drill = g::rect(
                hx,
                hy,
                hw,
                hh,
                if round {
                    hw.min(hh) / 2.
                } else {
                    n(&d["hole"], "cornerRadius", 0.)
                },
                angle + n(d, "relativeAngle", 0.),
                tf,
            )?;
            g::subtract(&mut geometry, &drill);
            self.paint(
                r,
                drill,
                47,
                true,
                0.,
                4,
                component,
                net.clone(),
                false,
                Some(HOLE_COLOR.into()),
            )?;
        }
        let face = remap(layer(d), bottom);
        let lid = if through || face == 0 { 12 } else { face };
        self.paint(
            r,
            geometry,
            lid,
            true,
            0.,
            3,
            component,
            net.clone(),
            true,
            None,
        )?;
        if d["specialPad"].as_array().is_some_and(|s| !s.is_empty()) {
            self.warn("PAD specialPad 使用默认焊盘预览，特殊层形状待支持");
        }
        for (lid, field, key, rule) in [
            (5, "topSolderExpansion", "padTopExpan", self.solder),
            (6, "bottomSolderExpansion", "padBotExpan", self.solder),
            (7, "topPasteExpansion", "padTopExpan", self.paste),
            (8, "bottomPasteExpansion", "padBotExpan", self.paste),
        ] {
            let target = remap(lid, bottom);
            let target_face = if target % 2 == 1 { 1 } else { 2 };
            if !through && target_face != face {
                continue;
            }
            let expansion = d[field]
                .as_f64()
                .or_else(|| rule.and_then(|r| r[key].as_f64()));
            if let Some(e) = expansion.filter(|e| *e > -900.) {
                if polygon {
                    if e == 0. {
                        self.paint(
                            r,
                            g::path(&dp["path"], tf, 1., true)?,
                            target,
                            true,
                            0.,
                            0,
                            component,
                            net.clone(),
                            false,
                            None,
                        )?;
                    } else {
                        self.warn("多边形焊盘阻焊/助焊扩展待支持");
                    }
                    continue;
                }
                if width + 2. * e > 0. && height + 2. * e > 0. {
                    self.paint(
                        r,
                        g::rect(
                            cx,
                            cy,
                            width + 2. * e,
                            height + 2. * e,
                            (radius + e).max(0.),
                            angle,
                            tf,
                        )?,
                        target,
                        true,
                        0.,
                        0,
                        component,
                        net.clone(),
                        false,
                        None,
                    )?;
                }
            }
        }
        Ok(())
    }
    fn label(
        &mut self,
        r: &Record,
        d: &Value,
        tf: Transform,
        lid: i32,
        component: Option<&str>,
    ) -> Result<()> {
        let label = d["text"]
            .as_str()
            .or_else(|| d["value"].as_str())
            .unwrap_or("");
        if label.is_empty() {
            return Ok(());
        }
        if label.len() > 16_384 {
            return Err(invalid());
        }
        let (x, y) = position(d)?;
        let size = n(d, "fontSize", 30.);
        if size <= 0. {
            return Err(invalid());
        }
        let angle = n(d, "angle", n(d, "rotation", 0.));
        let font_key = font_key(
            label,
            d["fontFamily"].as_str().unwrap_or("default"),
            size / 10.,
            n(d, "strokeWidth", 0.),
            truth(&d["bold"]),
            truth(&d["italic"]),
        );
        let font = self.fonts.get(&font_key).copied();
        let origin = d["origin"]
            .as_str()
            .unwrap_or_else(|| match d["origin"].as_u64() {
                Some(0) => "LEFT_TOP",
                Some(1) => "CENTER_TOP",
                Some(2) => "RIGHT_TOP",
                Some(3) => "LEFT_MIDDLE",
                Some(4) => "CENTER_MIDDLE",
                Some(5) => "RIGHT_MIDDLE",
                Some(7) => "CENTER_BOTTOM",
                Some(8) => "RIGHT_BOTTOM",
                _ => "LEFT_BOTTOM",
            });
        let align = if origin.contains("RIGHT") {
            2.
        } else if origin.contains("CENTER") {
            1.
        } else {
            0.
        };
        if let Some(font) = font {
            let w = n(font, "width", size);
            let h = n(font, "height", size);
            let oy = if origin.contains("TOP") {
                -h
            } else if origin.contains("MIDDLE") {
                -h / 2.
            } else {
                0.
            };
            let mirror = if truth(&d["mirror"]) { -1. } else { 1. };
            let local = tf.component(x, y, angle, false).affine(
                mirror,
                0.,
                0.,
                1.,
                -mirror * w * align / 2.,
                oy,
            );
            self.paint(
                r,
                g::path(&font["path"], local, 10., true)?,
                lid,
                true,
                0.,
                3,
                component,
                None,
                true,
                None,
            )?;
        } else {
            let p = tf.point((x, y));
            let local = tf.component(x, y, angle, false);
            self.texts.push(TextLabel {
                layer: lid,
                x: p.0,
                y: p.1,
                text: label.to_owned(),
                size,
                rotation: local.b.atan2(local.a).to_degrees(),
                align: if align == 2. {
                    "right"
                } else if align == 1. {
                    "center"
                } else {
                    "left"
                }
                .into(),
            });
            self.ensure_layer(lid);
            self.warn("部分文本缺少嵌入 FONT 轮廓，使用系统字体近似");
        }
        if truth(&d["reverse"]) {
            self.warn("文字反相扩展待支持");
        }
        Ok(())
    }
    fn component(&mut self, r: &Record) -> Result<()> {
        let d = r.data.as_ref().unwrap();
        let id = rid(r);
        let attrs = self.attrs.get(&id).cloned();
        let get_attr = |key: &str| {
            attrs
                .as_ref()
                .and_then(|a| a.get(key))
                .or_else(|| d["attrs"].get(key))
                .and_then(Value::as_str)
        };
        let footprint = get_attr("Footprint").or_else(|| get_attr("Device"));
        let mut fp = footprint
            .and_then(|s| self.project.docs.get(s))
            .filter(|d| !d.deleted());
        if fp.is_some_and(|d| d.head["docType"] == "DEVICE") {
            fp = fp
                .and_then(|d| d.first("META"))
                .and_then(|v| v["attributes"]["Footprint"].as_str())
                .and_then(|s| self.project.docs.get(s));
        }
        let Some(fp) = fp.filter(|d| d.head["docType"] == "FOOTPRINT" && !d.deleted()) else {
            self.warn(format!(
                "缺少封装 {}，无法完整显示器件 {}",
                footprint.unwrap_or("unknown"),
                id
            ));
            return Ok(());
        };
        let (x, y) = position(d)?;
        let bottom = layer(d) == 2;
        let tf = self.tf.component(x, y, n(d, "angle", 0.), bottom);
        let before = self.paints.len();
        let before_texts = self.texts.len();
        for child in fp.active() {
            if let Err(e) = self.primitive(child, tf, bottom, Some(&id)) {
                if e.code == "pcb_limit" {
                    return Err(e);
                }
                self.warn(format!(
                    "封装 {} 图元 {} 无法解析",
                    footprint.unwrap_or(""),
                    child.kind
                ));
            }
        }
        let mut bounds = Bounds::empty();
        let mut pick_layers = BTreeSet::new();
        for paint in &self.paints[before..] {
            bounds.union(paint.result.bounds);
            if paint.result.layer != 47 && !paint.geometry.path.is_empty() {
                pick_layers.insert(paint.result.layer);
            }
        }
        for label in &self.texts[before_texts..] {
            bounds.union(text_bounds(label));
            pick_layers.insert(label.layer);
        }
        if bounds.finite() && !pick_layers.is_empty() {
            let rect = g::rect(
                (bounds.min_x + bounds.max_x) / 2.,
                (bounds.min_y + bounds.max_y) / 2.,
                bounds.max_x - bounds.min_x,
                bounds.max_y - bounds.min_y,
                0.,
                0.,
                Transform::board(0., 0.).affine(1., 0., 0., -1., 0., 0.),
            )?;
            let mut props = BTreeMap::new();
            if let Some(attrs) = attrs {
                for (k, v) in attrs.iter().take(24) {
                    props.insert(k.clone(), text(v).chars().take(1024).collect());
                }
            }
            self.paints.push(Paint {
                result: PickResult {
                    id,
                    kind: "COMPONENT".into(),
                    layer: layer(d),
                    net: None,
                    properties: props,
                    bounds,
                },
                geometry: Geometry {
                    path: String::new(),
                    contours: rect.contours,
                    bounds,
                },
                fill: true,
                width: 0.,
                stage: -1,
                color: None,
                pick_layers: pick_layers.into_iter().collect(),
                selectable: true,
            });
        }
        Ok(())
    }
}

pub fn compile(
    project: Project,
    document_id: &str,
    revision: String,
    mut metrics: Metrics,
) -> Result<Scene> {
    let start = Instant::now();
    let doc = project
        .docs
        .get(document_id)
        .filter(|d| d.head["docType"] == "PCB" && !d.deleted())
        .ok_or_else(|| AppError::new("pcb_not_found", "完整工程历史中未找到此 PCB"))?;
    let name = doc
        .first("META")
        .and_then(|d| d["title"].as_str())
        .unwrap_or("PCB")
        .to_owned();
    let mut b = Builder::new(&project, doc);
    for r in doc.active() {
        let result = if r.kind == "COMPONENT" {
            b.component(r)
        } else {
            b.primitive(r, b.tf, false, None)
        };
        if let Err(e) = result {
            if e.code == "pcb_limit" {
                return Err(e);
            }
            b.warn(format!("{} 图元无法解析，部分场景缺失", r.kind));
        }
    }
    let mut layers: Vec<_> = b.layers.into_values().collect();
    layers.sort_by_key(|l| l.order);
    let mut paints = b.paints;
    paints.sort_by_key(|p| (order(p.result.layer), p.stage));
    let mut batches: Vec<DrawBatch> = Vec::new();
    let mut objects = Vec::new();
    let mut bounds = Bounds::empty();
    for (sequence, paint) in paints.into_iter().enumerate() {
        let precise = !paint.geometry.path.is_empty();
        if precise {
            bounds.union(paint.result.bounds);
            let append = batches.last_mut().filter(|last| {
                last.layer == paint.result.layer
                    && last.width == paint.width
                    && last.fill == paint.fill
                    && last.color == paint.color
                    && last.path.len() + paint.geometry.path.len() < 512 * 1024
            });
            if let Some(batch) = append {
                batch.path.push_str(&paint.geometry.path);
                batch.bounds.union(paint.result.bounds);
            } else {
                batches.push(DrawBatch {
                    layer: paint.result.layer,
                    path: paint.geometry.path,
                    fill: paint.fill,
                    width: paint.width,
                    bounds: paint.result.bounds,
                    color: paint.color,
                });
            }
        }
        if paint.selectable {
            objects.push(HitObject {
                result: paint.result,
                contours: paint.geometry.contours,
                fill: paint.fill,
                width: paint.width,
                priority: (precise, paint.stage, sequence),
                layers: paint.pick_layers,
            });
        }
    }
    for t in &b.texts {
        bounds.union(text_bounds(t));
    }
    if !bounds.finite() {
        return Err(AppError::new("pcb_empty", "PCB 中没有支持的可绘制图元"));
    }
    bounds = bounds.expand(20.);
    metrics.scene_ms = start.elapsed().as_millis() as u64;
    let id = uuid::Uuid::new_v4().to_string();
    let warnings = b
        .warnings
        .into_iter()
        .map(|(s, n)| format!("{s} ({n})"))
        .collect();
    let scene = Scene {
        id: id.clone(),
        name,
        revision,
        bounds,
        layers: layers.clone(),
        batches,
        texts: b.texts,
        object_count: objects.len(),
        warnings,
        metrics,
    };
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    let mut broad = Vec::new();
    let mut entries = 0usize;
    for (i, obj) in objects.iter().enumerate() {
        let r = obj.result.bounds;
        let x0 = (r.min_x / CELL).floor() as i32;
        let x1 = (r.max_x / CELL).floor() as i32;
        let y0 = (r.min_y / CELL).floor() as i32;
        let y1 = (r.max_y / CELL).floor() as i32;
        let count = (x1 as i64 - x0 as i64 + 1) * (y1 as i64 - y0 as i64 + 1);
        if count > 256 || entries + count as usize > 2_000_000 {
            broad.push(i);
        } else {
            for x in x0..=x1 {
                for y in y0..=y1 {
                    grid.entry((x, y)).or_default().push(i);
                }
            }
            entries += count as usize;
        }
    }
    let mut cache = cache()
        .lock()
        .map_err(|_| AppError::new("internal", "场景缓存不可用"))?;
    cache.push_back(Stored {
        id,
        _project: project,
        objects,
        grid,
        broad,
        layers,
    });
    while cache.len() > 2 {
        cache.pop_front();
    }
    Ok(scene)
}

pub fn pick(
    scene_id: &str,
    x: f64,
    y: f64,
    tolerance: f64,
    layers: Option<&[i32]>,
) -> Result<Option<PickResult>> {
    if ![x, y, tolerance].iter().all(|v| v.is_finite()) || tolerance < 0. || tolerance > 1e6 {
        return Err(AppError::new("invalid_request", "拾取参数无效"));
    }
    let cache = cache()
        .lock()
        .map_err(|_| AppError::new("internal", "场景缓存不可用"))?;
    let s = cache
        .iter()
        .find(|s| s.id == scene_id)
        .ok_or_else(|| AppError::new("scene_expired", "场景已过期，请重新打开 PCB"))?;
    let allowed: HashSet<_> = layers
        .map(|l| l.iter().copied().collect())
        .unwrap_or_else(|| {
            s.layers
                .iter()
                .filter(|l| l.visible)
                .map(|l| l.id)
                .collect()
        });
    let p = (x, y);
    let x0 = ((x - tolerance) / CELL).floor() as i32;
    let x1 = ((x + tolerance) / CELL).floor() as i32;
    let y0 = ((y - tolerance) / CELL).floor() as i32;
    let y1 = ((y + tolerance) / CELL).floor() as i32;
    let mut candidates: HashSet<usize> = s.broad.iter().copied().collect();
    if (x1 as i64 - x0 as i64 + 1) * (y1 as i64 - y0 as i64 + 1) > 4096 {
        candidates.extend(0..s.objects.len());
    } else {
        for x in x0..=x1 {
            for y in y0..=y1 {
                if let Some(ids) = s.grid.get(&(x, y)) {
                    candidates.extend(ids);
                }
            }
        }
    }
    let found = candidates
        .into_iter()
        .map(|i| &s.objects[i])
        .filter(|o| {
            o.layers.iter().any(|l| allowed.contains(l))
                && o.result.bounds.contains(p, tolerance)
                && g::hit(&o.contours, p, tolerance, o.fill, o.width)
        })
        .max_by_key(|o| {
            (
                o.priority.0,
                o.layers
                    .iter()
                    .filter(|l| allowed.contains(l))
                    .map(|l| order(*l))
                    .max()
                    .unwrap_or(i32::MIN),
                o.priority.1,
                o.priority.2,
            )
        });
    Ok(found.map(|o| {
        let mut result = o.result.clone();
        result.layer = *o
            .layers
            .iter()
            .filter(|l| allowed.contains(l))
            .max_by_key(|l| order(**l))
            .expect("filtered visible candidate");
        result
    }))
}

pub fn demo() -> Scene {
    let log = r#"{"type":"DOCHEAD"}||{"docType":"PCB","uuid":"demo","client":"local"}|
{"type":"META","ticket":1}||{"title":"二维 PCB 引擎演示"}|
{"type":"POLY","id":"outline","ticket":1}||{"layerId":11,"width":2,"path":[0,0,"L",400,0,400,240,0,240,0,0]}|
{"type":"LINE","id":"trace-1","ticket":1}||{"layerId":1,"netName":"3V3","startX":80,"startY":80,"endX":160,"endY":160,"width":8}|
{"type":"ARC","id":"arc","ticket":1}||{"layerId":2,"netName":"GND","startX":160,"startY":160,"endX":300,"endY":80,"angle":-90,"width":8}|
{"type":"VIA","id":"via","ticket":1}||{"centerX":160,"centerY":160,"viaDiameter":20,"holeDiameter":8,"netName":"GND"}|
{"type":"PAD","id":"pad","ticket":1}||{"layerId":1,"centerX":80,"centerY":80,"num":"1","defaultPad":{"padType":"RECT","width":40,"height":28,"radius":4},"padAngle":0,"netName":"3V3"}|
{"type":"STRING","id":"name","ticket":1}||{"layerId":3,"x":30,"y":210,"fontSize":20,"text":"SMT / RUST CORE"}|"#;
    let mut p = Project::default();
    history::replay(&mut p, log).expect("authored demo records");
    compile(p, "demo", "local-demo".into(), Metrics::default()).expect("authored demo scene")
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = r#"{"type":"DOCHEAD"}||{"uuid":"fp","docType":"FOOTPRINT","client":"a"}|
{"type":"PAD","id":"p1","ticket":1}||{"num":"1","layerId":1,"centerX":10,"centerY":0,"padAngle":0,"defaultPad":{"padType":"RECT","width":8,"height":6,"radius":1},"hole":{"holeType":"ROUND","width":2,"height":2}}|
{"type":"DOCHEAD"}||{"uuid":"board","docType":"PCB","client":"a"}|
{"type":"META","ticket":1}||{"title":"Authored PCB fixture"}|
{"type":"COMPONENT","id":"c1","ticket":1}||{"x":100,"y":100,"angle":90,"layerId":1}|
{"type":"ATTR","id":"fpref","ticket":1}||{"parentId":"c1","key":"Footprint","value":"fp","valueVisible":false}|
{"type":"PAD_NET","id":"[\"PAD_NET\",\"c1\",\"1\",\"p1\"]","ticket":1}||{"padNet":"GND"}|
{"type":"LINE","id":"trace","ticket":1}||{"startX":100,"startY":110,"endX":100,"endY":150,"width":3,"layerId":1,"netName":"GND"}|
{"type":"LINE","id":"hidden","ticket":1}||{"startX":300,"startY":100,"endX":320,"endY":100,"width":3,"layerId":15}|
{"type":"LAYER","id":["LAYER",15],"ticket":1}||{"layerId":15,"layerName":"Hidden Inner1","show":false}|
{"type":"VIA","id":"via","ticket":1}||{"centerX":130,"centerY":120,"viaDiameter":8,"holeDiameter":3,"netName":"GND"}|
{"type":"FILL","id":"fill","ticket":1}||{"layerId":1,"fillStyle":"SOLID","path":[[150,100,"L",200,100,200,150,150,150,150,100],[160,110,"L",190,110,190,140,160,140,160,110]]}|
{"type":"POUR","id":"pour","ticket":1}||{"layerId":2,"netName":"GND","path":[[200,100,"L",210,100,210,110,200,110,200,100]]}|
{"type":"POURED","id":["POURED","pour"],"ticket":1}||{"pourFill":[{"fill":true,"strokeWidth":0,"path":[[20,10,"L",21,10,21,11,20,11,20,10]]}]}|
{"type":"STRING","id":"label","ticket":1}||{"layerId":3,"x":120,"y":180,"text":"TEST","fontSize":12,"angle":30}|
{"type":"FANCY","id":"future","ticket":1}||{"important":true}|
{"type":"LINE","id":"bottom-track","ticket":1}||{"layerId":2,"startX":500,"startY":-5,"endX":500,"endY":5,"width":1}|
{"type":"COMPONENT","id":"silk-only","ticket":1}||{"layerId":1,"x":400,"y":0,"attrs":{"Footprint":"silk-fp"}}|
{"type":"COMPONENT","id":"top-bbox","ticket":1}||{"layerId":1,"x":500,"y":0,"attrs":{"Footprint":"pads-fp"}}|
{"type":"COMPONENT","id":"bottom-attr","ticket":1}||{"layerId":2,"x":600,"y":100,"attrs":{"Footprint":"attrs-fp"}}|
{"type":"ATTR","id":"ref-visible","ticket":1}||{"parentId":"c1","key":"Designator","value":"R1","valueVisible":1,"x":120,"y":100,"fontSize":10,"layerId":3}|
{"type":"ATTR","id":"key-only","ticket":1}||{"key":"Voltage","value":"3.3","keyVisible":true,"valueVisible":false,"x":130,"y":100,"fontSize":10,"layerId":3}|
{"type":"ATTR","id":"both-visible","ticket":1}||{"key":"Value","value":"10k","keyVisible":true,"valueVisible":true,"x":140,"y":100,"fontSize":10,"layerId":3}|
{"type":"ATTR","id":"empty-visible","ticket":1}||{"key":"","value":"","valueVisible":true,"x":150,"y":100,"fontSize":10,"layerId":3}|
{"type":"ATTR","id":"missing-location","ticket":1}||{"key":"Designator","value":"BAD","valueVisible":true,"fontSize":10,"layerId":3}|
{"type":"ATTR","id":"hide-master","ticket":1}||{"parentId":"top-bbox","key":"Designator","value":"U1","valueVisible":false}|
{"type":"ATTR","id":"board-value","ticket":1}||{"parentId":"bottom-attr","key":"Value","value":"B1","valueVisible":true,"x":620,"y":110,"angle":90,"fontSize":10,"layerId":4}|
{"type":"STRING","id":"rotated","ticket":1}||{"layerId":3,"x":1000,"y":1000,"text":"ABCDEFGHIJKLMNOP","fontSize":10,"angle":90}|
{"type":"DOCHEAD"}||{"uuid":"silk-fp","docType":"FOOTPRINT"}|
{"type":"LINE","id":"silk","ticket":1}||{"layerId":3,"startX":-10,"startY":0,"endX":10,"endY":0,"width":1}|
{"type":"DOCHEAD"}||{"uuid":"pads-fp","docType":"FOOTPRINT"}|
{"type":"PAD","id":"left","ticket":1}||{"layerId":1,"centerX":-10,"centerY":0,"defaultPad":{"padType":"RECT","width":2,"height":2}}|
{"type":"PAD","id":"right","ticket":1}||{"layerId":1,"centerX":10,"centerY":0,"defaultPad":{"padType":"RECT","width":2,"height":2}}|
{"type":"ATTR","id":"master-ref","ticket":1}||{"key":"Designator","value":"X?","valueVisible":true,"x":0,"y":-3,"fontSize":10,"layerId":3}|
{"type":"DOCHEAD"}||{"uuid":"attrs-fp","docType":"FOOTPRINT"}|
{"type":"ATTR","id":"master-value","ticket":1}||{"key":"Value","value":"DEFAULT","valueVisible":true,"x":0,"y":0,"fontSize":10,"layerId":3}|
{"type":"ATTR","id":"master-description","ticket":1}||{"key":"Description","value":"MASTER","valueVisible":true,"x":10,"y":20,"fontSize":10,"layerId":3}|"#;
    fn fixture_scene() -> &'static Scene {
        static SCENE: OnceLock<Scene> = OnceLock::new();
        SCENE.get_or_init(|| {
            let mut p = Project::default();
            history::replay(&mut p, FIXTURE).unwrap();
            compile(p, "board", "fixture".into(), Metrics::default()).unwrap()
        })
    }
    #[test]
    fn scene_resolves_footprints_pad_nets_pours_and_geometry_picking_without_filling_holes() {
        let s = fixture_scene();
        assert!(s.object_count >= 7);
        assert!(
            s.batches
                .iter()
                .any(|b| b.layer == 2 && b.fill && b.bounds.min_x >= 199.9)
        );
        assert!(s.warnings.iter().any(|s| s.contains("FANCY")));
        assert!(s.texts.iter().any(|t| t.text == "TEST"
            && t.x == 120.
            && t.y == -180.
            && (t.rotation + 30.).abs() < 1e-8));
        let found = pick(&s.id, 100., -111.5, 0.1, Some(&[1, 12]))
            .unwrap()
            .unwrap();
        assert_eq!(found.kind, "PAD");
        assert_eq!(found.net.as_deref(), Some("GND"));
        assert!(found.id.contains("c1"));
        assert!(
            pick(&s.id, 175., -125., 0.01, Some(&[1]))
                .unwrap()
                .is_none()
        );
        assert!(
            pick(&s.id, 154., -104., 0.01, Some(&[1]))
                .unwrap()
                .is_some()
        );
        assert!(pick(&s.id, 310., -100., 0.1, None).unwrap().is_none());
        assert_eq!(
            pick(&s.id, 310., -100., 0.1, Some(&[15]))
                .unwrap()
                .unwrap()
                .id,
            "hidden"
        );
        assert!(
            pick(&s.id, 154., -104., 0.01, Some(&[2]))
                .unwrap()
                .is_none()
        );
        assert!(
            pick(&s.id, 133., -120., 0.01, Some(&[1]))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            pick(&s.id, 133., -120., 0.01, Some(&[12]))
                .unwrap()
                .unwrap()
                .id,
            "via"
        );
    }

    #[test]
    fn component_bbox_is_not_pickable_when_all_its_actual_layers_are_hidden() {
        let s = fixture_scene();
        assert!(pick(&s.id, 400., 0., 0.01, Some(&[1])).unwrap().is_none());
        // The component's empty space is selectable on its actual silkscreen layer.
        let found = pick(&s.id, 400., 0.54, 0.01, Some(&[3])).unwrap().unwrap();
        assert_eq!(found.layer, 3);
    }

    #[test]
    fn actual_bottom_track_has_priority_over_empty_top_component_bbox() {
        let s = fixture_scene();
        let found = pick(&s.id, 500., 0., 0.01, Some(&[1, 2])).unwrap().unwrap();
        assert_eq!(
            (found.kind.as_str(), found.id.as_str()),
            ("LINE", "bottom-track")
        );
        assert_eq!(
            pick(&s.id, 500., 0., 0.01, Some(&[1]))
                .unwrap()
                .unwrap()
                .kind,
            "COMPONENT"
        );
    }

    #[test]
    fn rotated_fallback_text_extents_are_inside_scene_bounds() {
        let s = fixture_scene();
        assert!(s.bounds.min_y <= -1160.);
    }

    #[test]
    fn visible_attributes_draw_instance_and_inherited_labels_without_restoring_hidden_master_values()
     {
        let s = fixture_scene();
        assert!(
            s.texts
                .iter()
                .any(|t| t.text == "R1" && t.x == 120. && t.y == -100. && t.layer == 3)
        );
        assert!(s.texts.iter().any(|t| t.text == "Voltage"));
        assert!(s.texts.iter().any(|t| t.text == "Value: 10k"));
        assert!(s.texts.iter().any(|t| t.text == "B1"
            && t.x == 620.
            && t.y == -110.
            && t.layer == 4
            && (t.rotation + 90.).abs() < 1e-8));
        assert!(
            s.texts
                .iter()
                .any(|t| t.text == "MASTER" && t.x == 610. && t.y == -80. && t.layer == 4)
        );
        assert!(
            !s.texts
                .iter()
                .any(|t| matches!(t.text.as_str(), "X?" | "U1" | "DEFAULT" | "BAD"))
        );
        assert!(s.warnings.iter().any(|w| w.contains("ATTR")));
        assert!(
            s.warnings
                .iter()
                .any(|w| w.contains("可见 ATTR 缺少可显示文本"))
        );
        assert!(s.warnings.iter().any(|w| w.contains("ATTR 图元无法解析")));
        assert!(s.warnings.iter().any(|w| w.contains("缺少嵌入 FONT")));
    }

    #[test]
    fn polygon_pad_zero_expansion_draws_the_original_shape_on_the_mask_layer() {
        let mut p = Project::default();
        history::replay(&mut p,r#"{"type":"DOCHEAD"}||{"uuid":"board","docType":"PCB"}|
{"type":"PAD","id":"polygon","ticket":1}||{"layerId":1,"centerX":0,"centerY":0,"defaultPad":{"padType":"POLYGON","path":[-5,-5,"L",5,-5,5,5,-5,5,-5,-5]},"topSolderExpansion":0}|"#).unwrap();
        let d = &p.docs["board"];
        let mut b = Builder::new(&p, d);
        b.primitive(d.active().next().unwrap(), b.tf, false, None)
            .unwrap();
        let copper = b.paints.iter().find(|p| p.result.layer == 1).unwrap();
        let mask = b
            .paints
            .iter()
            .find(|p| p.result.layer == 5)
            .expect("zero expansion must preserve polygon shape");
        assert_eq!(mask.geometry.path, copper.geometry.path);
        assert!(b.warnings.is_empty());
    }

    #[test]
    fn deleted_device_cannot_restore_its_live_referenced_footprint() {
        let mut p = Project::default();
        history::replay(&mut p,r#"{"type":"DOCHEAD"}||{"uuid":"board","docType":"PCB"}|
{"type":"COMPONENT","id":"c","ticket":1}||{"layerId":1,"x":0,"y":0,"attrs":{"Device":"device"}}|
{"type":"DOCHEAD"}||{"uuid":"device","docType":"DEVICE"}|
{"type":"META","ticket":1}||{"attributes":{"Footprint":"fp"}}|
{"type":"DELETE_DOC","ticket":2}||{"isDelete":true}|
{"type":"DOCHEAD"}||{"uuid":"fp","docType":"FOOTPRINT"}|
{"type":"PAD","id":"p","ticket":1}||{"layerId":1,"centerX":0,"centerY":0,"defaultPad":{"padType":"RECT","width":2,"height":2}}|"#).unwrap();
        let d = &p.docs["board"];
        let mut b = Builder::new(&p, d);
        b.component(d.active().next().unwrap()).unwrap();
        assert!(b.paints.is_empty());
        assert!(b.warnings.keys().any(|s| s.contains("缺少封装")));
    }

    #[test]
    fn vector_image_scales_about_source_bounds_and_anchors_at_top_left() {
        let mut p = Project::default();
        history::replay(&mut p, r#"{"type":"DOCHEAD"}||{"uuid":"board","docType":"PCB"}|
{"type":"IMAGE","id":"image","ticket":1}||{"layerId":3,"startX":200,"startY":300,"width":100,"height":60,"mirror":true,"path":[[10,20,"L",30,20,10,40,10,20]]}|"#).unwrap();
        let d = &p.docs["board"];
        let mut b = Builder::new(&p, d);
        b.primitive(d.active().next().unwrap(), b.tf, false, None)
            .unwrap();
        let g = &b.paints[0].geometry;
        assert!((g.bounds.min_x - 200.).abs() < 0.1);
        assert!((g.bounds.max_x - 300.).abs() < 0.1);
        assert!((g.bounds.min_y + 300.).abs() < 0.1);
        assert!((g.bounds.max_y + 240.).abs() < 0.1);
        assert!(g::hit(&g.contours, (290., -290.), 0., true, 0.));
        assert!(!g::hit(&g.contours, (210., -290.), 0., true, 0.));
    }

    #[test]
    fn embedded_font_uses_normalized_numeric_keys_and_numeric_center_origin() {
        let mut p = Project::default();
        history::replay(&mut p,r#"{"type":"DOCHEAD"}||{"uuid":"font","docType":"FONT"}|
{"type":"FONT","id":["AB","fixture",1,0,0,0,0,0],"ticket":1}||{"width":20,"height":10,"path":[[0,0,"L",0.8,0,0.8,1,0,1,0,0],[1.2,0,"L",2,0,2,1,1.2,1,1.2,0]]}|
{"type":"DOCHEAD"}||{"uuid":"board","docType":"PCB"}|
{"type":"STRING","id":"label","ticket":1}||{"x":100,"y":200,"fontSize":10,"fontFamily":"fixture","text":"AB","origin":4,"layerId":3}|
{"type":"ATTR","id":"ref","ticket":1}||{"x":300,"y":200,"fontSize":10,"fontFamily":"fixture","key":"Designator","value":"AB","valueVisible":true,"origin":4,"layerId":3}|"#).unwrap();
        let d = &p.docs["board"];
        let mut b = Builder::new(&p, d);
        b.primitive(
            d.active().find(|r| r.kind == "STRING").unwrap(),
            b.tf,
            false,
            None,
        )
        .unwrap();
        assert!(b.texts.is_empty());
        assert_eq!(b.paints.len(), 1);
        let geometry = &b.paints[0].geometry;
        assert!((geometry.bounds.min_x - 90.).abs() < 0.1);
        assert!((geometry.bounds.max_x - 110.).abs() < 0.1);
        assert!((geometry.bounds.min_y + 205.).abs() < 0.1);
        assert!((geometry.bounds.max_y + 195.).abs() < 0.1);
        b.primitive(
            d.active().find(|r| r.kind == "ATTR").unwrap(),
            b.tf,
            false,
            None,
        )
        .unwrap();
        assert!(b.texts.is_empty());
        assert_eq!(b.paints.len(), 2);
        assert_eq!(b.paints[1].result.kind, "ATTR");
        assert!((b.paints[1].geometry.bounds.min_x - 290.).abs() < 0.1);
    }
}
