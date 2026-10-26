use crate::{
    client::HttpClient,
    error::{AppError, Result},
    pcb::{
        self, Bounds, DrawBatch, Metrics, PickResult, Scene, SceneLayer, TextLabel, geometry,
        history,
    },
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

const CELL: f64 = 100.;
const MAX_OBJECTS: usize = 200_000;
const MAX_PATH_BYTES: usize = 64 * 1024 * 1024;

struct Paint {
    result: PickResult,
    geometry: geometry::Geometry,
    fill: bool,
    width: f64,
    stage: i32,
    color: Option<String>,
    selectable: bool,
}

struct HitObject {
    result: PickResult,
    contours: Vec<Vec<geometry::Point>>,
    fill: bool,
    width: f64,
    priority: (bool, i32, usize),
    layers: Vec<i32>,
}

struct Stored {
    id: String,
    objects: Vec<HitObject>,
    grid: HashMap<(i32, i32), Vec<usize>>,
    broad: Vec<usize>,
    layers: Vec<SceneLayer>,
}

fn cache() -> &'static Mutex<VecDeque<Arc<Stored>>> {
    static CACHE: OnceLock<Mutex<VecDeque<Arc<Stored>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub async fn load(client: &HttpClient, project_id: &str, document_id: &str) -> Result<Scene> {
    crate::api::validate_id(project_id)?;
    crate::api::validate_id(document_id)?;
    let document_id = document_id.to_owned();
    let (project, revision, metrics) = pcb::load_project(client, project_id, &document_id).await?;
    tokio::task::spawn_blocking(move || compile(project, &document_id, revision, metrics))
        .await
        .map_err(|_| AppError::new("internal", "原理图场景处理失败"))?
}

fn invalid() -> AppError {
    AppError::new("schematic_format", "原理图图元坐标、尺寸或路径无效")
}

fn text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

fn record_id(record: &history::Record) -> String {
    record
        .id
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| record.id.to_string())
}

fn number(value: &Value, key: &str, default: f64) -> f64 {
    value[key]
        .as_f64()
        .filter(|v| v.is_finite() && v.abs() < 1e8)
        .unwrap_or(default)
}

fn required(value: &Value, key: &str) -> Result<f64> {
    value[key]
        .as_f64()
        .filter(|v| v.is_finite() && v.abs() < 1e8)
        .ok_or_else(invalid)
}

fn truth(value: &Value) -> bool {
    value == true || value == 1 || value.as_str() == Some("true")
}

fn position(value: &Value) -> Result<geometry::Point> {
    let x = if value.get("x").is_some() {
        required(value, "x")?
    } else if value.get("pointX").is_some() {
        required(value, "pointX")?
    } else {
        required(value, "positionX")?
    };
    let y = if value.get("y").is_some() {
        required(value, "y")?
    } else if value.get("pointY").is_some() {
        required(value, "pointY")?
    } else {
        required(value, "positionY")?
    };
    Ok((x, y))
}

fn valid_color(value: &Value) -> Option<String> {
    let color = value.as_str()?;
    (color.len() == 7
        && color.starts_with('#')
        && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit))
    .then(|| color.to_owned())
}

fn width(value: &Value, keys: &[&str], default: f64) -> f64 {
    keys.iter()
        .filter_map(|key| {
            value[key]
                .as_f64()
                .filter(|v| v.is_finite() && v.abs() < 1e8 && *v > 0.)
        })
        .next()
        .unwrap_or(default)
}

fn points(value: &Value) -> Result<Vec<f64>> {
    let array = value.as_array().ok_or_else(invalid)?;
    if array.iter().all(Value::is_number) {
        if array.len() < 4 || array.len() % 2 != 0 {
            return Err(invalid());
        }
        return array
            .iter()
            .map(|v| {
                v.as_f64()
                    .filter(|n| n.is_finite() && n.abs() < 1e8)
                    .ok_or_else(invalid)
            })
            .collect();
    }
    let mut flat = Vec::with_capacity(array.len() * 2);
    for point in array {
        let point = point.as_array().ok_or_else(invalid)?;
        if point.len() < 2 {
            return Err(invalid());
        }
        let values = if point.len() == 2 {
            &point[..2]
        } else {
            &point[..]
        };
        if values.len() % 2 != 0 {
            return Err(invalid());
        }
        for value in values {
            flat.push(
                value
                    .as_f64()
                    .filter(|n| n.is_finite() && n.abs() < 1e8)
                    .ok_or_else(invalid)?,
            );
        }
    }
    if flat.len() < 4 {
        Err(invalid())
    } else {
        Ok(flat)
    }
}

fn path_value(value: &Value, key: &str) -> Option<Value> {
    value
        .get(key)
        .cloned()
        .or_else(|| value.get("points").cloned())
        .or_else(|| value.get("dots").cloned())
}

fn text_bounds(label: &TextLabel) -> Bounds {
    let width = label.size * label.text.chars().count() as f64 * 0.6;
    let left = match label.align.as_str() {
        "right" => -width,
        "center" => -width / 2.,
        _ => 0.,
    };
    let tf = geometry::Transform::identity().component(label.x, label.y, label.rotation, false);
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

struct Builder<'a> {
    project: &'a history::Project,
    page: &'a history::Document,
    page_attrs: HashMap<String, BTreeMap<String, Value>>,
    symbol_attrs: HashMap<String, BTreeMap<String, Value>>,
    paints: Vec<Paint>,
    texts: Vec<TextLabel>,
    layers: BTreeMap<i32, SceneLayer>,
    warnings: BTreeMap<String, usize>,
    path_bytes: usize,
}

impl<'a> Builder<'a> {
    fn new(project: &'a history::Project, page: &'a history::Document) -> Self {
        let mut page_attrs = HashMap::new();
        Self::index_attrs(page, &mut page_attrs);
        let mut symbol_attrs = HashMap::new();
        for doc in project
            .docs
            .values()
            .filter(|doc| doc.head["docType"] == "SYMBOL" && !doc.deleted())
        {
            Self::index_attrs(doc, &mut symbol_attrs);
        }
        Self {
            project,
            page,
            page_attrs,
            symbol_attrs,
            paints: Vec::new(),
            texts: Vec::new(),
            layers: BTreeMap::new(),
            warnings: BTreeMap::new(),
            path_bytes: 0,
        }
    }

    fn index_attrs(doc: &history::Document, output: &mut HashMap<String, BTreeMap<String, Value>>) {
        for record in doc.active().filter(|record| record.kind == "ATTR") {
            let Some(data) = record.data.as_ref() else {
                continue;
            };
            let Some(parent) = data["parentId"].as_str() else {
                continue;
            };
            let Some(key) = data["key"].as_str().or_else(|| data["name"].as_str()) else {
                continue;
            };
            output
                .entry(parent.to_owned())
                .or_default()
                .insert(key.to_owned(), data.clone());
        }
    }

    fn warn(&mut self, message: impl Into<String>) {
        *self.warnings.entry(message.into()).or_default() += 1;
    }

    fn ensure_layer(&mut self, id: i32, name: &'static str, color: &'static str, order: i32) {
        self.layers.entry(id).or_insert_with(|| SceneLayer {
            id,
            name: name.to_owned(),
            color: color.to_owned(),
            visible: true,
            order,
        });
    }

    fn attr_value<'b>(attrs: Option<&'b BTreeMap<String, Value>>, key: &str) -> Option<&'b Value> {
        attrs.and_then(|values| values.get(key)).and_then(|data| {
            data.get("value")
                .or_else(|| data.get("text"))
                .or_else(|| data.get("content"))
        })
    }

    fn properties(
        &self,
        record: &history::Record,
        parent: Option<&str>,
    ) -> BTreeMap<String, String> {
        let mut properties = BTreeMap::new();
        properties.insert("recordId".into(), record_id(record));
        properties.insert(
            "ticket".into(),
            if record.has_ticket {
                record.ticket.to_string()
            } else {
                "null".into()
            },
        );
        if let Some(parent) = parent {
            properties.insert("componentId".into(), parent.to_owned());
        }
        if let Some(attrs) = parent.and_then(|id| self.page_attrs.get(id)) {
            for (key, value) in attrs.iter().take(24) {
                if let Some(value) = value.get("value") {
                    properties.insert(key.to_owned(), text(value).chars().take(1024).collect());
                }
            }
        }
        properties
    }

    // A single sink keeps rendering and picking metadata consistent for every primitive.
    #[allow(clippy::too_many_arguments)]
    fn add_paint(
        &mut self,
        record: &history::Record,
        geometry: geometry::Geometry,
        layer: i32,
        fill: bool,
        width: f64,
        stage: i32,
        parent: Option<&str>,
        net: Option<String>,
        color: Option<String>,
    ) -> Result<()> {
        if self.paints.len() >= MAX_OBJECTS {
            return Err(AppError::new("pcb_limit", "场景对象超过限制"));
        }
        self.path_bytes = self.path_bytes.saturating_add(geometry.path.len());
        if self.path_bytes > MAX_PATH_BYTES {
            return Err(AppError::new("pcb_limit", "绘图路径超过 64 MiB 限制"));
        }
        let (name, default_color, order) = match layer {
            1 => ("连线", "#5ca9e6", 100),
            2 => ("总线", "#45b7b0", 200),
            3 => ("符号", "#e8d38b", 300),
            4 => ("引脚", "#e38c8c", 400),
            5 => ("文本", "#ffffff", 500),
            6 => ("形状", "#95c77b", 600),
            7 => ("连接点", "#f2c14e", 700),
            _ => ("原理图", "#aab4c0", 800),
        };
        self.ensure_layer(layer, name, default_color, order);
        let mut properties = self.properties(record, parent);
        if let Some(parent) = parent
            && let Some(attrs) = self.page_attrs.get(parent)
        {
            for key in ["Device", "Symbol", "Designator", "Value"] {
                if let Some(value) = Self::attr_value(Some(attrs), key) {
                    properties.insert(key.into(), text(value).chars().take(1024).collect());
                }
            }
        }
        let id = parent
            .map(|parent| format!("{parent}/{}", record_id(record)))
            .unwrap_or_else(|| record_id(record));
        self.paints.push(Paint {
            result: PickResult {
                id,
                kind: record.kind.clone(),
                layer,
                net,
                properties,
                bounds: geometry.bounds.expand(width / 2.),
            },
            geometry,
            fill,
            width,
            stage,
            color: color.or_else(|| Some(default_color.to_owned())),
            selectable: true,
        });
        Ok(())
    }

    fn add_text(
        &mut self,
        record: &history::Record,
        value: &Value,
        tf: geometry::Transform,
        parent: Option<&str>,
    ) -> Result<()> {
        let label = value["value"]
            .as_str()
            .or_else(|| value["text"].as_str())
            .or_else(|| value["content"].as_str())
            .unwrap_or("");
        if label.is_empty() {
            return Ok(());
        }
        if label.len() > 16_384 {
            return Err(invalid());
        }
        let (x, y) = position(value)?;
        let size = width(value, &["fontSize", "size"], 6.).max(0.1);
        let angle = number(value, "rotation", number(value, "angle", 0.));
        let local = tf.component(x, y, angle, truth(&value["isMirror"]));
        let align = match value["align"].as_str() {
            Some("center") | Some("CENTER") => "center",
            Some("right") | Some("RIGHT") => "right",
            _ => "left",
        };
        let point = local.point((0., 0.));
        self.ensure_layer(5, "文本", "#ffffff", 500);
        self.texts.push(TextLabel {
            layer: 5,
            x: point.0,
            y: point.1,
            text: label.to_owned(),
            size,
            rotation: local.b.atan2(local.a).to_degrees(),
            align: align.into(),
        });
        let text_width = size * label.chars().count() as f64 * 0.6;
        let text_geometry = geometry::rect(
            x + if align == "right" {
                -text_width
            } else if align == "center" {
                -text_width / 2.
            } else {
                0.
            },
            y,
            text_width.max(size),
            size,
            0.,
            angle,
            tf,
        )?;
        self.add_paint(
            record,
            text_geometry,
            5,
            true,
            0.,
            0,
            parent,
            None,
            valid_color(&value["color"]),
        )?;
        Ok(())
    }

    fn net(&self, record_id: &str) -> Option<String> {
        self.page_attrs
            .get(record_id)
            .and_then(|attrs| {
                ["NET", "Net", "net"]
                    .iter()
                    .find_map(|key| Self::attr_value(Some(attrs), key))
            })
            .map(text)
            .filter(|value| !value.is_empty())
    }

    fn page_primitive(&mut self, record: &history::Record) -> Result<()> {
        let data = record.data.as_ref().ok_or_else(invalid)?;
        let id = record_id(record);
        match record.kind.as_str() {
            "WIRE" | "BUS" => {
                let value = path_value(data, "dots").or_else(|| path_value(data, "path"));
                let Some(value) = value else {
                    return Err(invalid());
                };
                let path = points(&value)?;
                let width = width(
                    data,
                    &["strokeWidth", "width"],
                    if record.kind == "BUS" { 2. } else { 1. },
                );
                self.add_paint(
                    record,
                    geometry::path(
                        &Value::Array(path.into_iter().map(Value::from).collect()),
                        geometry::Transform::identity(),
                        1.,
                        false,
                    )?,
                    if record.kind == "BUS" { 2 } else { 1 },
                    false,
                    width,
                    2,
                    None,
                    self.net(&id),
                    valid_color(&data["color"]),
                )?;
            }
            "BUSENTRY" => {
                let point = position(data)?;
                let angle = number(data, "rotation", number(data, "angle", 0.)).to_radians();
                let length = width(data, &["length", "size"], 8.);
                let end = (
                    point.0 + length * angle.cos(),
                    point.1 + length * angle.sin(),
                );
                self.add_paint(
                    record,
                    geometry::path(
                        &json!([point.0, point.1, end.0, end.1]),
                        geometry::Transform::identity(),
                        1.,
                        false,
                    )?,
                    2,
                    false,
                    width(data, &["strokeWidth", "width"], 1.),
                    3,
                    None,
                    None,
                    valid_color(&data["color"]),
                )?;
            }
            "TEXT" => self.add_text(record, data, geometry::Transform::identity(), None)?,
            "ATTR" => {
                if truth(&data["valueVisible"]) || truth(&data["keyVisible"]) {
                    if data.get("x").is_none()
                        && data.get("positionX").is_none()
                        && data.get("pointX").is_none()
                    {
                        return Ok(());
                    }
                    let mut label = data.clone();
                    let key = data["key"].as_str().unwrap_or("");
                    let value = data["value"].as_str().unwrap_or("");
                    let mut parts = Vec::new();
                    if truth(&data["keyVisible"]) && !key.is_empty() {
                        parts.push(key.to_owned());
                    }
                    if truth(&data["valueVisible"]) && !value.is_empty() {
                        parts.push(value.to_owned());
                    }
                    if !parts.is_empty() {
                        label["text"] = Value::String(parts.join(": "));
                        self.add_text(
                            record,
                            &label,
                            geometry::Transform::identity(),
                            data["parentId"].as_str(),
                        )?;
                    }
                }
            }
            "CIRCLE" => {
                let center = if data.get("centerX").is_some() {
                    (required(data, "centerX")?, required(data, "centerY")?)
                } else {
                    position(data)?
                };
                let radius = required(data, "radius")?;
                self.add_paint(
                    record,
                    geometry::circle(center.0, center.1, radius, geometry::Transform::identity())?,
                    6,
                    false,
                    width(data, &["strokeWidth", "width"], 1.),
                    2,
                    None,
                    None,
                    valid_color(&data["color"]),
                )?;
            }
            "POLY" | "POLYGON" | "RECT" | "ELLIPSE" | "ARC" | "BEZIER" | "JUNCTION" => {
                self.shape(record, data, geometry::Transform::identity(), None)?;
            }
            "COMPONENT" => self.component(record)?,
            "DOCHEAD" | "META" | "PART" | "LAYER" => {}
            _ => self.warn(format!("未渲染图元类型 {}，原始记录已保留", record.kind)),
        }
        Ok(())
    }

    fn shape(
        &mut self,
        record: &history::Record,
        data: &Value,
        tf: geometry::Transform,
        parent: Option<&str>,
    ) -> Result<()> {
        let (geometry, fill, layer) = match record.kind.as_str() {
            "RECT" => {
                let x1 = number(data, "dotX1", number(data, "x1", 0.));
                let y1 = number(data, "dotY1", number(data, "y1", 0.));
                let x2 = number(data, "dotX2", number(data, "x2", x1));
                let y2 = number(data, "dotY2", number(data, "y2", y1));
                (
                    geometry::rect(
                        (x1 + x2) / 2.,
                        (y1 + y2) / 2.,
                        (x2 - x1).abs(),
                        (y2 - y1).abs(),
                        number(data, "radius", 0.),
                        number(data, "rotation", number(data, "angle", 0.)),
                        tf,
                    )?,
                    truth(&data["filled"]) || truth(&data["fill"]),
                    3,
                )
            }
            "ELLIPSE" => {
                let center = position(data)?;
                let rx = number(data, "radiusX", number(data, "width", 1.) / 2.);
                let ry = number(data, "radiusY", number(data, "height", 1.) / 2.);
                let path = json!([[
                    "R",
                    center.0,
                    center.1,
                    rx * 2.,
                    ry * 2.,
                    number(data, "rotation", 0.),
                    0.,
                    0.
                ]]);
                (geometry::path(&path, tf, 1., true)?, false, 6)
            }
            "JUNCTION" => {
                let center = position(data)?;
                (
                    geometry::circle(center.0, center.1, number(data, "radius", 2.), tf)?,
                    true,
                    7,
                )
            }
            _ => {
                let path = path_value(data, "points")
                    .or_else(|| path_value(data, "dots"))
                    .or_else(|| path_value(data, "path"))
                    .ok_or_else(invalid)?;
                let closed = !matches!(record.kind.as_str(), "ARC" | "BEZIER");
                (
                    geometry::path(&path, tf, 1., closed)?,
                    truth(&data["filled"]) || truth(&data["fill"]),
                    6,
                )
            }
        };
        self.add_paint(
            record,
            geometry,
            layer,
            fill,
            width(data, &["strokeWidth", "width"], 1.),
            1,
            parent,
            None,
            valid_color(&data["color"]),
        )
    }

    fn symbol_document(&self, symbol_ref: Option<&str>) -> Option<&history::Document> {
        symbol_ref
            .and_then(|id| self.project.docs.get(id))
            .filter(|doc| doc.head["docType"] == "SYMBOL" && !doc.deleted())
            .or_else(|| {
                self.project
                    .docs
                    .values()
                    .find(|doc| doc.head["docType"] == "SYMBOL" && !doc.deleted())
            })
    }

    fn component(&mut self, record: &history::Record) -> Result<()> {
        let data = record.data.as_ref().ok_or_else(invalid)?;
        let id = record_id(record);
        let attrs = self.page_attrs.get(&id).cloned();
        let symbol_ref = Self::attr_value(attrs.as_ref(), "Symbol")
            .and_then(Value::as_str)
            .or_else(|| data["symbolId"].as_str())
            .or_else(|| data["symbol"].as_str());
        let Some(symbol) = self.symbol_document(symbol_ref).cloned() else {
            self.warn(format!("器件 {} 缺少符号，已保留属性", id));
            return self.fallback_component(record, data, attrs.as_ref());
        };
        let part_id = data["partId"]
            .as_str()
            .or_else(|| Self::attr_value(attrs.as_ref(), "Part").and_then(Value::as_str));
        let part = part_id
            .and_then(|id| {
                symbol
                    .records
                    .values()
                    .find(|record| record.kind == "PART" && record.id.as_str() == Some(id))
            })
            .or_else(|| symbol.active().find(|record| record.kind == "PART"));
        let Some(part) = part else {
            self.warn(format!(
                "符号 {} 缺少 PART，已保留属性",
                symbol.head["uuid"].as_str().unwrap_or("unknown")
            ));
            return self.fallback_component(record, data, attrs.as_ref());
        };
        let (x, y) = position(data)?;
        let tf = geometry::Transform::identity().component(
            x,
            y,
            number(data, "rotation", number(data, "angle", 0.)),
            truth(&data["isMirror"]) || truth(&data["mirror"]),
        );
        let mut bounds = Bounds::empty();
        let before = self.paints.len();
        let part_id = record_id(part);
        for child in symbol.active().filter(|child| {
            child.kind != "PART"
                && child
                    .data
                    .as_ref()
                    .and_then(|data| data["partId"].as_str())
                    .is_none_or(|child_part| child_part == part_id)
        }) {
            let result = match child.kind.as_str() {
                "PIN" => self.pin(child, tf, Some(&id)),
                "ATTR" => self.symbol_attr(child, tf, Some(&id)),
                "RECT" | "POLY" | "POLYGON" | "CIRCLE" | "ELLIPSE" | "ARC" | "BEZIER"
                | "JUNCTION" => child.data.as_ref().ok_or_else(invalid).and_then(|data| {
                    if child.kind == "CIRCLE" {
                        let center = if data.get("centerX").is_some() {
                            (required(data, "centerX")?, required(data, "centerY")?)
                        } else {
                            position(data)?
                        };
                        self.add_paint(
                            child,
                            geometry::circle(center.0, center.1, required(data, "radius")?, tf)?,
                            3,
                            false,
                            width(data, &["strokeWidth", "width"], 1.),
                            1,
                            Some(&id),
                            None,
                            valid_color(&data["color"]),
                        )
                    } else {
                        self.shape(child, data, tf, Some(&id))
                    }
                }),
                _ => {
                    self.warn(format!("符号图元 {} 未渲染", child.kind));
                    Ok(())
                }
            };
            if let Err(error) = result {
                if error.code == "pcb_limit" {
                    return Err(error);
                }
                self.warn(format!("符号 {} 图元 {} 无法解析", id, child.kind));
            }
        }
        for paint in &self.paints[before..] {
            bounds.union(paint.result.bounds);
        }
        if !bounds.finite()
            && let Some(bbox) = part.data.as_ref().and_then(|data| data["BBOX"].as_array())
            && bbox.len() >= 4
        {
            let x1 = bbox[0].as_f64().ok_or_else(invalid)?;
            let y1 = bbox[1].as_f64().ok_or_else(invalid)?;
            let x2 = bbox[2].as_f64().ok_or_else(invalid)?;
            let y2 = bbox[3].as_f64().ok_or_else(invalid)?;
            bounds = geometry::rect(
                (x1 + x2) / 2.,
                (y1 + y2) / 2.,
                (x2 - x1).abs(),
                (y2 - y1).abs(),
                0.,
                0.,
                tf,
            )?
            .bounds;
        }
        if !bounds.finite() {
            return self.fallback_component(record, data, attrs.as_ref());
        }
        let component_geometry = geometry::rect(
            (bounds.min_x + bounds.max_x) / 2.,
            (bounds.min_y + bounds.max_y) / 2.,
            bounds.max_x - bounds.min_x,
            bounds.max_y - bounds.min_y,
            0.,
            0.,
            geometry::Transform::identity(),
        )?;
        let mut properties = self.properties(record, None);
        if let Some(attrs) = attrs.as_ref() {
            for key in ["Device", "Symbol", "Designator", "Value"] {
                if let Some(value) = Self::attr_value(Some(attrs), key) {
                    properties.insert(key.into(), text(value).chars().take(1024).collect());
                }
            }
        }
        self.paints.push(Paint {
            result: PickResult {
                id,
                kind: "COMPONENT".into(),
                layer: 3,
                net: None,
                properties,
                bounds,
            },
            geometry: component_geometry,
            fill: true,
            width: 0.,
            stage: 5,
            color: None,
            selectable: true,
        });
        Ok(())
    }

    fn fallback_component(
        &mut self,
        record: &history::Record,
        data: &Value,
        attrs: Option<&BTreeMap<String, Value>>,
    ) -> Result<()> {
        let (x, y) = position(data)?;
        let w = width(data, &["width", "sizeX"], 20.);
        let h = width(data, &["height", "sizeY"], 12.);
        let geometry = geometry::rect(
            x,
            y,
            w,
            h,
            0.,
            number(data, "rotation", number(data, "angle", 0.)),
            geometry::Transform::identity(),
        )?;
        let id = record_id(record);
        let mut properties = self.properties(record, None);
        if let Some(attrs) = attrs {
            for key in ["Device", "Symbol", "Designator", "Value"] {
                if let Some(value) = Self::attr_value(Some(attrs), key) {
                    properties.insert(key.into(), text(value).chars().take(1024).collect());
                }
            }
        }
        self.paints.push(Paint {
            result: PickResult {
                id,
                kind: "COMPONENT".into(),
                layer: 3,
                net: None,
                properties,
                bounds: geometry.bounds,
            },
            geometry,
            fill: false,
            width: 1.,
            stage: 5,
            color: Some("#e8d38b".into()),
            selectable: true,
        });
        Ok(())
    }

    fn pin(
        &mut self,
        record: &history::Record,
        tf: geometry::Transform,
        parent: Option<&str>,
    ) -> Result<()> {
        let data = record.data.as_ref().ok_or_else(invalid)?;
        let (x, y) = position(data)?;
        let angle = number(data, "rotation", number(data, "angle", 0.));
        let length = width(data, &["length", "pinLength"], 20.);
        let local = tf.component(
            x,
            y,
            angle,
            truth(&data["isMirror"]) || truth(&data["mirror"]),
        );
        let geometry = geometry::path(&json!([0., 0., length, 0.]), local, 1., false)?;
        let mut properties = self.properties(record, parent);
        let attrs = self.symbol_attrs.get(&record_id(record));
        if let Some(value) = Self::attr_value(attrs, "NAME") {
            properties.insert("PinName".into(), text(value));
        }
        if let Some(value) = Self::attr_value(attrs, "NUMBER") {
            properties.insert("PinNumber".into(), text(value));
        }
        let mut result = PickResult {
            id: parent
                .map(|parent| format!("{parent}/{}", record_id(record)))
                .unwrap_or_else(|| record_id(record)),
            kind: "PIN".into(),
            layer: 4,
            net: None,
            properties,
            bounds: geometry
                .bounds
                .expand(width(data, &["strokeWidth", "width"], 1.) / 2.),
        };
        if let Some(parent) = parent {
            result
                .properties
                .insert("componentId".into(), parent.to_owned());
        }
        self.ensure_layer(4, "引脚", "#e38c8c", 400);
        self.paints.push(Paint {
            result,
            geometry,
            fill: false,
            width: width(data, &["strokeWidth", "width"], 1.),
            stage: 10,
            color: valid_color(&data["color"]).or_else(|| Some("#e38c8c".into())),
            selectable: true,
        });
        Ok(())
    }

    fn symbol_attr(
        &mut self,
        record: &history::Record,
        tf: geometry::Transform,
        parent: Option<&str>,
    ) -> Result<()> {
        let data = record.data.as_ref().ok_or_else(invalid)?;
        if !truth(&data["valueVisible"]) && !truth(&data["keyVisible"]) {
            return Ok(());
        }
        let key = data["key"].as_str().unwrap_or("");
        let value = data["value"].as_str().unwrap_or("");
        let mut label = data.clone();
        let mut parts = Vec::new();
        if truth(&data["keyVisible"]) && !key.is_empty() {
            parts.push(key.to_owned());
        }
        if truth(&data["valueVisible"]) && !value.is_empty() {
            parts.push(value.to_owned());
        }
        if parts.is_empty() {
            return Ok(());
        }
        label["text"] = Value::String(parts.join(": "));
        self.add_text(record, &label, tf, parent)
    }

    fn finish(self, revision: String, mut metrics: Metrics) -> Result<Scene> {
        let start = Instant::now();
        let mut paints = self.paints;
        paints.sort_by_key(|paint| (paint.result.layer, paint.stage));
        let mut batches: Vec<DrawBatch> = Vec::new();
        let mut objects = Vec::new();
        let mut bounds = Bounds::empty();
        for (sequence, paint) in paints.into_iter().enumerate() {
            if !paint.geometry.path.is_empty() {
                bounds.union(paint.result.bounds);
                if let Some(batch) = batches.last_mut().filter(|batch| {
                    batch.layer == paint.result.layer
                        && batch.fill == paint.fill
                        && batch.width == paint.width
                        && batch.color == paint.color
                        && batch.path.len() + paint.geometry.path.len() < 512 * 1024
                }) {
                    batch.path.push_str(&paint.geometry.path);
                    batch.bounds.union(paint.result.bounds);
                } else {
                    batches.push(DrawBatch {
                        layer: paint.result.layer,
                        path: paint.geometry.path.clone(),
                        fill: paint.fill,
                        width: paint.width,
                        bounds: paint.result.bounds,
                        color: paint.color.clone(),
                        net: None,
                        kind: String::new(),
                    });
                }
            }
            if paint.selectable {
                let layer = paint.result.layer;
                objects.push(HitObject {
                    result: paint.result,
                    contours: paint.geometry.contours,
                    fill: paint.fill,
                    width: paint.width,
                    priority: (!paint.geometry.path.is_empty(), paint.stage, sequence),
                    layers: vec![layer],
                });
            }
        }
        for text in &self.texts {
            bounds.union(text_bounds(text));
        }
        if !bounds.finite() {
            return Err(AppError::new(
                "schematic_empty",
                "原理图中没有支持的可绘制图元",
            ));
        }
        bounds = bounds.expand(20.);
        metrics.scene_ms = start.elapsed().as_millis() as u64;
        let id = uuid::Uuid::new_v4().to_string();
        let warnings = self
            .warnings
            .into_iter()
            .map(|(message, count)| format!("{message} ({count})"))
            .collect();
        let mut layers: Vec<_> = self.layers.into_values().collect();
        layers.sort_by_key(|layer| layer.order);
        let scene = Scene {
            id: id.clone(),
            name: self
                .page
                .first("META")
                .and_then(|data| data["title"].as_str())
                .unwrap_or("原理图")
                .to_owned(),
            revision,
            bounds,
            layers: layers.clone(),
            batches,
            texts: self.texts,
            net_labels: Vec::new(),
            object_count: objects.len(),
            warnings,
            metrics,
        };
        let mut grid = HashMap::new();
        let mut broad = Vec::new();
        let mut entries = 0usize;
        for (index, object) in objects.iter().enumerate() {
            let bounds = object.result.bounds;
            let x0 = (bounds.min_x / CELL).floor() as i32;
            let x1 = (bounds.max_x / CELL).floor() as i32;
            let y0 = (bounds.min_y / CELL).floor() as i32;
            let y1 = (bounds.max_y / CELL).floor() as i32;
            let count = (x1 as i64 - x0 as i64 + 1) * (y1 as i64 - y0 as i64 + 1);
            if count > 256 || entries + count as usize > 2_000_000 {
                broad.push(index);
            } else {
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        grid.entry((x, y)).or_insert_with(Vec::new).push(index);
                    }
                }
                entries += count as usize;
            }
        }
        let mut cache = cache()
            .lock()
            .map_err(|_| AppError::new("internal", "场景缓存不可用"))?;
        cache.push_back(Arc::new(Stored {
            id,
            objects,
            grid,
            broad,
            layers,
        }));
        while cache.len() > 2 {
            cache.pop_front();
        }
        Ok(scene)
    }
}

fn compile(
    project: history::Project,
    document_id: &str,
    revision: String,
    metrics: Metrics,
) -> Result<Scene> {
    let page = project
        .docs
        .get(document_id)
        .filter(|doc| doc.head["docType"] == "SCH_PAGE" && !doc.deleted())
        .ok_or_else(|| AppError::new("schematic_not_found", "完整工程历史中未找到此原理图"))?;
    let mut builder = Builder::new(&project, page);
    for record in page.active() {
        let result = builder.page_primitive(record);
        if let Err(error) = result {
            if error.code == "pcb_limit" {
                return Err(error);
            }
            builder.warn(format!("{} 图元无法解析，部分场景缺失", record.kind));
        }
    }
    builder.finish(revision, metrics)
}

pub fn pick(
    scene_id: &str,
    x: f64,
    y: f64,
    tolerance: f64,
    layers: Option<&[i32]>,
) -> Result<Option<PickResult>> {
    if ![x, y, tolerance].iter().all(|value| value.is_finite()) || tolerance < 0. || tolerance > 1e6
    {
        return Err(AppError::new("invalid_request", "拾取参数无效"));
    }
    let cache = cache()
        .lock()
        .map_err(|_| AppError::new("internal", "场景缓存不可用"))?;
    let scene = cache
        .iter()
        .find(|scene| scene.id == scene_id)
        .ok_or_else(|| AppError::new("scene_expired", "场景已过期，请重新打开原理图"))?;
    let allowed: HashSet<_> = layers
        .map(|layers| layers.iter().copied().collect())
        .unwrap_or_else(|| {
            scene
                .layers
                .iter()
                .filter(|layer| layer.visible)
                .map(|layer| layer.id)
                .collect()
        });
    let x0 = ((x - tolerance) / CELL).floor() as i32;
    let x1 = ((x + tolerance) / CELL).floor() as i32;
    let y0 = ((y - tolerance) / CELL).floor() as i32;
    let y1 = ((y + tolerance) / CELL).floor() as i32;
    let mut candidates: HashSet<usize> = scene.broad.iter().copied().collect();
    if (x1 as i64 - x0 as i64 + 1) * (y1 as i64 - y0 as i64 + 1) > 4096 {
        candidates.extend(0..scene.objects.len());
    } else {
        for cell_x in x0..=x1 {
            for cell_y in y0..=y1 {
                if let Some(objects) = scene.grid.get(&(cell_x, cell_y)) {
                    candidates.extend(objects);
                }
            }
        }
    }
    let point = (x, y);
    let found = candidates
        .into_iter()
        .map(|index| &scene.objects[index])
        .filter(|object| {
            object.layers.iter().any(|layer| allowed.contains(layer))
                && object.result.bounds.contains(point, tolerance)
                && geometry::hit(
                    &object.contours,
                    point,
                    tolerance,
                    object.fill,
                    object.width,
                )
        })
        .max_by_key(|object| {
            (
                object.priority.0,
                object
                    .layers
                    .iter()
                    .filter(|layer| allowed.contains(layer))
                    .max()
                    .copied()
                    .unwrap_or(i32::MIN),
                object.priority.1,
                object.priority.2,
            )
        });
    Ok(found.map(|object| object.result.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r###"
{"type":"DOCHEAD"}||{"docType":"SCH_PAGE","uuid":"page","client":"a"}|
{"type":"WIRE","id":"wire","ticket":1}||{"dots":[[0,0,50,0]],"strokeWidth":1}|
{"type":"BUS","id":"bus","ticket":1}||{"dots":[[0,10,50,10]],"strokeWidth":2}|
{"type":"BUSENTRY","id":"entry","ticket":1}||{"pointX":50,"pointY":10,"rotation":90}|
{"type":"TEXT","id":"note","ticket":1}||{"positionX":5,"positionY":25,"value":"POWER","fontSize":8,"color":"#ffffff"}|
{"type":"CIRCLE","id":"circle","ticket":1}||{"centerX":70,"centerY":20,"radius":5,"strokeWidth":1}|
{"type":"POLY","id":"poly","ticket":1}||{"points":[60,30,80,30,80,40,60,40],"closed":true,"strokeWidth":1}|
{"type":"ATTR","id":"net","ticket":1}||{"parentId":"wire","key":"NET","value":"GND","valueVisible":true}|
{"type":"COMPONENT","id":"component","ticket":1}||{"partId":"part","positionX":50,"positionY":40,"rotation":90,"isMirror":false}|
{"type":"ATTR","id":"symbol-ref","ticket":1}||{"parentId":"component","key":"Symbol","value":"symbol","valueVisible":false}|
{"type":"ATTR","id":"device-ref","ticket":1}||{"parentId":"component","key":"Device","value":"device","valueVisible":false}|
{"type":"ATTR","id":"designator","ticket":1}||{"parentId":"component","key":"Designator","value":"R1","valueVisible":true,"positionX":50,"positionY":30,"fontSize":6}|
{"type":"ATTR","id":"value","ticket":1}||{"parentId":"component","key":"Value","value":"10k","valueVisible":true,"positionX":50,"positionY":50,"fontSize":6}|
{"type":"UNKNOWN","id":"future","ticket":1}||{"visible":true}|
{"type":"DOCHEAD"}||{"docType":"SYMBOL","uuid":"symbol","client":"a"}|
{"type":"PART","id":"part","ticket":1}||{"BBOX":[-10,-5,10,5]}|
{"type":"RECT","id":"body","ticket":1}||{"partId":"part","dotX1":-10,"dotY1":-5,"dotX2":10,"dotY2":5,"strokeWidth":1}|
{"type":"PIN","id":"pin","ticket":1}||{"partId":"part","positionX":10,"positionY":0,"length":20,"rotation":0,"display":true}|
{"type":"ATTR","id":"pin-name","ticket":1}||{"parentId":"pin","key":"NAME","value":"IN","valueVisible":true,"positionX":10,"positionY":0,"fontSize":5}|
{"type":"ATTR","id":"pin-number","ticket":1}||{"parentId":"pin","key":"NUMBER","value":"1","valueVisible":true,"positionX":10,"positionY":0,"fontSize":5}|
"###;

    fn fixture_project() -> history::Project {
        let mut project = history::Project::default();
        history::replay(&mut project, FIXTURE).unwrap();
        project
    }

    #[tokio::test]
    async fn rejects_invalid_document_ids_before_network_access() {
        let client = HttpClient::new().unwrap();
        let error = load(&client, "bad", "bad").await.unwrap_err();
        assert_eq!(error.code, "INVALID_ID");
    }

    #[test]
    fn compiles_common_schematic_records_and_picks_components() {
        let scene = compile(
            fixture_project(),
            "page",
            "revision".into(),
            Metrics::default(),
        )
        .unwrap();
        assert!(scene.bounds.finite());
        assert!(scene.batches.len() >= 6);
        assert!(scene.texts.iter().any(|text| text.text == "POWER"));
        assert!(scene.texts.iter().any(|text| text.text == "R1"));
        assert_eq!(scene.warnings.len(), 1);

        let component = pick(&scene.id, 50., 40., 2., None).unwrap().unwrap();
        assert_eq!(component.kind, "COMPONENT");
        assert_eq!(component.properties.get("Designator").unwrap(), "R1");
        let pin = pick(&scene.id, 50., 60., 2., None).unwrap().unwrap();
        assert_eq!(pin.kind, "PIN");
        assert_eq!(pin.properties.get("PinName").unwrap(), "IN");
        let wire = pick(&scene.id, 25., 0., 2., None).unwrap().unwrap();
        assert_eq!(wire.kind, "WIRE");
        assert_eq!(wire.net.as_deref(), Some("GND"));
    }

    #[test]
    fn rejects_missing_page_and_warns_on_missing_symbol() {
        let mut project = fixture_project();
        project.docs.remove("page");
        let error = compile(project, "page", "revision".into(), Metrics::default()).unwrap_err();
        assert_eq!(error.code, "schematic_not_found");

        let mut project = fixture_project();
        project.docs.remove("symbol");
        let scene = compile(project, "page", "revision".into(), Metrics::default()).unwrap();
        assert!(scene.object_count > 0);
        assert!(
            scene
                .warnings
                .iter()
                .any(|warning| warning.contains("符号"))
        );
    }

    #[test]
    fn width_uses_the_first_positive_fallback_field() {
        assert_eq!(
            width(&json!({"pinLength": 12}), &["length", "pinLength"], 20.),
            12.
        );
        assert_eq!(
            width(
                &json!({"length": 0, "pinLength": 12}),
                &["length", "pinLength"],
                20.
            ),
            12.
        );
        assert_eq!(
            width(&json!({"length": -1}), &["length", "pinLength"], 20.),
            20.
        );
    }

    #[test]
    fn malformed_and_unknown_records_warn_in_a_partial_scene() {
        let log = r#"
{"type":"DOCHEAD"}||{"docType":"SCH_PAGE","uuid":"page","client":"a"}|
{"type":"WIRE","id":"good","ticket":1}||{"dots":[[0,0,20,0]]}|
{"type":"WIRE","id":"bad","ticket":1}||{"dots":[[0,0,1]]}|
{"type":"CIRCLE","id":"bad-circle","ticket":1}||{"centerX":10,"centerY":10,"radius":-1}|
{"type":"UNKNOWN","id":"future","ticket":1}||{"visible":true}|
"#;
        let mut project = history::Project::default();
        history::replay(&mut project, log).unwrap();
        let scene = compile(project, "page", "revision".into(), Metrics::default()).unwrap();
        assert!(scene.object_count > 0);
        assert!(
            scene
                .warnings
                .iter()
                .any(|warning| warning.contains("WIRE"))
        );
        assert!(
            scene
                .warnings
                .iter()
                .any(|warning| warning.contains("CIRCLE"))
        );
        assert!(
            scene
                .warnings
                .iter()
                .any(|warning| warning.contains("UNKNOWN"))
        );
    }

    #[test]
    fn an_empty_page_returns_a_stable_error() {
        let log = r#"
{"type":"DOCHEAD"}||{"docType":"SCH_PAGE","uuid":"page","client":"a"}|
{"type":"UNKNOWN","id":"future","ticket":1}||{"visible":true}|
"#;
        let mut project = history::Project::default();
        history::replay(&mut project, log).unwrap();
        let error = compile(project, "page", "revision".into(), Metrics::default()).unwrap_err();
        assert_eq!(error.code, "schematic_empty");
    }
}
