//! Atajos, pestañas y sincronización de tablas.

use std::sync::Arc;

use gpui::{Context, KeyDownEvent, Window};

use super::super::{AppView, Vista};

impl AppView {
    pub(in crate::ui::app_view) fn on_shortcut(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = event.keystroke.modifiers;
        if !(modifiers.control || modifiers.platform) {
            return;
        }
        let handled = match event.keystroke.key.to_ascii_lowercase().as_str() {
            "o" => {
                self.open_zip_dialog(cx);
                true
            }
            "e" if self.state.resultados.is_some() => {
                self.choose_export_path(cx);
                true
            }
            "1" => {
                self.select_tab(Vista::Cfdi, cx);
                true
            }
            "2" => {
                self.select_tab(Vista::Resumen, cx);
                true
            }
            "3" => {
                self.select_tab(Vista::Clasificacion, cx);
                true
            }
            "4" => {
                self.select_tab(Vista::Errores, cx);
                true
            }
            "," => {
                self.open_settings(cx);
                true
            }
            _ => false,
        };
        if handled {
            window.focus(&self.root_focus, cx);
        }
    }

    pub(in crate::ui::app_view) fn select_tab(&mut self, tab: Vista, cx: &mut Context<Self>) {
        self.state.vista = tab;
        self.tabs
            .update(cx, |tabs, cx| tabs.set_selected(tab.id().into(), cx));
        cx.notify();
    }

    pub(in crate::ui::app_view) fn sync_table_preferences(&mut self, cx: &mut Context<Self>) {
        let presentation = (
            self.state.config.tabla.altura_fila as f32,
            self.state.config.tabla.mostrar_hover,
            self.state.config.tabla.overscan_px,
        );
        for table in &self.tables {
            table.update(cx, |table, cx| {
                table.set_presentacion(presentation.0, presentation.1, presentation.2, cx);
            });
        }
    }

    pub(in crate::ui::app_view) fn sync_table_rows(&mut self, cx: &mut Context<Self>) {
        let rows = self
            .state
            .resultados
            .as_ref()
            .map(|resultados| {
                [
                    Arc::clone(&resultados.tablas.cfdi),
                    Arc::clone(&resultados.tablas.resumen),
                    Arc::clone(&resultados.tablas.clasificaciones),
                    Arc::clone(&resultados.tablas.errores),
                ]
            })
            .unwrap_or_else(|| std::array::from_fn(|_| Arc::new(Vec::new())));

        for (table, rows) in self.tables.iter().zip(rows) {
            table.update(cx, |table, cx| table.set_rows(rows, cx));
        }
    }
}
