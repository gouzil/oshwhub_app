//! OSHWHub's Rust NSH application, shared by the native and Web runners.

mod components;

use nsh_base::theme;
use nsh_core::{element::AnyElement, platform::Appearance, prelude::*};

use components::action;

#[native_module("oshwhub/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

pub fn app_theme() -> nsh_base::Theme {
    nsh_base::Theme::from_json(include_str!("../theme.json"))
        .expect("the bundled OSHWHub theme is valid")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Discover,
    Lab,
    Settings,
    About,
}

const PAGES: [(Page, &str, &str); 4] = [
    (Page::Discover, "发现", "打开发现项目"),
    (Page::Lab, "实验室", "打开 PCB 实验室"),
    (Page::Settings, "设置", "打开设置"),
    (Page::About, "关于", "打开关于"),
];

/// Keep page and appearance state above responsive layouts and page disposal.
#[component]
pub fn App() -> impl IntoElement {
    let page = signal(Page::Discover);
    let appearance = signal(Appearance::System);
    effect(move || set_color_scheme(appearance.get()));

    view()
        .flex_1()
        .p_safe_area()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(
            view()
                .flex_1()
                .style_when(|| window().width.0 >= 700.0, |s| s.flex_row())
                .child(
                    view()
                        .p_3()
                        .gap_3()
                        .border_b_1()
                        .border_color(move || theme().border)
                        .style_when(|| window().width.0 >= 700.0, |s| s.w(px(200.0)))
                        .child(
                            text("OSHWHub")
                                .text_size(px(24.0))
                                .font_weight(FontWeight::BOLD),
                        )
                        .child(
                            view()
                                .flex_row()
                                .gap_1()
                                .style_when(|| window().width.0 >= 700.0, |s| s.flex_col())
                                .children(PAGES.map(|(destination, title, label)| {
                                    action(
                                        title,
                                        label,
                                        move || page.set(destination),
                                        move || page.get() == destination,
                                    )
                                })),
                        ),
                )
                .child(
                    scroll()
                        .flex_1()
                        .child(
                            view()
                                .p_6()
                                .gap_4()
                                .child(dynamic(move || match page.get() {
                                    Page::Discover => discover(),
                                    Page::Lab => laboratory(),
                                    Page::Settings => settings(appearance),
                                    Page::About => about(),
                                })),
                        ),
                ),
        )
}

fn heading(title: &'static str, description: &'static str) -> AnyElement {
    view()
        .gap_2()
        .child(
            text(title)
                .text_size(px(28.0))
                .font_weight(FontWeight::BOLD),
        )
        .child(text(description).text_color(move || theme().muted_foreground))
        .into_element()
}

fn discover() -> AnyElement {
    heading("发现下一个好想法", "随时发现开源硬件，随处探索电路设计。")
}

fn laboratory() -> AnyElement {
    heading("PCB 实验室", "看清每一个连接")
}

fn settings(appearance: Signal<Appearance>) -> AnyElement {
    view()
        .gap_4()
        .child(heading("设置", "按你的习惯浏览开源硬件"))
        .child(text("外观").text_size(px(20.0)))
        .children(
            [
                (Appearance::System, "跟随系统"),
                (Appearance::Light, "浅色模式"),
                (Appearance::Dark, "暗色模式"),
            ]
            .map(|(mode, label)| {
                action(
                    label,
                    label,
                    move || appearance.set(mode),
                    move || appearance.get() == mode,
                )
            }),
        )
        .child(text(move || {
            format!(
                "当前外观：{}",
                match appearance.get() {
                    Appearance::System => "跟随系统",
                    Appearance::Light => "浅色模式",
                    Appearance::Dark => "暗色模式",
                }
            )
        }))
        .into_element()
}

fn about() -> AnyElement {
    view()
        .gap_4()
        .child(heading(
            "从创意到电路",
            "随时发现开源硬件，随处探索电路设计。",
        ))
        .child(text("公开工程只读浏览").text_size(px(20.0)))
        .child(text(
            "本应用是开源硬件社区的只读客户端。工程内容和设计文件的权利及许可证归原作者所有。",
        ))
        .into_element()
}
