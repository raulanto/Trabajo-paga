use std::{path::PathBuf, sync::Arc};

use crate::application::procesar_zip::ResultadoProceso;

#[derive(Clone, Debug, Default)]
pub(crate) enum AppState {
    #[default]
    Vacio,
    ZipCargado {
        ruta_zip: PathBuf,
    },
    Procesando {
        ruta_zip: PathBuf,
        solicitud: u64,
        procesados: usize,
        total: usize,
    },
    Resultados {
        ruta_zip: PathBuf,
        solicitud: u64,
        resultado: Arc<ResultadoProceso>,
        exportando: bool,
        ruta_exportada: Option<PathBuf>,
        error_exportacion: Option<String>,
    },
    Error {
        ruta_zip: Option<PathBuf>,
        mensaje: String,
    },
}

impl AppState {
    pub(crate) fn cargar_zip(&mut self, ruta_zip: PathBuf) {
        *self = Self::ZipCargado { ruta_zip };
    }

    pub(crate) fn iniciar_proceso(&mut self, solicitud: u64) -> Option<PathBuf> {
        let ruta_zip = match self {
            Self::ZipCargado { ruta_zip } | Self::Resultados { ruta_zip, .. } => ruta_zip.clone(),
            _ => return None,
        };
        *self = Self::Procesando {
            ruta_zip: ruta_zip.clone(),
            solicitud,
            procesados: 0,
            total: 0,
        };
        Some(ruta_zip)
    }

    pub(crate) fn actualizar_progreso(
        &mut self,
        solicitud: u64,
        procesados: usize,
        total: usize,
    ) -> bool {
        let Self::Procesando {
            solicitud: actual,
            procesados: actualizados,
            total: total_actual,
            ..
        } = self
        else {
            return false;
        };
        if *actual != solicitud {
            return false;
        }
        *actualizados = procesados.min(total);
        *total_actual = total;
        true
    }

    pub(crate) fn terminar_proceso(&mut self, solicitud: u64, resultado: ResultadoProceso) -> bool {
        let Self::Procesando {
            ruta_zip,
            solicitud: actual,
            ..
        } = self
        else {
            return false;
        };
        if *actual != solicitud {
            return false;
        }

        let ruta_zip = ruta_zip.clone();
        if resultado.total_xml_leidos == 0 {
            let mensaje = if resultado.errores.is_empty() {
                "El ZIP no contiene archivos XML.".to_owned()
            } else {
                "No se pudo abrir el ZIP. Comprueba que sea un archivo válido.".to_owned()
            };
            *self = Self::Error {
                ruta_zip: Some(ruta_zip),
                mensaje,
            };
        } else {
            *self = Self::Resultados {
                ruta_zip,
                solicitud,
                resultado: Arc::new(resultado),
                exportando: false,
                ruta_exportada: None,
                error_exportacion: None,
            };
        }
        true
    }

    pub(crate) fn fallar(&mut self, mensaje: impl Into<String>) {
        let ruta_zip = self.ruta_zip().cloned();
        self.fallar_con_ruta(ruta_zip, mensaje);
    }

    pub(crate) fn fallar_con_ruta(
        &mut self,
        ruta_zip: Option<PathBuf>,
        mensaje: impl Into<String>,
    ) {
        *self = Self::Error {
            ruta_zip,
            mensaje: mensaje.into(),
        };
    }

    pub(crate) fn iniciar_exportacion(&mut self) -> Option<(u64, Arc<ResultadoProceso>)> {
        let Self::Resultados {
            solicitud,
            resultado,
            exportando,
            error_exportacion,
            ..
        } = self
        else {
            return None;
        };
        if *exportando {
            return None;
        }
        *exportando = true;
        *error_exportacion = None;
        Some((*solicitud, Arc::clone(resultado)))
    }

    pub(crate) fn terminar_exportacion(
        &mut self,
        solicitud: u64,
        resultado: Result<PathBuf, String>,
    ) -> bool {
        let Self::Resultados {
            solicitud: actual,
            exportando,
            ruta_exportada,
            error_exportacion,
            ..
        } = self
        else {
            return false;
        };
        if *actual != solicitud || !*exportando {
            return false;
        }

        *exportando = false;
        match resultado {
            Ok(ruta) => {
                *ruta_exportada = Some(ruta);
                *error_exportacion = None;
            }
            Err(error) => {
                *error_exportacion = Some(error);
            }
        }
        true
    }

    pub(crate) fn ruta_zip(&self) -> Option<&PathBuf> {
        match self {
            Self::ZipCargado { ruta_zip }
            | Self::Procesando { ruta_zip, .. }
            | Self::Resultados { ruta_zip, .. } => Some(ruta_zip),
            Self::Error { ruta_zip, .. } => ruta_zip.as_ref(),
            Self::Vacio => None,
        }
    }

    pub(crate) fn tiene_resultados(&self) -> bool {
        matches!(self, Self::Resultados { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{application::resumen::ResumenProducto, domain::error::ErrorArchivo};

    fn resultado(total_xml_leidos: usize) -> ResultadoProceso {
        ResultadoProceso {
            filas: Vec::new(),
            resumen: Vec::<ResumenProducto>::new(),
            clasificaciones: Vec::new(),
            errores: Vec::<ErrorArchivo>::new(),
            total_xml_leidos,
            total_cfdi_ok: total_xml_leidos,
        }
    }

    #[test]
    fn transiciona_por_el_flujo_y_cargar_otro_zip_limpia_resultados() {
        let ruta = PathBuf::from("cfdi.zip");
        let mut state = AppState::default();
        assert!(matches!(&state, AppState::Vacio));

        state.cargar_zip(ruta.clone());
        assert!(matches!(&state, AppState::ZipCargado { .. }));
        assert_eq!(state.iniciar_proceso(1), Some(ruta.clone()));
        assert!(matches!(&state, AppState::Procesando { .. }));
        assert!(state.actualizar_progreso(1, 2, 1));
        assert!(state.terminar_proceso(1, resultado(1)));
        assert!(state.tiene_resultados());
        assert!(state.iniciar_exportacion().is_some());
        assert!(state.terminar_exportacion(1, Ok(PathBuf::from("salida.xlsx"))));

        state.cargar_zip(PathBuf::from("otro.zip"));
        assert!(matches!(&state, AppState::ZipCargado { .. }));
        assert!(!state.tiene_resultados());
    }

    #[test]
    fn rechaza_mensajes_de_trabajos_anteriores_y_maneja_zip_sin_xml() {
        let mut state = AppState::default();
        state.cargar_zip(PathBuf::from("cfdi.zip"));
        state.iniciar_proceso(7);
        assert!(!state.actualizar_progreso(6, 1, 2));
        assert!(!state.terminar_proceso(6, resultado(1)));
        assert!(state.terminar_proceso(7, resultado(0)));
        assert!(matches!(&state, AppState::Error { .. }));
    }
}
