//! Apertura del modal y persistencia de la configuración.

use std::time::Duration;

use gpui::{Context, px};

use crate::{
    application::config_service::ConfigService,
    domain::config::{
        AparienciaConfig, AppConfig, AtajosConfig, AvanzadoConfig, ClasificacionConfig,
        ExcelConfig, ExtraccionConfig, GeneralConfig, TablaConfig,
    },
};

use super::super::{AppView, SettingsSection, state::ConfigSaveStatus};

impl AppView {
    pub(in crate::ui::app_view) fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.state.settings_open = true;
        self.state.settings_section = SettingsSection::General;
        self.settings_scroll.set_offset(gpui::point(px(0.), px(0.)));
        cx.notify();
    }

    pub(in crate::ui::app_view) fn select_settings_section(
        &mut self,
        section: SettingsSection,
        cx: &mut Context<Self>,
    ) {
        self.state.settings_section = section;
        self.settings_scroll.set_offset(gpui::point(px(0.), px(0.)));
        cx.notify();
    }

    pub(in crate::ui::app_view) fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.state.settings_open = false;
        self.confirm_restore_all = false;
        cx.notify();
    }

    pub(in crate::ui::app_view) fn restore_section_defaults(&mut self, cx: &mut Context<Self>) {
        match self.state.settings_section {
            SettingsSection::General => self.state.config.general = GeneralConfig::default(),
            SettingsSection::Extraccion => {
                self.state.config.extraccion = ExtraccionConfig::default()
            }
            SettingsSection::Clasificacion => {
                self.state.config.clasificacion = ClasificacionConfig::default()
            }
            SettingsSection::Excel => self.state.config.excel = ExcelConfig::default(),
            SettingsSection::Tabla => self.state.config.tabla = TablaConfig::default(),
            SettingsSection::Apariencia => {
                self.state.config.apariencia = AparienciaConfig::default()
            }
            SettingsSection::Atajos => self.state.config.atajos = AtajosConfig::default(),
            SettingsSection::Avanzado => self.state.config.avanzado = AvanzadoConfig::default(),
        }
        self.state.sync_rfc_from_config();
        self.sync_config_inputs(cx);
        self.pending_toast = Some(("Sección restaurada".to_owned(), true));
        self.config_changed(cx);
    }

    pub(in crate::ui::app_view) fn restore_all_defaults(&mut self, cx: &mut Context<Self>) {
        self.state.config = AppConfig::default();
        self.state.sync_rfc_from_config();
        self.sync_config_inputs(cx);
        self.confirm_restore_all = false;
        self.pending_toast = Some(("Configuración restaurada".to_owned(), true));
        self.config_changed(cx);
    }

    fn sync_config_inputs(&mut self, cx: &mut Context<Self>) {
        let rfc = self.state.rfc_propio.clone();
        self.rfc_input
            .update(cx, |input, cx| input.set_content_silent(rfc, cx));

        let filename_format = self.state.config.excel.formato_nombre_archivo.clone();
        self.excel_format_input.update(cx, |input, cx| {
            input.set_content_silent(filename_format, cx)
        });
    }

    pub(in crate::ui::app_view) fn save_config(&mut self, cx: &mut Context<Self>) {
        if let Some(error) = self.state.config.validar().first() {
            self.config_save_status = ConfigSaveStatus::Invalid;
            self.pending_toast = Some((error.mensaje.clone(), false));
            cx.notify();
            return;
        }
        let config = self.state.config.clone();
        let result = ConfigService::new(self.config_repository.clone()).guardar(&config);
        self.last_config_snapshot = config;
        self.config_save_generation = self.config_save_generation.wrapping_add(1);
        if result.is_ok() {
            self.config_save_status = ConfigSaveStatus::Saved;
            self.pending_toast = Some(("Configuración guardada".to_owned(), true));
        } else {
            self.config_save_status = ConfigSaveStatus::Failed;
            self.pending_toast = Some(("No se pudo guardar la configuración".to_owned(), false));
        }
        cx.notify();
    }

    pub(in crate::ui::app_view) fn schedule_config_save(&mut self, cx: &mut Context<Self>) {
        if self.state.config == self.last_config_snapshot {
            return;
        }

        self.last_config_snapshot = self.state.config.clone();
        self.config_save_generation = self.config_save_generation.wrapping_add(1);
        let generation = self.config_save_generation;
        if !self.state.config.validar().is_empty() {
            self.config_save_status = ConfigSaveStatus::Invalid;
            return;
        }

        self.config_save_status = ConfigSaveStatus::Saving;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;

            let pending = this
                .update(cx, |this, _| {
                    if this.config_save_generation != generation {
                        return None;
                    }
                    if !this.state.config.validar().is_empty() {
                        this.config_save_status = ConfigSaveStatus::Invalid;
                        return None;
                    }
                    Some((this.config_repository.clone(), this.state.config.clone()))
                })
                .ok()
                .flatten();

            let Some((repository, config)) = pending else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move { ConfigService::new(repository).guardar(&config) })
                .await;

            if let Err(error) = &result {
                eprintln!("No se pudo guardar la configuración automáticamente: {error}");
            }
            this.update(cx, |this, cx| {
                if this.config_save_generation == generation {
                    this.config_save_status = if result.is_ok() {
                        ConfigSaveStatus::Saved
                    } else {
                        ConfigSaveStatus::Failed
                    };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(in crate::ui::app_view) fn config_changed(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }
}
