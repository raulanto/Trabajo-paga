//! Conversión de datos del dominio a filas ya formateadas para las tablas.

use std::sync::Arc;

use rust_decimal::Decimal;

use crate::{
    application::{procesar_zip::ResultadoProceso, resumen::ResumenProducto},
    domain::{cfdi::CfdiRow, clasificacion::FilaClasificada, error::ErrorArchivo},
};

use crate::ui::data_table::{
    CFDI_COLUMNS, CLASIFICACION_COLUMNS, ERROR_COLUMNS, Fila, RESUMEN_COLUMNS, construir_filas,
};

use super::util::{format_decimal, format_integer};

#[derive(Clone, Debug)]
pub struct TablasPreparadas {
    pub cfdi: Arc<Vec<Fila>>,
    pub resumen: Arc<Vec<Fila>>,
    pub clasificaciones: Arc<Vec<Fila>>,
    pub errores: Arc<Vec<Fila>>,
}

impl TablasPreparadas {
    pub fn desde(resultado: &ResultadoProceso) -> Self {
        Self {
            cfdi: construir_filas(resultado.filas.as_slice(), CFDI_COLUMNS.len(), cfdi_cell),
            resumen: construir_filas(
                resultado.resumen.as_slice(),
                RESUMEN_COLUMNS.len(),
                resumen_cell,
            ),
            clasificaciones: construir_filas(
                resultado.clasificaciones.as_slice(),
                CLASIFICACION_COLUMNS.len(),
                clasificacion_cell,
            ),
            errores: construir_filas(
                resultado.errores.as_slice(),
                ERROR_COLUMNS.len(),
                error_cell,
            ),
        }
    }
}

fn cfdi_cell(row: &CfdiRow, column: usize) -> String {
    let field: Option<String> = match column {
        0 => row.uuid.clone(),
        1 => row.fecha.clone(),
        2 => row.rfc_emisor.clone(),
        3 => row.nombre_emisor.clone(),
        4 => row.rfc_receptor.clone(),
        5 => row.tipo_comprobante.clone(),
        6 => decimal_cell(row.subtotal, 2),
        7 => decimal_cell(row.descuento, 2),
        8 => decimal_cell(row.iva_trasladado_002, 2),
        9 => decimal_cell(row.isr_retenido_001, 2),
        10 => decimal_cell(row.iva_retenido_002, 2),
        11 => decimal_cell(row.total, 2),
        12 => row.uso_cfdi.clone(),
        13 => row.metodo_pago.clone(),
        14 => row.forma_pago.clone(),
        15 => row.moneda.clone(),
        16 => decimal_cell(row.tipo_cambio, 6),
        17 => row.clave_prod_serv.clone(),
        18 => row.descripcion.clone(),
        19 => decimal_cell(row.importe_concepto, 2),
        20 => Some(row.archivo_origen.clone()),
        _ => None,
    };
    field.unwrap_or_default()
}

fn resumen_cell(row: &ResumenProducto, column: usize) -> String {
    match column {
        0 => row.clave_prod_serv.clone().unwrap_or_default(),
        1 => row.descripcion.clone().unwrap_or_default(),
        2 => format_decimal(row.importe_total, 2),
        3 => format_integer(row.num_conceptos),
        _ => String::new(),
    }
}

fn clasificacion_cell(row: &FilaClasificada, column: usize) -> String {
    match column {
        0 => row.fila.archivo_origen.clone(),
        1 => row.fila.uuid.clone().unwrap_or_default(),
        2 => row.fila.tipo_comprobante.clone().unwrap_or_default(),
        3 => row.fila.clave_prod_serv.clone().unwrap_or_default(),
        4 => row.fila.descripcion.clone().unwrap_or_default(),
        5 => row.clasificacion.categoria.nombre().to_owned(),
        6 => row.clasificacion.criterio.clone(),
        _ => String::new(),
    }
}

fn error_cell(row: &ErrorArchivo, column: usize) -> String {
    match column {
        0 => row.archivo.clone(),
        1 => row.motivo.clone(),
        _ => String::new(),
    }
}

fn decimal_cell(value: Option<Decimal>, precision: usize) -> Option<String> {
    value.map(|value| format_decimal(value, precision))
}
