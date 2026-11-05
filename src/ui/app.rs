use std::{
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

use chrono::Local;
use ely_gpui_component::{
    buttons::Button,
    data_display::{Badge, Tone},
    feedback::{Toast, ToastViewport, Toaster},
    forms::{Choice, Input, InputEvent, TextInput},
    motion::ProgressBar,
    navigation::Tabs,
    primitives::Severity,
    tables::{DataTable, FilterBuilder, FilterRule},
    theme::{ActiveTheme, Mode, Radius, Theme},
};
use gpui::{
    ClipboardItem, Context, Entity, ExternalPaths, FontWeight, IntoElement, KeyDownEvent,
    ParentElement, PathPromptOptions, Render, SharedString, Styled, Subscription, Window, div,
    prelude::*, rems,
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
    table_data::{
        self, CFDI_COLUMNS, CFDI_HEADERS, CLASIFICACION_COLUMNS, CLASIFICACION_HEADERS,
        ERROR_COLUMNS, ERROR_HEADERS, ImporteTotal, RESUMEN_COLUMNS, RESUMEN_HEADERS, UiTables,
    },
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
        tablas: Box<table_data::PreparedTables>,
    },
}

pub struct AppView {
    state: AppState,
    rfc_input: Entity<TextInput>,
    toaster: Entity<Toaster>,
    tablas: Option<UiTables>,
    active_tab: usize,
    selected_rows: [Vec<SharedString>; 4],
    table_search: [Entity<TextInput>; 4],
    filter_rules: [Vec<FilterRule>; 4],
    filter_any: [bool; 4],
    filter_panel_open: bool,
    next_request: u64,
    _rfc_subscription: Subscription,
    _table_search_subscriptions: [Subscription; 4],
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

        let table_search = std::array::from_fn(|_| {
            cx.new(|cx| {
                TextInput::new(window, cx)
                    .label("Buscar")
                    .placeholder("Buscar en esta tabla")
            })
        });
        let table_search_subscriptions = std::array::from_fn(|index| {
            cx.subscribe(&table_search[index], |_, _, event: &InputEvent, cx| {
                if *event == InputEvent::Changed {
                    cx.notify();
                }
            })
        });

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
            tablas: None,
            active_tab: 0,
            selected_rows: std::array::from_fn(|_| Vec::new()),
            table_search,
            filter_rules: std::array::from_fn(|_| Vec::new()),
            filter_any: [false; 4],
            filter_panel_open: false,
            next_request: 0,
            _rfc_subscription: rfc_subscription,
            _table_search_subscriptions: table_search_subscriptions,
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
        self.tablas = None;
        self.active_tab = 0;
        self.selected_rows = std::array::from_fn(|_| Vec::new());
        self.filter_rules = std::array::from_fn(|_| Vec::new());
        self.filter_any = [false; 4];
        self.filter_panel_open = false;
        for search in &self.table_search {
            search.update(cx, |input, cx| input.set_text("", cx));
        }
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
                let tablas = table_data::PreparedTables::from_result(&resultado);
                let _ = sender.send_blocking(WorkerMessage::Complete {
                    solicitud,
                    resultado,
                    tablas: Box::new(tablas),
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
                tablas,
            } => {
                if self.state.terminar_proceso(solicitud, resultado) {
                    if self.state.tiene_resultados() {
                        self.tablas = Some((*tablas).into_ui());
                        self.active_tab = 0;
                        self.selected_rows = std::array::from_fn(|_| Vec::new());
                    } else if let AppState::Error { mensaje, .. } = &self.state {
                        self.toast(mensaje.clone(), Severity::Danger, cx);
                    }
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

    fn render_results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(tables) = &self.tablas else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        let (table, columns, table_id) = match self.active_tab {
            1 => (&tables.resumen, &RESUMEN_COLUMNS[..], "result-resumen"),
            2 => (
                &tables.clasificacion,
                &CLASIFICACION_COLUMNS[..],
                "result-clasificacion",
            ),
            3 => (&tables.errores, &ERROR_COLUMNS[..], "result-errores"),
            _ => (&tables.cfdi, &CFDI_COLUMNS[..], "result-cfdi"),
        };
        let row_count = table.rows.len();
        let search = self.table_search[self.active_tab]
            .read(cx)
            .text()
            .to_owned();
        let filters = self.filter_rules[self.active_tab].clone();
        let filter_any = self.filter_any[self.active_tab];
        let visible_count = table.visible_count(&search, columns, &filters, filter_any);
        let selection = self.selected_rows[self.active_tab].clone();
        let weak_view = cx.weak_entity();
        let selection_tab = self.active_tab;

        let table_body = if visible_count == 0 {
            let empty_text = if row_count == 0 && self.active_tab == 3 {
                "No se registraron errores en este ZIP"
            } else if row_count > 0 {
                "Ninguna fila coincide con la búsqueda y los filtros."
            } else {
                "Sin filas para mostrar"
            };
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .items_center()
                .justify_center()
                .text_color(cx.theme().colors.fg_muted)
                .child(empty_text)
                .into_any_element()
        } else {
            DataTable::new(table_id, table_data::columns(columns))
                .rows(Rc::clone(&table.rows))
                .query(search)
                .filters(filters, filter_any)
                .selected(selection)
                .on_select(move |keys, _, cx| {
                    let keys = keys.to_vec();
                    if let Err(error) = weak_view.update(cx, |view, cx| {
                        view.selected_rows[selection_tab] = keys;
                        cx.notify();
                    }) {
                        eprintln!("No se pudo actualizar la selección de la tabla: {error}");
                    }
                })
                .virtualized()
                .size_full()
                .into_any_element()
        };
        let search_field = div()
            .w(rems(24.))
            .child(Input::new(&self.table_search[self.active_tab]));
        let filter_count = self.filter_rules[self.active_tab]
            .iter()
            .filter(|rule| {
                rule.test == ely_gpui_component::tables::Test::Empty
                    || !rule.value.trim().is_empty()
            })
            .count();
        let filter_label = if filter_count == 0 {
            "Filtros…".to_owned()
        } else {
            format!("Filtros ({filter_count})")
        };
        let filter_button =
            Button::new("table-filters", filter_label).on_click(cx.listener(|view, _, _, cx| {
                view.filter_panel_open = !view.filter_panel_open;
                cx.notify();
            }));
        let search_toolbar = div()
            .flex()
            .items_end()
            .justify_between()
            .gap_3()
            .px_3()
            .py_2()
            .child(search_field)
            .child(filter_button);
        let filter_panel = if self.filter_panel_open {
            let active_tab = self.active_tab;
            let rules = self.filter_rules[active_tab].clone();
            let any = self.filter_any[active_tab];
            let weak_view = cx.weak_entity();
            Some(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.colors.border)
                    .child(
                        FilterBuilder::new(
                            ("result-filter-builder", active_tab),
                            columns.iter().map(|column| (column.key, column.title)),
                        )
                        .rules(rules, any)
                        .on_change(move |rules, any, _, cx| {
                            let rules = rules.to_vec();
                            if let Err(error) = weak_view.update(cx, |view, cx| {
                                view.filter_rules[active_tab] = rules;
                                view.filter_any[active_tab] = any;
                                cx.notify();
                            }) {
                                eprintln!("No se pudieron actualizar los filtros: {error}");
                            }
                        }),
                    ),
            )
        } else {
            None
        };

        let total_importe = if self.active_tab == 0 {
            match &tables.importe_total {
                ImporteTotal::SinDatos => None,
                ImporteTotal::Etiqueta(label) => Some(label.clone()),
            }
        } else {
            None
        };
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px_3()
            .py_2()
            .border_t_1()
            .border_color(theme.colors.border)
            .text_size(rems(0.75))
            .text_color(theme.colors.fg_muted)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(format!(
                        "{} filas · {} columnas",
                        format_integer(visible_count),
                        format_integer(columns.len())
                    ))
                    .when_some(total_importe, |row, total| row.child(total)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .text_ellipsis()
                    .child(
                        "Ordenar: clic en encabezado · Reordenar: arrastra encabezado · Ancho: arrastra borde",
                    )
                    .child(
                        "Desplazar: rueda o Shift+rueda · Copiar TSV: filas seleccionadas o pestaña completa",
                ),
            );
        let panel = div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(search_toolbar)
            .children(filter_panel)
            .child(div().flex_1().min_h_0().child(table_body))
            .child(footer);

        let choices = [
            Choice::new(
                "cfdi",
                format!("CFDI ({})", format_integer(tables.cfdi.rows.len())),
            ),
            Choice::new(
                "resumen",
                format!(
                    "Resumen por producto ({})",
                    format_integer(tables.resumen.rows.len())
                ),
            ),
            Choice::new(
                "clasificacion",
                format!(
                    "Clasificación sugerida ({})",
                    format_integer(tables.clasificacion.rows.len())
                ),
            ),
            Choice::new(
                "errores",
                format!("Errores ({})", format_integer(tables.errores.rows.len())),
            ),
        ];
        let selected_id = match self.active_tab {
            1 => "resumen",
            2 => "clasificacion",
            3 => "errores",
            _ => "cfdi",
        };
        let weak_view = cx.weak_entity();
        let tabs = Tabs::new("result-tabs", choices, selected_id)
            .panel(panel)
            .on_change(move |value, _, cx| {
                let tab = match value.as_ref() {
                    "resumen" => 1,
                    "clasificacion" => 2,
                    "errores" => 3,
                    _ => 0,
                };
                if let Err(error) = weak_view.update(cx, |view, cx| {
                    view.active_tab = tab;
                    cx.notify();
                }) {
                    eprintln!("No se pudo cambiar la pestaña de resultados: {error}");
                }
            });
        let copy_button = ely_gpui_component::buttons::Button::new("copy-tsv", "Copiar TSV")
            .disabled(row_count == 0)
            .on_click(cx.listener(|view, _, _, cx| view.copiar_tabla(cx)));

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .w_full()
            .gap_2()
            .child(self.render_metrics(cx))
            .child(div().flex().justify_end().child(copy_button))
            .child(div().flex_1().min_h_0().child(tabs))
            .into_any_element()
    }

    fn copiar_tabla(&mut self, cx: &mut Context<Self>) {
        let Some(tables) = &self.tablas else {
            return;
        };
        let (table, headers) = match self.active_tab {
            1 => (&tables.resumen, &RESUMEN_HEADERS[..]),
            2 => (&tables.clasificacion, &CLASIFICACION_HEADERS[..]),
            3 => (&tables.errores, &ERROR_HEADERS[..]),
            _ => (&tables.cfdi, &CFDI_HEADERS[..]),
        };
        let tsv = table.to_tsv(headers, &self.selected_rows[self.active_tab]);
        let rows = if self.selected_rows[self.active_tab].is_empty() {
            table.rows.len()
        } else {
            self.selected_rows[self.active_tab].len()
        };
        cx.write_to_clipboard(ClipboardItem::new_string(tsv));
        self.toast(
            format!("{} filas copiadas como TSV.", format_integer(rows)),
            Severity::Success,
            cx,
        );
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
            "1" if self.state.tiene_resultados() => {
                self.active_tab = 0;
                cx.notify();
            }
            "2" if self.state.tiene_resultados() => {
                self.active_tab = 1;
                cx.notify();
            }
            "3" if self.state.tiene_resultados() => {
                self.active_tab = 2;
                cx.notify();
            }
            "4" if self.state.tiene_resultados() => {
                self.active_tab = 3;
                cx.notify();
            }
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
            .child(if self.state.tiene_resultados() {
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .p_4()
                    .child(self.render_results(cx))
                    .into_any_element()
            } else {
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
                    .into_any_element()
            })
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
