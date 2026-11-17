use nsh_base::{button, theme};
use nsh_core::{AccessibilityRole, element::AnyElement, prelude::*};
use oshwhub_core::model::BomItem;
use std::{rc::Rc, time::Duration};

const COLUMNS: [(&str, f32); 8] = [
    ("名称 / 参数", 128.0),
    ("位号", 168.0),
    ("封装", 136.0),
    ("数量", 60.0),
    ("制造商型号", 156.0),
    ("制造商", 128.0),
    ("供应商", 88.0),
    ("供应商料号", 128.0),
];

pub fn bom(items: Vec<BomItem>) -> AnyElement {
    let items = Rc::new(items);
    let query = signal(String::new());
    let search = signal(String::new());
    let version = signal(0u64);
    view()
        .gap_3()
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    nsh_core::element::input()
                        .label("搜索物料清单")
                        .role(AccessibilityRole::Search)
                        .placeholder("搜索名称、位号、型号或料号…")
                        .value(move || query.get())
                        .max_length(100)
                        .min_h(px(44.0))
                        .flex_1()
                        .px_3()
                        .border_1()
                        .border_color(move || theme().border)
                        .rounded(px(10.0))
                        .text_color(move || theme().foreground)
                        .on_change(move |value: &str| {
                            query.set(value.into());
                            version.update(|version| *version += 1);
                            let ticket = version.get();
                            let value = value.trim().to_owned();
                            set_timeout(Duration::from_millis(180), move || {
                                if ticket == version.get() {
                                    search.set(value);
                                }
                            });
                        })
                        .on_submit(move |value: &str| {
                            version.update(|version| *version += 1);
                            query.set(value.into());
                            search.set(value.trim().into());
                        }),
                )
                .child(when(
                    move || !query.get().is_empty(),
                    move || {
                        button()
                            .accessibility_label("清除物料搜索")
                            .min_h(px(44.0))
                            .on_press(move || {
                                version.update(|version| *version += 1);
                                query.set(String::new());
                                search.set(String::new());
                            })
                            .child(text("清除"))
                    },
                )),
        )
        .child(dynamic(move || {
            if query.get().trim() != search.get() {
                return text("正在筛选物料…")
                    .role(AccessibilityRole::Summary)
                    .into_element();
            }
            match oshwhub_core::search_bom(&items, &search.get()) {
                Err(error) => view()
                    .gap_2()
                    .child(text("物料搜索失败").role(AccessibilityRole::Alert))
                    .child(text(error.message).text_color(move || theme().destructive))
                    .child(
                        button()
                            .accessibility_label("重试物料搜索")
                            .min_h(px(44.0))
                            .on_press(move || search.set(query.get().trim().into()))
                            .child(text("重试")),
                    )
                    .into_element(),
                Ok(matches) => {
                    let wide = window().width.0 >= 720.0;
                    let count = text(format!("显示 {} / {} 种物料", matches.len(), items.len()))
                        .role(AccessibilityRole::Summary)
                        .text_size(px(12.0))
                        .text_color(move || theme().muted_foreground);
                    let rows = if matches.is_empty() {
                        view()
                            .gap_2()
                            .p_5()
                            .child(text("没有匹配的物料").font_weight(FontWeight::SEMIBOLD))
                            .child(text(
                                "试试名称、位号、封装、厂家或料号，或清除搜索查看全部。",
                            ))
                            .into_element()
                    } else if wide {
                        let table = view()
                            .w(px(1040.0))
                            .role(AccessibilityRole::Table)
                            .label("物料清单")
                            .child(
                                view()
                                    .flex_row()
                                    .bg(move || theme().muted)
                                    .child(
                                        text("序号")
                                            .w(px(48.0))
                                            .p_3()
                                            .role(AccessibilityRole::ColumnHeader),
                                    )
                                    .children(COLUMNS.map(|(label, width)| {
                                        text(label)
                                            .w(px(width))
                                            .p_3()
                                            .role(AccessibilityRole::ColumnHeader)
                                    })),
                            )
                            .children(
                                matches
                                    .into_iter()
                                    .map(|index| row(&items[index], index, true)),
                            );
                        view()
                            .gap_2()
                            .child(
                                text("位号对应电路图上的元件编号；窄窗口可左右滑动表格。")
                                    .text_size(px(12.0))
                                    .text_color(move || theme().muted_foreground),
                            )
                            .child(
                                scroll()
                                    .horizontal()
                                    .label("物料表格，可左右滚动查看完整字段")
                                    .child(table),
                            )
                            .into_element()
                    } else {
                        view()
                            .gap_3()
                            .children(
                                matches
                                    .into_iter()
                                    .map(|index| row(&items[index], index, false)),
                            )
                            .into_element()
                    };
                    view().gap_3().child(count).child(rows).into_element()
                }
            }
        }))
        .into_element()
}

pub(crate) fn fields(item: &BomItem) -> [(&'static str, String); 8] {
    let value = |value: &str| {
        if value.trim().is_empty() {
            "未提供".into()
        } else {
            value.trim().to_owned()
        }
    };
    [
        ("名称 / 参数", value(&item.name)),
        ("位号", value(&item.designator)),
        ("封装", value(&item.footprint)),
        ("数量", item.quantity.to_string()),
        ("制造商型号", value(&item.part_number)),
        ("制造商", value(&item.manufacturer)),
        ("供应商", value(&item.supplier)),
        ("供应商料号", value(&item.supplier_part)),
    ]
}

fn row(item: &BomItem, index: usize, wide: bool) -> AnyElement {
    let mut row = view()
        .label(format!("物料 {}", index + 1))
        .border_b_1()
        .border_color(move || theme().border)
        .bg(move || {
            if index % 2 == 1 {
                theme().muted
            } else {
                theme().card
            }
        });
    if wide {
        row = row.flex_row().role(AccessibilityRole::Row);
    } else {
        row = row.p_3().gap_2().rounded(px(10.0)).border_1();
    }
    row.child(
        text(if wide {
            (index + 1).to_string()
        } else {
            format!("物料 {:02}", index + 1)
        })
        .text_size(px(12.0))
        .style_when(move || wide, |style| style.w(px(48.0)).p_3()),
    )
    .children(
        fields(item)
            .into_iter()
            .enumerate()
            .map(|(column, (label, value))| {
                let missing = value == "未提供";
                let body = text(value.clone())
                    .selectable(true)
                    .text_size(px(13.0))
                    .text_color(move || {
                        if missing {
                            theme().muted_foreground
                        } else if column == 3 {
                            theme().primary
                        } else {
                            theme().foreground
                        }
                    });
                let mut field = view()
                    .label(format!("{label}：{value}"))
                    .accessible(true)
                    .min_h(px(30.0));
                if wide {
                    field = field
                        .w(px(COLUMNS[column].1))
                        .p_3()
                        .role(AccessibilityRole::Cell);
                } else {
                    field = field.flex_row().gap_3().child(
                        text(label)
                            .w(px(84.0))
                            .text_size(px(12.0))
                            .text_color(move || theme().muted_foreground),
                    );
                }
                field.child(body.flex_1()).into_element()
            }),
    )
    .into_element()
}
