//! Procesamiento del ZIP en segundo plano y mensajes del worker.

use std::time::{Duration, Instant};

use gpui::Context;

use crate::{
    application::procesar_zip::{ProcesarZip, ResultadoProceso},
    infrastructure::{xml_parser::XmlCfdiParser, zip_reader::ZipXmlReader},
};

use super::super::{AppView, EstadoApp, TablasPreparadas};

enum WorkerMessage {
    Progress(usize, usize),
    Complete(ResultadoProceso, TablasPreparadas),
}

impl AppView {
    pub(in crate::ui::app_view) fn process_zip(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.state.ruta_zip.clone() else {
            return;
        };
        if !self.state.start_processing() {
            return;
        }
        self.sync_table_rows(cx);
        cx.notify();

        let rfc = (!self.state.rfc_propio.trim().is_empty())
            .then(|| self.state.rfc_propio.trim().to_owned());
        let (sender, receiver) = async_channel::unbounded();
        cx.background_executor()
            .spawn(async move {
                let processor = ProcesarZip::new(ZipXmlReader, XmlCfdiParser);
                let mut last_update = Instant::now();
                let result = processor.ejecutar_con_progreso(path, rfc, |processed, total| {
                    if processed == 0
                        || processed == total
                        || last_update.elapsed() >= Duration::from_millis(50)
                    {
                        last_update = Instant::now();
                        let _ = sender.send_blocking(WorkerMessage::Progress(processed, total));
                    }
                });
                let tablas = TablasPreparadas::desde(&result);
                let _ = sender.send_blocking(WorkerMessage::Complete(result, tablas));
            })
            .detach();

        cx.spawn(async move |this, cx| {
            while let Ok(message) = receiver.recv().await {
                let complete = matches!(message, WorkerMessage::Complete(..));
                if this
                    .update(cx, |this, cx| this.handle_worker_message(message, cx))
                    .is_err()
                    || complete
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn handle_worker_message(&mut self, message: WorkerMessage, cx: &mut Context<Self>) {
        match message {
            WorkerMessage::Progress(processed, total) => {
                self.state.set_progress(processed, total);
            }
            WorkerMessage::Complete(result, tablas) => {
                self.state.finish_processing(result, tablas);
                self.replace_tabs(cx);
                self.sync_table_rows(cx);
                if self.state.estado == EstadoApp::Error {
                    let message = self
                        .state
                        .mensaje_error
                        .clone()
                        .unwrap_or_else(|| "No se pudo procesar el ZIP.".to_owned());
                    self.pending_toast = Some((message, false));
                }
            }
        }
        cx.notify();
    }
}
