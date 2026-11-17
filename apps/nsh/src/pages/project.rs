use crate::{
    components::remote_image::{external_link, remote_image},
    pages::{ViewerRoute, bom::bom},
};
use nsh_base::{button, theme};
use nsh_core::{
    AccessibilityRole,
    element::{AnyElement, Owner, ViewableItems},
    prelude::*,
};
use oshwhub_core::{
    api::Api,
    error::Result,
    model::{Board, ContentBlock, ProjectDetail},
};
use std::{collections::HashSet, future::Future, pin::Pin, rc::Rc};

pub(crate) type ProjectLoader =
    Rc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<ProjectDetail>>>>>;

pub fn project(
    id: String,
    on_back: Rc<dyn Fn()>,
    on_viewer: Rc<dyn Fn(ViewerRoute)>,
) -> AnyElement {
    let request_id = id.clone();
    project_with_loader(
        id,
        on_back,
        on_viewer,
        Rc::new(move || {
            let id = request_id.clone();
            Box::pin(async move {
                crate::services::run(async move {
                    Api::new(crate::services::client()?).project(&id).await
                })
                .await
            })
        }),
    )
}

pub(crate) fn project_with_loader(
    id: String,
    on_back: Rc<dyn Fn()>,
    on_viewer: Rc<dyn Fn(ViewerRoute)>,
    load: ProjectLoader,
) -> AnyElement {
    let owner = Owner::current();
    let project = signal::<Option<Rc<ProjectDetail>>>(None);
    let error = signal::<Option<String>>(None);
    let loading = signal(true);
    let hardware = signal(false);
    let selected_board = signal::<Option<String>>(None);
    let task = stored::<Option<Spawned<()>>>(None);
    let reload: Rc<dyn Fn()> = Rc::new(move || {
        // A retry removes its error panel. Keep the request with the page owner.
        owner.run(|| {
            task.with(|task| {
                if let Some(task) = task.take() {
                    task.cancel();
                }
            });
            loading.set(true);
            error.set(None);
            let load = load.clone();
            let request = spawn(async move {
                match load().await {
                    Ok(detail) => project.set(Some(Rc::new(detail))),
                    Err(failure) => error.set(Some(failure.message)),
                }
                loading.set(false);
            });
            task.with(|task| *task = Some(request));
        });
    });
    reload();
    view()
        .flex_1()
        .min_h(px(0.0))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .px_4()
                .py_2()
                .border_b_1()
                .border_color(move || theme().border)
                .child(
                    button()
                        .accessibility_label("返回项目列表")
                        .min_h(px(44.0))
                        .px_3()
                        .on_press(move || on_back())
                        .child(text("← 返回")),
                )
                .child(text("开源项目").font_weight(FontWeight::SEMIBOLD)),
        )
        .child(dynamic(move || {
            if loading.get() {
                return view()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(text("正在加载项目详情…").role(AccessibilityRole::Summary))
                    .into_element();
            }
            if let Some(error) = error.get() {
                let reload = reload.clone();
                return view()
                    .flex_1()
                    .justify_center()
                    .p_5()
                    .gap_3()
                    .child(text("项目暂时无法加载").font_weight(FontWeight::SEMIBOLD))
                    .child(text(error).role(AccessibilityRole::Alert))
                    .child(
                        button()
                            .accessibility_label("重试加载项目")
                            .min_h(px(44.0))
                            .on_press(move || reload())
                            .child(text("重试")),
                    )
                    .into_element();
            }
            let Some(detail) = project.get() else {
                return view().into_element();
            };
            if hardware.get() {
                let body_detail = detail.clone();
                let id = id.clone();
                let on_viewer = on_viewer.clone();
                scroll()
                    .flex_1()
                    .min_h(px(0.0))
                    .label("项目板子与物料清单")
                    .child(
                        view()
                            .p_4()
                            .gap_4()
                            .child(summary(&detail))
                            .child(tabs(detail.boards.len(), hardware))
                            .child(dynamic(move || {
                                hardware_panel(&id, &body_detail, selected_board, on_viewer.clone())
                            })),
                    )
                    .into_element()
            } else {
                introduction(detail, hardware)
            }
        }))
        .into_element()
}

fn summary(detail: &ProjectDetail) -> AnyElement {
    let project = &detail.summary;
    let mut seen = HashSet::new();
    let tags = project
        .tags
        .iter()
        .map(|tag| tag.trim())
        .filter(|tag| !tag.is_empty() && seen.insert(*tag))
        .map(|tag| {
            text(tag.to_owned())
                .text_size(px(12.0))
                .px_2()
                .py_1()
                .rounded(px(6.0))
                .bg(move || theme().muted)
        })
        .collect::<Vec<_>>();
    let author = if project.author.trim().is_empty() {
        "开源创作者"
    } else {
        &project.author
    };
    view()
        .gap_3()
        .child(
            view()
                .h(px(240.0))
                .overflow_hidden()
                .rounded(px(14.0))
                .child(remote_image(
                    project.cover.clone(),
                    None,
                    project.name.clone(),
                    false,
                )),
        )
        .child(
            text(project.name.clone())
                .selectable(true)
                .role(AccessibilityRole::Header)
                .text_size(px(28.0))
                .font_weight(FontWeight::BOLD),
        )
        .child(
            text(format!("由 {author} 分享"))
                .text_size(px(13.0))
                .text_color(move || theme().muted_foreground),
        )
        .child(
            text(project.description.clone())
                .selectable(true)
                .text_size(px(15.0))
                .line_height(px(24.0)),
        )
        .child(view().flex_row().flex_wrap().gap_2().children(tags))
        .child(
            text(format!("♡ {} · {} 次浏览", project.likes, project.views))
                .text_size(px(12.0))
                .text_color(move || theme().muted_foreground),
        )
        .child(external_link(
            project.url.clone(),
            "在开源广场查看原项目".into(),
        ))
        .into_element()
}

fn tabs(board_count: usize, hardware: Signal<bool>) -> AnyElement {
    view()
        .flex_row()
        .gap_2()
        .role(AccessibilityRole::Tablist)
        .label("项目内容")
        .children(
            [
                (false, "项目介绍".to_owned()),
                (true, format!("板子与 BOM ({board_count})")),
            ]
            .map(|(tab, title)| {
                button()
                    .accessibility_label(title.clone())
                    .role(AccessibilityRole::Tab)
                    .selected(move || hardware.get() == tab)
                    .min_h(px(44.0))
                    .px_3()
                    .py_2()
                    .rounded(px(10.0))
                    .bg(move || {
                        if hardware.get() == tab {
                            theme().accent
                        } else {
                            theme().muted
                        }
                    })
                    .on_press(move || hardware.set(tab))
                    .child(text(title))
            }),
        )
        .into_element()
}

fn introduction(detail: Rc<ProjectDetail>, hardware: Signal<bool>) -> AnyElement {
    let ready_through = signal(Some(1usize));
    let rows = detail.clone();
    virtual_list(detail.content.len(), move |at| {
        let rows = rows.clone();
        dynamic(move || {
            rows.content.get(at.index()).map_or_else(
                || view().into_element(),
                |block| content_block(block, at.index(), ready_through),
            )
        })
    })
    .flex_1()
    .min_h(px(0.0))
    .label("项目正文")
    .estimated_item_size(px(180.0))
    .row_gap(px(16.0))
    .content_padding(px(16.0))
    .on_viewable_items_change(move |items: &ViewableItems| {
        if let Some((_, last)) = items.viewable {
            let next = last.saturating_add(1);
            if ready_through.get().is_none_or(|ready| next > ready) {
                ready_through.set(Some(next));
            }
        }
    })
    .header(
        view()
            .gap_4()
            .pb_4()
            .child(summary(&detail))
            .child(tabs(detail.boards.len(), hardware)),
    )
    .empty(
        text("作者尚未添加项目正文")
            .py_5()
            .text_color(move || theme().muted_foreground),
    )
    .footer(
        view()
            .gap_3()
            .py_5()
            .children((!detail.attachments.is_empty()).then(|| {
                text("项目附件")
                    .role(AccessibilityRole::Header)
                    .font_weight(FontWeight::SEMIBOLD)
            }))
            .children(
                detail.attachments.iter().map(|attachment| {
                    external_link(attachment.url.clone(), attachment.name.clone())
                }),
            )
            .child(
                button()
                    .accessibility_label("查看板子与物料清单")
                    .min_h(px(44.0))
                    .px_3()
                    .py_2()
                    .rounded(px(10.0))
                    .bg(move || theme().accent)
                    .on_press(move || hardware.set(true))
                    .child(text("查看板子与物料清单")),
            ),
    )
    .into_element()
}

fn content_block(
    block: &ContentBlock,
    index: usize,
    ready_through: Signal<Option<usize>>,
) -> AnyElement {
    match block.kind.as_str() {
        "image" => {
            let enabled = computed(move || ready_through.get().is_some_and(|last| index <= last));
            let source = block.url.clone();
            let label = if block.text.trim().is_empty() {
                format!("项目图片 {}", index + 1)
            } else {
                block.text.clone()
            };
            let original = source.clone();
            view()
                .gap_2()
                .child(dynamic(move || {
                    if enabled.get() {
                        remote_image(source.clone(), None, label.clone(), true)
                    } else {
                        view()
                            .w_full()
                            .h(px(180.0))
                            .items_center()
                            .justify_center()
                            .bg(move || theme().muted)
                            .child(text("滚动到此处后加载图片"))
                            .into_element()
                    }
                }))
                .children(original.map(|url| external_link(url, "查看原图".into())))
                .into_element()
        }
        "heading" => text(block.text.clone())
            .selectable(true)
            .role(AccessibilityRole::Header)
            .text_size(px(21.0))
            .font_weight(FontWeight::SEMIBOLD)
            .line_height(px(30.0))
            .into_element(),
        "code" => view()
            .p_3()
            .rounded(px(10.0))
            .bg(move || theme().muted)
            .child(
                text(block.text.clone())
                    .selectable(true)
                    .font_family("monospace")
                    .text_size(px(13.0))
                    .line_height(px(21.0)),
            )
            .into_element(),
        "link" => external_link(
            block.url.clone().unwrap_or_default(),
            if block.text.is_empty() {
                "打开链接".into()
            } else {
                block.text.clone()
            },
        ),
        _ => text(block.text.clone())
            .selectable(true)
            .text_size(px(15.0))
            .line_height(px(27.0))
            .into_element(),
    }
}

fn hardware_panel(
    project_id: &str,
    detail: &ProjectDetail,
    selected_board: Signal<Option<String>>,
    on_viewer: Rc<dyn Fn(ViewerRoute)>,
) -> AnyElement {
    let Some(board) = detail
        .boards
        .iter()
        .find(|board| selected_board.get().as_deref() == Some(board.id.as_str()))
        .or_else(|| detail.boards.first())
    else {
        return empty_state("暂无公开板子", "作者尚未公开板子与物料清单。");
    };
    let choices = detail.boards.iter().map(|choice| {
        let id = choice.id.clone();
        let selected = board.id == choice.id;
        let name = board_name(choice);
        button()
            .accessibility_label(format!("选择板子 {name}"))
            .role(AccessibilityRole::Radio)
            .selected(selected)
            .min_h(px(64.0))
            .min_w(px(180.0))
            .flex_1()
            .flex_row()
            .items_center()
            .gap_3()
            .p_3()
            .border_1()
            .border_color(move || {
                if selected {
                    theme().primary
                } else {
                    theme().border
                }
            })
            .rounded(px(10.0))
            .bg(move || {
                if selected {
                    theme().accent
                } else {
                    theme().card
                }
            })
            .on_press(move || selected_board.set(Some(id.clone())))
            .child(
                view()
                    .w(px(72.0))
                    .h(px(48.0))
                    .overflow_hidden()
                    .rounded(px(6.0))
                    .child(remote_image(
                        choice.thumbnail.clone(),
                        None,
                        name.clone(),
                        false,
                    )),
            )
            .child(text(name).font_weight(FontWeight::SEMIBOLD))
    });
    let total: u64 = board.bom.iter().map(|item| u64::from(item.quantity)).sum();
    view()
        .gap_4()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_3()
                .role(AccessibilityRole::Radiogroup)
                .label("选择板子")
                .children(choices),
        )
        .child(
            text(board_name(board))
                .role(AccessibilityRole::Header)
                .text_size(px(21.0))
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(
            text(format!("{} 种物料 · {total} 个元件", board.bom.len()))
                .text_color(move || theme().muted_foreground),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(viewer_button(project_id, board, false, on_viewer.clone()))
                .child(viewer_button(project_id, board, true, on_viewer)),
        )
        .child(if board.bom.is_empty() {
            empty_state("暂无物料清单", "作者尚未公开此板的 BOM。")
        } else {
            bom(board.bom.clone())
        })
        .into_element()
}

fn board_name(board: &Board) -> String {
    if board.name.trim().is_empty() {
        "未命名板子".into()
    } else {
        board.name.clone()
    }
}

fn viewer_button(
    project_id: &str,
    board: &Board,
    schematic: bool,
    on_viewer: Rc<dyn Fn(ViewerRoute)>,
) -> AnyElement {
    let available = if schematic {
        board.schematic_id.is_some()
    } else {
        board.pcb_id.is_some()
    };
    let title = match (schematic, available) {
        (false, true) => "查看 PCB",
        (false, false) => "未公开 PCB 文件",
        (true, true) => "查看原理图",
        (true, false) => "未公开原理图文件",
    };
    let route = ViewerRoute::Project {
        project_id: project_id.into(),
        pcb_document_id: board.pcb_id.clone(),
        schematic_document_id: board.schematic_id.clone(),
        title: board_name(board),
        initial_schematic: schematic,
    };
    button()
        .accessibility_label(title)
        .disabled(!available)
        .min_h(px(44.0))
        .px_3()
        .py_2()
        .border_1()
        .border_color(move || theme().border)
        .rounded(px(10.0))
        .opacity(if available { 1.0 } else { 0.5 })
        .on_press(move || {
            if available {
                on_viewer(route.clone());
            }
        })
        .child(text(title))
        .into_element()
}

fn empty_state(title: &str, description: &str) -> AnyElement {
    view()
        .p_5()
        .gap_2()
        .child(text(title.to_owned()).font_weight(FontWeight::SEMIBOLD))
        .child(text(description.to_owned()).text_color(move || theme().muted_foreground))
        .into_element()
}
