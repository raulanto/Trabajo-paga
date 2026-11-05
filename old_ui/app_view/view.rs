//! Componente principal: estado, suscripciones y composición de la ventana.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use gpui::{
    Context, Entity, FileDropEvent, FocusHandle, Focusable, IntoElement, KeyDownEvent, Render,
    ScrollHandle, Subscription, Window, canvas, div, prelude::*, px,
};
use gpuikit::{
    elements::{
        progress::progress as progress_bar,
        tabs::{TabChanged, Tabs as KitTabs},
        toast::ToastExt,
        typography::small,
    },
    input::{InputState, InputStateEvent},
    layout::v_stack,
    theme::{ActiveTheme, Themeable},
};

use crate::{
    application::config_service::ConfigService,
    domain::config::AppConfig,
    infrastructure::config_toml::TomlConfigRepository,
    ui::{
        data_table::{
            CFDI_COLUMNS, CLASIFICACION_COLUMNS, ERROR_COLUMNS, RESUMEN_COLUMNS, TablaVista,
        },
        metrics::Metrics,
        status_bar::StatusBar,
        tabs, theme,
        toolbar::Toolbar,
    },
};

use super::{
    AppState, EstadoApp, Vista,
    state::ConfigSaveStatus,
    util::{format_integer, normalizar_caja_rfc},
};

pub struct AppView {
    pub state: AppState,
    pub(super) root_focus: FocusHandle,
    pub(super) rfc_input: Entity<InputState>,
    pub(super) excel_format_input: Entity<InputState>,
    pub(super) tabs: Entity<KitTabs>,
    pub(super) config_repository: TomlConfigRepository,
    pub(super) tables: [Entity<TablaVista>; 4],
    pub(super) settings_scroll: ScrollHandle,
    pub(super) last_config_snapshot: AppConfig,
    pub(super) config_save_generation: u64,
    pub(super) config_save_status: ConfigSaveStatus,
    pub(super) confirm_restore_all: bool,
    _rfc_subscription: Subscription,
    _excel_format_subscription: Subscription,
    _shortcut_subscriptions: [Subscription; 5],
    _config_observer: Subscription,
    pub(super) tab_subscription: Subscription,
    pub(super) pending_drop: Option<PathBuf>,
    pub(super) pending_toast: Option<(String, bool)>,
    _appearance_subscription: Option<Subscription>,
}

impl AppView {
    pub fn new(cx: &mut Context<Self>, window: &mut Window) -> Self {
        let config_repository = TomlConfigRepository::new();
        let mut state = AppState::default();
        let loaded = ConfigService::new(config_repository.clone()).cargar_o_default();
        state.config = loaded.clone();
        state.sync_rfc_from_config();
        theme::apply_configured_appearance(&state.config.apariencia.tema, window.appearance(), cx);

        let rfc_input = cx.new(InputState::new_singleline);
        rfc_input.update(cx, |input, cx| {
            input.set_content_silent(state.rfc_propio.clone(), cx);
        });

        let excel_format_input = cx.new(InputState::new_singleline);
        excel_format_input.update(cx, |input, cx| {
            input.set_content_silent(state.config.excel.formato_nombre_archivo.clone(), cx);
        });

        let shortcut_inputs: [Entity<InputState>; 5] = std::array::from_fn(|index| {
            let input = cx.new(InputState::new_singleline);
            let value = match index {
                0 => state.config.atajos.abrir_zip.clone(),
                1 => state.config.atajos.exportar.clone(),
                2 => state.config.atajos.abrir_configuracion.clone(),
                3 => state.config.atajos.pestaña_1.clone(),
                4 => state.config.atajos.pestaña_2.clone(),
                _ => String::new(),
            };
            input.update(cx, |input, cx| input.set_content_silent(value, cx));
            input
        });

        let tabs = tabs::create(cx, None, Vista::Cfdi);
        let empty_rows = Arc::new(Vec::new());
        let tables = [
            cx.new(|cx| TablaVista::new(&CFDI_COLUMNS, empty_rows.clone(), "cfdi-rows", cx)),
            cx.new(|cx| TablaVista::new(&RESUMEN_COLUMNS, empty_rows.clone(), "summary-rows", cx)),
            cx.new(|cx| {
                TablaVista::new(
                    &CLASIFICACION_COLUMNS,
                    empty_rows.clone(),
                    "classification-rows",
                    cx,
                )
            }),
            cx.new(|cx| TablaVista::new(&ERROR_COLUMNS, empty_rows, "error-rows", cx)),
        ];
        let table_presentation = (
            state.config.tabla.altura_fila as f32,
            state.config.tabla.mostrar_hover,
            state.config.tabla.overscan_px,
        );
        for table in &tables {
            table.update(cx, |table, cx| {
                table.set_presentacion(
                    table_presentation.0,
                    table_presentation.1,
                    table_presentation.2,
                    cx,
                );
            });
        }
        let appearance_subscription = cx.observe_window_appearance(window, |this, window, cx| {
            theme::apply_configured_appearance(
                &this.state.config.apariencia.tema,
                window.appearance(),
                cx,
            );
            cx.notify();
        });
        let rfc_subscription =
            cx.subscribe(&rfc_input, |this, input, event: &InputStateEvent, cx| {
                if matches!(event, InputStateEvent::TextChanged) {
                    let value = normalizar_caja_rfc(input.read(cx).content());
                    if value != input.read(cx).content() {
                        input.update(cx, |input, cx| input.set_content_silent(value.clone(), cx));
                    }
                    this.state.rfc_propio = value.clone();
                    this.state.config.general.rfc_propio = if value.trim().is_empty() {
                        None
                    } else {
                        Some(value)
                    };
                    this.config_changed(cx);
                }
            });
        let excel_format_subscription = cx.subscribe(
            &excel_format_input,
            |this, input, event: &InputStateEvent, cx| {
                if matches!(event, InputStateEvent::TextChanged) {
                    let value = input.read(cx).content();
                    let trimmed = value.trim();
                    this.state.config.excel.formato_nombre_archivo = if trimmed.is_empty() {
                        "cfdi_{YYYYMMDD}_{HHmm}.xlsx".to_owned()
                    } else {
                        trimmed.to_owned()
                    };
                    this.config_changed(cx);
                }
            },
        );
        let shortcut_subscriptions = std::array::from_fn(|index| {
            let input = shortcut_inputs[index].clone();
            cx.subscribe(&input, move |this, input, event: &InputStateEvent, cx| {
                if matches!(event, InputStateEvent::TextChanged) {
                    let value = input.read(cx).content().trim().to_owned();
                    let current = match index {
                        0 => &mut this.state.config.atajos.abrir_zip,
                        1 => &mut this.state.config.atajos.exportar,
                        2 => &mut this.state.config.atajos.abrir_configuracion,
                        3 => &mut this.state.config.atajos.pestaña_1,
                        4 => &mut this.state.config.atajos.pestaña_2,
                        _ => return,
                    };
                    *current = value;
                    this.config_changed(cx);
                }
            })
        });
        let tab_subscription = Self::subscribe_tabs(cx, &tabs);
        let config_observer = cx.observe_self(|this, cx| {
            this.schedule_config_save(cx);
            this.sync_table_preferences(cx);
        });
        let last_config_snapshot = state.config.clone();
        Self {
            state,
            root_focus: cx.focus_handle(),
            rfc_input,
            excel_format_input,
            tabs,
            config_repository,
            tables,
            settings_scroll: ScrollHandle::new(),
            last_config_snapshot,
            config_save_generation: 0,
            config_save_status: ConfigSaveStatus::Saved,
            confirm_restore_all: false,
            _rfc_subscription: rfc_subscription,
            _excel_format_subscription: excel_format_subscription,
            _shortcut_subscriptions: shortcut_subscriptions,
            _config_observer: config_observer,
            tab_subscription,
            pending_drop: None,
            pending_toast: None,
            _appearance_subscription: Some(appearance_subscription),
        }
    }

    fn subscribe_tabs(cx: &mut Context<Self>, tabs: &Entity<KitTabs>) -> Subscription {
        cx.subscribe(tabs, |this, _, event: &TabChanged, cx| {
            if let Some(tab) = tabs::from_id(event.tab_id.as_ref()) {
                this.state.vista = tab;
                cx.notify();
            }
        })
    }

    pub(super) fn replace_tabs(&mut self, cx: &mut Context<Self>) {
        let counts = self.state.resultados.as_ref().map(|r| tabs::TabCounts {
            cfdi: r.filas.len(),
            resumen: r.resumen.len(),
            clasificacion: r.clasificaciones.len(),
            errores: r.errores.len(),
        });
        let tabs = tabs::create(cx, counts, self.state.vista);
        self.tab_subscription = Self::subscribe_tabs(cx, &tabs);
        self.tabs = tabs;
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        theme::apply_configured_appearance(
            &self.state.config.apariencia.tema,
            window.appearance(),
            cx,
        );
        let status = self.state.status_text();
        let exported = self.state.ruta_exportada.clone();
        let is_processing = self.state.estado == EstadoApp::Procesando;
        let progress_fraction = if self.state.total == 0 {
            0.0
        } else {
            self.state.procesados as f32 / self.state.total as f32
        };
        let progress_label = format!(
            "Procesando {} de {}",
            format_integer(self.state.procesados),
            format_integer(self.state.total)
        );
        let is_exportable = self.state.resultados.is_some() && !self.state.exportando;

        let drop_view = cx.entity().downgrade();
        let drop_listener = canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                let drop_view = drop_view.clone();
                window.on_mouse_event(move |event: &FileDropEvent, _, _, app| {
                    drop_view
                        .update(app, |this, cx| match event {
                            FileDropEvent::Entered { paths, .. } => {
                                this.pending_drop = paths.paths().first().cloned();
                            }
                            FileDropEvent::Submit { .. } => {
                                if let Some(path) = this.pending_drop.take() {
                                    this.load_zip(path, cx);
                                }
                            }
                            FileDropEvent::Exited => this.pending_drop = None,
                            FileDropEvent::Pending { .. } => {}
                        })
                        .ok();
                });
            },
        )
        .absolute()
        .size_full();

        let root_focus = self.root_focus.clone();
        let state = self.state.clone();
        if let Some((message, success)) = self.pending_toast.take() {
            window.defer(cx, move |window, cx| {
                let toast = cx.toast(message);
                if success {
                    toast.success().show(window, cx);
                } else {
                    toast.destructive().show(window, cx);
                }
            });
        }

        let progress_element = if is_processing {
            progress_bar(progress_fraction)
                .height(px(2.))
                .into_any_element()
        } else {
            div().h(px(2.)).into_any_element()
        };

        v_stack()
            .id("app-root")
            .track_focus(&root_focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.state.settings_open && event.keystroke.key.eq_ignore_ascii_case("escape") {
                    this.close_settings(cx);
                    window.focus(&this.root_focus, cx);
                    return;
                }
                if this.state.settings_open {
                    return;
                }
                let command =
                    event.keystroke.modifiers.control || event.keystroke.modifiers.platform;
                if command {
                    this.on_shortcut(event, window, cx);
                }
            }))
            .on_click(cx.listener(|this, _, window, cx| {
                let rfc_focus = this.rfc_input.read(cx).focus_handle(cx);
                if !rfc_focus.is_focused(window) {
                    window.focus(&this.root_focus, cx);
                }
            }))
            .size_full()
            .font(gpui::Font {
                features: theme::tabular_numbers(),
                ..gpui::font("system-ui")
            })
            .text_color(cx.theme().fg())
            .bg(cx.theme().bg())
            .child(Toolbar::new(
                self.state
                    .ruta_zip
                    .as_deref()
                    .and_then(Path::file_name)
                    .map(|n| n.to_string_lossy().to_string()),
                is_exportable,
                cx.listener(|this, _, _, cx| this.open_zip_dialog(cx)),
                cx.listener(|this, _, _, cx| this.choose_export_path(cx)),
            ))
            .child(self.render_options(cx))
            .child(progress_element)
            .when(is_processing, |element| {
                element.child(div().px_6().py_1().child(small(progress_label).muted()))
            })
            .when(state.resultados.is_some(), |element| {
                element
                    .child(Metrics::new(state.clone()))
                    .child(div().px_6().py_1().child(self.tabs.clone()))
            })
            .child(
                v_stack()
                    .flex_1()
                    .w_full()
                    .min_w_0()
                    .min_h_0()
                    .items_center()
                    .justify_center()
                    .px_6()
                    .py_2()
                    .when(state.resultados.is_some(), |element| {
                        element.items_start().justify_start().px_6().py_2()
                    })
                    .child(self.render_content(cx)),
            )
            .child(StatusBar::new(
                state.ruta_zip.clone(),
                status,
                exported.clone(),
                cx.listener(move |this, _, _, cx| {
                    if let Some(path) = this.state.ruta_exportada.as_deref()
                        && let Some(directory) = path.parent()
                    {
                        cx.reveal_path(directory);
                    }
                }),
            ))
            .when(self.state.settings_open, |element| {
                element.child(self.render_settings_modal(cx))
            })
            .child(drop_listener)
    }
}
