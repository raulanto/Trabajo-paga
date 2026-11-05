//! Composición de las vistas de contenido, opciones y vista previa.

use std::path::Path;

use gpui::{Context, IntoElement, div, prelude::*, px};
use gpuikit::{
    elements::{
        badge::badge,
        button::button,
        text_field::text_field,
        typography::{small, text},
    },
    layout::{h_stack, v_stack},
    theme::{ActiveTheme, ControlSize, Themeable},
    traits::control_sized::ControlSized,
};

use super::{AppView, EstadoApp, components::state_card, util::validar_rfc};
use crate::ui::drop_zone::DropZone;

impl AppView {
    pub(super) fn render_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        match self.state.estado {
            EstadoApp::Vacio => DropZone::new(
                "Seleccionar archivo ZIP",
                cx.listener(|this, _, _, cx| this.open_zip_dialog(cx)),
            )
            .into_any_element(),
            EstadoApp::ZipCargado => {
                let detail = v_stack()
                    .gap_1()
                    .child(text(self.zip_file_name()).small().muted().truncate(true))
                    .child(
                        text("Pulsa «Procesar ZIP» para revisar los comprobantes.")
                            .small()
                            .muted(),
                    )
                    .into_any_element();
                state_card(
                    cx,
                    "ZIP listo para procesar",
                    detail,
                    Some(
                        button("replace-zip", "Cambiar ZIP…")
                            .on_click(cx.listener(|this, _, _, cx| this.open_zip_dialog(cx)))
                            .into_any_element(),
                    ),
                )
            }
            EstadoApp::Error => {
                let detail = text(self.state.mensaje_error.clone().unwrap_or_default())
                    .small()
                    .destructive()
                    .into_any_element();
                state_card(
                    cx,
                    "No se pudo procesar el ZIP",
                    detail,
                    Some(
                        button("replace-zip-after-error", "Elegir otro ZIP…")
                            .on_click(cx.listener(|this, _, _, cx| this.open_zip_dialog(cx)))
                            .into_any_element(),
                    ),
                )
            }
            EstadoApp::Procesando => v_stack()
                .items_center()
                .justify_center()
                .gap_2()
                .child(gpuikit::elements::typography::h3("Procesando comprobantes"))
                .child(text(self.zip_file_name()).small().muted())
                .into_any_element(),
            EstadoApp::Resultados => self.render_preview().into_any_element(),
        }
    }

    fn zip_file_name(&self) -> String {
        self.state
            .ruta_zip
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    pub(super) fn render_preview(&mut self) -> impl IntoElement {
        if self.state.resultados.is_none() {
            return small("Preparando vista previa…").muted().into_any_element();
        }
        self.tables[self.state.vista.index()]
            .clone()
            .into_any_element()
    }

    pub(super) fn render_options(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = self.state.ruta_zip.is_some()
            && matches!(
                self.state.estado,
                EstadoApp::ZipCargado | EstadoApp::Resultados
            );
        let is_processing = self.state.estado == EstadoApp::Procesando;
        let validation = validar_rfc(&self.state.rfc_propio);
        let has_rfc = !self.state.rfc_propio.trim().is_empty();

        h_stack()
            .w_full()
            .items_center()
            .justify_between()
            .gap_4()
            .px_6()
            .py_3()
            .border_b_1()
            .border_color(cx.theme().border())
            .bg(cx.theme().surface())
            .child(
                h_stack()
                    .gap_2()
                    .items_start()
                    .child(
                        v_stack().gap_1().child(text("RFC propio").small()).child(
                            text("Opcional · Para sugerir Ingreso/Gasto")
                                .small()
                                .muted(),
                        ),
                    )
                    .child(
                        div()
                            .w(px(200.))
                            .child(text_field(&self.rfc_input, cx).placeholder("AAA010101AAA")),
                    )
                    .when_some(validation, |stack, err| {
                        stack.child(
                            div()
                                .pt_1()
                                .child(badge(err).destructive().control_size(ControlSize::Small)),
                        )
                    })
                    .when(validation.is_none() && has_rfc, |stack| {
                        stack.child(
                            div().pt_1().child(
                                badge("RFC válido")
                                    .secondary()
                                    .control_size(ControlSize::Small),
                            ),
                        )
                    }),
            )
            .child(
                h_stack()
                    .gap_3()
                    .items_center()
                    .child(
                        button(
                            "process-button",
                            if is_processing {
                                "Procesando ZIP"
                            } else {
                                "Procesar ZIP"
                            },
                        )
                        .disabled(!enabled || is_processing)
                        .on_click(cx.listener(|this, _, _, cx| this.process_zip(cx))),
                    )
                    .child(
                        button("settings-button", "Configuración…")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| this.open_settings(cx))),
                    ),
            )
    }
}
