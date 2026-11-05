//! Contratos entre los casos de uso y sus adaptadores de infraestructura.

use std::path::Path;

use crate::domain::{
    cfdi::CfdiRow,
    clasificacion::FilaClasificada,
    config::AppConfig,
    error::{ErrorArchivo, ErrorCfdi},
};

use super::resumen::ResumenProducto;

pub type ResultadoEntradaXml = Result<(String, Vec<u8>), ErrorArchivo>;

pub trait CfdiParser {
    fn parsear(&self, nombre: &str, xml: &[u8]) -> Result<Vec<CfdiRow>, ErrorCfdi>;
}

pub trait ZipReader {
    fn leer_xmls(&self, ruta_zip: &Path) -> Result<Vec<ResultadoEntradaXml>, ErrorArchivo>;
}

pub trait ExcelExporter {
    type Error: std::error::Error + Send + Sync + 'static;

    fn exportar(
        &self,
        filas: &[CfdiRow],
        resumen: &[ResumenProducto],
        clasificaciones: &[FilaClasificada],
        errores: &[ErrorArchivo],
        config: &AppConfig,
        destino: &Path,
    ) -> Result<(), Self::Error>;
}
