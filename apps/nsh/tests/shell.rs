use nsh_core::{Size, element::Root, prelude::*};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;
use oshwhub_nsh::preferences::PreferenceStore;

#[derive(Default)]
struct MemoryStore(std::collections::HashMap<String, String>);

impl PreferenceStore for MemoryStore {
    fn read(&self, key: &str) -> oshwhub_core::error::Result<Option<String>> {
        Ok(self.0.get(key).cloned())
    }
    fn write(&mut self, key: &str, value: &str) -> oshwhub_core::error::Result<()> {
        self.0.insert(key.into(), value.into());
        Ok(())
    }
}

#[component]
fn TestApp() -> impl IntoElement {
    oshwhub_nsh::app_with_store(MemoryStore::default())
}

fn app() -> Headless {
    Headless::start(EngineConfig {
        viewport: Size::new(px(390.0), px(844.0)),
        ..EngineConfig::new(Root::of::<TestApp>().tokens(oshwhub_nsh::app_theme().into_tokens()))
    })
    .unwrap()
}

#[test]
fn appearance_selection_survives_page_navigation() {
    let mut app = app();
    app.first_render().unwrap();
    app.press("label:打开设置").unwrap();
    app.wait_for("text:外观").unwrap();
    app.press("label:暗色模式").unwrap();
    app.wait_for("text:当前外观：暗色模式").unwrap();
    app.press("label:打开关于").unwrap();
    app.wait_for("text:公开工程只读浏览").unwrap();
    app.press("label:打开设置").unwrap();
    app.wait_for("text:当前外观：暗色模式").unwrap();
}

#[test]
fn laboratory_and_discover_remain_reachable_after_about() {
    let mut app = app();
    app.first_render().unwrap();
    app.press("label:打开关于").unwrap();
    app.wait_for("text:公开工程只读浏览").unwrap();
    app.press("label:打开 PCB 实验室").unwrap();
    app.wait_for("text:PCB 实验室").unwrap();
    app.press("label:打开发现项目").unwrap();
    app.wait_for("text:发现下一个好想法").unwrap();
}

#[test]
fn settings_uses_the_full_body_height_while_discover_is_retained() {
    use nsh_core::MountItem;
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };
    let frames = Arc::new(Mutex::new(HashMap::new()));
    let body = Arc::new(Mutex::new(None));
    let observed_frames = Arc::clone(&frames);
    let observed_body = Arc::clone(&body);
    let mut app = app();
    app.observe(move |_, batch| {
        for item in &batch.items {
            match item {
                MountItem::ViewProps { id, props }
                    if props.accessibility.label.as_deref() == Some("页面内容") =>
                {
                    *observed_body.lock().unwrap() = Some(*id);
                }
                MountItem::AccessibilityLabel { id, label }
                    if label.as_deref() == Some("页面内容") =>
                {
                    *observed_body.lock().unwrap() = Some(*id);
                }
                MountItem::Frame { id, frame } => {
                    observed_frames.lock().unwrap().insert(*id, *frame);
                }
                _ => {}
            }
        }
    });
    app.first_render().unwrap();
    app.press("label:打开设置").unwrap();
    app.wait_for("text:外观").unwrap();
    let id = body.lock().unwrap().unwrap();
    assert!(
        frames.lock().unwrap()[&id].size.height.0 > 500.,
        "the inactive feed must not consume half of the body"
    );
    app.press("label:打开发现项目").unwrap();
    app.wait_for("text:发现下一个好想法").unwrap();
}
