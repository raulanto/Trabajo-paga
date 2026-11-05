//! Modal de configuración: navegación, secciones y controles.

use gpui::{Context, Div, IntoElement, div, prelude::*, px};
use gpuikit::{
    elements::{button::button, typography::text},
    layout::{h_stack, v_stack},
    theme::{ActiveTheme, ControlSize, Themeable},
    traits::control_sized::ControlSized,
};

use super::{AppView, SettingsSection, state::ConfigSaveStatus};

const SECTIONS: [(&str, &str, SettingsSection); 8] = [
    ("settings-general", "General", SettingsSection::General),
    (
        "settings-extraccion",
        "Extracción",
        SettingsSection::Extraccion,
    ),
    (
        "settings-clasificacion",
        "Clasificación",
        SettingsSection::Clasificacion,
    ),
    ("settings-excel", "Excel", SettingsSection::Excel),
    ("settings-tabla", "Tabla", SettingsSection::Tabla),
    (
        "settings-apariencia",
        "Apariencia",
        SettingsSection::Apariencia,
    ),
    ("settings-atajos", "Atajos", SettingsSection::Atajos),
    ("settings-avanzado", "Avanzado", SettingsSection::Avanzado),
];

impl AppView {
    pub(super) fn render_settings_modal(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let config = self.state.config.clone();
        let invalid = config.validar();
        let section = self.state.settings_section;

        let nav = self.render_settings_nav(section, cx);
        let section_content = self.render_section_content(section, cx);
        let (section_title, section_description) = section_meta(section);

        let content_panel = v_stack()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .gap_3()
            .child(
                h_stack()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        v_stack()
                            .gap_1()
                            .child(text(section_title).small())
                            .child(text(section_description).small().muted()),
                    )
                    .child(
                        button("restore-section", "Restaurar sección")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.restore_section_defaults(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .id("settings-section-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.settings_scroll)
                    .pr_2()
                    .child(section_content),
            )
            .when(!invalid.is_empty(), |panel| {
                panel.child(div().child(text(invalid[0].mensaje.clone()).small().destructive()))
            });

        let form = v_stack()
            .w_full()
            .h_full()
            .min_h_0()
            .gap_3()
            .child(
                h_stack()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        v_stack()
                            .gap_1()
                            .child(text("Configuración").small())
                            .child(
                                text("Personaliza cómo se presenta y exporta la información")
                                    .small()
                                    .muted(),
                            ),
                    )
                    .child(
                        h_stack()
                            .gap_3()
                            .items_center()
                            .child(
                                text(match self.config_save_status {
                                    ConfigSaveStatus::Saved => "Guardado",
                                    ConfigSaveStatus::Saving => "Guardando cambios…",
                                    ConfigSaveStatus::Invalid => "Revisa los campos",
                                    ConfigSaveStatus::Failed => "Error al guardar",
                                })
                                .small()
                                .muted(),
                            )
                            .child(button("close-settings", "Cerrar").on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.close_settings(cx);
                                    window.focus(&this.root_focus, cx);
                                },
                            ))),
                    ),
            )
            .child(
                h_stack()
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .gap_3()
                    .child(
                        nav.h_full()
                            .p_2()
                            .rounded_md()
                            .bg(cx.theme().surface_secondary()),
                    )
                    .child(content_panel),
            )
            .child(
                h_stack()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .pt_2()
                    .border_t_1()
                    .border_color(cx.theme().border())
                    .child(
                        h_stack()
                            .gap_2()
                            .when(self.confirm_restore_all, |actions| {
                                actions
                                    .child(
                                        text("¿Restaurar todos los valores predeterminados?")
                                            .small(),
                                    )
                                    .child(button("cancel-restore-all", "Cancelar").on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.confirm_restore_all = false;
                                            cx.notify();
                                        }),
                                    ))
                                    .child(button("confirm-restore-all", "Restaurar").on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.restore_all_defaults(cx);
                                        }),
                                    ))
                            })
                            .when(!self.confirm_restore_all, |actions| {
                                actions.child(button("restore-all", "Restaurar todo").on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.confirm_restore_all = true;
                                        cx.notify();
                                    }),
                                ))
                            }),
                    )
                    .child(
                        button("save-settings", "Guardar ahora")
                            .on_click(cx.listener(|this, _, _, cx| this.save_config(cx))),
                    ),
            );

        div()
            .id("settings-overlay")
            .absolute()
            .inset_0()
            .p_4()
            .bg(cx.theme().bg().opacity(0.82))
            .flex()
            .items_center()
            .justify_center()
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .w_full()
                    .max_w(px(920.))
                    .h_full()
                    .max_h(px(680.))
                    .min_h_0()
                    .overflow_hidden()
                    .bg(cx.theme().surface())
                    .border_1()
                    .border_color(cx.theme().border())
                    .rounded_lg()
                    .p_4()
                    .child(form),
            )
    }

    fn render_settings_nav(&mut self, section: SettingsSection, cx: &mut Context<Self>) -> Div {
        v_stack()
            .w(px(184.))
            .h_full()
            .flex_shrink_0()
            .gap_1()
            .child(text("SECCIONES").small().muted())
            .children(SECTIONS.into_iter().map(|(id, label, target)| {
                let selected = section == target;
                div()
                    .w_full()
                    .when(selected, |row| {
                        row.rounded_md().bg(cx.theme().surface_tertiary())
                    })
                    .child(
                        button(
                            id,
                            if selected {
                                format!("›  {label}")
                            } else {
                                format!("   {label}")
                            },
                        )
                        .control_size(ControlSize::Small)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_settings_section(target, cx);
                        })),
                    )
            }))
            .child(div().h(px(1.)).my_2().bg(cx.theme().border()))
            .child(
                text("Los cambios se guardan automáticamente")
                    .small()
                    .muted(),
            )
    }
}

fn section_meta(section: SettingsSection) -> (&'static str, &'static str) {
    match section {
        SettingsSection::General => ("General", "Preferencias de cuenta y carpetas"),
        SettingsSection::Extraccion => ("Extracción", "Qué tipos de comprobante se muestran"),
        SettingsSection::Clasificacion => ("Clasificación", "Reglas de clasificación sugerida"),
        SettingsSection::Excel => ("Excel", "Formato y opciones del libro exportado"),
        SettingsSection::Tabla => ("Tabla", "Comportamiento de las vistas previas"),
        SettingsSection::Apariencia => ("Apariencia", "Tema y densidad de la interfaz"),
        SettingsSection::Atajos => ("Atajos", "Combinaciones de teclado"),
        SettingsSection::Avanzado => ("Avanzado", "Opciones de procesamiento y diagnóstico"),
    }
}
