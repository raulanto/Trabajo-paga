//! Estado de la aplicación y transiciones independientes de GPUI.

use std::{path::PathBuf, sync::Arc};

use crate::{
    application::{procesar_zip::ResultadoProceso, resumen::ResumenProducto},
    domain::{
        cfdi::CfdiRow, clasificacion::FilaClasificada, config::AppConfig, error::ErrorArchivo,
    },
};

use super::cells::TablasPreparadas;
use super::util::format_integer;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Vista {
    #[default]
    Cfdi,
    Resumen,
    Clasificacion,
    Errores,
}

impl Vista {
    pub fn index(self) -> usize {
        match self {
            Self::Cfdi => 0,
            Self::Resumen => 1,
            Self::Clasificacion => 2,
            Self::Errores => 3,
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Cfdi => "cfdi",
            Self::Resumen => "resumen",
            Self::Clasificacion => "clasificacion",
            Self::Errores => "errores",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EstadoApp {
    #[default]
    Vacio,
    ZipCargado,
    Procesando,
    Resultados,
    Error,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SettingsSection {
    #[default]
    General,
    Extraccion,
    Clasificacion,
    Excel,
    Tabla,
    Apariencia,
    Atajos,
    Avanzado,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ConfigSaveStatus {
    #[default]
    Saved,
    Saving,
    Invalid,
    Failed,
}

#[derive(Clone, Debug)]
pub struct ResultadosVista {
    pub filas: Arc<Vec<CfdiRow>>,
    pub resumen: Arc<Vec<ResumenProducto>>,
    pub clasificaciones: Arc<Vec<FilaClasificada>>,
    pub errores: Arc<Vec<ErrorArchivo>>,
    pub tablas: TablasPreparadas,
    pub total_xml_leidos: usize,
    pub total_cfdi_ok: usize,
}

impl ResultadosVista {
    pub(super) fn new(resultado: ResultadoProceso, tablas: TablasPreparadas) -> Self {
        Self {
            filas: Arc::new(resultado.filas),
            resumen: Arc::new(resultado.resumen),
            clasificaciones: Arc::new(resultado.clasificaciones),
            errores: Arc::new(resultado.errores),
            tablas,
            total_xml_leidos: resultado.total_xml_leidos,
            total_cfdi_ok: resultado.total_cfdi_ok,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub estado: EstadoApp,
    pub ruta_zip: Option<PathBuf>,
    pub rfc_propio: String,
    pub config: AppConfig,
    pub vista: Vista,
    pub resultados: Option<Arc<ResultadosVista>>,
    pub procesados: usize,
    pub total: usize,
    pub mensaje_error: Option<String>,
    pub ruta_exportada: Option<PathBuf>,
    pub exportando: bool,
    pub settings_open: bool,
    pub settings_section: SettingsSection,
}

impl Default for AppState {
    fn default() -> Self {
        let config = AppConfig::default();
        Self {
            estado: EstadoApp::Vacio,
            ruta_zip: None,
            rfc_propio: config.general.rfc_propio.clone().unwrap_or_default(),
            config,
            vista: Vista::Cfdi,
            resultados: None,
            procesados: 0,
            total: 0,
            mensaje_error: None,
            ruta_exportada: None,
            exportando: false,
            settings_open: false,
            settings_section: SettingsSection::General,
        }
    }
}

impl AppState {
    pub fn set_zip(&mut self, path: PathBuf) {
        self.ruta_zip = Some(path);
        self.estado = EstadoApp::ZipCargado;
        self.resultados = None;
        self.procesados = 0;
        self.total = 0;
        self.mensaje_error = None;
        self.ruta_exportada = None;
        self.exportando = false;
        self.vista = Vista::Cfdi;
    }

    pub fn start_processing(&mut self) -> bool {
        if self.ruta_zip.is_none()
            || !matches!(self.estado, EstadoApp::ZipCargado | EstadoApp::Resultados)
        {
            return false;
        }
        self.estado = EstadoApp::Procesando;
        self.resultados = None;
        self.procesados = 0;
        self.total = 0;
        self.mensaje_error = None;
        self.ruta_exportada = None;
        true
    }

    pub fn set_progress(&mut self, processed: usize, total: usize) {
        self.procesados = processed.min(total);
        self.total = total;
    }

    pub fn sync_rfc_from_config(&mut self) {
        self.rfc_propio = self.config.general.rfc_propio.clone().unwrap_or_default();
    }

    pub fn finish_processing(&mut self, resultado: ResultadoProceso, tablas: TablasPreparadas) {
        if resultado.total_xml_leidos == 0 {
            let mensaje = if resultado.errores.is_empty() {
                "El ZIP no contiene archivos .xml.".to_owned()
            } else {
                "No se pudo abrir el ZIP. Comprueba que sea un archivo ZIP válido.".to_owned()
            };
            self.set_error(mensaje);
            return;
        }
        self.resultados = Some(Arc::new(ResultadosVista::new(resultado, tablas)));
        self.estado = EstadoApp::Resultados;
        self.procesados = self.total;
        self.mensaje_error = None;
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.estado = EstadoApp::Error;
        self.resultados = None;
        self.mensaje_error = Some(message.into());
        self.exportando = false;
    }

    pub fn metric_text(&self) -> String {
        let Some(resultados) = &self.resultados else {
            return String::new();
        };
        let conceptos = resultados.filas.len();
        format!(
            "{} XML · {} CFDI · {} errores · {} conceptos",
            format_integer(resultados.total_xml_leidos),
            format_integer(resultados.total_cfdi_ok),
            format_integer(resultados.errores.len()),
            format_integer(conceptos)
        )
    }

    pub(super) fn status_text(&self) -> String {
        if self.exportando {
            return "Exportando Excel…".to_owned();
        }
        if let Some(message) = &self.mensaje_error {
            return message.clone();
        }
        if self.estado == EstadoApp::Procesando {
            return format!(
                "Procesando {} de {}",
                format_integer(self.procesados),
                format_integer(self.total)
            );
        }
        if let Some(path) = &self.ruta_exportada {
            return format!("Exportado en {}", path.display());
        }
        "Listo".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_state_recoge_transiciones_de_carga_a_resultados_y_error() {
        let mut state = AppState::default();
        assert_eq!(state.estado, EstadoApp::Vacio);
        state.set_zip(PathBuf::from("archivo.zip"));
        assert_eq!(state.estado, EstadoApp::ZipCargado);
        assert!(state.start_processing());
        assert_eq!(state.estado, EstadoApp::Procesando);
        let resultado = ResultadoProceso {
            filas: Vec::new(),
            resumen: Vec::new(),
            clasificaciones: Vec::new(),
            errores: Vec::new(),
            total_xml_leidos: 1,
            total_cfdi_ok: 1,
        };
        let tablas = TablasPreparadas::desde(&resultado);
        state.finish_processing(resultado, tablas);
        assert_eq!(state.estado, EstadoApp::Resultados);
        assert_eq!(
            state.metric_text(),
            "1 XML · 1 CFDI · 0 errores · 0 conceptos"
        );
        assert!(state.start_processing());
        state.set_error("falló");
        assert_eq!(state.estado, EstadoApp::Error);
        state.set_zip(PathBuf::from("otro.zip"));
        assert_eq!(state.estado, EstadoApp::ZipCargado);
        assert!(state.resultados.is_none());
    }
}
