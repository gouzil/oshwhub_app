use nsh_base::{button, theme};
use nsh_core::{element::AnyElement, prelude::*};
use oshwhub_core::error::{AppError, Result};
use std::{
    cell::{Cell, RefCell},
    fmt::Write,
    future::Future,
    pin::Pin,
    rc::Rc,
};
use url::Url;

const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LoadedImage {
    pub uri: String,
    pub ratio: f32,
}

pub(crate) type ImageLoader =
    Rc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<LoadedImage>>>>>;

pub fn safe_external_url(value: &str) -> Option<String> {
    if value.encode_utf16().count() > 8192
        || value
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '\u{feff}')
    {
        return None;
    }
    let url = Url::parse(value).ok()?;
    (matches!(url.scheme(), "https" | "http")
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.into())
}

pub(crate) fn image_sources(
    source: Option<&str>,
    fallback: Option<&str>,
    pixel_width: f32,
) -> (Option<String>, Option<String>) {
    let allowed = |value: &str| {
        safe_external_url(value).and_then(|value| {
            oshwhub_core::client::validate_url(&value)
                .ok()
                .map(String::from)
        })
    };
    let original = source.and_then(allowed);
    let mut preview = original.clone();
    if let Some(original) = original.as_deref()
        && let Ok(mut url) = Url::parse(original)
        && url.host_str() == Some("image.lceda.cn")
        && url.query().is_none()
        && pixel_width.is_finite()
        && pixel_width > 0.0
    {
        let width = [320, 640, 960, 1280, 1920]
            .into_iter()
            .find(|width| *width as f32 >= pixel_width)
            .unwrap_or(1920);
        url.query_pairs_mut().append_pair(
            "x-oss-process",
            &format!("image/resize,w_{width}/quality,q_80/format,webp"),
        );
        preview = Some(url.into());
    }
    let backup = fallback
        .and_then(allowed)
        .or_else(|| (preview != original).then_some(original).flatten());
    if preview.is_none() {
        return (backup, None);
    }
    let backup = backup.filter(|backup| Some(backup) != preview.as_ref());
    (preview, backup)
}

pub(crate) fn prepare_image(bytes: &[u8]) -> Result<LoadedImage> {
    let invalid = || AppError::new("INVALID_IMAGE", "图片格式或尺寸无效");
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(AppError::new("IMAGE_LIMIT", "图片超过 8 MiB 限制"));
    }
    let mime = nsh_core::platform::clipboard::image_mime(bytes).ok_or_else(invalid)?;
    let (width, height) = nsh_core::platform::clipboard::image_size(bytes).ok_or_else(invalid)?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 32_000_000 {
        return Err(invalid());
    }
    let mut uri = String::with_capacity(bytes.len() * 3 + 32);
    write!(uri, "data:{mime},").unwrap();
    for byte in bytes {
        write!(uri, "%{byte:02X}").unwrap();
    }
    Ok(LoadedImage {
        uri,
        ratio: width as f32 / height as f32,
    })
}

pub fn remote_image(
    source: Option<String>,
    fallback: Option<String>,
    label: String,
    natural: bool,
) -> AnyElement {
    remote_image_with_loader(
        source,
        fallback,
        label,
        natural,
        Rc::new(|url| {
            Box::pin(async move {
                crate::services::run(async move {
                    let bytes = crate::services::client()?
                        .get_bytes(&url, MAX_IMAGE_BYTES)
                        .await?;
                    prepare_image(&bytes)
                })
                .await
            })
        }),
    )
}

pub(crate) fn remote_image_with_loader(
    source: Option<String>,
    fallback: Option<String>,
    label: String,
    natural: bool,
    load: ImageLoader,
) -> AnyElement {
    if source.is_none() && fallback.is_none() {
        return view()
            .w_full()
            .aspect_ratio(1.6)
            .items_center()
            .justify_center()
            .bg(move || theme().muted)
            .rounded(px(10.0))
            .child(text("开源硬件项目").text_color(move || theme().muted_foreground))
            .into_element();
    }
    let source = Rc::new(source);
    let fallback = Rc::new(fallback);
    let width = signal(0f32);
    let ratio = signal(1.6f32);
    let using_fallback = signal(false);
    let attempt = signal(0u64);
    let failed = signal(false);
    let loaded = signal::<Option<Rc<LoadedImage>>>(None);
    let generation = Rc::new(Cell::new(0u64));
    let last_request = Rc::new(RefCell::new(None::<(String, u64)>));
    let owner = Owner::current();
    let task = stored::<Option<Spawned<()>>>(None);
    {
        let source = source.clone();
        let fallback = fallback.clone();
        let generation = generation.clone();
        effect(move || {
            let width = width.get();
            if width <= 0.0 {
                return;
            }
            let (primary, backup) = image_sources(
                source.as_deref(),
                fallback.as_deref(),
                width * pixel_ratio(),
            );
            let is_fallback = using_fallback.get();
            let selected = if is_fallback { backup.clone() } else { primary };
            let Some(selected) = selected else {
                failed.set(true);
                return;
            };
            let key = (selected.clone(), attempt.get());
            if last_request.borrow().as_ref() == Some(&key) {
                return;
            }
            *last_request.borrow_mut() = Some(key);
            generation.set(generation.get() + 1);
            let ticket = generation.get();
            let generation = generation.clone();
            let load = load.clone();
            loaded.set(None);
            failed.set(false);
            owner.run(|| {
                task.with(|task| {
                    if let Some(task) = task.take() {
                        task.cancel();
                    }
                });
                let request = spawn(async move {
                    let result = load(selected).await;
                    if generation.get() != ticket {
                        return;
                    }
                    match result {
                        Ok(image) => {
                            if natural {
                                ratio.set(image.ratio);
                            }
                            loaded.set(Some(Rc::new(image)));
                        }
                        Err(_) if !is_fallback && backup.is_some() => using_fallback.set(true),
                        Err(_) => failed.set(true),
                    }
                });
                task.with(|task| *task = Some(request));
            });
        });
    }
    let label = Rc::new(label);
    view()
        .w_full()
        .aspect_ratio(move || ratio.get())
        .rounded(px(10.0))
        .overflow_hidden()
        .bg(move || theme().muted)
        .on_layout(move |bounds: &nsh_core::Bounds<nsh_core::Points>| {
            width.set(bounds.size.width.0)
        })
        .child(dynamic(move || {
            if failed.get() {
                return view()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .p_3()
                    .child(
                        text("图片暂时无法显示")
                            .text_size(px(13.0))
                            .text_color(move || theme().muted_foreground),
                    )
                    .child(
                        button()
                            .accessibility_label(format!("重新加载图片：{}", label.as_str()))
                            .min_h(px(44.0))
                            .on_press(move || {
                                owner.run(|| {
                                    using_fallback.set(false);
                                    attempt.update(|attempt| *attempt += 1);
                                });
                            })
                            .child(text("重新加载图片")),
                    )
                    .into_element();
            }
            if let Some(data) = loaded.get() {
                let generation = generation.clone();
                let ticket = generation.get();
                let error_generation = generation.clone();
                let source = source.clone();
                let fallback = fallback.clone();
                return image(data.uri.clone())
                    .w_full()
                    .h_full()
                    .object_fit(if natural {
                        nsh_core::ResizeMode::Contain
                    } else {
                        nsh_core::ResizeMode::Cover
                    })
                    .label(label.as_str().to_owned())
                    .on_load(
                        move |size: &nsh_core::Size<nsh_core::geometry::DevicePixels>| {
                            if natural
                                && generation.get() == ticket
                                && size.width.0 > 0
                                && size.height.0 > 0
                            {
                                ratio.set(size.width.0 as f32 / size.height.0 as f32);
                            }
                        },
                    )
                    .on_error(move |_: &str| {
                        if error_generation.get() != ticket {
                            return;
                        }
                        let (_, backup) = image_sources(
                            source.as_deref(),
                            fallback.as_deref(),
                            width.get() * pixel_ratio(),
                        );
                        loaded.set(None);
                        if !using_fallback.get() && backup.is_some() {
                            using_fallback.set(true);
                        } else {
                            failed.set(true);
                        }
                    })
                    .into_element();
            }
            view()
                .flex_1()
                .items_center()
                .justify_center()
                .child(
                    text(format!("正在加载图片：{}", label.as_str()))
                        .text_size(px(12.0))
                        .text_color(move || theme().muted_foreground),
                )
                .into_element()
        }))
        .into_element()
}

pub fn external_link(url: String, label: String) -> AnyElement {
    let Some(target) = safe_external_url(&url) else {
        return text(format!("{label}（链接不可用）"))
            .text_color(move || theme().muted_foreground)
            .into_element();
    };
    let failed = signal(false);
    view()
        .child(
            button()
                .role(nsh_core::AccessibilityRole::Link)
                .accessibility_label(format!("{label}，在浏览器中打开"))
                .min_h(px(44.0))
                .on_press(move || {
                    failed.set(false);
                    let target = target.clone();
                    spawn(async move {
                        failed.set(!open_url(target).await);
                    });
                })
                .child(text(label).text_color(move || theme().primary)),
        )
        .child(when(
            move || failed.get(),
            move || {
                text("无法打开链接，请稍后重试。")
                    .role(nsh_core::AccessibilityRole::Alert)
                    .text_color(move || theme().destructive)
            },
        ))
        .into_element()
}
