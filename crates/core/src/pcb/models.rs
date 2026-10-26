use super::{
    geometry::Transform,
    history::{self, Project},
    solid::{self, Bounds3, MM, Mesh, MeshBuilder, PackedMesh},
};
use crate::{
    client::HttpClient,
    error::{AppError, Result},
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    io::Cursor,
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

const MODEL_LIMIT: usize = 12 * 1024 * 1024;

#[derive(Clone)]
pub struct ModelTransform([f64; 9]);
impl ModelTransform {
    pub fn parse(text: &str) -> Result<Self> {
        let values = text
            .split(',')
            .map(str::trim)
            .map(str::parse::<f64>)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| solid::invalid())?;
        let values: [f64; 9] = values.try_into().map_err(|_| solid::invalid())?;
        if !values.iter().all(|v| v.is_finite() && v.abs() <= 1e6)
            || values[0] <= 0.
            || values[1] <= 0.
            || values[2] < 0.
        {
            return Err(solid::invalid());
        }
        Ok(Self(values))
    }
}

#[derive(Clone)]
pub struct Binding {
    pub key: String,
    pub title: String,
    pub position: [f64; 2],
    pub angle: f64,
    pub bottom: bool,
    pub transform: ModelTransform,
}
impl Binding {
    pub fn matrix(&self, b: &Bounds3, thickness: f64) -> [f32; 16] {
        let v = self.transform.0;
        let sx = v[0] * MM / (b.max[0] - b.min[0]).max(1e-8);
        let sy = v[1] * MM / (b.max[1] - b.min[1]).max(1e-8);
        // A zero source sizeZ requests automatic height using the XY scale.
        let sz = if v[2] == 0. {
            (sx * sy).sqrt()
        } else {
            v[2] * MM / (b.max[2] - b.min[2]).max(1e-8)
        };
        let rotate = |a: f64, b: f64, deg: f64| {
            let (s, c) = deg.to_radians().sin_cos();
            (c * a - s * b, s * a + c * b)
        };
        let apply = |p: [f64; 3]| {
            let mut x = (p[0] - (b.max[0] + b.min[0]) / 2.) * sx;
            let mut y = (p[1] - (b.max[1] + b.min[1]) / 2.) * sy;
            let mut z = (p[2] - b.min[2]) * sz;
            (z, x) = rotate(z, x, v[5]);
            (y, z) = rotate(y, z, v[4]);
            (x, y) = rotate(x, y, v[3]);
            x += v[6] * MM;
            y += v[7] * MM;
            z += v[8] * MM;
            if self.bottom {
                y = -y;
                z = -z;
            }
            (x, y) = rotate(x, y, self.angle);
            [
                x + self.position[0],
                y + self.position[1],
                z + if self.bottom { 0. } else { thickness },
            ]
        };
        let o = apply([0., 0., 0.]);
        let x = apply([1., 0., 0.]);
        let y = apply([0., 1., 0.]);
        let z = apply([0., 0., 1.]);
        [
            x[0] - o[0],
            x[1] - o[1],
            x[2] - o[2],
            0.,
            y[0] - o[0],
            y[1] - o[1],
            y[2] - o[2],
            0.,
            z[0] - o[0],
            z[1] - o[1],
            z[2] - o[2],
            0.,
            o[0],
            o[1],
            o[2],
            1.,
        ]
        .map(|v| v as f32)
    }
}

fn model_reference(key: &str) -> Result<(&str, Option<&str>)> {
    let (component, path) = key
        .split_once('|')
        .map_or((key, None), |(id, path)| (id, Some(path)));
    crate::api::validate_id(component)?;
    if let Some(path) = path
        && path != "user"
    {
        crate::api::validate_id(path)?;
    }
    Ok((component, path))
}

fn model_request(key: &str) -> Result<(String, Option<&str>)> {
    let (component, path) = model_reference(key)?;
    if path == Some("user") {
        return Ok((
            format!("https://pro.lceda.cn/api/components/{component}?uuid={component}"),
            None,
        ));
    }
    let url = match path {
        Some(path) => format!(
            "https://pro.lceda.cn/api/v5/components/{component}?uuid={component}&path={path}"
        ),
        None => format!("https://pro.lceda.cn/api/v5/components/{component}?uuid={component}"),
    };
    Ok((url, path))
}

pub fn binding(
    project: &Project,
    d: &Value,
    overrides: &BTreeMap<String, Value>,
    tf: Transform,
) -> Result<Option<Binding>> {
    let inline = d["attrs"].as_object();
    let device = overrides
        .get("Device")
        .or_else(|| inline.and_then(|a| a.get("Device")))
        .and_then(Value::as_str)
        .and_then(|id| project.docs.get(id))
        .filter(|d| !d.deleted() && d.head["docType"] == "DEVICE");
    let defaults = device
        .and_then(|d| d.first("META"))
        .and_then(|v| v["attributes"].as_object());
    let attr = |key: &str| {
        overrides
            .get(key)
            .or_else(|| inline.and_then(|a| a.get(key)))
            .or_else(|| defaults.and_then(|a| a.get(key)))
            .and_then(Value::as_str)
    };
    let Some(key) = attr("3D Model").map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    model_reference(key)?;
    let transform = ModelTransform::parse(attr("3D Model Transform").unwrap_or_default())?;
    let coordinate = |a: &str, b: &str| {
        d.get(a)
            .or_else(|| d.get(b))
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite() && v.abs() < 1e8)
            .ok_or_else(solid::invalid)
    };
    let p = tf.point((coordinate("x", "positionX")?, coordinate("y", "positionY")?));
    let angle = d["angle"].as_f64().unwrap_or(0.);
    if !angle.is_finite() || angle.abs() > 1e6 {
        return Err(solid::invalid());
    }
    Ok(Some(Binding {
        key: key.into(),
        title: attr("Designator")
            .or_else(|| attr("3D Model Title"))
            .unwrap_or("器件")
            .chars()
            .take(128)
            .collect(),
        position: [p.0 * MM, -p.1 * MM],
        angle,
        bottom: d["layerId"] == 2,
        transform,
    }))
}

pub fn parse_obj(bytes: &[u8]) -> Result<Mesh> {
    if bytes.len() > MODEL_LIMIT {
        return Err(AppError::new("pcb_3d_limit", "器件模型超过 12 MiB 限制"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| solid::invalid())?;
    let mut lines = 0;
    let mut materials = 0;
    for line in text.lines() {
        let line = line.trim();
        let kind = line.split_whitespace().next().unwrap_or_default();
        lines += 1;
        if kind == "newmtl" {
            materials += 1;
        }
        if lines > 800_000
            || materials > 256
            || line.len() > 65536
            || (kind == "f" && line.split_whitespace().count() > 257)
        {
            return Err(AppError::new("pcb_3d_limit", "模型结构超过安全大小限制"));
        }
    }
    let mut data = String::from("mtllib embedded\n");
    for line in text
        .lines()
        .filter(|line| line.split_whitespace().next() != Some("mtllib"))
    {
        data.push_str(line);
        data.push('\n');
    }
    let options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ignore_lines: true,
        ignore_points: true,
    };
    // Official OBJ resources inline MTL records. Never fetch filenames or map_* URLs.
    let (models, materials) =
        tobj::load_obj_buf(&mut Cursor::new(data.as_bytes()), &options, |_| {
            tobj::load_mtl_buf(&mut Cursor::new(text.as_bytes()))
        })
        .map_err(|_| solid::invalid())?;
    let materials = materials.map_err(|_| solid::invalid())?;
    let mut result = MeshBuilder::new();
    for model in models {
        let m = model.mesh;
        if m.indices.len() + result.mesh.indices.len() > 400_000
            || !m.positions.iter().all(|v| v.is_finite() && v.abs() < 1e6)
        {
            return Err(AppError::new(
                "pcb_3d_limit",
                "器件模型的顶点数或坐标超过限制",
            ));
        }
        let color = m
            .material_id
            .and_then(|i| materials.get(i))
            .and_then(|m| m.diffuse)
            .unwrap_or([0.42, 0.45, 0.48])
            .map(|v| v.clamp(0., 1.));
        for tri in m.indices.as_chunks::<3>().0 {
            let mut points = [[0.; 3]; 3];
            for (p, index) in points.iter_mut().zip(tri) {
                let i = *index as usize * 3;
                *p = m
                    .positions
                    .get(i..i + 3)
                    .ok_or_else(solid::invalid)?
                    .try_into()
                    .map_err(|_| solid::invalid())?;
            }
            let mut normals = [None; 3];
            for (corner, index) in tri.iter().enumerate() {
                if let Some(n) = m.normals.get(*index as usize * 3..*index as usize * 3 + 3) {
                    let length = n.iter().map(|v| v * v).sum::<f32>().sqrt();
                    if !length.is_finite() {
                        return Err(solid::invalid());
                    }
                    if length > 1e-8 {
                        normals[corner] = Some(n.try_into().unwrap());
                    }
                }
            }
            result.triangle_with_normals(points, normals, color)?;
        }
    }
    if result.mesh.vertices.is_empty() {
        return Err(solid::invalid());
    }
    Ok(result.finish())
}

#[derive(Serialize)]
pub struct Instance {
    pub title: String,
    pub matrix: [f32; 16],
}
#[derive(Serialize)]
pub struct Model3d {
    pub key: String,
    #[serde(rename = "meshKey")]
    pub mesh_key: String,
    pub mesh: PackedMesh,
    pub instances: Vec<Instance>,
}
type Cache = VecDeque<(String, String, Arc<Mesh>)>;
fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn unavailable() -> AppError {
    AppError::new("pcb_3d_unavailable", "官方未提供可读取的器件模型")
}

fn descriptor_model(data: &str) -> Result<String> {
    let value: Value = serde_json::from_str(data).map_err(|_| unavailable())?;
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .or_else(|| value.get("3d_model_uuid").and_then(Value::as_str))
        .filter(|model| !model.is_empty())
        .ok_or_else(unavailable)?;
    crate::api::validate_id(model)?;
    Ok(model.to_owned())
}

async fn resolve_model(client: &HttpClient, metadata: &Value) -> Result<String> {
    if metadata["success"] != true || metadata["code"] != 0 {
        return Err(unavailable());
    }
    let result = metadata["result"].as_object().ok_or_else(unavailable)?;
    if let Some(data_str_id) = result
        .get("dataStrId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        let key = result
            .get("key")
            .and_then(Value::as_str)
            .ok_or_else(unavailable)?;
        let iv = result
            .get("iv")
            .and_then(Value::as_str)
            .ok_or_else(unavailable)?;
        let bytes = client.get_bytes(data_str_id, MODEL_LIMIT).await?;
        let data = history::decode(key, iv, &bytes, MODEL_LIMIT)?;
        return descriptor_model(&data);
    }
    if let Some(data_str) = result
        .get("dataStr")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        return descriptor_model(data_str);
    }
    let model = result
        .get("3d_model_uuid")
        .and_then(Value::as_str)
        .filter(|model| !model.is_empty())
        .ok_or_else(unavailable)?;
    crate::api::validate_id(model)?;
    Ok(model.to_owned())
}

pub async fn load(
    client: &HttpClient,
    key: &str,
    bindings: Vec<Binding>,
    thickness: f64,
) -> Result<Model3d> {
    if !bindings.iter().any(|b| b.key == key) {
        return Err(AppError::new("invalid_request", "模型不属于当前 PCB"));
    }
    let start = Instant::now();
    let (metadata_url, path) = model_request(key)?;
    let cached = cache()
        .lock()
        .map_err(|_| solid::invalid())?
        .iter()
        .find(|(k, _, _)| k == key)
        .map(|(_, model, mesh)| (model.clone(), Arc::clone(mesh)));
    let (mesh_key, mesh) = if let Some((model, mesh)) = cached {
        (model, mesh)
    } else {
        let metadata = if let Some(path) = path {
            client
                .get_json_with_headers(
                    &metadata_url,
                    &[("Content-Type", "application/json"), ("path", path)],
                )
                .await?
        } else {
            client.get_json(&metadata_url).await?
        };
        let model = resolve_model(client, &metadata).await?;
        let bytes = client
            .get_bytes(
                &format!("https://modules.lceda.cn/3dmodel/{model}"),
                MODEL_LIMIT,
            )
            .await?;
        let mesh = tokio::task::spawn_blocking(move || parse_obj(&bytes).map(Arc::new))
            .await
            .map_err(|_| solid::invalid())??;
        let mut cache = cache().lock().map_err(|_| solid::invalid())?;
        cache.push_back((key.into(), model.clone(), Arc::clone(&mesh)));
        while cache.len() > 16
            || cache
                .iter()
                .map(|(_, _, m)| (m.vertices.len() + m.indices.len()) * 4)
                .sum::<usize>()
                > 48 * 1024 * 1024
        {
            cache.pop_front();
        }
        (model, mesh)
    };
    let key = key.to_owned();
    let result = tokio::task::spawn_blocking(move || {
        let instances = bindings
            .iter()
            .filter(|b| b.key == key)
            .map(|b| Instance {
                title: b.title.clone(),
                matrix: b.matrix(&mesh.bounds, thickness),
            })
            .collect::<Vec<_>>();
        if !instances
            .iter()
            .flat_map(|i| i.matrix)
            .all(|v| v.is_finite() && v.abs() < 1e8)
        {
            return Err(solid::invalid());
        }
        Ok(Model3d {
            key,
            mesh_key,
            mesh: mesh.packed(),
            instances,
        })
    })
    .await
    .map_err(|_| solid::invalid())??;
    tracing::info!(
        instances = result.instances.len(),
        vertices = result.mesh.vertex_count,
        elapsed_ms = start.elapsed().as_millis() as u64,
        "pcb.model.complete"
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OBJ: &str =
        "newmtl gold\nKd 0.8 0.6 0.2\nendmtl\nv 0 0 0\nv 2 0 0\nv 0 1 0\nusemtl gold\nf 1 2 3\n";

    #[test]
    fn official_obj_style_inline_materials_produce_finite_colored_triangles() {
        let mesh = parse_obj(OBJ.as_bytes()).unwrap();
        assert_eq!(mesh.vertices.len(), 27);
        assert_eq!(mesh.bounds.max, [2., 1., 0.]);
        assert!((mesh.vertices[6] - 0.8).abs() < 0.0001);
        assert!(mesh.vertices.iter().all(|v| v.is_finite()));
        assert!((mesh.vertices[5] - 1.).abs() < 0.001);
    }

    #[test]
    fn shared_obj_positions_keep_distinct_corner_normals() {
        let obj = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvn 0 0 2\nvn 0 2 0\nf 1//1 2//1 3//1\nf 1//2 3//2 4//2\n";
        let mesh = parse_obj(obj.as_bytes()).unwrap();
        assert_eq!(mesh.vertices.len(), 54);
        assert_eq!(mesh.indices, [0, 1, 2, 3, 4, 5]);
        assert_eq!(&mesh.vertices[3..6], &[0., 0., 1.]);
        assert_eq!(&mesh.vertices[30..33], &[0., 1., 0.]);
    }

    #[test]
    fn malformed_and_oversized_models_are_rejected() {
        for bytes in [
            b"v NaN 0 0\nf 1 1 1".as_slice(),
            b"v 0 0 0\nf 1 2 9",
            b"<html>blocked</html>",
        ] {
            assert!(parse_obj(bytes).is_err());
        }
        assert!(parse_obj(&vec![b' '; 12 * 1024 * 1024 + 1]).is_err());
    }

    #[test]
    fn official_model_descriptors_support_data_str_and_legacy_metadata() {
        let model = "d4738b125a044f51b0a4c55602341077";
        assert_eq!(
            descriptor_model(&format!(r#"{{"model":"{model}"}}"#)).unwrap(),
            model
        );
        assert_eq!(
            descriptor_model(&format!(r#"{{"3d_model_uuid":"{model}"}}"#)).unwrap(),
            model
        );
        assert!(descriptor_model(r#"{"model":"not-an-id"}"#).is_err());
    }

    #[test]
    fn bottom_placement_mirrors_local_y_and_height_before_board_rotation() {
        let transform = ModelTransform::parse("78.74015748,39.37007874,0,0,0,0,0,0,0").unwrap();
        let binding = Binding {
            key: "a".repeat(32),
            title: "R1".into(),
            position: [10., 20.],
            angle: 90.,
            bottom: true,
            transform,
        };
        let bounds = super::super::solid::Bounds3 {
            min: [0., 0., 0.],
            max: [2., 1., 0.5],
        };
        let matrix = binding.matrix(&bounds, 1.6);
        let point = |p: [f64; 3]| -> [f64; 3] {
            std::array::from_fn(|r| {
                matrix[r] as f64 * p[0]
                    + matrix[4 + r] as f64 * p[1]
                    + matrix[8 + r] as f64 * p[2]
                    + matrix[12 + r] as f64
            })
        };
        let p = point([2., 1., 0.5]);
        assert!(
            (p[0] - 10.5).abs() < 0.001 && (p[1] - 21.).abs() < 0.001 && (p[2] + 0.5).abs() < 0.001,
            "{p:?}"
        );
        for bad in ["", "1,2", "NaN,1,0,0,0,0,0,0,0", "-1,2,0,0,0,0,0,0,0"] {
            assert!(ModelTransform::parse(bad).is_err());
        }
    }

    #[test]
    fn device_defaults_and_explicit_empty_component_override_are_distinct() {
        use super::super::{geometry::Transform, history};
        let mut p = history::Project::default();
        history::replay(&mut p,r#"{"type":"DOCHEAD"}||{"uuid":"device","docType":"DEVICE"}|
{"type":"META"}||{"attributes":{"3D Model":"36a51122f27c49738f86b3bfa98863e0|0819f05c4eef4c71ace90d822a990e87","3D Model Transform":"40,20,0,0,0,0,0,0,0"}}|"#).unwrap();
        let d = serde_json::json!({"x":100.,"y":200.,"layerId":1,"attrs":{"Device":"device","Designator":"R1"}});
        let defaults = std::collections::BTreeMap::new();
        let found = binding(&p, &d, &defaults, Transform::board(0., 0.))
            .unwrap()
            .unwrap();
        assert_eq!(found.title, "R1");
        assert_eq!(found.position, [2.54, 5.08]);
        let overrides =
            std::collections::BTreeMap::from([("3D Model".into(), serde_json::json!(""))]);
        assert!(
            binding(&p, &d, &overrides, Transform::board(0., 0.))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn user_library_model_references_preserve_their_path_when_binding() {
        let d = serde_json::json!({
            "x": 100., "y": 200., "layerId": 1,
            "attrs": {
                "Designator": "AFB1",
                "3D Model": "58113d7c82f846a381a07583ea0be0ec|user",
                "3D Model Transform": "62.992,31.496,31.496,0,0,0,0,0,0"
            }
        });
        let found = binding(
            &Project::default(),
            &d,
            &BTreeMap::new(),
            Transform::board(0., 0.),
        )
        .unwrap()
        .unwrap();
        assert_eq!(found.key, "58113d7c82f846a381a07583ea0be0ec|user");
        assert_eq!(found.title, "AFB1");
    }

    #[test]
    fn model_references_allow_user_paths_but_still_validate_ids() {
        let id = "58113d7c82f846a381a07583ea0be0ec";
        assert_eq!(model_reference(id).unwrap(), (id, None));
        let user = format!("{id}|user");
        assert_eq!(model_reference(&user).unwrap(), (id, Some("user")));
        let library = format!("{id}|0819f05c4eef4c71ace90d822a990e87");
        assert_eq!(
            model_reference(&library).unwrap(),
            (id, Some("0819f05c4eef4c71ace90d822a990e87"))
        );
        assert!(model_reference("user").is_err());
        for suffix in [
            "",
            "USER",
            "other",
            "../user",
            "user|user",
            "user?path=other",
        ] {
            assert!(model_reference(&format!("{id}|{suffix}")).is_err());
        }
    }

    #[test]
    fn metadata_requests_select_legacy_user_models_and_preserve_modern_paths() {
        assert_eq!(
            model_request("58113d7c82f846a381a07583ea0be0ec|user").unwrap(),
            (
                "https://pro.lceda.cn/api/components/58113d7c82f846a381a07583ea0be0ec?uuid=58113d7c82f846a381a07583ea0be0ec".into(),
                None
            )
        );
        assert_eq!(
            model_request("0436eca5925c4476b8c12039190a48c8").unwrap(),
            (
                "https://pro.lceda.cn/api/v5/components/0436eca5925c4476b8c12039190a48c8?uuid=0436eca5925c4476b8c12039190a48c8".into(),
                None
            )
        );
        assert_eq!(
            model_request("36a51122f27c49738f86b3bfa98863e0|0819f05c4eef4c71ace90d822a990e87").unwrap(),
            (
                "https://pro.lceda.cn/api/v5/components/36a51122f27c49738f86b3bfa98863e0?uuid=36a51122f27c49738f86b3bfa98863e0&path=0819f05c4eef4c71ace90d822a990e87".into(),
                Some("0819f05c4eef4c71ace90d822a990e87")
            )
        );
    }
}
