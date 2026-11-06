use nsh_core::{Size, element::Root, prelude::px};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;

fn app() -> Headless {
    Headless::start(EngineConfig {
        viewport: Size::new(px(390.0), px(844.0)),
        ..EngineConfig::new(
            Root::of::<oshwhub_nsh::App>().tokens(oshwhub_nsh::app_theme().into_tokens()),
        )
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
