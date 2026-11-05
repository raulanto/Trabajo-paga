//! Apertura y carga de archivos ZIP.

use std::path::PathBuf;

use gpui::{Context, PathPromptOptions};

use super::super::{AppView, util::es_zip};

impl AppView {
    pub(in crate::ui::app_view) fn open_zip_dialog(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Seleccionar archivo ZIP".into()),
        });
        cx.spawn(async move |this, cx| match receiver.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next() {
                    this.update(cx, |this, cx| this.load_zip(path, cx)).ok();
                }
            }
            Ok(Err(error)) => {
                eprintln!("No se pudo abrir el selector de archivos: {error}");
                this.update(cx, |this, cx| {
                    this.state
                        .set_error("No se pudo abrir el selector de archivos.");
                    this.replace_tabs(cx);
                    this.sync_table_rows(cx);
                    this.pending_toast = Some((
                        "No se pudo abrir el selector de archivos.".to_owned(),
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

    pub(in crate::ui::app_view) fn load_zip(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.state.set_zip(path.clone());
        if !es_zip(&path) {
            let message = "Selecciona un archivo con extensión .zip.";
            self.state.set_error(message);
            self.pending_toast = Some((message.to_owned(), false));
        }
        self.replace_tabs(cx);
        self.sync_table_rows(cx);
        cx.notify();
    }
}
