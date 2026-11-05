//! Métricas en tarjetas con la escala tipográfica y tokens del tema gpuikit.

use gpui::{IntoElement, ParentElement, RenderOnce, Window, div, prelude::*};
use gpuikit::{
    elements::typography::{h4, small, text},
    layout::{h_stack, v_stack},
    theme::{ActiveTheme, Themeable},
};

use super::app_view::{AppState, format_integer};

#[derive(IntoElement)]
pub struct Metrics {
    state: AppState,
}

impl Metrics {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }
}

impl RenderOnce for Metrics {
    fn render(self, _: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let theme = cx.theme();
        let (xml_count, cfdi_count, error_count, conceptos_count) = match &self.state.resultados {
            Some(r) => (
                r.total_xml_leidos,
                r.total_cfdi_ok,
                r.errores.len(),
                r.filas.len(),
            ),
            None => (0, 0, 0, 0),
        };

        let card_metric = |label: &'static str, value: String, is_error: bool| {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(small(label).muted())
                .child(h4(value).map(|this| {
                    if is_error && error_count > 0 {
                        this.destructive()
                    } else {
                        this
                    }
                }))
        };

        v_stack()
            .w_full()
            .gap_2()
            .px_6()
            .py_2()
            .child(text("Resultados del ZIP").small().muted())
            .child(
                h_stack()
                    .w_full()
                    .items_center()
                    .gap_4()
                    .px_4()
                    .py_3()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border())
                    .bg(theme.surface_secondary())
                    .child(card_metric("XML leídos", format_integer(xml_count), false))
                    .child(card_metric(
                        "CFDI válidos",
                        format_integer(cfdi_count),
                        false,
                    ))
                    .child(card_metric(
                        "Conceptos",
                        format_integer(conceptos_count),
                        false,
                    ))
                    .child(card_metric("Errores", format_integer(error_count), true)),
            )
    }
}
