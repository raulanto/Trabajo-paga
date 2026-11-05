use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

use chrono::Local;
use ely_gpui_component::{
    buttons::Button,
    data_display::{Badge, Tone},
    feedback::{Toast, ToastViewport, Toaster},
    forms::{Choice, ColorPicker, Input, InputEvent, Select, Switch, TextInput},
    motion::ProgressBar,
    navigation::{NavItem, Tabs},
    overlays::Dialog,
    primitives::Severity,
    tables::{DataTable, FilterBuilder, FilterRule},
    theme::{ActiveTheme, Density, Mode, Radius, Theme},
};
use gpui::{
    ClipboardItem, Context, Entity, ExternalPaths, FontWeight, Hsla, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, PathPromptOptions, Render, Rgba, SharedString,
    Styled, Subscription, Window, black, div, prelude::*, rems,
};

use crate::{
    application::{
        config_service::{ConfigRepository, ConfigService},
        ports::ExcelExporter,
        procesar_zip::{ProcesarZip, ResultadoProceso},
    },
    domain::config::{AppConfig, DensidadApp, TemaApp},
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SettingField {
    Rfc,
    HojaCfdi,
    HojaResumen,
    HojaClasificacion,
    HojaErrores,
    ColumnasCfdi,
    DecimalesMonto,
    DecimalesTipoCambio,
    NombreArchivo,
    CategoriaPorDefecto,
    PalabrasHonorarios,
    PalabrasInversion,
    AlturaFila,
    TamañoFuente,
    AtajoAbrirZip,
    AtajoProcesar,
    AtajoExportar,
    AtajoPestana1,
    AtajoPestana2,
    AtajoPestana3,
    AtajoPestana4,
    AtajoConfiguracion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettingsSection {
    General,
    Extraccion,
    Clasificacion,
    Excel,
    Tabla,
    Apariencia,
    Atajos,
    Avanzado,
}

#[derive(Clone, Copy)]
enum BoolSetting {
    RecordarRfc,
    RepetirImpuestos,
    TipoI,
    TipoE,
    TipoP,
    TipoN,
    TipoT,
    Nomina,
    IngresoGastoPorRfc,
    HojaCfdi,
    HojaResumen,
    HojaClasificacion,
    HojaErrores,
    CongelarEncabezado,
    Autofiltro,
    AnchoAutomatico,
    AbrirCarpeta,
}

impl SettingsSection {
    const ALL: [Self; 8] = [
        Self::General,
        Self::Extraccion,
        Self::Clasificacion,
        Self::Excel,
        Self::Tabla,
        Self::Apariencia,
        Self::Atajos,
        Self::Avanzado,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Extraccion => "Extracción",
            Self::Clasificacion => "Clasificación",
            Self::Excel => "Excel",
            Self::Tabla => "Tabla",
            Self::Apariencia => "Apariencia",
            Self::Atajos => "Atajos",
            Self::Avanzado => "Avanzado",
        }
    }

    fn icon(self) -> ely_gpui_component::primitives::IconName {
        use ely_gpui_component::primitives::IconName;
        match self {
            Self::General => IconName::Settings,
            Self::Extraccion => IconName::FileText,
            Self::Clasificacion => IconName::Tag,
            Self::Excel => IconName::FileSpreadsheet,
            Self::Tabla => IconName::Table,
            Self::Apariencia => IconName::Palette,
            Self::Atajos => IconName::Keyboard,
            Self::Avanzado => IconName::Wrench,
        }
    }
}

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
        config: Box<AppConfig>,
    },
}

pub struct AppView {
    state: AppState,
    config: AppConfig,
    system_mode: Mode,
    settings_open: bool,
    settings_section: SettingsSection,
    settings_inputs: HashMap<SettingField, Entity<TextInput>>,
    settings_save_status: String,
    settings_field_error: Option<String>,
    settings_revision: u64,
    restore_confirmation_open: bool,
    last_processed_config: Option<AppConfig>,
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
    _settings_subscriptions: Vec<Subscription>,
    _table_search_subscriptions: [Subscription; 4],
    _appearance_subscription: Subscription,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = cargar_configuracion();
        let system_mode = window.appearance().into();
        Theme::set_mode(modo_tema(&config.apariencia.tema, system_mode), cx);
        let appearance_subscription = cx.observe_window_appearance(window, |view, window, cx| {
            view.system_mode = window.appearance().into();
            if matches!(&view.config.apariencia.tema, TemaApp::Sistema) {
                view.apply_live_appearance(Some(window), cx);
            }
        });

        let rfc = if config.general.recordar_rfc {
            config.general.rfc_propio.clone().unwrap_or_default()
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
        let mut settings_inputs = HashMap::new();
        settings_inputs.insert(SettingField::Rfc, rfc_input.clone());
        for (field, value) in setting_text_values(&config) {
            let input = cx.new(|cx| {
                TextInput::new(window, cx)
                    .label(setting_field_label(field))
                    .placeholder(setting_field_label(field))
            });
            input.update(cx, |input, cx| input.set_text(value, cx));
            settings_inputs.insert(field, input);
        }
        let settings_subscriptions = settings_inputs
            .iter()
            .filter(|(field, _)| **field != SettingField::Rfc)
            .map(|(field, input)| {
                let field = *field;
                cx.subscribe(input, move |view, input, event: &InputEvent, cx| {
                    if *event == InputEvent::Changed {
                        view.update_setting_text(field, input.read(cx).text().to_owned(), cx);
                    }
                })
            })
            .collect();

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

        let rfc_subscription = cx.subscribe(&rfc_input, |view, input, event: &InputEvent, cx| {
            if *event != InputEvent::Changed {
                return;
            }

            let value = input.read(cx).text().to_owned();
            let uppercase = normalizar_rfc(&value);
            view.config.general.rfc_propio = if view.config.general.recordar_rfc {
                (!uppercase.trim().is_empty()).then_some(uppercase.clone())
            } else {
                None
            };
            view.schedule_config_save(cx);
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

        let view = Self {
            state: AppState::default(),
            config,
            system_mode,
            settings_open: false,
            settings_section: SettingsSection::General,
            settings_inputs,
            settings_save_status: "Guardado".to_owned(),
            settings_field_error: None,
            settings_revision: 0,
            restore_confirmation_open: false,
            last_processed_config: None,
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
            _settings_subscriptions: settings_subscriptions,
            _table_search_subscriptions: table_search_subscriptions,
            _appearance_subscription: appearance_subscription,
        };
        view.apply_live_appearance(Some(window), cx);
        view
    }

    fn update_setting_text(&mut self, field: SettingField, value: String, cx: &mut Context<Self>) {
        self.settings_field_error = None;
        let invalid_number = || "Introduce un número válido.".to_owned();
        let result = match field {
            SettingField::Rfc => {
                self.config.general.rfc_propio = (!value.trim().is_empty()).then_some(value);
                Ok(())
            }
            SettingField::HojaCfdi => {
                self.config.excel.nombre_hoja_cfdi = value;
                Ok(())
            }
            SettingField::HojaResumen => {
                self.config.excel.nombre_hoja_resumen = value;
                Ok(())
            }
            SettingField::HojaClasificacion => {
                self.config.excel.nombre_hoja_clasificacion = value;
                Ok(())
            }
            SettingField::HojaErrores => {
                self.config.excel.nombre_hoja_errores = value;
                Ok(())
            }
            SettingField::ColumnasCfdi => {
                self.config.excel.columnas_visibles_cfdi = value
                    .split(',')
                    .map(str::trim)
                    .filter(|column| !column.is_empty())
                    .map(str::to_owned)
                    .collect();
                Ok(())
            }
            SettingField::DecimalesMonto => value
                .parse::<usize>()
                .map(|number| self.config.excel.decimales_monto = number)
                .map_err(|_| invalid_number()),
            SettingField::DecimalesTipoCambio => value
                .parse::<usize>()
                .map(|number| self.config.excel.decimales_tipo_cambio = number)
                .map_err(|_| invalid_number()),
            SettingField::NombreArchivo => {
                self.config.excel.formato_nombre_archivo = value;
                Ok(())
            }
            SettingField::CategoriaPorDefecto => {
                self.config.clasificacion.categoria_por_defecto = value;
                Ok(())
            }
            SettingField::PalabrasHonorarios => {
                set_palabras_clave(
                    &mut self.config.clasificacion.reglas_honorarios,
                    value,
                    "Honorarios",
                );
                Ok(())
            }
            SettingField::PalabrasInversion => {
                set_palabras_clave(
                    &mut self.config.clasificacion.reglas_inversion,
                    value,
                    "Inversión",
                );
                Ok(())
            }
            SettingField::AlturaFila => value
                .parse::<i32>()
                .map(|number| self.config.tabla.altura_fila = number)
                .map_err(|_| invalid_number()),
            SettingField::TamañoFuente => value
                .parse::<f32>()
                .map(|number| self.config.apariencia.tamaño_fuente = number)
                .map_err(|_| invalid_number()),
            SettingField::AtajoAbrirZip => {
                self.config.atajos.abrir_zip = value;
                Ok(())
            }
            SettingField::AtajoProcesar => {
                self.config.atajos.procesar = value;
                Ok(())
            }
            SettingField::AtajoExportar => {
                self.config.atajos.exportar = value;
                Ok(())
            }
            SettingField::AtajoPestana1 => {
                self.config.atajos.pestaña_1 = value;
                Ok(())
            }
            SettingField::AtajoPestana2 => {
                self.config.atajos.pestaña_2 = value;
                Ok(())
            }
            SettingField::AtajoPestana3 => {
                self.config.atajos.pestaña_3 = value;
                Ok(())
            }
            SettingField::AtajoPestana4 => {
                self.config.atajos.pestaña_4 = value;
                Ok(())
            }
            SettingField::AtajoConfiguracion => {
                self.config.atajos.abrir_configuracion = value;
                Ok(())
            }
        };
        if let Err(error) = result {
            self.settings_field_error = Some(error);
            self.settings_save_status = "Hay cambios sin guardar".to_owned();
            cx.notify();
            return;
        }

        self.settings_save_status = if self.config.is_valid() {
            "Guardando…".to_owned()
        } else {
            "Corrige los errores antes de guardar".to_owned()
        };
        self.apply_live_appearance(None, cx);
        self.schedule_config_save(cx);
        cx.notify();
    }

    fn update_setting_bool(
        &mut self,
        field: BoolSetting,
        value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match field {
            BoolSetting::RecordarRfc => {
                self.config.general.recordar_rfc = value;
                if !value {
                    self.config.general.rfc_propio = None;
                } else {
                    let rfc = self.rfc_input.read(cx).text().trim().to_owned();
                    self.config.general.rfc_propio = (!rfc.is_empty()).then_some(rfc);
                }
            }
            BoolSetting::RepetirImpuestos => {
                self.config.extraccion.repetir_impuestos_en_cada_concepto = value
            }
            BoolSetting::TipoI => self.config.extraccion.incluir_tipos.i = value,
            BoolSetting::TipoE => self.config.extraccion.incluir_tipos.e = value,
            BoolSetting::TipoP => self.config.extraccion.incluir_tipos.p = value,
            BoolSetting::TipoN => self.config.extraccion.incluir_tipos.n = value,
            BoolSetting::TipoT => self.config.extraccion.incluir_tipos.t = value,
            BoolSetting::Nomina => self.config.clasificacion.regla_nomina_tipo_n = value,
            BoolSetting::IngresoGastoPorRfc => {
                self.config.clasificacion.regla_ingreso_gasto_por_rfc = value
            }
            BoolSetting::HojaCfdi => self.config.excel.hojas_incluidas.cfdi = value,
            BoolSetting::HojaResumen => self.config.excel.hojas_incluidas.resumen = value,
            BoolSetting::HojaClasificacion => {
                self.config.excel.hojas_incluidas.clasificacion = value
            }
            BoolSetting::HojaErrores => self.config.excel.hojas_incluidas.errores = value,
            BoolSetting::CongelarEncabezado => self.config.excel.congelar_encabezado = value,
            BoolSetting::Autofiltro => self.config.excel.autofiltro = value,
            BoolSetting::AnchoAutomatico => self.config.excel.ancho_automatico = value,
            BoolSetting::AbrirCarpeta => self.config.excel.abrir_carpeta_al_exportar = value,
        }
        self.apply_live_appearance(Some(window), cx);
        self.schedule_config_save(cx);
        cx.notify();
    }

    fn update_theme(&mut self, tema: TemaApp, window: &mut Window, cx: &mut Context<Self>) {
        self.config.apariencia.tema = tema;
        self.apply_live_appearance(Some(window), cx);
        self.schedule_config_save(cx);
        cx.notify();
    }

    fn update_density(&mut self, density: DensidadApp, cx: &mut Context<Self>) {
        self.config.apariencia.densidad = density;
        self.apply_live_appearance(None, cx);
        self.schedule_config_save(cx);
        cx.notify();
    }

    fn apply_live_appearance(&self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        let system_mode = window
            .as_deref()
            .map(|window| window.appearance().into())
            .unwrap_or(self.system_mode);
        Theme::set_mode(modo_tema(&self.config.apariencia.tema, system_mode), cx);
        Theme::update(cx, |theme| {
            theme.density = match self.config.apariencia.densidad {
                DensidadApp::Normal => Density::Standard,
                DensidadApp::Compacta => Density::Compact,
            };
            theme.font_scale = self.config.apariencia.tamaño_fuente / 13.0;
        });
        if let Some(color) = color_config_a_hsla(&self.config.apariencia.color_acento) {
            let mode = cx.theme().mode();
            let mut palette = cx.theme().palette();
            palette.accent = color;
            palette.on_accent = if ely_gpui_component::theme::contrast(black(), color)
                >= ely_gpui_component::theme::contrast(gpui::white(), color)
            {
                black()
            } else {
                gpui::white()
            };
            Theme::set_palette(mode, Some(palette), cx);
        }
    }

    fn schedule_config_save(&mut self, cx: &mut Context<Self>) {
        self.settings_revision = self.settings_revision.wrapping_add(1);
        let revision = self.settings_revision;
        if !self.config.is_valid() || self.settings_field_error.is_some() {
            self.settings_save_status = "Corrige los errores antes de guardar".to_owned();
            return;
        }
        self.settings_save_status = "Guardando…".to_owned();
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let config = match this.update(cx, |view, _| {
                (view.settings_revision == revision
                    && view.config.is_valid()
                    && view.settings_field_error.is_none())
                .then(|| view.config.clone())
            }) {
                Ok(Some(config)) => config,
                Ok(None) => return,
                Err(error) => {
                    eprintln!("No se pudo leer la configuración para guardarla: {error}");
                    return;
                }
            };
            let result = cx
                .background_spawn(async move {
                    ConfigService::new(TomlConfigRepository::new()).guardar(&config)
                })
                .await;
            if let Err(error) = this.update(cx, |view, cx| {
                if view.settings_revision == revision {
                    view.settings_save_status = match result {
                        Ok(()) => "Guardado".to_owned(),
                        Err(error) => {
                            eprintln!("No se pudo guardar la configuración: {error}");
                            "No se pudo guardar".to_owned()
                        }
                    };
                    cx.notify();
                }
            }) {
                eprintln!("No se pudo actualizar el estado de guardado: {error}");
            }
        })
        .detach();
    }

    fn sync_settings_inputs(&mut self, cx: &mut Context<Self>) {
        for (field, value) in setting_text_values(&self.config) {
            if let Some(input) = self.settings_inputs.get(&field) {
                input.update(cx, |input, cx| input.set_text(value, cx));
            }
        }
        let rfc = self.config.general.rfc_propio.clone().unwrap_or_default();
        self.rfc_input
            .update(cx, |input, cx| input.set_text(rfc, cx));
        self.settings_field_error = None;
    }

    fn restore_settings_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let defaults = AppConfig::default();
        match self.settings_section {
            SettingsSection::General => self.config.general = defaults.general,
            SettingsSection::Extraccion => self.config.extraccion = defaults.extraccion,
            SettingsSection::Clasificacion => self.config.clasificacion = defaults.clasificacion,
            SettingsSection::Excel => self.config.excel = defaults.excel,
            SettingsSection::Tabla => self.config.tabla = defaults.tabla,
            SettingsSection::Apariencia => self.config.apariencia = defaults.apariencia,
            SettingsSection::Atajos => self.config.atajos = defaults.atajos,
            SettingsSection::Avanzado => self.config.avanzado = defaults.avanzado,
        }
        self.sync_settings_inputs(cx);
        self.apply_live_appearance(Some(window), cx);
        self.schedule_config_save(cx);
        cx.notify();
    }

    fn restore_all_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.config = AppConfig::default();
        self.settings_section = SettingsSection::General;
        self.settings_save_status = "Guardando…".to_owned();
        self.sync_settings_inputs(cx);
        self.apply_live_appearance(Some(window), cx);
        self.schedule_config_save(cx);
        cx.notify();
    }

    fn importar_configuracion(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Seleccionar archivo de configuración".into()),
        });
        cx.spawn(async move |this, cx| match receiver.await {
            Ok(Ok(Some(paths))) => {
                let Some(path) = paths.into_iter().next() else {
                    return;
                };
                let result = cx
                    .background_spawn(async move {
                        ConfigService::new(TomlConfigRepository::new()).importar(&path)
                    })
                    .await;
                if let Err(error) = &result {
                    eprintln!("No se pudo importar la configuración: {error}");
                }
                if let Err(error) = this.update(cx, |view, cx| match result {
                    Ok(config) => {
                        view.config = config;
                        view.settings_section = SettingsSection::General;
                        view.sync_settings_inputs(cx);
                        view.apply_live_appearance(None, cx);
                        view.schedule_config_save(cx);
                        view.toast("Configuración importada.", Severity::Success, cx);
                        cx.notify();
                    }
                    Err(_) => view.toast(
                        "No se pudo importar la configuración. Comprueba el archivo TOML.",
                        Severity::Danger,
                        cx,
                    ),
                }) {
                    eprintln!("No se pudo actualizar la configuración importada: {error}");
                }
            }
            Ok(Err(_)) => {
                if let Err(error) = this.update(cx, |view, cx| {
                    view.toast(
                        "No se pudo abrir el selector de archivos.",
                        Severity::Danger,
                        cx,
                    );
                }) {
                    eprintln!("No se pudo mostrar el error de importación: {error}");
                }
            }
            Ok(Ok(None)) | Err(_) => {}
        })
        .detach();
    }

    fn exportar_configuracion(&mut self, cx: &mut Context<Self>) {
        let directory = self
            .config
            .general
            .ultima_carpeta_exportacion
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));
        let receiver = cx.prompt_for_new_path(&directory, Some("config.toml"));
        let config = self.config.clone();
        cx.spawn(async move |this, cx| match receiver.await {
            Ok(Ok(Some(path))) => {
                let result = cx
                    .background_spawn(async move {
                        ConfigService::new(TomlConfigRepository::new()).exportar(&path, &config)
                    })
                    .await;
                if let Err(error) = &result {
                    eprintln!("No se pudo exportar la configuración: {error}");
                }
                if let Err(error) = this.update(cx, |view, cx| match result {
                    Ok(()) => view.toast("Configuración exportada.", Severity::Success, cx),
                    Err(_) => view.toast(
                        "No se pudo exportar la configuración.",
                        Severity::Danger,
                        cx,
                    ),
                }) {
                    eprintln!("No se pudo mostrar el resultado de exportación: {error}");
                }
            }
            Ok(Err(_)) => {
                if let Err(error) = this.update(cx, |view, cx| {
                    view.toast(
                        "No se pudo abrir el selector para guardar.",
                        Severity::Danger,
                        cx,
                    );
                }) {
                    eprintln!("No se pudo mostrar el error de exportación: {error}");
                }
            }
            Ok(Ok(None)) | Err(_) => {}
        })
        .detach();
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
        self.config.general.ultima_carpeta_zip = path.parent().map(Path::to_path_buf);
        self.schedule_config_save(cx);
        self.state.cargar_zip(path);
        self.last_processed_config = None;
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
        self.config.general.rfc_propio = if self.config.general.recordar_rfc {
            rfc.clone()
        } else {
            None
        };
        let mut config = self.config.clone();
        config.general.rfc_propio = rfc.clone();
        let process_config = config.clone();
        self.schedule_config_save(cx);
        let (sender, receiver) = async_channel::unbounded();
        cx.notify();

        cx.background_executor()
            .spawn(async move {
                let processor = ProcesarZip::new(ZipXmlReader, XmlCfdiParser);
                let mut last_update = Instant::now();
                let resultado =
                    processor.ejecutar_con_config(path, rfc, &config, |procesados, total| {
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
                    config: Box::new(process_config),
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
                config,
            } => {
                if self.state.terminar_proceso(solicitud, resultado) {
                    self.last_processed_config = Some(*config);
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
        let suggested = nombre_archivo_sugerido(&self.config.excel.formato_nombre_archivo);
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
        let config = self.config.clone();
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
                            &config,
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
                    let exported_path = match &view.state {
                        AppState::Resultados {
                            ruta_exportada: Some(path),
                            ..
                        } => Some(path.clone()),
                        _ => None,
                    };
                    if let Some(path) = exported_path {
                        view.config.general.ultima_carpeta_exportacion =
                            path.parent().map(Path::to_path_buf);
                        view.schedule_config_save(cx);
                        if view.config.excel.abrir_carpeta_al_exportar
                            && let Some(folder) = path.parent()
                        {
                            cx.open_url(&file_url(folder));
                        }
                    }
                    match &view.state {
                        AppState::Resultados {
                            ruta_exportada: Some(path),
                            ..
                        } => {
                            let path = path.clone();
                            let folder_url = path.parent().map(file_url).unwrap_or_default();
                            view.toaster.update(cx, |toaster, cx| {
                                toaster.push(
                                    Toast::new("Exportación lista")
                                        .body(format!("Exportado en {}", path.display()))
                                        .severity(Severity::Success)
                                        .action("Abrir carpeta", move |_, cx| {
                                            if !folder_url.is_empty() {
                                                cx.open_url(&folder_url);
                                            }
                                        }),
                                    cx,
                                );
                            });
                        }
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
                    .child(
                        Button::new("settings", "Configuración…").on_click(cx.listener(
                            |view, _, _, cx| {
                                view.settings_open = true;
                                cx.notify();
                            },
                        )),
                    ),
            )
    }

    fn text_setting(&self, field: SettingField, cx: &mut Context<Self>) -> impl IntoElement {
        let label = setting_field_label(field);
        match self.settings_inputs.get(&field) {
            Some(input) => div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_size(rems(0.875)).child(label))
                .child(Input::new(input))
                .into_any_element(),
            None => div()
                .text_color(cx.theme().colors.danger)
                .child("No se pudo cargar este ajuste.")
                .into_any_element(),
        }
    }

    fn text_settings(&self, fields: &[SettingField], cx: &mut Context<Self>) -> gpui::Div {
        let controls = fields
            .iter()
            .copied()
            .map(|field| self.text_setting(field, cx).into_any_element())
            .collect::<Vec<_>>();
        div().flex().flex_col().gap_3().children(controls)
    }

    fn switch_setting(
        &self,
        label: &'static str,
        id: &'static str,
        setting: BoolSetting,
        checked: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let weak = cx.weak_entity();
        Switch::new(id, checked)
            .label(label)
            .on_change(move |value, window, cx| {
                if let Err(error) = weak.update(cx, |view, cx| {
                    view.update_setting_bool(setting, value, window, cx)
                }) {
                    eprintln!("No se pudo actualizar el ajuste {label}: {error}");
                }
            })
    }

    fn render_settings_section(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let colors = cx.theme().colors.clone();
        match self.settings_section {
            SettingsSection::General => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(self.text_setting(SettingField::Rfc, cx))
                .child(
                    self.switch_setting(
                        "Recordar RFC propio",
                        "setting-remember-rfc",
                        BoolSetting::RecordarRfc,
                        self.config.general.recordar_rfc,
                        cx,
                    ),
                )
                .child(
                    div()
                        .text_size(rems(0.75))
                        .text_color(colors.fg_muted)
                        .child("Las carpetas recientes se actualizan automáticamente."),
                )
                .into_any_element(),
            SettingsSection::Extraccion => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    self.switch_setting(
                        "Repetir impuestos en cada concepto",
                        "setting-repeat-taxes",
                        BoolSetting::RepetirImpuestos,
                        self.config.extraccion.repetir_impuestos_en_cada_concepto,
                        cx,
                    ),
                )
                .child(self.switch_setting(
                    "Ingreso",
                    "setting-type-i",
                    BoolSetting::TipoI,
                    self.config.extraccion.incluir_tipos.i,
                    cx,
                ))
                .child(self.switch_setting(
                    "Egreso",
                    "setting-type-e",
                    BoolSetting::TipoE,
                    self.config.extraccion.incluir_tipos.e,
                    cx,
                ))
                .child(self.switch_setting(
                    "Pago",
                    "setting-type-p",
                    BoolSetting::TipoP,
                    self.config.extraccion.incluir_tipos.p,
                    cx,
                ))
                .child(self.switch_setting(
                    "Nómina",
                    "setting-type-n",
                    BoolSetting::TipoN,
                    self.config.extraccion.incluir_tipos.n,
                    cx,
                ))
                .child(self.switch_setting(
                    "Traslado",
                    "setting-type-t",
                    BoolSetting::TipoT,
                    self.config.extraccion.incluir_tipos.t,
                    cx,
                ))
                .child(
                    div()
                        .text_size(rems(0.75))
                        .text_color(colors.fg_muted)
                        .child("La columna emitida/recibida no se ofrece mientras siga abierta la decisión del README."),
                )
                .into_any_element(),
            SettingsSection::Clasificacion => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.text_setting(SettingField::CategoriaPorDefecto, cx))
                .child(self.text_setting(SettingField::PalabrasHonorarios, cx))
                .child(self.text_setting(SettingField::PalabrasInversion, cx))
                .child(self.switch_setting(
                    "Sugerir Nómina para tipo N",
                    "setting-rule-payroll",
                    BoolSetting::Nomina,
                    self.config.clasificacion.regla_nomina_tipo_n,
                    cx,
                ))
                .child(self.switch_setting(
                    "Sugerir Ingreso/Gasto según RFC propio",
                    "setting-rule-rfc",
                    BoolSetting::IngresoGastoPorRfc,
                    self.config.clasificacion.regla_ingreso_gasto_por_rfc,
                    cx,
                ))
                .child(
                    div()
                        .text_size(rems(0.75))
                        .text_color(colors.fg_muted)
                        .child("La clasificación es una sugerencia por palabras clave, no una interpretación fiscal."),
                )
                .into_any_element(),
            SettingsSection::Excel => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    self.text_settings(&[
                        SettingField::HojaCfdi,
                        SettingField::HojaResumen,
                        SettingField::HojaClasificacion,
                        SettingField::HojaErrores,
                        SettingField::ColumnasCfdi,
                        SettingField::DecimalesMonto,
                        SettingField::DecimalesTipoCambio,
                        SettingField::NombreArchivo,
                    ], cx)
                    .into_any_element(),
                )
                .child(self.switch_setting(
                    "Incluir hoja CFDI",
                    "setting-sheet-cfdi",
                    BoolSetting::HojaCfdi,
                    self.config.excel.hojas_incluidas.cfdi,
                    cx,
                ))
                .child(self.switch_setting(
                    "Incluir hoja Resumen",
                    "setting-sheet-summary",
                    BoolSetting::HojaResumen,
                    self.config.excel.hojas_incluidas.resumen,
                    cx,
                ))
                .child(self.switch_setting(
                    "Incluir hoja Clasificación",
                    "setting-sheet-classification",
                    BoolSetting::HojaClasificacion,
                    self.config.excel.hojas_incluidas.clasificacion,
                    cx,
                ))
                .child(self.switch_setting(
                    "Incluir hoja Errores",
                    "setting-sheet-errors",
                    BoolSetting::HojaErrores,
                    self.config.excel.hojas_incluidas.errores,
                    cx,
                ))
                .child(self.switch_setting(
                    "Congelar encabezado",
                    "setting-freeze-header",
                    BoolSetting::CongelarEncabezado,
                    self.config.excel.congelar_encabezado,
                    cx,
                ))
                .child(self.switch_setting(
                    "Activar autofiltro",
                    "setting-autofilter",
                    BoolSetting::Autofiltro,
                    self.config.excel.autofiltro,
                    cx,
                ))
                .child(self.switch_setting(
                    "Ajustar ancho automáticamente",
                    "setting-autowidth",
                    BoolSetting::AnchoAutomatico,
                    self.config.excel.ancho_automatico,
                    cx,
                ))
                .child(self.switch_setting(
                    "Abrir carpeta al exportar",
                    "setting-open-export-folder",
                    BoolSetting::AbrirCarpeta,
                    self.config.excel.abrir_carpeta_al_exportar,
                    cx,
                ))
                .child(
                    div()
                        .text_size(rems(0.75))
                        .text_color(colors.fg_muted)
                        .child("Columnas CFDI: escribe los nombres separados por comas. Formato de nombre: usa {YYYYMMDD} y {HHmm}."),
                )
                .into_any_element(),
            SettingsSection::Tabla => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.text_setting(SettingField::AlturaFila, cx))
                .child(
                    div()
                        .text_size(rems(0.75))
                        .text_color(colors.fg_muted)
                        .child("Ely aplica alturas de fila discretas según este valor; no expone un ajuste de overscan ni la opción de quitar el resaltado al pasar el puntero."),
                )
                .into_any_element(),
            SettingsSection::Apariencia => {
                let weak_theme = cx.weak_entity();
                let weak_density = cx.weak_entity();
                let weak_color = cx.weak_entity();
                let theme_select = Select::new(
                    "settings-theme",
                    [
                        Choice::new("system", "Sistema"),
                        Choice::new("light", "Claro"),
                        Choice::new("dark", "Oscuro"),
                    ],
                )
                .label("Tema")
                .selected(match self.config.apariencia.tema {
                    TemaApp::Sistema => "system",
                    TemaApp::Claro => "light",
                    TemaApp::Oscuro => "dark",
                })
                .on_change(move |value, window, cx| {
                    let tema = match value.as_ref() {
                        "light" => TemaApp::Claro,
                        "dark" => TemaApp::Oscuro,
                        _ => TemaApp::Sistema,
                    };
                    if let Err(error) =
                        weak_theme.update(cx, |view, cx| view.update_theme(tema, window, cx))
                    {
                        eprintln!("No se pudo cambiar el tema: {error}");
                    }
                });
                let density_select = Select::new(
                    "settings-density",
                    [
                        Choice::new("standard", "Normal"),
                        Choice::new("compact", "Compacta"),
                    ],
                )
                .label("Densidad")
                .selected(match self.config.apariencia.densidad {
                    DensidadApp::Normal => "standard",
                    DensidadApp::Compacta => "compact",
                })
                .on_change(move |value, _, cx| {
                    let density = if value.as_ref() == "compact" {
                        DensidadApp::Compacta
                    } else {
                        DensidadApp::Normal
                    };
                    if let Err(error) =
                        weak_density.update(cx, |view, cx| view.update_density(density, cx))
                    {
                        eprintln!("No se pudo cambiar la densidad: {error}");
                    }
                });
                let color = ColorPicker::new("settings-accent", colors.accent)
                    .opaque()
                    .on_change(move |color, _, cx| {
                        let value = hsla_a_color_config(color);
                        if let Err(error) = weak_color.update(cx, |view, cx| {
                            view.config.apariencia.color_acento = value;
                            view.apply_live_appearance(None, cx);
                            view.schedule_config_save(cx);
                            cx.notify();
                        }) {
                            eprintln!("No se pudo cambiar el color de acento: {error}");
                        }
                    });
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child("Tema")
                            .child(theme_select),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child("Densidad")
                            .child(density_select),
                    )
                    .child(self.text_setting(SettingField::TamañoFuente, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child("Color de acento")
                            .child(color),
                    )
                    .into_any_element()
            }
            SettingsSection::Atajos => self.text_settings(&[
                SettingField::AtajoAbrirZip,
                SettingField::AtajoProcesar,
                SettingField::AtajoExportar,
                SettingField::AtajoPestana1,
                SettingField::AtajoPestana2,
                SettingField::AtajoPestana3,
                SettingField::AtajoPestana4,
                SettingField::AtajoConfiguracion,
            ], cx)
            .into_any_element(),
            SettingsSection::Avanzado => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_size(rems(0.875))
                        .child("El procesamiento se ejecuta en segundo plano y continúa ante XML dañados."),
                )
                .child(
                    div()
                        .text_size(rems(0.75))
                        .text_color(colors.fg_muted)
                        .child("Ely no ofrece configuración de overscan, registro de depuración ni control de hilos en esta aplicación; esos campos no se exponen aquí."),
                )
                .into_any_element(),
        }
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.settings_open {
            return div().into_any_element();
        }
        let weak = cx.weak_entity();
        let current_section = self.settings_section;
        let navigation = div().flex().flex_col().gap_1().w(rems(14.)).children(
            SettingsSection::ALL.into_iter().map(|section| {
                let weak = cx.weak_entity();
                NavItem::new(
                    ("settings-section", section as usize),
                    section.icon(),
                    section.label(),
                )
                .active(section == current_section)
                .on_click(move |_, cx| {
                    if let Err(error) = weak.update(cx, |view, cx| {
                        view.settings_section = section;
                        cx.notify();
                    }) {
                        eprintln!("No se pudo cambiar la sección de configuración: {error}");
                    }
                })
            }),
        );
        let validation = self
            .config
            .validar()
            .into_iter()
            .find(|error| {
                error
                    .campo
                    .starts_with(settings_section_prefix(current_section))
            })
            .map(|error| error.mensaje);
        let has_validation = validation.is_some();
        let status = self
            .settings_field_error
            .clone()
            .or(validation)
            .unwrap_or_else(|| self.settings_save_status.clone());
        let current_rfc = self.rfc_input.read(cx).text().trim().to_owned();
        let current_rfc = (!current_rfc.is_empty()).then_some(current_rfc);
        let processing_changed = self.last_processed_config.as_ref().is_some_and(|previous| {
            previous.general.rfc_propio != current_rfc
                || previous.extraccion != self.config.extraccion
                || previous.clasificacion != self.config.clasificacion
        });
        let colors = cx.theme().colors.clone();
        let body = div()
            .flex()
            .flex_1()
            .min_h_0()
            .gap_5()
            .child(navigation)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(rems(1.125))
                            .child(current_section.label()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(self.render_settings_section(cx)),
                    )
                    .child(
                        div()
                            .text_size(rems(0.75))
                            .text_color(if self.settings_field_error.is_some() || has_validation {
                                colors.danger
                            } else {
                                colors.fg_muted
                            })
                            .child(status),
                    ),
            );
        let body = body.when(processing_changed, |body| {
            body.child(
                div()
                    .text_size(rems(0.75))
                    .text_color(colors.warning)
                    .child("Se aplicará al procesar de nuevo."),
            )
        });
        let close_weak = weak.clone();
        let restore_weak = weak.clone();
        let import_weak = weak.clone();
        let export_weak = weak.clone();
        let reprocess_weak = weak.clone();
        let dialog = Dialog::new("settings-dialog", "Configuración", move |_, cx| {
            if let Err(error) = close_weak.update(cx, |view, cx| {
                view.settings_open = false;
                cx.notify();
            }) {
                eprintln!("No se pudo cerrar Configuración: {error}");
            }
        })
        .fullscreen()
        .child(body)
        .action(move |_| {
            Button::new("restore-section", "Restaurar sección").on_click(move |_, window, cx| {
                if let Err(error) = restore_weak.update(cx, |view, cx| {
                    view.restore_settings_section(window, cx);
                }) {
                    eprintln!("No se pudo restaurar la sección: {error}");
                }
            })
        })
        .action(move |_| {
            Button::new("restore-all", "Restaurar todo").on_click(move |_, _, cx| {
                if let Err(error) = weak.update(cx, |view, cx| {
                    view.restore_confirmation_open = true;
                    cx.notify();
                }) {
                    eprintln!("No se pudo confirmar la restauración: {error}");
                }
            })
        })
        .action(move |_| {
            Button::new("import-settings", "Importar").on_click(move |_, _, cx| {
                if let Err(error) = import_weak.update(cx, |view, cx| {
                    view.importar_configuracion(cx);
                }) {
                    eprintln!("No se pudo iniciar la importación: {error}");
                }
            })
        })
        .action(move |_| {
            Button::new("export-settings", "Exportar").on_click(move |_, _, cx| {
                if let Err(error) = export_weak.update(cx, |view, cx| {
                    view.exportar_configuracion(cx);
                }) {
                    eprintln!("No se pudo iniciar la exportación de configuración: {error}");
                }
            })
        })
        .action(move |_| {
            Button::new("reprocess-settings", "Reprocesar")
                .disabled(!processing_changed)
                .on_click(move |_, _, cx| {
                    if let Err(error) = reprocess_weak.update(cx, |view, cx| {
                        view.settings_open = false;
                        view.procesar_zip(cx);
                        cx.notify();
                    }) {
                        eprintln!("No se pudo reprocesar el ZIP: {error}");
                    }
                })
        })
        .action(|close| {
            Button::new("close-settings", "Cerrar").on_click(move |_, window, cx| {
                close(window, cx);
            })
        });
        let reset_dialog = self
            .restore_confirmation_open
            .then(|| self.render_restore_confirmation(cx));
        div()
            .children([dialog.into_any_element()])
            .children(reset_dialog)
            .into_any_element()
    }

    fn render_restore_confirmation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        let close_weak = weak.clone();
        Dialog::new(
            "restore-settings-dialog",
            "¿Restaurar toda la configuración?",
            move |_, cx| {
                if let Err(error) = close_weak.update(cx, |view, cx| {
                    view.restore_confirmation_open = false;
                    cx.notify();
                }) {
                    eprintln!("No se pudo cerrar la confirmación: {error}");
                }
            },
        )
        .detail("Se reemplazarán las preferencias actuales por los valores iniciales.")
        .action(|close| {
            Button::new("cancel-restore", "Cancelar").on_click(move |_, window, cx| {
                close(window, cx);
            })
        })
        .action(move |close| {
            Button::new("confirm-restore", "Restaurar")
                .primary()
                .on_click(move |_, window, cx| {
                    if let Err(error) =
                        weak.update(cx, |view, cx| view.restore_all_settings(window, cx))
                    {
                        eprintln!("No se pudo restaurar la configuración: {error}");
                    }
                    close(window, cx);
                })
        })
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
                .density(densidad_tabla(self.config.tabla.altura_fila))
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
        let scroll_guidance = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_6()
            .px_3()
            .py_1()
            .text_size(rems(0.75))
            .text_color(theme.colors.fg_muted)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child("Columnas")
                    .child("←")
                    .child(
                        div()
                            .w(rems(3.))
                            .h(rems(0.5))
                            .border_t_1()
                            .border_color(theme.colors.border),
                    )
                    .child("→")
                    .child("Shift+rueda"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child("Filas")
                    .child("↑")
                    .child(
                        div()
                            .w(rems(0.5))
                            .h(rems(1.))
                            .border_l_1()
                            .border_color(theme.colors.border),
                    )
                    .child("↓")
                    .child("Rueda"),
            );
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
                    .text_ellipsis(                    )
                    .child(
                        "Ordenar: clic en encabezado · Reordenar: arrastra encabezado · Ancho: arrastra borde",
                    )
                    .child("Copiar TSV: filas seleccionadas o pestaña completa"),
            );
        let panel = div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(search_toolbar)
            .children(filter_panel)
            .child(div().flex_1().min_h_0().child(table_body))
            .child(scroll_guidance)
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
            .child(div().flex().flex_1().min_h_0().child(tabs))
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
        if atajo_presionado(&self.config.atajos.abrir_zip, event) {
            self.open_zip_dialog(cx);
        } else if atajo_presionado(&self.config.atajos.exportar, event)
            && self.state.tiene_resultados()
        {
            self.elegir_destino_excel(cx);
        } else if atajo_presionado(&self.config.atajos.procesar, event)
            && matches!(
                &self.state,
                AppState::ZipCargado { .. } | AppState::Resultados { .. }
            )
        {
            self.procesar_zip(cx);
        } else if atajo_presionado(&self.config.atajos.abrir_configuracion, event) {
            self.settings_open = true;
            cx.notify();
        } else {
            for (shortcut, tab) in [
                (&self.config.atajos.pestaña_1, 0),
                (&self.config.atajos.pestaña_2, 1),
                (&self.config.atajos.pestaña_3, 2),
                (&self.config.atajos.pestaña_4, 3),
            ] {
                if atajo_presionado(shortcut, event) && self.state.tiene_resultados() {
                    self.active_tab = tab;
                    cx.notify();
                    break;
                }
            }
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
            .child(self.render_settings(cx))
    }
}

fn setting_text_values(config: &AppConfig) -> Vec<(SettingField, String)> {
    let words = |rules: &[crate::domain::config::ReglaClasificacion]| {
        rules
            .first()
            .map(|rule| rule.palabras_clave.join(", "))
            .unwrap_or_default()
    };
    vec![
        (
            SettingField::HojaCfdi,
            config.excel.nombre_hoja_cfdi.clone(),
        ),
        (
            SettingField::HojaResumen,
            config.excel.nombre_hoja_resumen.clone(),
        ),
        (
            SettingField::HojaClasificacion,
            config.excel.nombre_hoja_clasificacion.clone(),
        ),
        (
            SettingField::HojaErrores,
            config.excel.nombre_hoja_errores.clone(),
        ),
        (
            SettingField::ColumnasCfdi,
            config.excel.columnas_visibles_cfdi.join(", "),
        ),
        (
            SettingField::DecimalesMonto,
            config.excel.decimales_monto.to_string(),
        ),
        (
            SettingField::DecimalesTipoCambio,
            config.excel.decimales_tipo_cambio.to_string(),
        ),
        (
            SettingField::NombreArchivo,
            config.excel.formato_nombre_archivo.clone(),
        ),
        (
            SettingField::CategoriaPorDefecto,
            config.clasificacion.categoria_por_defecto.clone(),
        ),
        (
            SettingField::PalabrasHonorarios,
            words(&config.clasificacion.reglas_honorarios),
        ),
        (
            SettingField::PalabrasInversion,
            words(&config.clasificacion.reglas_inversion),
        ),
        (
            SettingField::AlturaFila,
            config.tabla.altura_fila.to_string(),
        ),
        (
            SettingField::TamañoFuente,
            config.apariencia.tamaño_fuente.to_string(),
        ),
        (SettingField::AtajoAbrirZip, config.atajos.abrir_zip.clone()),
        (SettingField::AtajoProcesar, config.atajos.procesar.clone()),
        (SettingField::AtajoExportar, config.atajos.exportar.clone()),
        (SettingField::AtajoPestana1, config.atajos.pestaña_1.clone()),
        (SettingField::AtajoPestana2, config.atajos.pestaña_2.clone()),
        (SettingField::AtajoPestana3, config.atajos.pestaña_3.clone()),
        (SettingField::AtajoPestana4, config.atajos.pestaña_4.clone()),
        (
            SettingField::AtajoConfiguracion,
            config.atajos.abrir_configuracion.clone(),
        ),
    ]
}

fn setting_field_label(field: SettingField) -> &'static str {
    match field {
        SettingField::Rfc => "RFC propio",
        SettingField::HojaCfdi => "Nombre de hoja CFDI",
        SettingField::HojaResumen => "Nombre de hoja Resumen",
        SettingField::HojaClasificacion => "Nombre de hoja Clasificación",
        SettingField::HojaErrores => "Nombre de hoja Errores",
        SettingField::ColumnasCfdi => "Columnas CFDI visibles",
        SettingField::DecimalesMonto => "Decimales de montos (0–6)",
        SettingField::DecimalesTipoCambio => "Decimales de tipo de cambio (0–8)",
        SettingField::NombreArchivo => "Formato del nombre de archivo",
        SettingField::CategoriaPorDefecto => "Categoría por defecto",
        SettingField::PalabrasHonorarios => "Palabras para Honorarios",
        SettingField::PalabrasInversion => "Palabras para Inversión",
        SettingField::AlturaFila => "Altura de fila (28–44 px)",
        SettingField::TamañoFuente => "Tamaño de fuente (px)",
        SettingField::AtajoAbrirZip => "Abrir ZIP",
        SettingField::AtajoProcesar => "Procesar ZIP",
        SettingField::AtajoExportar => "Exportar Excel",
        SettingField::AtajoPestana1 => "Pestaña CFDI",
        SettingField::AtajoPestana2 => "Pestaña Resumen",
        SettingField::AtajoPestana3 => "Pestaña Clasificación",
        SettingField::AtajoPestana4 => "Pestaña Errores",
        SettingField::AtajoConfiguracion => "Configuración",
    }
}

fn settings_section_prefix(section: SettingsSection) -> &'static str {
    match section {
        SettingsSection::General => "general.",
        SettingsSection::Extraccion => "extraccion.",
        SettingsSection::Clasificacion => "clasificacion.",
        SettingsSection::Excel => "excel.",
        SettingsSection::Tabla => "tabla.",
        SettingsSection::Apariencia => "apariencia.",
        SettingsSection::Atajos => "atajos",
        SettingsSection::Avanzado => "avanzado.",
    }
}

fn set_palabras_clave(
    rules: &mut Vec<crate::domain::config::ReglaClasificacion>,
    value: String,
    category: &str,
) {
    let palabras = value
        .split(',')
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect();
    if let Some(rule) = rules.first_mut() {
        rule.palabras_clave = palabras;
    } else {
        rules.push(crate::domain::config::ReglaClasificacion {
            categoria: category.to_owned(),
            palabras_clave: palabras,
            activa: true,
        });
    }
}

fn densidad_tabla(altura: i32) -> Density {
    match altura {
        0..=29 => Density::Compact,
        38..=i32::MAX => Density::Comfortable,
        _ => Density::Standard,
    }
}

fn nombre_archivo_sugerido(formato: &str) -> String {
    let now = Local::now();
    let name = formato
        .replace("{YYYYMMDD}", &now.format("%Y%m%d").to_string())
        .replace("{HHmm}", &now.format("%H%M").to_string());
    let mut name = PathBuf::from(name);
    if name.file_name().is_none() {
        name = PathBuf::from(format!("cfdi_{}.xlsx", now.format("%Y%m%d_%H%M")));
    }
    if !name
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("xlsx"))
    {
        name.set_extension("xlsx");
    }
    name.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("cfdi_{}.xlsx", now.format("%Y%m%d_%H%M")))
}

fn atajo_presionado(shortcut: &str, event: &KeyDownEvent) -> bool {
    let mut pieces = shortcut.split('+').collect::<Vec<_>>();
    let Some(key) = pieces.pop() else {
        return false;
    };
    if pieces.is_empty()
        || !pieces.iter().any(|modifier| {
            modifier.eq_ignore_ascii_case("ctrl") || modifier.eq_ignore_ascii_case("cmd")
        })
    {
        return false;
    }
    let modifiers = event.keystroke.modifiers;
    if !(modifiers.control || modifiers.platform) {
        return false;
    }
    event.keystroke.key.eq_ignore_ascii_case(key)
        || (key == "," && event.keystroke.key.eq_ignore_ascii_case("comma"))
}

fn color_config_a_hsla(value: &str) -> Option<Hsla> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16)
        .ok()
        .map(|value| gpui::rgb(value).into())
}

fn hsla_a_color_config(color: Hsla) -> String {
    let rgb = Rgba::from(color);
    let channel = |value: f32| (value * 255.0).round().clamp(0.0, 255.0) as u8;
    format!(
        "#{:02X}{:02X}{:02X}",
        channel(rgb.r),
        channel(rgb.g),
        channel(rgb.b)
    )
}

fn file_url(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let encoded = absolute
        .to_string_lossy()
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric()
                || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~' | b':')
            {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect::<String>();
    format!("file://{encoded}")
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

fn modo_tema(tema: &TemaApp, system_mode: Mode) -> Mode {
    match tema {
        TemaApp::Sistema => system_mode,
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
