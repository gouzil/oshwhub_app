#[path = "../src/pages/bom.rs"]
pub mod bom;
#[path = "../src/pages/project.rs"]
pub mod project;
#[path = "../src/components/remote_image.rs"]
pub mod remote_image;

pub use oshwhub_nsh::services;
pub mod components {
    pub use crate::remote_image;
}
pub mod pages {
    pub use crate::bom;
    pub use oshwhub_nsh::pages::ViewerRoute;
}

use nsh_core::{Size, element::Root, prelude::*};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;
use oshwhub_core::{
    error::AppError,
    model::{Attachment, Board, BomItem, ContentBlock, ProjectDetail, ProjectSummary},
};
use std::rc::Rc;

fn items() -> Vec<BomItem> {
    vec![
        BomItem {
            name: "电容 10nF".into(),
            designator: "C6, C16".into(),
            footprint: "C0402".into(),
            quantity: 0,
            part_number: "CL05B103KB5NNNC".into(),
            manufacturer: "SAMSUNG(三星)".into(),
            supplier: "LCSC".into(),
            supplier_part: "C15195".into(),
        },
        BomItem {
            name: "电阻".into(),
            designator: "R1".into(),
            footprint: "0603".into(),
            quantity: 7,
            part_number: String::new(),
            manufacturer: String::new(),
            supplier: String::new(),
            supplier_part: String::new(),
        },
    ]
}

fn detail() -> ProjectDetail {
    let mut content = vec![ContentBlock {
        kind: "heading".into(),
        text: "项目正文开头".into(),
        url: None,
    }];
    content.extend((0..100).map(|index| ContentBlock {
        kind: "paragraph".into(),
        text: format!("段落 {index}\n{}", "开源硬件说明正文。\n".repeat(8)),
        url: None,
    }));
    content.push(ContentBlock {
        kind: "heading".into(),
        text: "正文末尾".into(),
        url: None,
    });
    ProjectDetail {
        summary: ProjectSummary {
            id: "fixture".into(),
            name: "开源电源".into(),
            path: "fixture".into(),
            description: "公开工程说明".into(),
            cover: None,
            author: "创作者".into(),
            author_avatar: None,
            likes: 3,
            views: 42,
            tags: vec!["电源".into(), "电源".into()],
            updated_at: None,
            url: "https://oshwhub.com/creator/fixture".into(),
        },
        content,
        boards: vec![
            Board {
                id: "power".into(),
                name: "电源板".into(),
                pcb_id: Some("pcb-power".into()),
                schematic_id: Some("sch-power".into()),
                thumbnail: None,
                bom: items(),
            },
            Board {
                id: "sensor".into(),
                name: "传感板".into(),
                pcb_id: None,
                schematic_id: Some("sch-sensor".into()),
                thumbnail: None,
                bom: vec![],
            },
        ],
        attachments: vec![Attachment {
            name: "下载附件".into(),
            url: "javascript:alert(1)".into(),
        }],
    }
}

#[component]
fn ProjectFixture() -> impl IntoElement {
    let attempts = signal(0usize);
    let callback = signal(String::new());
    let load: project::ProjectLoader = Rc::new(move || {
        let attempt = attempts.get();
        attempts.set(attempt + 1);
        Box::pin(async move {
            if attempt == 0 {
                Err(AppError::new("offline", "工程详情离线"))
            } else {
                Ok(detail())
            }
        })
    });
    view()
        .flex_1()
        .child(project::project_with_loader(
            "fixture".into(),
            Rc::new(move || callback.set("已返回".into())),
            Rc::new(move |route| {
                let pages::ViewerRoute::Project {
                    project_id,
                    pcb_document_id,
                    schematic_document_id,
                    title,
                    initial_schematic,
                } = route
                else {
                    panic!("unexpected demo route");
                };
                callback.set(format!(
                    "viewer:{project_id}|{}|{}|{title}|{}",
                    pcb_document_id.as_deref().unwrap_or("none"),
                    schematic_document_id.as_deref().unwrap_or("none"),
                    if initial_schematic {
                        "schematic"
                    } else {
                        "pcb"
                    }
                ));
            }),
            load,
        ))
        .child(text(move || callback.get()))
}

#[component]
fn BomFixture() -> impl IntoElement {
    scroll().flex_1().child(bom::bom(items()))
}

#[component]
fn EmptyProjectFixture() -> impl IntoElement {
    project::project_with_loader(
        "empty".into(),
        Rc::new(|| {}),
        Rc::new(|_| panic!("an empty project must not open the viewer")),
        Rc::new(|| {
            Box::pin(async {
                let mut detail = detail();
                detail.summary.author.clear();
                detail.content.clear();
                detail.boards.clear();
                detail.attachments.clear();
                Ok(detail)
            })
        }),
    )
}

#[component]
fn RetryImageFixture() -> impl IntoElement {
    let requests = signal(0usize);
    let load: remote_image::ImageLoader = Rc::new(move |_| {
        let attempt = requests.get() + 1;
        requests.set(attempt);
        Box::pin(async move {
            if attempt <= 2 {
                Err(AppError::new("offline", "图片离线"))
            } else {
                // A complete 1 × 1 PNG; the image decoder also exercises the data URI.
                remote_image::prepare_image(&[
                    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0,
                    0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156,
                    99, 96, 96, 96, 248, 15, 0, 1, 4, 1, 0, 95, 229, 195, 75, 0, 0, 0, 0, 73, 69,
                    78, 68, 174, 66, 96, 130,
                ])
            }
        })
    });
    view()
        .w(px(300.0))
        .child(remote_image::remote_image_with_loader(
            Some("https://image.lceda.cn/retry.png".into()),
            None,
            "可重试图片".into(),
            true,
            load,
        ))
        .child(text(move || format!("图片请求 {0}", requests.get())))
}

fn app<C: nsh_core::element::Component<Props = ()>>(width: f32) -> Headless {
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(width), px(1100.0)),
        ..EngineConfig::new(Root::of::<C>().tokens(oshwhub_nsh::app_theme().into_tokens()))
    })
    .unwrap();
    app.first_render().unwrap();
    app
}

fn ready_project() -> Headless {
    let mut app = app::<ProjectFixture>(1200.0);
    app.wait_for("text:工程详情离线").unwrap();
    app.press("label:重试加载项目").unwrap();
    app.wait_for("text:开源电源").unwrap();
    app
}

#[test]
fn project_retry_loads_typed_details_and_back_callback_works() {
    let mut app = ready_project();
    assert!(app.shows("由 创作者 分享"));
    assert!(app.shows("公开工程说明"));
    assert!(!app.shows("工程详情离线"));
    app.press("label:返回项目列表").unwrap();
    app.wait_for("text:已返回").unwrap();
}

#[test]
fn changing_boards_updates_empty_bom_and_routes_both_document_ids() {
    let mut app = ready_project();
    app.press("label:板子与 BOM (2)").unwrap();
    app.wait_for("text:2 种物料 · 7 个元件").unwrap();
    app.press("label:查看 PCB").unwrap();
    app.wait_for("text:viewer:fixture|pcb-power|sch-power|电源板|pcb")
        .unwrap();
    app.press("label:选择板子 传感板").unwrap();
    app.wait_for("text:暂无物料清单").unwrap();
    assert!(app.press("label:未公开 PCB 文件").is_err());
    app.press("label:查看原理图").unwrap();
    app.wait_for("text:viewer:fixture|none|sch-sensor|传感板|schematic")
        .unwrap();
    app.press("label:项目介绍").unwrap();
    app.press("label:板子与 BOM (2)").unwrap();
    app.wait_for("text:暂无物料清单").unwrap();
    assert!(app.press("label:未公开 PCB 文件").is_err());
}

#[test]
fn projects_without_content_or_boards_explain_the_empty_state() {
    let mut app = app::<EmptyProjectFixture>(390.0);
    app.wait_for("text:由 开源创作者 分享").unwrap();
    app.wait_for("text:作者尚未添加项目正文").unwrap();
    app.press("label:板子与 BOM (0)").unwrap();
    app.wait_for("text:暂无公开板子").unwrap();
    app.wait_for("text:作者尚未公开板子与物料清单。").unwrap();
}

#[test]
fn failed_preview_and_original_can_retry_after_the_error_panel_disposes() {
    let mut app = app::<RetryImageFixture>(390.0);
    app.wait_for("text:图片暂时无法显示").unwrap();
    assert!(app.shows("图片请求 2"));
    app.press("label:重新加载图片：可重试图片").unwrap();
    app.wait_for("text:图片请求 3").unwrap();
    app.wait_for("label:可重试图片").unwrap();
    assert!(!app.shows("图片暂时无法显示"));
}

#[test]
fn long_intro_uses_virtual_rows_and_reaches_attachments() {
    let mut app = ready_project();
    assert!(!app.shows("正文末尾"));
    for _ in 0..8 {
        app.scroll("label:项目正文", px(100000.0)).unwrap();
        if app.shows("正文末尾") {
            break;
        }
    }
    app.wait_for("text:正文末尾").unwrap();
    app.wait_for("text:下载附件（链接不可用）").unwrap();
}

#[test]
fn bom_shows_all_fields_zero_quantity_and_original_row_indices() {
    let mut app = app::<BomFixture>(390.0);
    for label in [
        "名称 / 参数：电容 10nF",
        "位号：C6, C16",
        "封装：C0402",
        "数量：0",
        "制造商型号：CL05B103KB5NNNC",
        "制造商：SAMSUNG(三星)",
        "供应商：LCSC",
        "供应商料号：C15195",
    ] {
        app.wait_for(&format!("label:{label}")).unwrap();
    }
    app.type_text("label:搜索物料清单", "R1").unwrap();
    app.wait_for("text:显示 1 / 2 种物料").unwrap();
    app.wait_for("text:物料 02").unwrap();
    app.wait_for("label:制造商型号：未提供").unwrap();
    assert!(!app.shows("电容 10nF"));
}

#[test]
fn bom_filter_empty_and_clear_restore_rows_on_wide_layout() {
    let mut app = app::<BomFixture>(1200.0);
    app.type_text("label:搜索物料清单", "找不到").unwrap();
    app.wait_for("text:没有匹配的物料").unwrap();
    app.press("label:清除物料搜索").unwrap();
    app.wait_for("text:显示 2 / 2 种物料").unwrap();
    app.wait_for("label:物料清单").unwrap();
}

#[test]
fn bom_fields_do_not_turn_zero_into_missing_data() {
    assert_eq!(
        bom::fields(&items()[0]),
        [
            ("名称 / 参数", "电容 10nF".into()),
            ("位号", "C6, C16".into()),
            ("封装", "C0402".into()),
            ("数量", "0".into()),
            ("制造商型号", "CL05B103KB5NNNC".into()),
            ("制造商", "SAMSUNG(三星)".into()),
            ("供应商", "LCSC".into()),
            ("供应商料号", "C15195".into()),
        ]
    );
}

#[test]
fn external_urls_match_the_desktop_security_boundary() {
    assert_eq!(
        remote_image::safe_external_url("HTTPS://example.com/a?x=1").as_deref(),
        Some("https://example.com/a?x=1")
    );
    assert_eq!(
        remote_image::safe_external_url("http://example.com").as_deref(),
        Some("http://example.com/")
    );
    for value in [
        "javascript:alert(1)",
        "file:///tmp/a",
        "data:image/png,x",
        "https://user:secret@example.com/a",
        "https://example.com/a b",
        "https://example.com/\n",
        "https://example.com/\u{feff}",
        "/relative",
    ] {
        assert!(
            remote_image::safe_external_url(value).is_none(),
            "{value:?}"
        );
    }
    let prefix = "https://example.com/";
    assert!(
        remote_image::safe_external_url(&format!("{prefix}{}", "a".repeat(8192 - prefix.len())))
            .is_some()
    );
    assert!(
        remote_image::safe_external_url(&format!("{prefix}{}", "a".repeat(8193 - prefix.len())))
            .is_none()
    );
}

#[test]
fn images_use_bounded_previews_preserve_signed_urls_and_restrict_hosts() {
    assert_eq!(remote_image::image_sources(Some("https://image.lceda.cn/a.png"), None, 900.0), (
        Some("https://image.lceda.cn/a.png?x-oss-process=image%2Fresize%2Cw_960%2Fquality%2Cq_80%2Fformat%2Cwebp".into()),
        Some("https://image.lceda.cn/a.png".into()),
    ));
    assert_eq!(
        remote_image::image_sources(
            Some("https://image.lceda.cn/a.png?token=secret"),
            None,
            900.0
        ),
        (
            Some("https://image.lceda.cn/a.png?token=secret".into()),
            None
        )
    );
    for value in [
        "https://image.lceda.cn.attacker.com/a.png",
        "http://image.lceda.cn/a.png",
        "https://127.0.0.1/a.png",
        "https://example.com/a.png",
        "data:image/png,x",
    ] {
        assert_eq!(
            remote_image::image_sources(Some(value), None, 640.0),
            (None, None)
        );
    }
}

fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut header = vec![
        0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R',
    ];
    header.extend(width.to_be_bytes());
    header.extend(height.to_be_bytes());
    header.extend([8, 6, 0, 0, 0]);
    header
}

#[test]
fn image_bytes_keep_natural_long_image_ratio_and_never_expose_a_remote_source() {
    let bytes = png_header(240, 6000);
    let loaded = remote_image::prepare_image(&bytes).unwrap();
    assert_eq!(loaded.ratio, 0.04);
    assert!(loaded.uri.starts_with("data:image/png,%89%50%4E%47"));
    let (mime, decoded) = nsh_core::image::decode_data_uri(&loaded.uri).unwrap();
    assert_eq!(mime, "image/png");
    assert_eq!(decoded, bytes);
    for bytes in [
        b"<html>not an image</html>".to_vec(),
        png_header(0, 20),
        png_header(100000, 100000),
    ] {
        assert!(remote_image::prepare_image(&bytes).is_err());
    }
}
