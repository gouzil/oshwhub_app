use crate::{
    client::HttpClient,
    error::{AppError, Result},
    pcb::{
        self, Bounds, DrawBatch, Metrics, PickResult, Scene, SceneImage, SceneLayer, TextLabel,
        geometry, history,
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
        if point.is_object() {
            flat.extend([required(point, "x")?, required(point, "y")?]);
            continue;
        }
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

fn resolve_template(input: &str, values: &BTreeMap<String, Value>) -> String {
    if !input.starts_with('=') && !input.contains("={") {
        return input.to_owned();
    }
    let mut result = input.strip_prefix('=').unwrap_or(input).to_owned();
    for _ in 0..8 {
        let Some(start) = result.find('{') else {
            break;
        };
        let Some(end) = result[start + 1..].find('}') else {
            break;
        };
        let end = start + 1 + end;
        let replacement = values
            .get(&result[start + 1..end])
            .filter(|value| !value.is_null())
            .map(text)
            .unwrap_or_default();
        if result.len() + replacement.len() > 16_384 {
            break;
        }
        let start = if start > 0 && result.as_bytes()[start - 1] == b'=' {
            start - 1
        } else {
            start
        };
        result.replace_range(start..=end, &replacement);
    }
    result
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
    let top = match label.vertical_align.as_str() {
        "top" => 0.,
        "middle" => -label.size / 2.,
        "bottom" => -label.size,
        _ => -label.size,
    };
    for point in [
        (left, top),
        (left + width, top),
        (left + width, top + label.size),
        (left, top + label.size),
    ] {
        bounds.include(tf.point(point));
    }
    bounds
}

struct Builder<'a> {
    project: &'a history::Project,
    page: &'a history::Document,
    page_attrs: HashMap<String, BTreeMap<String, Value>>,
    wire_groups: HashMap<String, &'a history::Record>,
    paints: Vec<Paint>,
    texts: Vec<TextLabel>,
    images: Vec<SceneImage>,
    layers: BTreeMap<i32, SceneLayer>,
    warnings: BTreeMap<String, usize>,
    path_bytes: usize,
}

impl<'a> Builder<'a> {
    fn new(project: &'a history::Project, page: &'a history::Document) -> Self {
        let mut page_attrs = HashMap::new();
        Self::index_attrs(page, &mut page_attrs);
        Self {
            project,
            page,
            page_attrs,
            wire_groups: page
                .active()
                .filter(|record| matches!(record.kind.as_str(), "WIRE" | "BUS"))
                .map(|record| (record_id(record), record))
                .collect(),
            paints: Vec::new(),
            texts: Vec::new(),
            images: Vec::new(),
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
        let id = parent
            .map(str::to_owned)
            .or_else(|| (record.kind == "COMPONENT").then(|| record_id(record)));
        if let Some(id) = id {
            let values = self.component_values(&id);
            // shortcut: Picking keeps 64 attributes per object; add paged metadata for larger parts.
            for (key, value) in values
                .iter()
                .filter(|(key, _)| !key.starts_with('@'))
                .take(64)
            {
                if !value.is_null() {
                    let value = resolve_template(&text(value), &values);
                    properties.insert(key.clone(), value.chars().take(1024).collect());
                }
            }
            for (key, name) in [("Device", "DeviceName"), ("Footprint", "FootprintName")] {
                if let Some(title) = values
                    .get(key)
                    .and_then(Value::as_str)
                    .and_then(|id| self.project.docs.get(id))
                    .and_then(|doc| doc.first("META"))
                    .and_then(|meta| meta["title"].as_str())
                {
                    properties.insert(name.into(), title.chars().take(1024).collect());
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
            8 => ("图框与图片", "#aab4c0", 50),
            _ => ("原理图", "#aab4c0", 800),
        };
        self.ensure_layer(layer, name, default_color, order);
        let properties = self.properties(record, parent);
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
        let label = value["text"]
            .as_str()
            .or_else(|| value["value"].as_str())
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
        let angle = -number(value, "rotation", number(value, "angle", 0.));
        let local = tf.component(x, y, angle, truth(&value["isMirror"]));
        let alignment = value["align"]
            .as_str()
            .unwrap_or("LEFT_BOTTOM")
            .to_ascii_uppercase();
        let mut align = if alignment.starts_with("CENTER") {
            "center"
        } else if alignment.starts_with("RIGHT") {
            "right"
        } else {
            "left"
        };
        let mut vertical_align = if alignment.ends_with("MIDDLE") {
            "middle"
        } else if alignment.ends_with("TOP") {
            "top"
        } else {
            "bottom"
        };
        let mut rotation = local.b.atan2(local.a).to_degrees();
        if tf.a * tf.d - tf.b * tf.c < 0. {
            vertical_align = match vertical_align {
                "top" => "bottom",
                "bottom" => "top",
                v => v,
            };
        }
        if !(-90. ..=90.).contains(&rotation) {
            rotation = if rotation > 90. {
                rotation - 180.
            } else {
                rotation + 180.
            };
            align = match align {
                "left" => "right",
                "right" => "left",
                v => v,
            };
            vertical_align = match vertical_align {
                "top" => "bottom",
                "bottom" => "top",
                v => v,
            };
        }
        let point = local.point((0., 0.));
        self.ensure_layer(5, "文本", "#ffffff", 500);
        self.texts.push(TextLabel {
            layer: 5,
            x: point.0,
            y: point.1,
            text: label.to_owned(),
            size,
            rotation,
            align: align.into(),
            vertical_align: vertical_align.into(),
            color: valid_color(&value["color"]),
            bold: truth(&value["fontWeight"]),
        });
        let bounds = text_bounds(self.texts.last().ok_or_else(invalid)?);
        let mut text_geometry = geometry::rect(
            (bounds.min_x + bounds.max_x) / 2.,
            (bounds.min_y + bounds.max_y) / 2.,
            (bounds.max_x - bounds.min_x).max(size),
            (bounds.max_y - bounds.min_y).max(size),
            0.,
            0.,
            geometry::Transform::identity(),
        )?;
        text_geometry.path.clear();
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
                    // Current histories store geometry in LINE records linked by lineGroup.
                    return Ok(());
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
                let angle = -number(data, "rotation", number(data, "angle", 0.)).to_radians();
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
            "TABLE" => self.table(
                record,
                data,
                geometry::Transform::identity(),
                None,
                &BTreeMap::new(),
            )?,
            "OBJ" => self.image(record, data, geometry::Transform::identity(), None)?,
            "TEXT" => self.add_text(record, data, geometry::Transform::identity(), None)?,
            "ATTR" => {
                let parent = data["parentId"].as_str().unwrap_or("");
                let values = self.component_values(parent);
                let mut label = data.clone();
                let key = data["key"].as_str().unwrap_or("");
                if key == "Device" {
                    label["value"] = json!(resolve_template(
                        values.get("Name").and_then(Value::as_str).unwrap_or(""),
                        &values
                    ));
                } else if let Some(value) = values.get(key) {
                    label["value"] = json!(resolve_template(value.as_str().unwrap_or(""), &values));
                }
                // Null coordinates are inherited metadata, not page labels.
                if position(&label).is_ok() {
                    self.symbol_attr(
                        record,
                        &label,
                        geometry::Transform::identity(),
                        Some(parent),
                    )?;
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
            "LINE" | "POLY" | "POLYGON" | "RECT" | "ELLIPSE" | "ARC" | "BEZIER" | "JUNCTION" => {
                self.shape(record, data, geometry::Transform::identity(), None)?;
            }
            "COMPONENT" => self.component(record)?,
            "DOCHEAD" | "META" | "PART" | "LAYER" | "CANVAS" | "ELE_PLACEHOLDER" => {}
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
            "LINE" => (
                geometry::path(
                    &json!([
                        required(data, "startX")?,
                        required(data, "startY")?,
                        required(data, "endX")?,
                        required(data, "endY")?
                    ]),
                    tf,
                    1.,
                    false,
                )?,
                false,
                6,
            ),
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
                        -number(data, "rotation", number(data, "angle", 0.)),
                        tf,
                    )?,
                    truth(&data["filled"]) || truth(&data["fill"]),
                    3,
                )
            }
            "ELLIPSE" => {
                let center = if data.get("centerX").is_some() {
                    (required(data, "centerX")?, required(data, "centerY")?)
                } else {
                    position(data)?
                };
                let rx = number(data, "radiusX", number(data, "width", 1.) / 2.).abs();
                let ry = number(data, "radiusY", number(data, "height", 1.) / 2.).abs();
                if rx == 0. || ry == 0. {
                    return Err(invalid());
                }
                let local = tf
                    .component(center.0, center.1, -number(data, "rotation", 0.), false)
                    .affine(rx, 0., 0., ry, 0., 0.);
                (geometry::circle(0., 0., 1., local)?, false, 6)
            }
            "ARC" if data.get("startX").is_some() => {
                let start = (required(data, "startX")?, required(data, "startY")?);
                let end = (required(data, "endX")?, required(data, "endY")?);
                let refer = (required(data, "referX")?, required(data, "referY")?);
                let a = (start.0 - refer.0, start.1 - refer.1);
                let b = (end.0 - refer.0, end.1 - refer.1);
                let angle = (a.0 * b.1 - a.1 * b.0)
                    .atan2(a.0 * b.0 + a.1 * b.1)
                    .to_degrees();
                // The reference point lies on the requested arc, opposite the inscribed angle.
                let sweep = 2. * angle - angle.signum() * 360.;
                if sweep.abs() < 1e-8 || sweep.abs() >= 360. {
                    return Err(invalid());
                }
                (
                    geometry::path(
                        &json!([start.0, start.1, "ARC", sweep, end.0, end.1]),
                        tf,
                        1.,
                        false,
                    )?,
                    false,
                    6,
                )
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
                let path = if path
                    .as_array()
                    .and_then(|values| values.first())
                    .is_some_and(Value::is_object)
                {
                    json!(points(&path)?)
                } else {
                    path
                };
                let closed = data["closed"]
                    .as_bool()
                    .unwrap_or(!matches!(record.kind.as_str(), "ARC" | "BEZIER"));
                (
                    geometry::path(&path, tf, 1., closed)?,
                    truth(&data["filled"]) || truth(&data["fill"]),
                    6,
                )
            }
        };
        let group = if parent.is_none() {
            data["lineGroup"]
                .as_str()
                .and_then(|id| self.wire_groups.get(id))
                .copied()
        } else {
            None
        };
        let layer = group.map_or(layer, |group| if group.kind == "BUS" { 2 } else { 1 });
        if let Some(color) = valid_color(&data["fillColor"]) {
            self.add_paint(
                record,
                geometry.clone(),
                layer,
                true,
                0.,
                0,
                parent,
                None,
                Some(color),
            )?;
        }
        if data["strokeColor"] == "none" {
            return Ok(());
        }
        self.add_paint(
            group.unwrap_or(record),
            geometry,
            layer,
            fill,
            width(
                data,
                &["strokeWidth", "width"],
                if layer == 2 { 2. } else { 1. },
            ),
            1,
            parent,
            group.and_then(|group| self.net(&record_id(group))),
            valid_color(&data["strokeColor"]).or_else(|| valid_color(&data["color"])),
        )
    }

    fn table(
        &mut self,
        record: &history::Record,
        data: &Value,
        tf: geometry::Transform,
        parent: Option<&str>,
        values: &BTreeMap<String, Value>,
    ) -> Result<()> {
        let offsets = |key: &str| -> Result<Vec<f64>> {
            let sizes = data[key]
                .as_array()
                .filter(|v| !v.is_empty() && v.len() <= 1000)
                .ok_or_else(invalid)?;
            let mut offsets = vec![0.];
            for size in sizes {
                let size = size
                    .as_f64()
                    .filter(|n| n.is_finite() && *n > 0. && *n < 1e6)
                    .ok_or_else(invalid)?;
                offsets.push(offsets.last().unwrap() + size);
            }
            Ok(offsets)
        };
        let cols = offsets("colSizes")?;
        let rows = offsets("rowSizes")?;
        let local = tf.component(
            required(data, "startX")?,
            required(data, "startY")?,
            -number(data, "rotation", 0.),
            false,
        );
        let cells = data["tableCell"]
            .as_array()
            .filter(|v| v.len() <= 100_000)
            .ok_or_else(invalid)?;
        let mut covered = HashSet::new();
        for cell in cells {
            let col = cell["columnIndex"].as_u64().ok_or_else(invalid)? as usize;
            let row = cell["rowIndex"].as_u64().ok_or_else(invalid)? as usize;
            if covered.contains(&(col, row)) {
                continue;
            }
            let cs = cell["colSpan"].as_u64().unwrap_or(1) as usize;
            let rs = cell["rowSpan"].as_u64().unwrap_or(1) as usize;
            if cs == 0
                || rs == 0
                || cs > cols.len()
                || rs > rows.len()
                || col >= cols.len() - cs
                || row >= rows.len() - rs
            {
                return Err(invalid());
            }
            if cs.saturating_mul(rs) > 100_000 || covered.len().saturating_add(cs * rs) > 100_000 {
                return Err(invalid());
            }
            for c in col..col + cs {
                for r in row..row + rs {
                    covered.insert((c, r));
                }
            }
            let (x, y, w, h) = (
                cols[col],
                rows[row],
                cols[col + cs] - cols[col],
                rows[row + rs] - rows[row],
            );
            for (style, points) in [
                ("topStyle", json!([x, y, x + w, y])),
                ("bottomStyle", json!([x, y + h, x + w, y + h])),
                ("leftStyle", json!([x, y, x, y + h])),
                ("rightStyle", json!([x + w, y, x + w, y + h])),
            ] {
                if cell[style]["strokeColor"] != "none" {
                    self.add_paint(
                        record,
                        geometry::path(&points, local, 1., false)?,
                        8,
                        false,
                        width(&cell[style], &["strokeWidth"], 0.5),
                        0,
                        parent,
                        None,
                        valid_color(&cell[style]["strokeColor"]),
                    )?;
                }
            }
            let style = &cell["fontStyle"];
            let ha = style["hAlign"].as_str().unwrap_or("LEFT");
            let va = style["vAlign"].as_str().unwrap_or("MIDDLE");
            let value = resolve_template(cell["value"].as_str().unwrap_or(""), values);
            let label = json!({"x":x + match ha {"CENTER"=>w/2.,"RIGHT"=>w-3.,_=>3.}, "y":y + match va {"TOP"=>2.,"BOTTOM"=>h-2.,_=>h/2.}, "value":value,"fontSize":width(style,&["fontSize"],15.),"align":format!("{ha}_{va}"),"color":style["color"],"fontWeight":style["fontWeight"]});
            self.add_text(record, &label, local, parent)?;
        }
        Ok(())
    }

    fn image(
        &mut self,
        record: &history::Record,
        data: &Value,
        tf: geometry::Transform,
        parent: Option<&str>,
    ) -> Result<()> {
        use base64::Engine;
        let source = data["content"].as_str().ok_or_else(invalid)?;
        let content = if let Some(id) = source.strip_prefix("blob:") {
            self.project
                .docs
                .get("BLOB")
                .and_then(|doc| doc.active().find(|r| r.id.as_str() == Some(id)))
                .and_then(|r| r.data.as_ref())
                .and_then(|d| d["content"].as_str())
                .ok_or_else(invalid)?
        } else {
            source
        };
        let (header, encoded) = content.split_once(',').ok_or_else(invalid)?;
        if !matches!(
            header,
            "data:image/svg+xml;base64"
                | "data:image/png;base64"
                | "data:image/jpeg;base64"
                | "data:image/jpg;base64"
                | "data:image/webp;base64"
        ) || content.len() > 16 * 1024 * 1024
        {
            return Err(invalid());
        }
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| invalid())?;
        self.path_bytes = self.path_bytes.saturating_add(content.len());
        if self.path_bytes > MAX_PATH_BYTES {
            return Err(AppError::new("pcb_limit", "嵌入图片超过场景大小限制"));
        }
        let (w, h) = (required(data, "width")?, required(data, "height")?);
        if w <= 0. || h <= 0. {
            return Err(invalid());
        }
        let local = tf.component(
            required(data, "startX")?,
            required(data, "startY")?,
            -number(data, "rotation", 0.),
            truth(&data["isMirror"]),
        );
        let mut geometry = geometry::rect(w / 2., h / 2., w, h, 0., 0., local)?;
        geometry.path.clear();
        self.add_paint(record, geometry, 8, true, 0., 0, parent, None, None)?;
        self.images.push(SceneImage {
            layer: 8,
            content: content.to_owned(),
            width: w,
            height: h,
            transform: [local.a, local.b, local.c, local.d, local.tx, local.ty],
        });
        Ok(())
    }

    fn sheet_frame(
        &mut self,
        record: &history::Record,
        tf: geometry::Transform,
        values: &BTreeMap<String, Value>,
    ) -> Result<()> {
        if values.get("Border").and_then(Value::as_str) != Some("1") {
            return Ok(());
        }
        let value = |key: &str, fallback: f64| {
            values
                .get(key)
                .and_then(Value::as_str)
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0. && *v < 1e6)
                .unwrap_or(fallback)
        };
        let (w, h, b) = (
            value("Width", 1170.),
            value("Height", 825.),
            value("Blade Width", 10.),
        );
        if 2. * b >= w.min(h) {
            return Err(invalid());
        }
        for inset in [0., b] {
            self.add_paint(
                record,
                geometry::rect(w / 2., -h / 2., w - 2. * inset, h - 2. * inset, 0., 0., tf)?,
                8,
                false,
                0.5,
                0,
                None,
                None,
                None,
            )?;
        }
        for (count, horizontal) in [
            (value("X Region Count", 6.) as usize, true),
            (value("Y Region Count", 4.) as usize, false),
        ] {
            if count > 1000 {
                return Err(invalid());
            }
            let length = if horizontal { w } else { h };
            for index in 0..count {
                let p = length * (index as f64 + 0.5) / count as f64;
                for opposite in [false, true] {
                    let (x, y) = if horizontal {
                        (p, if opposite { -b / 2. } else { -h + b / 2. })
                    } else {
                        (if opposite { w - b / 2. } else { b / 2. }, -p)
                    };
                    self.add_text(record,&json!({"x":x,"y":y,"value":(index+1).to_string(),"fontSize":5,"align":"CENTER_MIDDLE"}),tf,None)?;
                    if index > 0 {
                        let p = length * index as f64 / count as f64;
                        let line = if horizontal {
                            let y = if opposite { -b } else { -h };
                            json!([p, y, p, y + b])
                        } else {
                            let x = if opposite { w - b } else { 0. };
                            json!([x, -p, x + b, -p])
                        };
                        self.add_paint(
                            record,
                            geometry::path(&line, tf, 1., false)?,
                            8,
                            false,
                            0.5,
                            0,
                            None,
                            None,
                            None,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn component_values(&self, id: &str) -> BTreeMap<String, Value> {
        let attrs = self.page_attrs.get(id);
        let mut values: BTreeMap<String, Value> = Self::attr_value(attrs, "Device")
            .and_then(Value::as_str)
            .and_then(|id| self.project.docs.get(id))
            .and_then(|doc| doc.first("META"))
            .and_then(|meta| meta["attributes"].as_object())
            .map(|attrs| attrs.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        if let Some(attrs) = attrs {
            for (key, attr) in attrs {
                if !attr["value"].is_null() {
                    values.insert(key.clone(), attr["value"].clone());
                }
            }
        }
        for (key, field) in [
            ("@Project Name", "title"),
            ("@Create Date", "created_at"),
            ("@Update Date", "updated_at"),
        ] {
            if let Some(value) = self.project.metadata[field].as_str() {
                values.insert(
                    key.into(),
                    json!(if key.ends_with("Date") {
                        value.split(['T', ' ']).next().unwrap_or(value)
                    } else {
                        value
                    }),
                );
            }
        }
        values
    }

    fn component(&mut self, record: &history::Record) -> Result<()> {
        let data = record.data.as_ref().ok_or_else(invalid)?;
        let id = record_id(record);
        let attrs = self.page_attrs.get(&id).cloned();
        let values = self.component_values(&id);
        let symbol_ref = values
            .get("Symbol")
            .and_then(Value::as_str)
            .or_else(|| data["symbolId"].as_str())
            .or_else(|| data["symbol"].as_str());
        let Some(symbol) = symbol_ref
            .and_then(|id| self.project.docs.get(id))
            .filter(|doc| doc.head["docType"] == "SYMBOL" && !doc.deleted())
            .cloned()
        else {
            self.warn(format!("器件 {} 缺少符号，已保留属性", id));
            return self.fallback_component(record, data);
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
            return self.fallback_component(record, data);
        };
        let (x, y) = position(data)?;
        let mirrored = truth(&data["isMirror"]) || truth(&data["mirror"]);
        // EDA mirrors across page X after rotating the symbol.
        let tf = geometry::Transform::identity()
            .affine(if mirrored { -1. } else { 1. }, 0., 0., 1., x, y)
            .component(
                0.,
                0.,
                -number(data, "rotation", number(data, "angle", 0.)),
                false,
            );
        let mut symbol_attrs = HashMap::new();
        Self::index_attrs(&symbol, &mut symbol_attrs);
        let mut bounds = Bounds::empty();
        let before = self.paints.len();
        if symbol
            .first("META")
            .is_some_and(|meta| meta["docType"] == 20)
        {
            self.sheet_frame(record, tf, &values)?;
        }
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
                "PIN" => self.pin(child, tf, Some(&id), symbol_attrs.get(&record_id(child))),
                "ATTR" => {
                    let mut label = child.data.clone().ok_or_else(invalid)?;
                    let key = label["key"].as_str().unwrap_or("").to_owned();
                    if label["parentId"].as_str() == Some("") {
                        if attrs.as_ref().is_some_and(|attrs| attrs.contains_key(&key)) {
                            continue;
                        }
                        if let Some(value) = values.get(&key) {
                            label["value"] = value.clone();
                        }
                    }
                    self.symbol_attr(child, &label, tf, Some(&id))
                }
                "TABLE" => self.table(
                    child,
                    child.data.as_ref().ok_or_else(invalid)?,
                    tf,
                    Some(&id),
                    &values,
                ),
                "OBJ" => self.image(
                    child,
                    child.data.as_ref().ok_or_else(invalid)?,
                    tf,
                    Some(&id),
                ),
                "TEXT" => self.add_text(
                    child,
                    child.data.as_ref().ok_or_else(invalid)?,
                    tf,
                    Some(&id),
                ),
                "META" | "CANVAS" | "ELE_PLACEHOLDER" => Ok(()),
                "LINE" | "RECT" | "POLY" | "POLYGON" | "CIRCLE" | "ELLIPSE" | "ARC" | "BEZIER"
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
            return self.fallback_component(record, data);
        }
        let mut component_geometry = geometry::rect(
            (bounds.min_x + bounds.max_x) / 2.,
            (bounds.min_y + bounds.max_y) / 2.,
            bounds.max_x - bounds.min_x,
            bounds.max_y - bounds.min_y,
            0.,
            0.,
            geometry::Transform::identity(),
        )?;
        component_geometry.path.clear();
        let properties = self.properties(record, None);
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

    fn fallback_component(&mut self, record: &history::Record, data: &Value) -> Result<()> {
        let (x, y) = position(data)?;
        let w = width(data, &["width", "sizeX"], 20.);
        let h = width(data, &["height", "sizeY"], 12.);
        let geometry = geometry::rect(
            x,
            y,
            w,
            h,
            0.,
            -number(data, "rotation", number(data, "angle", 0.)),
            geometry::Transform::identity(),
        )?;
        let id = record_id(record);
        let properties = self.properties(record, None);
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
        attrs: Option<&BTreeMap<String, Value>>,
    ) -> Result<()> {
        let data = record.data.as_ref().ok_or_else(invalid)?;
        let (x, y) = position(data)?;
        let angle = -number(data, "rotation", number(data, "angle", 0.));
        let length = width(data, &["length", "pinLength"], 20.);
        let local = tf.component(
            x,
            y,
            angle,
            truth(&data["isMirror"]) || truth(&data["mirror"]),
        );
        let geometry = geometry::path(&json!([0., 0., length, 0.]), local, 1., false)?;
        let mut properties = self.properties(record, parent);
        if let Some(value) =
            Self::attr_value(attrs, "Pin Name").or_else(|| Self::attr_value(attrs, "NAME"))
        {
            properties.insert("PinName".into(), text(value));
        }
        if let Some(value) =
            Self::attr_value(attrs, "Pin Number").or_else(|| Self::attr_value(attrs, "NUMBER"))
        {
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
        data: &Value,
        tf: geometry::Transform,
        parent: Option<&str>,
    ) -> Result<()> {
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
            images: self.images,
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

    // Compile/pick sequences share a two-scene cache across parallel tests.
    fn scene_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn compiles_grouped_wire_segments_and_object_point_symbols() {
        let _guard = scene_test_guard();
        let log = r#"
{"type":"DOCHEAD"}||{"docType":"SCH_PAGE","uuid":"page","client":"a"}|
{"type":"WIRE","id":"wire","ticket":1}||{"groupId":"","locked":false,"zIndex":1}|
{"type":"LINE","id":"segment","ticket":1}||{"lineGroup":"wire","startX":10,"startY":20,"endX":40,"endY":20,"strokeWidth":null}|
{"type":"ATTR","id":"net","ticket":1}||{"parentId":"wire","key":"NET","value":"+3V3","valueVisible":false}|
{"type":"BUS","id":"bus","ticket":1}||{"groupId":"","locked":false}|
{"type":"LINE","id":"bus-segment","ticket":1}||{"lineGroup":"bus","startX":10,"startY":30,"endX":40,"endY":30}|
{"type":"CANVAS","id":"CANVAS","ticket":1}||{"originX":0,"originY":0}|
{"type":"ELE_PLACEHOLDER","id":"placeholder","ticket":1}||{"dataType":"WIRE","max":1}|
{"type":"COMPONENT","id":"component","ticket":1}||{"partId":"part","x":100,"y":50,"rotation":90}|
{"type":"ATTR","id":"symbol-ref","ticket":1}||{"parentId":"component","key":"Symbol","value":"symbol"}|
{"type":"DOCHEAD"}||{"docType":"SYMBOL","uuid":"symbol","client":"a"}|
{"type":"PART","id":"part","ticket":1}||{"BBOX":[0,0,10,10]}|
{"type":"META","id":"meta","ticket":1}||{"title":"symbol"}|
{"type":"CANVAS","id":"CANVAS","ticket":1}||{"originX":0,"originY":0}|
{"type":"ELE_PLACEHOLDER","id":"placeholder","ticket":1}||{"dataType":"POLY","max":1}|
{"type":"POLY","id":"body","ticket":1}||{"partId":"part","points":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":10}],"closed":false}|
"#;
        let mut project = history::Project::default();
        history::replay(&mut project, log).unwrap();
        let scene = compile(project, "page", "revision".into(), Metrics::default()).unwrap();
        assert!(scene.warnings.is_empty(), "{:?}", scene.warnings);
        let wire = pick(&scene.id, 25., 20., 0.1, None).unwrap().unwrap();
        assert_eq!(wire.kind, "WIRE");
        assert_eq!(wire.id, "wire");
        assert_eq!(wire.net.as_deref(), Some("+3V3"));
        assert_eq!(
            pick(&scene.id, 25., 30., 0.1, None).unwrap().unwrap().kind,
            "BUS"
        );
        let symbol = pick(&scene.id, 100., 45., 0.1, Some(&[6]))
            .unwrap()
            .unwrap();
        assert_eq!(symbol.kind, "POLY");
        assert!(
            pick(&scene.id, 105., 45., 0.1, Some(&[6]))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn renders_three_point_arcs_and_signed_radius_ellipses() {
        let _guard = scene_test_guard();
        let log = r#"
{"type":"DOCHEAD"}||{"docType":"SCH_PAGE","uuid":"page","client":"a"}|
{"type":"ARC","id":"arc","ticket":1}||{"startX":10,"startY":0,"referX":0,"referY":10,"endX":-10,"endY":0}|
{"type":"ARC","id":"reverse","ticket":1}||{"startX":40,"startY":0,"referX":30,"referY":-10,"endX":20,"endY":0}|
{"type":"ELLIPSE","id":"ellipse","ticket":1}||{"centerX":60,"centerY":0,"radiusX":10,"radiusY":-5,"rotation":90}|
"#;
        let mut project = history::Project::default();
        history::replay(&mut project, log).unwrap();
        let scene = compile(project, "page", "revision".into(), Metrics::default()).unwrap();
        assert!(scene.warnings.is_empty(), "{:?}", scene.warnings);
        for (x, y, id) in [
            (0., 10., "arc"),
            (30., -10., "reverse"),
            (65., 0., "ellipse"),
            (60., 10., "ellipse"),
        ] {
            assert_eq!(pick(&scene.id, x, y, 0.1, None).unwrap().unwrap().id, id);
        }
        assert!(pick(&scene.id, 65., 10., 0.1, None).unwrap().is_none());
    }

    #[test]
    fn picked_component_parts_share_resolved_device_properties() {
        let mut project = fixture_project();
        history::replay(&mut project, r#"
{"type":"DOCHEAD"}||{"docType":"SCH_PAGE","uuid":"page","client":"a"}|
{"type":"ATTR","id":"value","ticket":2}||{"parentId":"component","key":"Value","value":null,"valueVisible":true,"positionX":50,"positionY":50,"fontSize":6}|
{"type":"DOCHEAD"}||{"docType":"DEVICE","uuid":"device","client":"a"}|
{"type":"META","id":"meta","ticket":1}||{"title":"RC0402FR-07100KL","attributes":{"Symbol":"symbol","Footprint":"footprint","Value":"100kΩ","Name":"={Value}","Designator":"R?","Manufacturer":"YAGEO(国巨)","Supplier Part":"C60491","Tolerance":"±1%"}}|
{"type":"DOCHEAD"}||{"docType":"FOOTPRINT","uuid":"footprint","client":"a"}|
{"type":"META","id":"meta","ticket":1}||{"title":"R0402"}|
"#).unwrap();
        let page = &project.docs["page"];
        let mut builder = Builder::new(&project, page);
        for record in page.active() {
            builder.page_primitive(record).unwrap();
        }
        for paint in builder.paints.iter().filter(|paint| {
            paint.result.id == "component" || paint.result.id.starts_with("component/")
        }) {
            let props = &paint.result.properties;
            assert_eq!(props["Value"], "100kΩ", "{}", paint.result.id);
            assert_eq!(props["Designator"], "R1");
            assert_eq!(props["Name"], "100kΩ");
            assert_eq!(props["DeviceName"], "RC0402FR-07100KL");
            assert_eq!(props["FootprintName"], "R0402");
            assert_eq!(props["Supplier Part"], "C60491");
            assert_eq!(props["Tolerance"], "±1%");
        }
        assert!(
            builder
                .paints
                .iter()
                .any(|paint| paint.result.id == "component/body")
        );
        assert!(builder.texts.iter().any(|label| label.text == "100kΩ"));
        drop(builder);
        project.docs.remove("symbol");
        history::replay(&mut project, r#"
{"type":"DOCHEAD"}||{"docType":"SCH_PAGE","uuid":"page","client":"a"}|
{"type":"ATTR","id":"value","ticket":3}||{"parentId":"component","key":"Value","value":0,"valueVisible":false}|
"#).unwrap();
        let page = &project.docs["page"];
        let mut builder = Builder::new(&project, page);
        let component = page
            .active()
            .find(|record| record.kind == "COMPONENT")
            .unwrap();
        builder.component(component).unwrap();
        let props = &builder.paints.last().unwrap().result.properties;
        assert_eq!(props["Value"], "0");
        assert_eq!(props["Name"], "0");
        assert_eq!(props["DeviceName"], "RC0402FR-07100KL");
    }

    #[test]
    fn resolves_device_symbols_values_and_scoped_pin_attributes() {
        let _guard = scene_test_guard();
        let log = r#"
{"type":"DOCHEAD"}||{"docType":"SCH_PAGE","uuid":"page","client":"a"}|
{"type":"COMPONENT","id":"c","ticket":1}||{"partId":"part","x":100,"y":50}|
{"type":"ATTR","id":"device","ticket":1}||{"parentId":"c","key":"Device","value":"device","valueVisible":true,"x":90,"y":30}|
{"type":"ATTR","id":"value","ticket":1}||{"parentId":"c","key":"Value","value":null,"valueVisible":true,"x":90,"y":40}|
{"type":"COMPONENT","id":"missing","ticket":1}||{"partId":"part","symbolId":"absent","x":200,"y":50}|
{"type":"DOCHEAD"}||{"docType":"DEVICE","uuid":"device","client":"a"}|
{"type":"META","id":"meta","ticket":1}||{"attributes":{"Symbol":"symbol","Value":"100nF","Name":"={Value}"}}|
{"type":"DOCHEAD"}||{"docType":"SYMBOL","uuid":"symbol","client":"a"}|
{"type":"PART","id":"part","ticket":1}||{"BBOX":[0,0,10,10]}|
{"type":"PIN","id":"pin","ticket":1}||{"partId":"part","x":0,"y":0,"length":10}|
{"type":"ATTR","id":"name","ticket":1}||{"parentId":"pin","partId":"part","key":"Pin Name","value":"IN","valueVisible":true,"x":12,"y":0,"align":"RIGHT_MIDDLE"}|
{"type":"ATTR","id":"number","ticket":1}||{"parentId":"pin","partId":"part","key":"Pin Number","value":"1","valueVisible":false}|
{"type":"DOCHEAD"}||{"docType":"SYMBOL","uuid":"zzz","client":"a"}|
{"type":"ATTR","id":"name","ticket":1}||{"parentId":"pin","key":"Pin Name","value":"WRONG"}|
"#;
        let mut project = history::Project::default();
        history::replay(&mut project, log).unwrap();
        let scene = compile(project, "page", "r".into(), Metrics::default()).unwrap();
        assert_eq!(scene.texts.iter().filter(|t| t.text == "100nF").count(), 2);
        assert!(!scene.texts.iter().any(|t| t.text == "device"));
        let label = scene.texts.iter().find(|t| t.text == "IN").unwrap();
        assert_eq!(label.align, "right");
        let pin = pick(&scene.id, 105., 50., 0.1, Some(&[4]))
            .unwrap()
            .unwrap();
        assert_eq!(pin.properties["PinName"], "IN");
        assert_eq!(pin.properties["PinNumber"], "1");
        assert!(
            scene
                .warnings
                .iter()
                .any(|w| w.contains("missing") && w.contains("符号"))
        );
    }

    #[test]
    fn rotated_pins_end_at_their_page_wire_connection() {
        let _guard = scene_test_guard();
        let log = r#"
{"type":"DOCHEAD"}||{"docType":"SCH_PAGE","uuid":"page","client":"a"}|
{"type":"COMPONENT","id":"screw","ticket":1}||{"partId":"part","symbolId":"symbol","x":50,"y":-320,"rotation":90}|
{"type":"COMPONENT","id":"mirror","ticket":1}||{"partId":"part","symbolId":"symbol","x":100,"y":0,"isMirror":true}|
{"type":"DOCHEAD"}||{"docType":"SYMBOL","uuid":"symbol","client":"a"}|
{"type":"PART","id":"part","ticket":1}||{}|
{"type":"PIN","id":"pin","ticket":1}||{"partId":"part","x":10,"y":40,"rotation":90,"length":20}|
"#;
        let mut project = history::Project::default();
        history::replay(&mut project, log).unwrap();
        let scene = compile(project, "page", "r".into(), Metrics::default()).unwrap();
        let pin = pick(&scene.id, 90., -330., 0.1, Some(&[4])).unwrap();
        assert_eq!(pin.unwrap().id, "screw/pin");
        assert_eq!(
            pick(&scene.id, 90., 40., 0.1, Some(&[4]))
                .unwrap()
                .unwrap()
                .id,
            "mirror/pin"
        );
        assert!(
            pick(&scene.id, 10., -310., 0.1, Some(&[4]))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn renders_merged_title_cells_with_instance_templates() {
        let _guard = scene_test_guard();
        let log = r#"
{"type":"DOCHEAD"}||{"docType":"SCH_PAGE","uuid":"page","client":"a"}|
{"type":"COMPONENT","id":"sheet","ticket":1}||{"partId":"part","symbolId":"symbol","x":0,"y":0}|
{"type":"ATTR","id":"title","ticket":1}||{"parentId":"sheet","key":"@Schematic Name","value":"Moji"}|
{"type":"ATTR","id":"page-no","ticket":1}||{"parentId":"sheet","key":"@Page No","value":"1"}|
{"type":"ATTR","id":"page-count","ticket":1}||{"parentId":"sheet","key":"@Page Count","value":"2"}|
{"type":"DOCHEAD"}||{"docType":"SYMBOL","uuid":"symbol","client":"a"}|
{"type":"PART","id":"part","ticket":1}||{}|
{"type":"TABLE","id":"table","ticket":1}||{"partId":"part","startX":10,"startY":-20,"colSizes":[40,40],"rowSizes":[10,10],"tableCell":[{"rowIndex":0,"columnIndex":0,"rowSpan":1,"colSpan":2,"value":"={@Schematic Name}","fontStyle":{"fontSize":8,"hAlign":"CENTER","vAlign":"MIDDLE"}},{"rowIndex":0,"columnIndex":1,"value":"covered"},{"rowIndex":1,"columnIndex":0,"value":"页码"},{"rowIndex":1,"columnIndex":1,"value":"=页 {@Page No} 共 {@Page Count}"}]}|
"#;
        let mut project = history::Project::default();
        history::replay(&mut project, log).unwrap();
        let scene = compile(project, "page", "r".into(), Metrics::default()).unwrap();
        assert!(scene.warnings.is_empty(), "{:?}", scene.warnings);
        let label = scene.texts.iter().find(|t| t.text == "Moji").unwrap();
        assert_eq!((label.x, label.y), (50., -15.));
        assert_eq!(label.vertical_align, "middle");
        assert!(!scene.texts.iter().any(|t| t.text == "covered"));
        assert!(scene.texts.iter().any(|t| t.text == "页码"));
        assert!(scene.texts.iter().any(|t| t.text == "页 1 共 2"));
    }

    #[test]
    fn resolves_nested_and_multi_field_expressions_without_changing_literal_braces() {
        let values = BTreeMap::from([
            ("Name".into(), json!("={Value}")),
            ("Value".into(), json!("10kΩ")),
            ("@Page No".into(), json!("1")),
            ("@Page Count".into(), json!("2")),
        ]);
        assert_eq!(resolve_template("={Name}", &values), "10kΩ");
        assert_eq!(
            resolve_template("=页 {@Page No} 共 {@Page Count}", &values),
            "页 1 共 2"
        );
        assert_eq!(resolve_template("{literal}", &values), "{literal}");
    }

    #[test]
    fn embedded_objects_accept_only_bounded_image_data() {
        let project = fixture_project();
        let page = &project.docs["page"];
        let mut builder = Builder::new(&project, page);
        let record = page.active().find(|r| r.kind == "COMPONENT").unwrap();
        for content in [
            "https://example.com/image.png",
            "data:text/html;base64,PHNjcmlwdD4=",
            "data:image/png;base64,invalid!",
        ] {
            assert!(
                builder
                    .image(
                        record,
                        &json!({"content":content,"startX":0,"startY":0,"width":10,"height":10}),
                        geometry::Transform::identity(),
                        None
                    )
                    .is_err()
            );
        }
        builder.image(record,&json!({"content":"data:image/png;base64,AA==","startX":5,"startY":7,"width":10,"height":20}),geometry::Transform::identity(),None).unwrap();
        assert_eq!(builder.images[0].transform, [1., 0., 0., 1., 5., 7.]);
        assert_eq!(builder.images[0].height, 20.);
    }

    #[tokio::test]
    async fn rejects_invalid_document_ids_before_network_access() {
        let client = HttpClient::new().unwrap();
        let error = load(&client, "bad", "bad").await.unwrap_err();
        assert_eq!(error.code, "INVALID_ID");
    }

    #[test]
    fn compiles_common_schematic_records_and_picks_components() {
        let _guard = scene_test_guard();
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
        let pin = pick(&scene.id, 50., 20., 2., None).unwrap().unwrap();
        assert_eq!(pin.kind, "PIN");
        assert_eq!(pin.properties.get("PinName").unwrap(), "IN");
        let wire = pick(&scene.id, 25., 0., 2., None).unwrap().unwrap();
        assert_eq!(wire.kind, "WIRE");
        assert_eq!(wire.net.as_deref(), Some("GND"));
    }

    #[test]
    fn picking_bounds_do_not_cover_symbols_and_labels_with_filled_boxes() {
        let _guard = scene_test_guard();
        let scene = compile(
            fixture_project(),
            "page",
            "revision".into(),
            Metrics::default(),
        )
        .unwrap();
        assert!(scene.texts.iter().any(|text| text.text == "POWER"));
        assert!(
            scene
                .batches
                .iter()
                .all(|batch| batch.layer != 5 && batch.color.is_some())
        );
        assert_eq!(
            pick(&scene.id, 50., 40., 0.1, Some(&[3]))
                .unwrap()
                .unwrap()
                .kind,
            "COMPONENT"
        );
        assert_eq!(
            pick(&scene.id, 8., 22., 0.1, Some(&[5]))
                .unwrap()
                .unwrap()
                .kind,
            "TEXT"
        );
    }

    #[test]
    fn rejects_missing_page_and_warns_on_missing_symbol() {
        let _guard = scene_test_guard();
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
        let _guard = scene_test_guard();
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
        let _guard = scene_test_guard();
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
