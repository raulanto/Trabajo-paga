use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use chrono::Local;
use ely_gpui_component::{
    buttons::Button,
    data_display::{Badge, Tone},
    feedback::{Toast, ToastViewport, Toaster},
    forms::{Input, InputEvent, TextInput},
    motion::ProgressBar,
    primitives::Severity,
    theme::{ActiveTheme, Mode, Radius, Theme},
};
use gpui::{
    Context, Entity, ExternalPaths, FontWeight, IntoElement, KeyDownEvent, ParentElement,
    PathPromptOptions, Render, SharedString, Styled, Subscription, Window, div, prelude::*, rems,
};

use crate::{
    application::{
        config_service::ConfigRepository,
        ports::ExcelExporter,
        procesar_zip::{ProcesarZip, ResultadoProceso},
    },
    domain::config::{AppConfig, TemaApp},
    infrastructure::{
        config_toml::TomlConfigRepository, xlsx_exporter::XlsxExporter, xml_parser::XmlCfdiParser,
        zip_reader::ZipXmlReader,
    },
};

use super::{
    rfc::{aviso_rfc, normalizar_rfc},
    state::AppState,
};

const APP_CONTEXT: &str = "CfdiZipProcessor";

enum WorkerMessage {
    Progress {
        solicitud: u64,
        procesados: usize,
        total: usize,
    },
    Complete {
        solicitud: u64,
        resultado: ResultadoProceso,
    },
}

pub struct AppView {
    state: AppState,
    rfc_input: Entity<TextInput>,
    toaster: Entity<Toaster>,
    next_request: u64,
    _rfc_subscription: Subscription,
    _appearance_subscription: Subscription,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = cargar_configuracion();
        let tema = config.apariencia.tema;
        Theme::set_mode(modo_tema(&tema, window), cx);
        let appearance_subscription = cx.observe_window_appearance(window, move |_, window, cx| {
            if matches!(&tema, TemaApp::Sistema) {
                Theme::set_mode(window.appearance().into(), cx);
            }
        });

        let rfc = if config.general.recordar_rfc {
            config.general.rfc_propio.unwrap_or_default()
        } else {
            String::new()
        };
        let rfc = normalizar_rfc(&rfc);
        let rfc_input = cx.new(|cx| {
            TextInput::new(window, cx)
                .label("RFC propio")
                .placeholder("AAA010101AAA")
        });
        rfc_input.update(cx, |input, cx| input.set_text(rfc, cx));

        let rfc_subscription = cx.subscribe(&rfc_input, |_, input, event: &InputEvent, cx| {
            if *event != InputEvent::Changed {
                return;
            }

            let value = input.read(cx).text().to_owned();
            let uppercase = normalizar_rfc(&value);
            if uppercase != value {
                let selection = input.read(cx).selection();
                let start = normalizar_rfc(&value[..selection.start]).len();
                let end = normalizar_rfc(&value[..selection.end]).len();
                let input = input.clone();
                cx.defer(move |cx| {
                    input.update(cx, |input, cx| {
                        input.set_text(uppercase, cx);
                        input.select(start..end, cx);
                    });
                });
            }
        });

        Self {
            state: AppState::default(),
            rfc_input,
            toaster: cx.new(|_| Toaster::default()),
            next_request: 0,
            _rfc_subscription: rfc_subscription,
            _appearance_subscription: appearance_subscription,
        }
    }

    fn open_zip_dialog(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Seleccionar archivo ZIP".into()),
        });
        cx.spawn(async move |this, cx| match receiver.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next()
                    && let Err(error) = this.update(cx, |view, cx| view.cargar_zip(path, cx))
                {
                    eprintln!("No se pudo actualizar la ventana tras elegir un ZIP: {error}");
                }
            }
            Ok(Err(_)) => {
                let _ = this.update(cx, |view, cx| {
                    view.state
                        .fallar("No se pudo abrir el selector de archivos.");
                    view.toast(
                        "No se pudo abrir el selector de archivos.",
                        Severity::Danger,
                        cx,
                    );
                    cx.notify();
                });
            }
            Ok(Ok(None)) | Err(_) => {}
        })
        .detach();
    }

    fn cargar_zip(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !es_zip(&path) {
            self.state
                .fallar_con_ruta(Some(path), "Selecciona un archivo con extensión .zip.");
            self.toast(
                "Selecciona un archivo con extensión .zip.",
                Severity::Warning,
                cx,
            );
            cx.notify();
            return;
        }
        self.state.cargar_zip(path);
        cx.notify();
    }

    fn procesar_zip(&mut self, cx: &mut Context<Self>) {
        self.next_request = self.next_request.wrapping_add(1);
        let solicitud = self.next_request;
        let Some(path) = self.state.iniciar_proceso(solicitud) else {
            return;
        };
        let rfc = self.rfc_input.read(cx).text().trim().to_owned();
        let rfc = (!rfc.is_empty()).then_some(rfc);
        let (sender, receiver) = async_channel::unbounded();
        cx.notify();

        cx.background_executor()
            .spawn(async move {
                let processor = ProcesarZip::new(ZipXmlReader, XmlCfdiParser);
                let mut last_update = Instant::now();
                let resultado = processor.ejecutar_con_progreso(path, rfc, |procesados, total| {
                    if procesados == 0
                        || procesados == total
                        || last_update.elapsed() >= Duration::from_millis(50)
                    {
                        last_update = Instant::now();
                        let _ = sender.send_blocking(WorkerMessage::Progress {
                            solicitud,
                            procesados,
                            total,
                        });
                    }
                });
                let _ = sender.send_blocking(WorkerMessage::Complete {
                    solicitud,
                    resultado,
                });
            })
            .detach();

        cx.spawn(async move |this, cx| {
            while let Ok(message) = receiver.recv().await {
                let complete = matches!(message, WorkerMessage::Complete { .. });
                if let Err(error) = this.update(cx, |view, cx| view.recibir_mensaje(message, cx)) {
                    eprintln!("No se pudo actualizar el progreso del ZIP: {error}");
                    break;
                }
                if complete {
                    break;
                }
            }
        })
        .detach();
    }

    fn recibir_mensaje(&mut self, message: WorkerMessage, cx: &mut Context<Self>) {
        match message {
            WorkerMessage::Progress {
                solicitud,
                procesados,
                total,
            } => {
                if self.state.actualizar_progreso(solicitud, procesados, total) {
                    cx.notify();
                }
            }
            WorkerMessage::Complete {
                solicitud,
                resultado,
            } => {
                if self.state.terminar_proceso(solicitud, resultado)
                    && let AppState::Error { mensaje, .. } = &self.state
                {
                    self.toast(mensaje.clone(), Severity::Danger, cx);
                    cx.notify();
                } else {
                    cx.notify();
                }
            }
        }
    }

    fn elegir_destino_excel(&mut self, cx: &mut Context<Self>) {
        if !self.state.tiene_resultados() {
            return;
        }
        let directory = self
            .state
            .ruta_zip()
            .and_then(|path| path.parent())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let suggested = format!("cfdi_{}.xlsx", Local::now().format("%Y%m%d_%H%M"));
        let receiver = cx.prompt_for_new_path(&directory, Some(&suggested));

        cx.spawn(async move |this, cx| match receiver.await {
            Ok(Ok(Some(path))) => {
                if let Err(error) = this.update(cx, |view, cx| view.exportar_a(path, cx)) {
                    eprintln!("No se pudo iniciar la exportación: {error}");
                }
            }
            Ok(Err(_)) => {
                let _ = this.update(cx, |view, cx| {
                    view.toast(
                        "No se pudo abrir el selector para guardar.",
                        Severity::Danger,
                        cx,
                    );
                });
            }
            Ok(Ok(None)) | Err(_) => {}
        })
        .detach();
    }

    fn exportar_a(&mut self, mut path: PathBuf, cx: &mut Context<Self>) {
        let Some((solicitud, resultado)) = self.state.iniciar_exportacion() else {
            return;
        };
        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("xlsx"))
        {
            path.set_extension("xlsx");
        }
        let destino = path.clone();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let exportacion = cx
                .background_spawn(async move {
                    XlsxExporter
                        .exportar(
                            &resultado.filas,
                            &resultado.resumen,
                            &resultado.clasificaciones,
                            &resultado.errores,
                            &destino,
                        )
                        .map(|()| destino)
                })
                .await;

            if let Err(error) = &exportacion {
                eprintln!("No se pudo exportar el archivo Excel: {error}");
            }
            let _ = this.update(cx, |view, cx| {
                let estado_exportacion =
                    exportacion.map_err(|_| "No se pudo exportar el archivo Excel.".to_owned());
                if view
                    .state
                    .terminar_exportacion(solicitud, estado_exportacion)
                {
                    match &view.state {
                        AppState::Resultados {
                            ruta_exportada: Some(path),
                            ..
                        } => view.toast(
                            format!("Exportado en {}", path.display()),
                            Severity::Success,
                            cx,
                        ),
                        AppState::Resultados {
                            error_exportacion: Some(error),
                            ..
                        } => view.toast(error.clone(), Severity::Danger, cx),
                        _ => {}
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn toast(
        &mut self,
        message: impl Into<SharedString>,
        severity: Severity,
        cx: &mut Context<Self>,
    ) {
        self.toaster.update(cx, |toaster, cx| {
            toaster.push(Toast::new(message).severity(severity), cx);
        });
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let zip_name = self
            .state
            .ruta_zip()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned());
        let can_export = matches!(
            &self.state,
            AppState::Resultados {
                exportando: false,
                ..
            }
        );

        div()
            .flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_4()
            .px_5()
            .py_3()
            .flex_wrap()
            .border_b_1()
            .border_color(theme.colors.border)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .flex_shrink_0()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(rems(1.25))
                                    .child("Procesamiento de CFDI"),
                            )
                            .child(
                                div()
                                    .text_size(rems(0.875))
                                    .text_color(theme.colors.fg_muted)
                                    .child("Extrae datos XML y exporta a Excel"),
                            ),
                    )
                    .when_some(zip_name, |bar, name| {
                        bar.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(rems(0.875))
                                .text_color(theme.colors.fg_muted)
                                .text_ellipsis()
                                .child(format!("ZIP · {name}")),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("open-zip-toolbar", "Abrir ZIP…")
                            .on_click(cx.listener(|view, _, _, cx| view.open_zip_dialog(cx))),
                    )
                    .child(
                        Button::new("export-excel-toolbar", "Exportar Excel…")
                            .disabled(!can_export)
                            .on_click(cx.listener(|view, _, _, cx| view.elegir_destino_excel(cx))),
                    ),
            )
    }

    fn render_options(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let can_process = matches!(
            &self.state,
            AppState::ZipCargado { .. } | AppState::Resultados { .. }
        );
        let is_processing = matches!(&self.state, AppState::Procesando { .. });
        let value = self.rfc_input.read(cx).text().to_owned();

        div()
            .flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_4()
            .px_5()
            .py_3()
            .flex_wrap()
            .border_b_1()
            .border_color(theme.colors.border)
            .bg(theme.colors.surface)
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(rems(0.875)).child("RFC propio"))
                            .child(
                                div()
                                    .text_size(rems(0.75))
                                    .text_color(theme.colors.fg_muted)
                                    .child("Opcional · Para sugerir Ingreso/Gasto"),
                            ),
                    )
                    .child(div().w(rems(13.)).child(Input::new(&self.rfc_input)))
                    .when_some(aviso_rfc(&value), |row, warning| {
                        row.child(
                            div()
                                .max_w(rems(15.))
                                .text_size(rems(0.75))
                                .text_color(theme.colors.warning)
                                .child(warning),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new(
                            "process-zip",
                            if is_processing {
                                "Procesando ZIP"
                            } else {
                                "Procesar ZIP"
                            },
                        )
                        .primary()
                        .disabled(!can_process || is_processing)
                        .on_click(cx.listener(|view, _, _, cx| view.procesar_zip(cx))),
                    )
                    .child(Button::new("settings", "Configuración…").disabled(true)),
            )
    }

    fn render_load_area(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let heading = match &self.state {
            AppState::Vacio => "Carga un archivo ZIP",
            AppState::ZipCargado { .. } => "ZIP listo para procesar",
            AppState::Procesando { .. } => "Procesando el ZIP",
            AppState::Resultados { .. } => "Procesamiento completado",
            AppState::Error { .. } => "No se pudo procesar el ZIP",
        };
        let detail = match &self.state {
            AppState::Vacio => {
                "Arrastra y suelta un archivo .zip o selecciónalo desde el equipo.".to_owned()
            }
            AppState::ZipCargado { .. } => {
                "Pulsa «Procesar ZIP» para revisar los comprobantes.".to_owned()
            }
            AppState::Procesando { .. } => {
                "Puedes seguir usando la ventana mientras se procesa.".to_owned()
            }
            AppState::Resultados { .. } => {
                "Los resultados están listos para exportarse a Excel.".to_owned()
            }
            AppState::Error { mensaje, .. } => mensaje.clone(),
        };
        let detail_color = if matches!(&self.state, AppState::Error { .. }) {
            theme.colors.danger
        } else {
            theme.colors.fg_muted
        };
        let listener = cx.listener(|view, paths: &ExternalPaths, _, cx| {
            if paths.paths().len() == 1 {
                if let Some(path) = paths.paths().first().cloned() {
                    view.cargar_zip(path, cx);
                }
            } else {
                view.toast("Suelta un solo archivo ZIP.", Severity::Warning, cx);
            }
        });

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .w_full()
            .max_w(rems(60.))
            .p_6()
            .rounded(theme.radius(Radius::Lg))
            .border_1()
            .border_dashed()
            .border_color(theme.colors.border)
            .bg(theme.colors.surface)
            .drag_over::<ExternalPaths>(|style, paths, _, cx| {
                let dropped = paths.paths();
                let takes_zip = dropped.len() == 1 && dropped.iter().all(|path| es_zip(path));
                if takes_zip {
                    style
                        .border_color(cx.theme().colors.focus)
                        .bg(cx.theme().colors.hover)
                } else {
                    style
                        .border_color(cx.theme().colors.danger)
                        .bg(cx.theme().colors.danger_subtle)
                }
            })
            .on_drop(listener)
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(rems(1.125))
                    .child(heading),
            )
            .child(
                div()
                    .max_w(rems(44.))
                    .text_size(rems(0.875))
                    .text_color(detail_color)
                    .text_center()
                    .child(detail),
            )
            .child(
                Button::new("select-zip", "Seleccionar archivo ZIP")
                    .on_click(cx.listener(|view, _, _, cx| view.open_zip_dialog(cx))),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(Badge::new("CFDI 3.3 / 4.0").tone(Tone::Neutral))
                    .child(Badge::new("Exportación Excel a 4 hojas").tone(Tone::Neutral))
                    .child(Badge::new("Continúa ante XML dañados").tone(Tone::Neutral)),
            )
    }

    fn render_progress(&self) -> impl IntoElement {
        match &self.state {
            AppState::Procesando {
                solicitud: _,
                procesados,
                total,
                ..
            } => {
                let fraction = if *total == 0 {
                    0.0
                } else {
                    *procesados as f32 / *total as f32
                };
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .w_full()
                    .max_w(rems(60.))
                    .child(format!(
                        "Procesando {} de {}",
                        format_integer(*procesados),
                        format_integer(*total)
                    ))
                    .child(ProgressBar::new("zip-progress", fraction))
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }

    fn render_metrics(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let AppState::Resultados { resultado, .. } = &self.state else {
            return div().into_any_element();
        };
        let values = [
            ("XML leídos", resultado.total_xml_leidos),
            ("CFDI válidos", resultado.total_cfdi_ok),
            ("Conceptos", resultado.filas.len()),
            ("Errores", resultado.errores.len()),
        ];

        div()
            .flex()
            .w_full()
            .max_w(rems(60.))
            .items_center()
            .gap_4()
            .p_4()
            .rounded(theme.radius(Radius::Md))
            .border_1()
            .border_color(theme.colors.border)
            .bg(theme.colors.surface)
            .children(values.into_iter().map(|(label, value)| {
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        div()
                            .text_size(rems(0.75))
                            .text_color(theme.colors.fg_muted)
                            .child(label),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(rems(1.25))
                            .text_color(if label == "Errores" && value > 0 {
                                theme.colors.danger
                            } else {
                                theme.colors.fg
                            })
                            .child(format_integer(value)),
                    )
            }))
            .into_any_element()
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let path = self
            .state
            .ruta_zip()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Sin archivo ZIP".to_owned());
        let status = match &self.state {
            AppState::Procesando {
                procesados, total, ..
            } => format!(
                "Procesando {} de {}",
                format_integer(*procesados),
                format_integer(*total)
            ),
            AppState::Resultados {
                exportando: true, ..
            } => "Exportando Excel…".to_owned(),
            AppState::Resultados {
                ruta_exportada: Some(path),
                ..
            } => format!("Exportado en {}", path.display()),
            AppState::Error { .. } => "Error".to_owned(),
            _ => "Listo".to_owned(),
        };

        div()
            .flex()
            .w_full()
            .items_center()
            .gap_3()
            .px_4()
            .py_2()
            .border_t_1()
            .border_color(theme.colors.border)
            .bg(theme.colors.surface)
            .text_color(theme.colors.fg_muted)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(rems(0.75))
                    .text_ellipsis()
                    .child(format!("ZIP · {path}")),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_center()
                    .text_size(rems(0.75))
                    .text_ellipsis()
                    .child("Ctrl/Cmd+O Abrir · Ctrl/Cmd+E Exportar · Ctrl/Cmd+1–4 Pestañas"),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .min_w_0()
                    .max_w(rems(22.))
                    .text_size(rems(0.75))
                    .text_ellipsis()
                    .child(status),
            )
    }

    fn on_shortcut(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if !(modifiers.control || modifiers.platform) {
            return;
        }
        match event.keystroke.key.to_ascii_lowercase().as_str() {
            "o" => self.open_zip_dialog(cx),
            "e" if self.state.tiene_resultados() => self.elegir_destino_excel(cx),
            _ => (),
        }
    }
}

impl Render for AppView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("cfdi-app")
            .key_context(APP_CONTEXT)
            .on_key_down(cx.listener(Self::on_shortcut))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.colors.bg)
            .text_color(theme.colors.fg)
            .child(self.render_toolbar(cx))
            .child(self.render_options(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .items_center()
                    .justify_center()
                    .gap_5()
                    .p_5()
                    .child(self.render_load_area(cx))
                    .child(self.render_progress())
                    .child(self.render_metrics(cx)),
            )
            .child(self.render_status_bar(cx))
            .child(ToastViewport::new("app-toasts", &self.toaster))
    }
}

fn cargar_configuracion() -> AppConfig {
    match TomlConfigRepository::new().cargar() {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "No se pudo cargar la configuración; se usarán los valores iniciales: {error}"
            );
            AppConfig::default()
        }
    }
}

fn modo_tema(tema: &TemaApp, window: &Window) -> Mode {
    match tema {
        TemaApp::Sistema => window.appearance().into(),
        TemaApp::Claro => Mode::Light,
        TemaApp::Oscuro => Mode::Dark,
    }
}

fn es_zip(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}

fn format_integer(value: usize) -> String {
    let mut result = String::new();
    for (index, character) in value.to_string().chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            result.push(',');
        }
        result.push(character);
    }
    result.chars().rev().collect()
}
