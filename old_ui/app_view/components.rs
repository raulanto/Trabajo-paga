//! Controles pequeños y reutilizables de la vista.

use gpui::{AnyElement, App, ClickEvent, IntoElement, Window, div, prelude::*, px};
use gpuikit::{
    elements::{
        button::button,
        typography::{h3, text},
    },
    layout::{h_stack, v_stack},
    theme::{ActiveTheme, Themeable},
};

use super::super::theme;

pub(super) fn settings_row(label: &'static str, control: impl IntoElement) -> impl IntoElement {
    h_stack()
        .gap_3()
        .items_center()
        .child(text(label).small().muted())
        .child(control)
}

pub(super) fn info_row(label: &'static str, value: &str) -> impl IntoElement {
    settings_row(label, text(value.to_owned()).small().muted())
}

pub(super) fn toggle_button(
    id: &'static str,
    active: bool,
    on_label: &'static str,
    off_label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    button(id, if active { on_label } else { off_label }).on_click(on_click)
}

pub(super) fn toggle_row(
    id: &'static str,
    label: &'static str,
    active: bool,
    on_label: &'static str,
    off_label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    settings_row(
        label,
        toggle_button(id, active, on_label, off_label, on_click),
    )
}

pub(super) fn choice_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = if active {
        format!("{label} ✓")
    } else {
        label.to_owned()
    };
    button(id, label).on_click(on_click)
}

pub(super) fn state_card(
    cx: &App,
    title: &'static str,
    detail: AnyElement,
    action: Option<AnyElement>,
) -> AnyElement {
    let theme = cx.theme();
    v_stack()
        .max_w(px(theme::DROP_ZONE_WIDTH))
        .items_center()
        .justify_center()
        .gap_3()
        .p_6()
        .rounded_lg()
        .border_1()
        .border_color(theme.border())
        .bg(theme.surface())
        .child(h3(title))
        .child(
            div()
                .max_w(px(theme::DROP_ZONE_WIDTH - 48.))
                .text_center()
                .child(detail),
        )
        .when_some(action, |card, action| card.child(action))
        .into_any_element()
}
