use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::io::Read;

#[derive(Clone, Debug, Default)]
pub struct Project {
    pub metadata: Value,
    pub docs: BTreeMap<String, Document>,
    pub record_count: usize,
}
#[derive(Clone, Debug, Default)]
pub struct Document {
    pub head: Value,
    pub records: BTreeMap<String, Record>,
}
#[derive(Clone, Debug)]
pub struct Record {
    pub kind: String,
    pub id: Value,
    pub ticket: u64,
    pub client: String,
    // Keep opaque original fields without retaining a JSON object tree per record.
    #[allow(dead_code)]
    pub header: String,
    pub has_ticket: bool,
    pub data: Option<Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct History {
    pub uuid: String,
    pub parent: Option<String>,
    pub key: String,
    #[serde(default)]
    pub iv: String,
    #[serde(default)]
    pub num: u32,
    pub snapshot: Option<String>,
    #[serde(default)]
    pub snapshot_num: u32,
    #[serde(rename = "dataStrUrl")]
    pub data_url: String,
    #[serde(rename = "snapshotUrl")]
    pub snapshot_url: Option<String>,
}

pub const MAX_DECODED: usize = 256 * 1024 * 1024;
// Replayed updates include superseded records from earlier versions.
pub const MAX_RECORDS: usize = 4_000_000;

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new("pcb_format", message)
}

pub fn extend_chain(
    histories: &mut Vec<History>,
    seen: &mut HashSet<String>,
    page: Vec<History>,
    mut cursor: String,
) -> Result<Option<String>> {
    let mut page: BTreeMap<_, _> = page.into_iter().map(|h| (h.uuid.clone(), h)).collect();
    if !page.contains_key(&cursor) {
        return Err(invalid("工程历史链不完整：分页未返回请求的版本"));
    }
    loop {
        if seen.contains(&cursor) {
            return Err(invalid("历史链存在循环"));
        }
        let Some(h) = page.remove(&cursor) else {
            if histories
                .last()
                .is_none_or(|h| h.parent.as_deref() != Some(cursor.as_str()))
            {
                return Err(invalid("工程历史链不完整"));
            }
            return Ok(Some(cursor));
        };
        seen.insert(cursor);
        let parent = if h.snapshot.as_ref().is_some_and(|s| !s.is_empty()) {
            None
        } else {
            h.parent.clone()
        };
        histories.push(h);
        if histories.len() > 256 {
            return Err(AppError::new("pcb_limit", "工程历史超过 256 个版本限制"));
        }
        let Some(parent) = parent else {
            return Ok(None);
        };
        cursor = parent;
    }
}

pub fn replay(project: &mut Project, text: &str) -> Result<()> {
    if text.len() > MAX_DECODED {
        return Err(invalid("PCB log exceeds size limit"));
    }
    let mut current: Option<(String, String)> = None;
    for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let error = || invalid(format!("Invalid PCB log at line {}", index + 1));
        if line.len() > 32 * 1024 * 1024 {
            return Err(error());
        }
        let (header, body) = line.split_once("||").ok_or_else(error)?;
        let header_text = header;
        let header: Value = serde_json::from_str(header).map_err(|_| error())?;
        let body = body.strip_suffix('|').unwrap_or(body).trim();
        if header.as_object().is_some_and(|h| h.is_empty()) && body.is_empty() {
            current = None;
            continue;
        }
        let kind = header["type"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 64)
            .ok_or_else(error)?;
        let data: Option<Value> = if body.is_empty() {
            None
        } else {
            let value: Value = serde_json::from_str(body).map_err(|_| error())?;
            if value.is_null() || value.as_str() == Some("") {
                None
            } else if value.is_object() {
                Some(value)
            } else {
                return Err(error());
            }
        };
        if kind == "EDIT_HEAD" {
            current = None;
            continue;
        }
        if kind == "DOCHEAD" {
            let head = data.ok_or_else(error)?;
            let uuid = head["uuid"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 128)
                .ok_or_else(error)?
                .to_owned();
            let client = head["client"].as_str().unwrap_or("");
            if client.len() > 128 {
                return Err(invalid("PCB client ID exceeds 128 bytes"));
            }
            let client = client.to_owned();
            if !project.docs.contains_key(&uuid) && project.docs.len() >= 20_000 {
                return Err(invalid("Too many project documents"));
            }
            let doc = project.docs.entry(uuid.clone()).or_default();
            if doc.head.is_null()
                || head["updateTime"].as_u64().unwrap_or(0)
                    >= doc.head["updateTime"].as_u64().unwrap_or(0)
            {
                doc.head = head;
            }
            current = Some((uuid, client));
            continue;
        }
        let (uuid, doc_client) = current.as_ref().ok_or_else(error)?;
        project.record_count += 1;
        if project.record_count > MAX_RECORDS {
            return Err(invalid("Too many PCB records"));
        }
        let mut id = header
            .get("id")
            .cloned()
            .unwrap_or_else(|| Value::String(kind.to_owned()));
        if let Some(s) = id.as_str().filter(|s| s.starts_with('['))
            && let Ok(value @ Value::Array(_)) = serde_json::from_str::<Value>(s)
        {
            id = value;
        }
        let key = format!("{}\u{1f}{}", kind, id);
        let ticket = header
            .get("ticket")
            .map(|v| v.as_u64().ok_or_else(error))
            .transpose()?
            .unwrap_or(0);
        let client = header["client"].as_str().unwrap_or(doc_client);
        if client.len() > 128 {
            return Err(invalid("PCB client ID exceeds 128 bytes"));
        }
        let client = client.to_owned();
        let records = &mut project.docs.get_mut(uuid).ok_or_else(error)?.records;
        let replace = records
            .get(&key)
            .is_none_or(|old| ticket > old.ticket || (ticket == old.ticket && client < old.client));
        if replace {
            records.insert(
                key,
                Record {
                    kind: kind.to_owned(),
                    id,
                    ticket,
                    client,
                    header: header_text.to_owned(),
                    has_ticket: header.get("ticket").is_some(),
                    data,
                },
            );
        }
    }
    Ok(())
}

impl Document {
    pub fn active(&self) -> impl Iterator<Item = &Record> {
        self.records.values().filter(|r| r.data.is_some())
    }
    pub fn first(&self, kind: &str) -> Option<&Value> {
        self.active()
            .find(|r| r.kind == kind)
            .and_then(|r| r.data.as_ref())
    }
    pub fn deleted(&self) -> bool {
        self.first("DELETE_DOC")
            .is_some_and(|d| d["isDelete"] == true || d["isDelete"] == 1)
    }
}

pub fn decode(key: &str, iv: &str, bytes: &[u8], limit: usize) -> Result<String> {
    use aes_gcm::{
        AesGcm, Nonce,
        aead::{
            Aead, KeyInit,
            consts::{U12, U16},
        },
        aes::{Aes128, Aes256},
    };
    let key = hex::decode(key).map_err(|_| invalid("Invalid history encryption metadata"))?;
    let iv = hex::decode(iv).map_err(|_| invalid("Invalid history encryption metadata"))?;
    macro_rules! decrypt {
        ($aes:ty,$nonce:ty) => {
            AesGcm::<$aes, $nonce>::new_from_slice(&key)
                .map_err(|_| invalid("Invalid AES key length"))?
                .decrypt(Nonce::<$nonce>::from_slice(&iv), bytes)
        };
    }
    let plain = match (key.len(), iv.len()) {
        (16, 12) => decrypt!(Aes128, U12),
        (16, 16) => decrypt!(Aes128, U16),
        (32, 12) => decrypt!(Aes256, U12),
        (32, 16) => decrypt!(Aes256, U16),
        _ => return Err(invalid("Unsupported history key or IV length")),
    }
    .map_err(|_| invalid("History authentication failed"))?;
    let mut output = Vec::new();
    flate2::read::MultiGzDecoder::new(plain.as_slice())
        .take((limit as u64).saturating_add(1))
        .read_to_end(&mut output)
        .map_err(|_| invalid("Invalid history gzip data"))?;
    if output.len() > limit {
        return Err(AppError::new(
            "pcb_limit",
            "Decompressed PCB history exceeds limit",
        ));
    }
    String::from_utf8(output).map_err(|_| invalid("History is not UTF-8"))
}

pub fn parts(history: &History) -> Result<Vec<(String, String)>> {
    let (url, iv, count) =
        if let Some(snapshot) = history.snapshot.as_ref().filter(|s| !s.is_empty()) {
            (
                history
                    .snapshot_url
                    .as_ref()
                    .ok_or_else(|| invalid("Snapshot URL missing"))?,
                snapshot.as_str(),
                history.snapshot_num,
            )
        } else {
            (
                &history.data_url,
                if history.iv.is_empty() {
                    history.uuid.as_str()
                } else {
                    history.iv.as_str()
                },
                history.num,
            )
        };
    if count > 512 {
        return Err(AppError::new("pcb_limit", "Too many history parts"));
    }
    Ok((0..=count)
        .map(|i| {
            (
                if i == 0 {
                    url.clone()
                } else {
                    format!("{url}-{i}")
                },
                iv.to_owned(),
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::{
        AesGcm,
        aead::{Aead, KeyInit, consts::U16},
        aes::Aes128,
    };
    use flate2::{Compression, write::GzEncoder};
    use std::io::Write;

    // Authored fixtures; no board artwork or proprietary implementation is embedded.
    const LOG: &str = r#"{"type":"DOCHEAD"}||{"docType":"PCB","uuid":"board","client":"b"}|
{"type":"LINE","id":"track","ticket":2}||{"startX":0,"startY":0,"endX":20,"endY":10,"width":5,"layerId":1}|
{"type":"LINE","id":"deleted","ticket":5}||{"width":5}|
{"type":"LINE","id":"deleted","ticket":6}|||
{"type":"LAYER","id":"[\"LAYER\",1]","ticket":3}||{"layerId":1,"layerName":"stale"}|
{"type":"LAYER","id":["LAYER",1],"ticket":4}||{"layerId":1,"layerName":"top"}|
{"type":"DOCHEAD"}||{"docType":"FOOTPRINT","uuid":"lib","client":"a"}|
{"type":"PAD","id":"track","ticket":1}||{"num":"1"}|"#;

    #[test]
    fn replay_retains_tombstones_and_normalizes_composite_ids_with_document_scope() {
        let mut p = Project::default();
        replay(&mut p, LOG).unwrap();
        assert_eq!(p.docs.len(), 2);
        let board = &p.docs["board"];
        assert_eq!(board.records.len(), 3);
        assert!(
            board
                .records
                .values()
                .any(|r| r.id == "deleted" && r.data.is_none() && r.ticket == 6)
        );
        assert!(
            board
                .records
                .values()
                .any(|r| r.kind == "LAYER" && r.data.as_ref().unwrap()["layerName"] == "top")
        );
        assert_eq!(p.docs["lib"].records.len(), 1);
    }
    #[test]
    fn larger_ticket_wins_and_equal_ticket_prefers_smaller_client() {
        let mut p = Project::default();
        replay(&mut p, LOG).unwrap();
        replay(
            &mut p,
            r#"{"type":"DOCHEAD"}||{"docType":"PCB","uuid":"board","client":"a"}|
{"type":"LINE","id":"track","ticket":2}||{"width":7}|
{"type":"LINE","id":"track","ticket":1}||{"width":99}|
{"type":"LINE","id":"deleted","ticket":4}||{"width":99}|"#,
        )
        .unwrap();
        assert!(
            p.docs["board"]
                .records
                .values()
                .any(|r| r.id == "track" && r.data.as_ref().unwrap()["width"] == 7)
        );
        assert!(
            p.docs["board"]
                .records
                .values()
                .any(|r| r.id == "deleted" && r.data.is_none())
        );
    }
    #[test]
    fn replay_preserves_original_header_json_with_unknown_fields_and_conflicts() {
        let header = r#"{ "type":"LINE", "id":"[\"LINE\",1]", "ticket":0, "client":"a", "opaque":{"future":[true,null,{"label":"值\\\""}]}} "#;
        let mut p = Project::default();
        replay(
            &mut p,
            &format!(
                "{{\"type\":\"DOCHEAD\"}}||{{\"uuid\":\"board\",\"client\":\"z\"}}|\n{header}||{{\"width\":5}}|"
            ),
        )
        .unwrap();
        let record = p.docs["board"].records.values().next().unwrap();
        assert_eq!(record.header, header);
        assert_eq!(record.id, serde_json::json!(["LINE", 1]));
        assert_eq!(record.ticket, 0);
        replay(
            &mut p,
            r#"{"type":"DOCHEAD"}||{"uuid":"board","client":"z"}|
{"type":"LINE","id":["LINE",1],"ticket":0,"client":"b","opaque":"loser"}||{"width":99}|"#,
        )
        .unwrap();
        assert_eq!(
            p.docs["board"].records.values().next().unwrap().header,
            header
        );
        let tombstone =
            r#"{"type":"LINE","id":["LINE",1],"ticket":1,"opaque":{"deletion":"future"}}"#;
        replay(
            &mut p,
            &format!(
                "{{\"type\":\"DOCHEAD\"}}||{{\"uuid\":\"board\",\"client\":\"z\"}}|\n{tombstone}|||"
            ),
        )
        .unwrap();
        let record = p.docs["board"].records.values().next().unwrap();
        assert_eq!(record.header, tombstone);
        assert_eq!(record.ticket, 1);
        assert_eq!(record.client, "z");
        assert!(record.data.is_none());
    }
    #[test]
    fn malformed_log_fails_instead_of_silently_discarding_geometry() {
        assert!(replay(&mut Project::default(), "not a record").is_err());
        assert!(replay(&mut Project::default(), "{\"type\":\"LINE\"}||{}|").is_err());
    }
    #[test]
    fn empty_records_end_documents_without_discarding_following_geometry() {
        let mut project = Project::default();
        replay(
            &mut project,
            r#"{"type":"DOCHEAD"}||{"uuid":"first","docType":"PCB"}|
{"type":"LINE","id":"track"}||{"width":5}|
{}|||
{"type":"DOCHEAD"}||{"uuid":"second","docType":"PCB"}|
{"type":"LINE","id":"track"}||{"width":7}|"#,
        )
        .unwrap();
        assert_eq!(project.record_count, 2);
        assert_eq!(project.docs["first"].first("LINE").unwrap()["width"], 5);
        assert_eq!(project.docs["second"].first("LINE").unwrap()["width"], 7);
        assert!(
            replay(
                &mut project,
                "{\"type\":\"DOCHEAD\"}||{\"uuid\":\"first\"}|\n{}|||\n{\"type\":\"LINE\"}||{}|"
            )
            .is_err()
        );
        assert!(replay(&mut project, "{}||{\"width\":9}|").is_err());
    }
    #[test]
    fn aes_gcm_with_sixteen_byte_iv_authenticates_then_gunzips_and_bounds_output() {
        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(LOG.as_bytes()).unwrap();
        let zipped = gz.finish().unwrap();
        let key = [7u8; 16];
        let iv = [9u8; 16];
        let cipher = AesGcm::<Aes128, U16>::new_from_slice(&key).unwrap();
        let encrypted = cipher.encrypt((&iv).into(), zipped.as_slice()).unwrap();
        let k = hex::encode(key);
        let n = hex::encode(iv);
        assert_eq!(decode(&k, &n, &encrypted, LOG.len()).unwrap(), LOG);
        assert!(decode(&k, &n, &encrypted, LOG.len() - 1).is_err());
        let mut tampered = encrypted.clone();
        tampered[0] ^= 1;
        assert!(decode(&k, &n, &tampered, 10000).is_err());
        assert!(decode("00", &n, &encrypted, 10000).is_err());
    }
    #[test]
    fn history_parts_include_zero_and_every_suffix_and_snapshot_uses_own_iv() {
        let h:History=serde_json::from_value(serde_json::json!({"uuid":"h","parent":null,"key":"k","iv":"i","num":2,"snapshot":null,"dataStrUrl":"https://modules.lceda.cn/history/h"})).unwrap();
        assert_eq!(
            parts(&h).unwrap(),
            vec![
                ("https://modules.lceda.cn/history/h".into(), "i".into()),
                ("https://modules.lceda.cn/history/h-1".into(), "i".into()),
                ("https://modules.lceda.cn/history/h-2".into(), "i".into())
            ]
        );
        let mut h = h;
        h.snapshot = Some("snapshot-iv".into());
        h.snapshot_num = 1;
        h.snapshot_url = Some("https://modules.lceda.cn/snap/s".into());
        assert_eq!(
            parts(&h).unwrap(),
            vec![
                (
                    "https://modules.lceda.cn/snap/s".into(),
                    "snapshot-iv".into()
                ),
                (
                    "https://modules.lceda.cn/snap/s-1".into(),
                    "snapshot-iv".into()
                )
            ]
        );
        h.snapshot_num = 10000;
        assert!(parts(&h).is_err());
    }

    #[test]
    fn history_pages_must_advance_the_requested_parent_and_reject_cycles() {
        let h = |id: &str, parent: Option<&str>| History {
            uuid: id.into(),
            parent: parent.map(str::to_owned),
            key: String::new(),
            iv: String::new(),
            num: 0,
            snapshot: None,
            snapshot_num: 0,
            data_url: String::new(),
            snapshot_url: None,
        };
        let mut histories = vec![];
        let mut seen = HashSet::new();
        assert_eq!(
            extend_chain(
                &mut histories,
                &mut seen,
                vec![h("head", Some("parent"))],
                "head".into()
            )
            .unwrap(),
            Some("parent".into())
        );
        assert!(
            extend_chain(
                &mut histories,
                &mut seen,
                vec![h("unrelated", None)],
                "parent".into()
            )
            .is_err()
        );
        assert!(
            extend_chain(
                &mut histories,
                &mut seen,
                vec![h("parent", Some("head"))],
                "parent".into()
            )
            .is_err()
        );
    }

    #[test]
    fn document_deletion_is_retained_and_record_budget_is_enforced() {
        let mut p = Project::default();
        replay(&mut p, LOG).unwrap();
        replay(
            &mut p,
            r#"{"type":"DOCHEAD"}||{"uuid":"board","docType":"PCB"}|
{"type":"DELETE_DOC","ticket":10}||{"isDelete":true}|"#,
        )
        .unwrap();
        assert!(p.docs["board"].deleted());
        p.record_count = MAX_RECORDS;
        assert!(replay(&mut p, LOG).is_err());
    }

    #[test]
    fn inherited_and_record_client_ids_are_bounded_before_per_record_copying() {
        let oversized = "a".repeat(129);
        let doc = serde_json::json!({"uuid":"board","docType":"PCB","client":oversized});
        let log =
            format!("{{\"type\":\"DOCHEAD\"}}||{doc}|\n{{\"type\":\"LINE\",\"id\":\"x\"}}||{{}}|");
        assert!(replay(&mut Project::default(), &log).is_err());
        let header = serde_json::json!({"type":"LINE","id":"x","client":oversized});
        let log = format!(
            "{{\"type\":\"DOCHEAD\"}}||{{\"uuid\":\"board\",\"client\":\"valid\"}}|\n{header}||{{}}|"
        );
        assert!(replay(&mut Project::default(), &log).is_err());
        let doc = serde_json::json!({"uuid":"board","client":"b".repeat(128)});
        assert!(
            replay(
                &mut Project::default(),
                &format!("{{\"type\":\"DOCHEAD\"}}||{doc}|\n{{\"type\":\"LINE\"}}||{{}}|")
            )
            .is_ok()
        );
    }

    #[test]
    fn authenticated_concatenated_gzip_members_replay_deletions_and_share_one_output_budget() {
        let first = "{\"type\":\"DOCHEAD\"}||{\"uuid\":\"board\"}|\n{\"type\":\"LINE\",\"id\":\"x\",\"ticket\":1}||{}|\n";
        let second = "{\"type\":\"DOCHEAD\"}||{\"uuid\":\"board\"}|\n{\"type\":\"LINE\",\"id\":\"x\",\"ticket\":2}|||\n";
        let mut members = vec![];
        for log in [first, second] {
            let mut gz = GzEncoder::new(vec![], Compression::default());
            gz.write_all(log.as_bytes()).unwrap();
            members.extend(gz.finish().unwrap());
        }
        let key = [7u8; 16];
        let iv = [9u8; 16];
        let cipher = AesGcm::<Aes128, U16>::new_from_slice(&key).unwrap();
        let encrypt = |bytes: &[u8]| cipher.encrypt((&iv).into(), bytes).unwrap();
        let encrypted = encrypt(&members);
        let key = hex::encode(key);
        let iv = hex::encode(iv);
        let decoded = decode(&key, &iv, &encrypted, first.len() + second.len()).unwrap();
        assert_eq!(decoded, format!("{first}{second}"));
        let mut p = Project::default();
        replay(&mut p, &decoded).unwrap();
        assert!(
            p.docs["board"]
                .records
                .values()
                .all(|r| r.data.is_none() && r.ticket == 2)
        );
        assert!(decode(&key, &iv, &encrypted, first.len()).is_err());
        members.extend(b"trailing unparsed bytes");
        assert!(decode(&key, &iv, &encrypt(&members), 10000).is_err());
    }
}
