use std::rc::Rc;

use nsh_base::{button, theme};
use nsh_core::{
    element::{AnyElement, Owner, input},
    prelude::*,
};
use oshwhub_core::{api::Api, error::AppError, model::ProjectSummary};

use crate::{
    components::{action, remote_image::remote_image},
    model::{FeedController, FeedRequest, FeedSnapshot},
    services,
};

#[derive(Clone, Copy)]
struct Feed {
    controller: Stored<FeedController>,
    revision: Signal<u64>,
    query: Signal<String>,
    draft: Signal<String>,
    sort: Signal<&'static str>,
    task: Stored<Option<Spawned<()>>>,
    list: ElementRef,
    owner: Owner,
}

impl Feed {
    fn publish(self) {
        self.revision.update(|revision| *revision += 1);
    }

    fn read<R>(self, read: impl FnOnce(&FeedSnapshot) -> R) -> R {
        self.revision.get();
        self.controller.with(|feed| read(feed.snapshot()))
    }

    fn fetch(self, request: FeedRequest) {
        self.owner.run(|| self.fetch_owned(request));
    }

    fn fetch_owned(self, request: FeedRequest) {
        self.task.with(|task| {
            if let Some(task) = task.take() {
                task.cancel();
            }
        });
        self.publish();
        let query = request.query.clone();
        let task = spawn(async move {
            let result =
                services::run(async move { Api::new(services::client()?).feed(query).await }).await;
            if self.controller.with(|feed| feed.complete(request, result)) {
                self.publish();
            }
        });
        self.task.with(|slot| *slot = Some(task));
    }

    fn search(self) {
        let query = self.draft.get().trim().to_owned();
        self.query.set(query.clone());
        let request = self
            .controller
            .with(|feed| feed.search(&query, self.sort.get()));
        self.list.scroll_to_offset(px(0.0), false);
        self.fetch(request);
    }

    fn load_more(self, active: bool, automatic: bool) {
        if !active {
            return;
        }
        let request = self.controller.with(|feed| {
            if automatic {
                feed.load_more_if_visible(active)
            } else {
                feed.load_more()
            }
        });
        if let Some(request) = request {
            self.fetch(request);
        }
    }

    fn refresh(self, active: bool) {
        if active && let Some(request) = self.controller.with(FeedController::refresh) {
            self.fetch(request);
        }
    }
}

pub fn discover(active: Signal<bool>, on_project: Rc<dyn Fn(String)>) -> AnyElement {
    let feed = Feed {
        controller: stored(FeedController::default()),
        revision: signal(0),
        query: signal(String::new()),
        draft: signal(String::new()),
        sort: signal("recommend"),
        task: stored(None),
        list: element_ref(),
        owner: Owner::current(),
    };
    let width = signal(390.0_f32);
    feed.search();
    view()
        .flex_1()
        .min_h(px(0.0))
        .on_layout(move |bounds: &nsh_core::Bounds<nsh_core::Points>| {
            width.set(bounds.size.width.0);
        })
        .child(
            view()
                .px_4()
                .py_3()
                .gap_3()
                .child(
                    view()
                        .gap_2()
                        .style_when(|| window().height.0 < 500.0, |s| s.hidden().invisible())
                        .child(
                            text("OSHWHUB · 开源硬件社区")
                                .text_size(px(11.0))
                                .text_color(move || theme().primary),
                        )
                        .child(
                            text("发现下一个好想法")
                                .text_size(px(28.0))
                                .font_weight(FontWeight::BOLD),
                        )
                        .child(
                            text("探索、学习，与创造者一起把灵感变成现实。")
                                .text_size(px(12.0))
                                .text_color(move || theme().muted_foreground),
                        ),
                )
                .child(
                    view()
                        .flex_row()
                        .gap_2()
                        .items_center()
                        .child(
                            input()
                                .flex_1()
                                .min_w(px(0.0))
                                .min_h(px(44.0))
                                .px_3()
                                .border_1()
                                .border_color(move || theme().input)
                                .rounded(px(10.0))
                                .label("搜索开源硬件项目")
                                .placeholder("搜索项目、开发板、灵感…")
                                .value(feed.draft)
                                .max_length(100)
                                .auto_correct(false)
                                .on_change(move |value: &str| feed.draft.set(value.to_owned()))
                                .on_submit(move |value: &str| {
                                    feed.draft.set(value.to_owned());
                                    feed.search();
                                }),
                        )
                        .child(action("搜索", "搜索", move || feed.search(), || false))
                        .child(
                            action(
                                "清除",
                                "清除搜索",
                                move || {
                                    feed.draft.set(String::new());
                                    feed.search();
                                },
                                || false,
                            )
                            .style_when(
                                move || feed.draft.get().is_empty(),
                                |s| s.hidden().invisible(),
                            ),
                        ),
                )
                .child(
                    view().flex_row().flex_wrap().gap_2().children(
                        [
                            ("recommend", "为你推荐"),
                            ("new", "最新发布"),
                            ("like", "最多喜欢"),
                        ]
                        .map(|(sort, label)| {
                            action(
                                label,
                                label,
                                move || {
                                    feed.sort.set(sort);
                                    feed.search();
                                },
                                move || feed.sort.get() == sort,
                            )
                        }),
                    ),
                ),
        )
        .child(
            virtual_list(
                move || feed.read(|state| state.items.len()),
                move |at| {
                    let on_project = Rc::clone(&on_project);
                    dynamic(move || {
                        feed.read(|state| state.items.get(at.index()).cloned())
                            .map(|project| card(project, Rc::clone(&on_project)))
                            .unwrap_or_else(|| view().into_element())
                    })
                },
            )
            .element_ref(&feed.list)
            .flex_1()
            .min_h(px(0.0))
            .test_id("project-feed")
            .columns(move || match width.get() {
                w if w >= 1100.0 => 5_u32,
                w if w >= 900.0 => 4,
                w if w >= 620.0 => 3,
                _ => 2,
            })
            .estimated_item_size(px(300.0))
            .row_gap(px(18.0))
            .column_gap(px(12.0))
            .content_padding(px(16.0))
            .keys(move || {
                feed.read(|state| {
                    state
                        .items
                        .iter()
                        .map(|item| item.id.clone())
                        .collect::<Vec<_>>()
                })
            })
            .refreshing(move || feed.read(|state| state.refreshing))
            .on_refresh(move || feed.refresh(active.get()))
            .on_load_more(move || feed.load_more(active.get(), true))
            .header(
                view()
                    .gap_3()
                    .pb_4()
                    .child(
                        text(move || {
                            if feed.query.get().is_empty() {
                                "精选开源项目".into()
                            } else {
                                format!("“{}”的搜索结果", feed.query.get())
                            }
                        })
                        .text_size(px(17.0))
                        .font_weight(FontWeight::SEMIBOLD),
                    )
                    .child(
                        text(move || format!("{} 个项目", feed.read(|state| state.total)))
                            .text_color(move || theme().muted_foreground),
                    )
                    .child(action(
                        "刷新项目",
                        "刷新项目",
                        move || feed.refresh(active.get()),
                        || false,
                    ))
                    .child(dynamic(move || {
                        match feed.read(|state| state.refresh_error.clone()) {
                            Some(error) => error_panel(error, move || feed.refresh(active.get())),
                            None => view().into_element(),
                        }
                    })),
            )
            .empty(dynamic(move || {
                let state = feed.read(Clone::clone);
                if state.loading {
                    text("正在寻找好项目").into_element()
                } else if let Some(error) = state.error {
                    error_panel(error, move || feed.load_more(active.get(), false))
                } else {
                    view()
                        .gap_2()
                        .child(text("还没有找到相关项目"))
                        .child(text("试试更短的关键词，或换一个搜索方向。"))
                        .into_element()
                }
            }))
            .footer(dynamic(move || {
                let state = feed.read(Clone::clone);
                if state.items.is_empty() || state.refreshing || state.refresh_error.is_some() {
                    return view().into_element();
                }
                if let Some(error) = state.error {
                    error_panel(error, move || feed.load_more(active.get(), false))
                } else if state.loading {
                    text("正在加载更多项目").into_element()
                } else if state.has_more {
                    action(
                        "加载更多项目",
                        "加载更多项目",
                        move || feed.load_more(active.get(), false),
                        || false,
                    )
                    .into_element()
                } else {
                    text("已经看到全部项目了")
                        .py_4()
                        .text_color(move || theme().muted_foreground)
                        .into_element()
                }
            })),
        )
        .into_element()
}

fn error_panel(error: AppError, retry: impl Fn() + 'static) -> AnyElement {
    view()
        .gap_3()
        .p_4()
        .child(text(error.message).text_color(move || theme().destructive))
        .child(text(error.code).text_size(px(11.0)))
        .child(action("重试", "重试加载项目", retry, || false))
        .into_element()
}

fn card(project: ProjectSummary, on_project: Rc<dyn Fn(String)>) -> AnyElement {
    let label = format!("打开项目：{}", project.name);
    let title = project.name.clone();
    button()
        .accessibility_label(label)
        .w_full()
        .min_w(px(0.0))
        .items_start()
        .p_0()
        .gap_2()
        .on_press(move || on_project(project.id.clone()))
        .child(
            view()
                .w_full()
                .aspect_ratio(1.17)
                .overflow_hidden()
                .rounded(px(14.0))
                .bg(move || theme().muted)
                .child(remote_image(project.cover, None, title.clone(), false)),
        )
        .child(
            text(title)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(14.0))
                .number_of_lines(2),
        )
        .child(
            text(project.author)
                .text_size(px(11.0))
                .text_color(move || theme().muted_foreground),
        )
        .child(
            text(format!("♡ {} · {} 次浏览", project.likes, project.views))
                .text_size(px(11.0))
                .text_color(move || theme().muted_foreground),
        )
        .into_element()
}
