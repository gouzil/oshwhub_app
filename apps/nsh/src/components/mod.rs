use nsh_base::{button, theme};
use nsh_core::prelude::*;

pub fn action(
    title: &'static str,
    label: &'static str,
    on_press: impl Fn() + 'static,
    selected: impl Fn() -> bool + Copy + 'static,
) -> impl IntoElement + Styled {
    button()
        .accessibility_label(label)
        .selected(selected)
        .min_h(px(44.0))
        .px_3()
        .py_2()
        .rounded(px(10.0))
        .bg(move || {
            if selected() {
                theme().accent
            } else {
                theme().background
            }
        })
        .border_1()
        .border_color(move || theme().border)
        .hover(|s| s.bg(move || theme().muted))
        .focus(|s| s.border_color(move || theme().ring))
        .active(|s| s.opacity(0.7))
        .on_press(on_press)
        .child(text(title).text_color(move || {
            if selected() {
                theme().accent_foreground
            } else {
                theme().foreground
            }
        }))
}
pub mod remote_image;
