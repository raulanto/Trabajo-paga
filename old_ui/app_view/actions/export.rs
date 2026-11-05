//! Selección de destino y exportación a Excel.

use std::path::{Path, PathBuf};

use chrono::Local;
use gpui::Context;

use crate::{application::ports::ExcelExporter, infrastructure::xlsx_exporter::XlsxExporter};

use super::super::{AppView, util::ensure_extension};

impl AppView {
    pub(in crate::ui::app_view) fn choose_export_path(&mut self, cx: &mut Context<Self>) {
        if self.state.resultados.is_none() || self.state.exportando {
            return;
        }
        let directory = self
            .state
            .ruta_zip
            .as_deref()
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."));
        let suggested = format!("cfdi_{}.xlsx", Local::now().format("%Y%m%d_%H%M"));
        let receiver = cx.prompt_for_new_path(directory, Some(&suggested));

        cx.spawn(async move |this, cx| match receiver.await {
            Ok(Ok(Some(path))) => {
                this.update(cx, |this, cx| this.export_to(path, cx)).ok();
            }
            Ok(Err(error)) => {
                eprintln!("No se pudo abrir el selector para guardar: {error}");
                this.update(cx, |this, cx| {
                    this.state
                        .set_error("No se pudo abrir el selector para guardar.");
                    this.replace_tabs(cx);
                    this.sync_table_rows(cx);
                    this.pending_toast = Some((
                        "No se pudo abrir el selector para guardar.".to_owned(),
                        false,
                    ));
                    cx.notify();
                })
                .ok();
            }
            Ok(Ok(None)) | Err(_) => {}
        })
        .detach();
    }

    pub(in crate::ui::app_view) fn export_to(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(resultados) = self.state.resultados.clone() else {
            return;
        };
        let path = ensure_extension(path, "xlsx");
        self.state.exportando = true;
        self.state.mensaje_error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let path_for_export = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    XlsxExporter.exportar(
                        resultados.filas.as_slice(),
                        resultados.resumen.as_slice(),
                        resultados.clasificaciones.as_slice(),
                        resultados.errores.as_slice(),
                        &path_for_export,
                    )
                })
                .await;

            this.update(cx, |this, cx| {
                this.state.exportando = false;
                match result {
                    Ok(()) => {
                        this.state.ruta_exportada = Some(path);
                        this.state.mensaje_error = None;
                        this.pending_toast =
                            Some(("Excel exportado correctamente.".to_owned(), true));
                    }
                    Err(error) => {
                        eprintln!("No se pudo exportar el Excel: {error}");
                        this.state.mensaje_error =
                            Some("No se pudo exportar el archivo Excel.".to_owned());
                        this.pending_toast =
                            Some(("No se pudo exportar el archivo Excel.".to_owned(), false));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
