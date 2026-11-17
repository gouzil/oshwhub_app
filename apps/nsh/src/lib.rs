//! OSHWHub's Rust NSH application, shared by the native and Web runners.

mod components;
pub mod model;
pub mod pages;
pub mod preferences;
pub mod services;
pub mod viewer;

use nsh_base::theme;
use nsh_core::{
    Rgba,
    element::{AdaptiveColor, AnyElement},
    platform::Appearance,
    prelude::*,
};
use std::rc::Rc;

use components::action;

#[native_module("oshwhub/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

#[native_components(macos)]
fn components_macos(components: &nsh_platform_macos::Components) {
    viewer::surface::register_macos(components);
    viewer::macos_input::register(components);
}

#[native_components(ios)]
fn components_ios(components: &nsh_platform_ios::Components) {
    viewer::surface::register_ios(components);
}

#[native_components(linux)]
fn components_linux(components: &nsh_platform_linux::Components) {
    viewer::surface::register_linux(components);
}

#[native_components(windows)]
fn components_windows(components: &nsh_platform_windows::Components) {
    viewer::surface::register_windows(components);
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
    app_with_store(preferences::system_store())
}

pub fn app_with_store(store: impl preferences::PreferenceStore + 'static) -> AnyElement {
    let page = signal(Page::Discover);
    let appearance = store.read(preferences::THEME_MODE_KEY);
    let accent = store.read(preferences::THEME_COLOR_KEY);
    let preference_error = signal(
        appearance
            .as_ref()
            .err()
            .or(accent.as_ref().err())
            .map(|error| error.message.clone()),
    );
    // A failed fallback must not discard the other saved preference.
    let preferences = signal(preferences::ThemePreferences::from_values(
        appearance.as_ref().ok().and_then(|value| value.as_deref()),
        accent.as_ref().ok().and_then(|value| value.as_deref()),
    ));
    let store = stored(store);
    effect(move || {
        set_color_scheme(preferences.get().appearance);
        let palette = preferences.get().accent.palette();
        let primary = AdaptiveColor {
            light: Rgba::try_from(palette.primary)
                .expect("valid bundled accent")
                .into(),
            dark: Rgba::try_from(palette.dark_primary)
                .expect("valid bundled accent")
                .into(),
        };
        let secondary = AdaptiveColor {
            light: Rgba::try_from(palette.light)
                .expect("valid bundled accent")
                .into(),
            dark: Rgba::try_from(palette.dark)
                .expect("valid bundled accent")
                .into(),
        };
        let mut theme = app_theme();
        for name in [
            "primary",
            "ring",
            "secondary-foreground",
            "accent-foreground",
        ] {
            theme.set_color(name, primary);
        }
        for name in ["secondary", "accent"] {
            theme.set_color(name, secondary);
        }
        set_tokens(theme.into_tokens());
    });
    let on_appearance: Rc<dyn Fn(Appearance)> = Rc::new(move |mode| {
        let mut next = preferences.get();
        let result = store.with(|store| next.set_appearance(store, mode));
        preferences.set(next);
        preference_error.set(
            result
                .err()
                .map(|_| "外观已应用，但无法保存；下次打开时可能恢复原来的模式。".into()),
        );
    });
    let on_accent: Rc<dyn Fn(preferences::Accent)> = Rc::new(move |accent| {
        let mut next = preferences.get();
        let result = store.with(|store| next.set_accent(store, accent));
        preferences.set(next);
        preference_error.set(
            result
                .err()
                .map(|_| "颜色已应用，但无法保存；下次打开时可能恢复原来的颜色。".into()),
        );
    });
    let project_id = signal(None::<String>);
    let viewer_route = signal(None::<pages::ViewerRoute>);
    let feed_active = signal(true);
    effect(move || {
        feed_active.set(
            page.get() == Page::Discover
                && project_id.get().is_none()
                && viewer_route.get().is_none(),
        )
    });

    view()
        .flex_1()
        .p_safe_area()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(
            view()
                .flex_1()
                .flex_col_reverse()
                .style_when(|| window().width.0 >= 1024.0, |s| s.flex_row())
                .child(
                    view()
                        .style_when(
                            move || viewer_route.get().is_some(),
                            |s| s.hidden().invisible(),
                        )
                        .accessibility_elements_hidden(move || viewer_route.get().is_some())
                        .p_3()
                        .gap_3()
                        .border_b_1()
                        .border_color(move || theme().border)
                        .style_when(|| window().width.0 >= 1024.0, |s| s.w(px(216.0)))
                        .child(
                            text("OSHWHub")
                                .style_when(
                                    || window().width.0 < 1024.0,
                                    |s| s.hidden().invisible(),
                                )
                                .text_size(px(24.0))
                                .font_weight(FontWeight::BOLD),
                        )
                        .child(
                            view()
                                .flex_row()
                                .gap_1()
                                .style_when(|| window().width.0 >= 1024.0, |s| s.flex_col())
                                .children(PAGES.map(|(destination, title, label)| {
                                    action(
                                        title,
                                        label,
                                        move || {
                                            page.set(destination);
                                            project_id.set(None);
                                            viewer_route.set(None);
                                        },
                                        move || page.get() == destination,
                                    )
                                })),
                        ),
                )
                .child(
                    view()
                        .flex_1()
                        .min_h(px(0.0))
                        .min_w(px(0.0))
                        .relative()
                        .child(
                            view()
                                .flex_1()
                                .min_h(px(0.0))
                                .style_when(move || !feed_active.get(), |s| s.hidden().invisible())
                                .accessibility_elements_hidden(move || {
                                    page.get() != Page::Discover
                                        || project_id.get().is_some()
                                        || viewer_route.get().is_some()
                                })
                                .child(pages::discover::discover(
                                    feed_active,
                                    Rc::new(move |id| project_id.set(Some(id))),
                                )),
                        )
                        .child(
                            scroll()
                                .flex_1()
                                .label("页面内容")
                                .style_when(
                                    move || {
                                        page.get() == Page::Discover || viewer_route.get().is_some()
                                    },
                                    |s| s.hidden().invisible(),
                                )
                                .accessibility_elements_hidden(move || {
                                    page.get() == Page::Discover || viewer_route.get().is_some()
                                })
                                .child(view().p_6().gap_4().child(dynamic(
                                    move || match page.get() {
                                        Page::Discover => view().into_element(),
                                        Page::Lab => laboratory(Rc::new(move || {
                                            viewer_route.set(Some(pages::ViewerRoute::Demo))
                                        })),
                                        Page::Settings => settings(
                                            preferences,
                                            preference_error,
                                            Rc::clone(&on_appearance),
                                            Rc::clone(&on_accent),
                                        ),
                                        Page::About => about(),
                                    },
                                ))),
                        )
                        .child(dynamic(move || match project_id.get() {
                            Some(id) => view()
                                .absolute()
                                .inset_0()
                                .bg(move || theme().background)
                                .style_when(
                                    move || viewer_route.get().is_some(),
                                    |style| style.hidden().invisible(),
                                )
                                .accessibility_elements_hidden(move || viewer_route.get().is_some())
                                .child(pages::project::project(
                                    id,
                                    Rc::new(move || project_id.set(None)),
                                    Rc::new(move |route| viewer_route.set(Some(route))),
                                ))
                                .into_element(),
                            None => view().into_element(),
                        }))
                        .child(dynamic(move || match viewer_route.get() {
                            Some(route) => view()
                                .absolute()
                                .inset_0()
                                .bg(move || theme().background)
                                .child(pages::viewer::viewer(
                                    route,
                                    Rc::new(move || viewer_route.set(None)),
                                ))
                                .into_element(),
                            None => view().into_element(),
                        })),
                ),
        )
        .into_element()
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

fn laboratory(on_demo: Rc<dyn Fn()>) -> AnyElement {
    view()
        .gap_4()
        .child(heading("PCB 实验室", "看清每一个连接"))
        .child(action(
            "打开示例查看器",
            "打开示例查看器",
            move || on_demo(),
            || false,
        ))
        .into_element()
}

fn settings(
    preferences: Signal<preferences::ThemePreferences>,
    error: Signal<Option<String>>,
    on_appearance: Rc<dyn Fn(Appearance)>,
    on_accent: Rc<dyn Fn(preferences::Accent)>,
) -> AnyElement {
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
                let on_appearance = Rc::clone(&on_appearance);
                action(
                    label,
                    label,
                    move || on_appearance(mode),
                    move || preferences.get().appearance == mode,
                )
            }),
        )
        .child(text(move || {
            format!(
                "当前外观：{}",
                match preferences.get().appearance {
                    Appearance::System => "跟随系统",
                    Appearance::Light => "浅色模式",
                    Appearance::Dark => "暗色模式",
                }
            )
        }))
        .child(text("主题颜色").text_size(px(20.0)))
        .children(preferences::Accent::ALL.map(|accent| {
            let on_accent = Rc::clone(&on_accent);
            action(
                accent.label(),
                accent.label(),
                move || on_accent(accent),
                move || preferences.get().accent == accent,
            )
        }))
        .child(
            text(move || error.get().unwrap_or_default()).text_color(move || theme().destructive),
        )
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
