use std::{collections::BTreeSet, sync::Arc};

use nsh_core::canvas::Drawing;

use super::drawing::PreparedScene;

/// Only successful preparation or a complete layer change replaces the visible drawing.
#[derive(Clone, Default)]
pub struct SceneCache {
    pub prepared: Option<Arc<PreparedScene>>,
    pub drawing: Arc<Drawing>,
    pub visible: BTreeSet<i32>,
    pub error: Option<String>,
    pub revision: u64,
}

impl SceneCache {
    pub fn replace(&mut self, result: Result<PreparedScene, String>) -> bool {
        let prepared = match result {
            Ok(prepared) => Arc::new(prepared),
            Err(error) => {
                self.error = Some(error);
                return false;
            }
        };
        let visible = prepared
            .layers
            .iter()
            .filter(|layer| layer.layer.visible)
            .map(|layer| layer.layer.id)
            .collect::<BTreeSet<_>>();
        let drawing = prepared.drawing(|id| visible.contains(&id));
        self.prepared = Some(prepared);
        self.drawing = Arc::new(drawing);
        self.visible = visible;
        self.error = None;
        self.revision += 1;
        true
    }

    pub fn set_visible(&mut self, id: i32, visible: bool) -> Result<(), String> {
        let prepared = self.prepared.as_ref().ok_or("场景尚未准备完成")?;
        if !prepared.layers.iter().any(|layer| layer.layer.id == id) {
            return Err(format!("场景不存在图层 {id}"));
        }
        if self.visible.contains(&id) == visible {
            return Ok(());
        }
        let mut candidate = self.visible.clone();
        if visible {
            candidate.insert(id);
        } else {
            candidate.remove(&id);
        }
        let drawing = prepared.drawing(|id| candidate.contains(&id));
        self.visible = candidate;
        self.drawing = Arc::new(drawing);
        self.revision += 1;
        Ok(())
    }
}
