//! Adaptador de rust_xlsxwriter para exportar las cuatro hojas del contrato.

use std::path::Path;

use rust_xlsxwriter::{Format, Workbook, Worksheet, XlsxError};

use crate::{
    application::{ports::ExcelExporter, resumen::ResumenProducto},
    domain::{
        cfdi::CfdiRow,
        clasificacion::FilaClasificada,
        config::{AppConfig, ExcelConfig},
        error::ErrorArchivo,
    },
};

const CFDI_HEADERS: [&str; 21] = [
    "UUID",
    "Fecha",
    "RFC Emisor",
    "Nombre Emisor",
    "RFC Receptor",
    "Tipo de Comprobante",
    "Subtotal",
    "Descuento",
    "IVA trasladado (002)",
    "ISR retenido (001)",
    "IVA retenido (002)",
    "Total",
    "Uso CFDI",
    "Método de Pago",
    "Forma de Pago",
    "Moneda",
    "Tipo de Cambio",
    "ClaveProdServ",
    "Descripción",
    "Importe",
    "Archivo",
];

const CFDI_WIDTHS: [f64; 21] = [
    38.0, 22.0, 18.0, 30.0, 18.0, 20.0, 15.0, 15.0, 19.0, 18.0, 18.0, 15.0, 14.0, 16.0, 14.0, 12.0,
    16.0, 16.0, 35.0, 15.0, 35.0,
];

const RESUMEN_HEADERS: [&str; 4] = [
    "ClaveProdServ",
    "Descripción",
    "Importe total",
    "Número de conceptos",
];

const CLASIFICACION_HEADERS: [&str; 7] = [
    "Archivo",
    "UUID",
    "Tipo de Comprobante",
    "ClaveProdServ",
    "Descripción",
    "Categoría sugerida",
    "Criterio",
];

const ERRORES_HEADERS: [&str; 2] = ["Archivo", "Motivo"];

pub struct XlsxExporter;

impl ExcelExporter for XlsxExporter {
    type Error = XlsxError;

    fn exportar(
        &self,
        filas: &[CfdiRow],
        resumen: &[ResumenProducto],
        clasificaciones: &[FilaClasificada],
        errores: &[ErrorArchivo],
        config: &AppConfig,
        destino: &Path,
    ) -> Result<(), Self::Error> {
        if let Some(error) = config
            .validar()
            .into_iter()
            .find(|error| error.campo.starts_with("excel."))
        {
            return Err(XlsxError::ParameterError(error.mensaje));
        }
        let included = &config.excel.hojas_incluidas;
        if !(included.cfdi || included.resumen || included.clasificacion || included.errores) {
            return Err(XlsxError::ParameterError(
                "debe incluirse al menos una hoja".into(),
            ));
        }

        let encabezado = Format::new().set_bold();
        let formato_dinero =
            Format::new().set_num_format(formato_decimal(config.excel.decimales_monto));
        let formato_tipo_cambio =
            Format::new().set_num_format(formato_decimal(config.excel.decimales_tipo_cambio));
        let mut workbook = Workbook::new();

        if included.cfdi {
            escribir_cfdi(
                workbook
                    .add_worksheet()
                    .set_name(&config.excel.nombre_hoja_cfdi)?,
                filas,
                &encabezado,
                &formato_dinero,
                &formato_tipo_cambio,
                &config.excel,
            )?;
        }
        if included.resumen {
            escribir_resumen(
                workbook
                    .add_worksheet()
                    .set_name(&config.excel.nombre_hoja_resumen)?,
                resumen,
                &encabezado,
                &formato_dinero,
                &config.excel,
            )?;
        }
        if included.clasificacion {
            escribir_clasificaciones(
                workbook
                    .add_worksheet()
                    .set_name(&config.excel.nombre_hoja_clasificacion)?,
                clasificaciones,
                &encabezado,
                &config.excel,
            )?;
        }
        if included.errores {
            escribir_errores(
                workbook
                    .add_worksheet()
                    .set_name(&config.excel.nombre_hoja_errores)?,
                errores,
                &encabezado,
                &config.excel,
            )?;
        }

        workbook.save(destino)
    }
}

fn escribir_cfdi(
    worksheet: &mut Worksheet,
    filas: &[CfdiRow],
    encabezado: &Format,
    formato_dinero: &Format,
    formato_tipo_cambio: &Format,
    config: &ExcelConfig,
) -> Result<(), XlsxError> {
    let columns = config
        .columnas_visibles_cfdi
        .iter()
        .filter_map(|name| CFDI_HEADERS.iter().position(|header| header == name))
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Err(XlsxError::ParameterError(
            "debe haber al menos una columna CFDI visible".into(),
        ));
    }
    let headers = columns
        .iter()
        .map(|&column| CFDI_HEADERS[column])
        .collect::<Vec<_>>();
    let widths = columns
        .iter()
        .map(|&column| CFDI_WIDTHS[column])
        .collect::<Vec<_>>();
    preparar_hoja(
        worksheet,
        &headers,
        &widths,
        encabezado,
        filas.len(),
        config,
    )?;

    for (index, fila) in filas.iter().enumerate() {
        let row = index as u32 + 1;
        for (export_column, &source_column) in columns.iter().enumerate() {
            let col = export_column as u16;
            match source_column {
                0 => escribir_texto_opcional(worksheet, row, col, fila.uuid.as_deref())?,
                1 => escribir_texto_opcional(worksheet, row, col, fila.fecha.as_deref())?,
                2 => escribir_texto_opcional(worksheet, row, col, fila.rfc_emisor.as_deref())?,
                3 => escribir_texto_opcional(worksheet, row, col, fila.nombre_emisor.as_deref())?,
                4 => escribir_texto_opcional(worksheet, row, col, fila.rfc_receptor.as_deref())?,
                5 => {
                    escribir_texto_opcional(worksheet, row, col, fila.tipo_comprobante.as_deref())?
                }
                6 => escribir_decimal_opcional(worksheet, row, col, fila.subtotal, formato_dinero)?,
                7 => {
                    escribir_decimal_opcional(worksheet, row, col, fila.descuento, formato_dinero)?
                }
                8 => escribir_decimal_opcional(
                    worksheet,
                    row,
                    col,
                    fila.iva_trasladado_002,
                    formato_dinero,
                )?,
                9 => escribir_decimal_opcional(
                    worksheet,
                    row,
                    col,
                    fila.isr_retenido_001,
                    formato_dinero,
                )?,
                10 => escribir_decimal_opcional(
                    worksheet,
                    row,
                    col,
                    fila.iva_retenido_002,
                    formato_dinero,
                )?,
                11 => escribir_decimal_opcional(worksheet, row, col, fila.total, formato_dinero)?,
                12 => escribir_texto_opcional(worksheet, row, col, fila.uso_cfdi.as_deref())?,
                13 => escribir_texto_opcional(worksheet, row, col, fila.metodo_pago.as_deref())?,
                14 => escribir_texto_opcional(worksheet, row, col, fila.forma_pago.as_deref())?,
                15 => escribir_texto_opcional(worksheet, row, col, fila.moneda.as_deref())?,
                16 => escribir_decimal_opcional(
                    worksheet,
                    row,
                    col,
                    fila.tipo_cambio,
                    formato_tipo_cambio,
                )?,
                17 => {
                    escribir_texto_opcional(worksheet, row, col, fila.clave_prod_serv.as_deref())?
                }
                18 => escribir_texto_opcional(worksheet, row, col, fila.descripcion.as_deref())?,
                19 => escribir_decimal_opcional(
                    worksheet,
                    row,
                    col,
                    fila.importe_concepto,
                    formato_dinero,
                )?,
                20 => {
                    worksheet.write_string(row, col, &fila.archivo_origen)?;
                }
                _ => unreachable!("la lista de columnas CFDI contiene índices validados"),
            }
        }
    }

    if config.ancho_automatico {
        worksheet.autofit();
    }
    Ok(())
}

fn escribir_resumen(
    worksheet: &mut Worksheet,
    filas: &[ResumenProducto],
    encabezado: &Format,
    formato_dinero: &Format,
    config: &ExcelConfig,
) -> Result<(), XlsxError> {
    preparar_hoja(
        worksheet,
        &RESUMEN_HEADERS,
        &[16.0, 35.0, 18.0, 20.0],
        encabezado,
        filas.len(),
        config,
    )?;

    for (index, fila) in filas.iter().enumerate() {
        let row = index as u32 + 1;
        escribir_texto_opcional(worksheet, row, 0, fila.clave_prod_serv.as_deref())?;
        escribir_texto_opcional(worksheet, row, 1, fila.descripcion.as_deref())?;
        worksheet.write_with_format(row, 2, fila.importe_total, formato_dinero)?;
        let num_conceptos = u32::try_from(fila.num_conceptos).map_err(|_| {
            XlsxError::ParameterError("el número de conceptos excede el límite de Excel".into())
        })?;
        worksheet.write_number(row, 3, num_conceptos)?;
    }

    if config.ancho_automatico {
        worksheet.autofit();
    }
    Ok(())
}

fn escribir_clasificaciones(
    worksheet: &mut Worksheet,
    filas: &[FilaClasificada],
    encabezado: &Format,
    config: &ExcelConfig,
) -> Result<(), XlsxError> {
    preparar_hoja(
        worksheet,
        &CLASIFICACION_HEADERS,
        &[35.0, 38.0, 20.0, 16.0, 35.0, 22.0, 48.0],
        encabezado,
        filas.len(),
        config,
    )?;

    for (index, clasificada) in filas.iter().enumerate() {
        let row = index as u32 + 1;
        escribir_texto_opcional(worksheet, row, 0, Some(&clasificada.fila.archivo_origen))?;
        escribir_texto_opcional(worksheet, row, 1, clasificada.fila.uuid.as_deref())?;
        escribir_texto_opcional(
            worksheet,
            row,
            2,
            clasificada.fila.tipo_comprobante.as_deref(),
        )?;
        escribir_texto_opcional(
            worksheet,
            row,
            3,
            clasificada.fila.clave_prod_serv.as_deref(),
        )?;
        escribir_texto_opcional(worksheet, row, 4, clasificada.fila.descripcion.as_deref())?;
        worksheet.write_string(row, 5, clasificada.clasificacion.categoria.nombre())?;
        worksheet.write_string(row, 6, &clasificada.clasificacion.criterio)?;
    }

    if config.ancho_automatico {
        worksheet.autofit();
    }
    Ok(())
}

fn escribir_errores(
    worksheet: &mut Worksheet,
    errores: &[ErrorArchivo],
    encabezado: &Format,
    config: &ExcelConfig,
) -> Result<(), XlsxError> {
    preparar_hoja(
        worksheet,
        &ERRORES_HEADERS,
        &[38.0, 70.0],
        encabezado,
        errores.len(),
        config,
    )?;

    for (index, error) in errores.iter().enumerate() {
        let row = index as u32 + 1;
        worksheet.write_string(row, 0, &error.archivo)?;
        worksheet.write_string(row, 1, &error.motivo)?;
    }

    if config.ancho_automatico {
        worksheet.autofit();
    }
    Ok(())
}

fn preparar_hoja(
    worksheet: &mut Worksheet,
    encabezados: &[&str],
    anchos: &[f64],
    formato_encabezado: &Format,
    filas: usize,
    config: &ExcelConfig,
) -> Result<(), XlsxError> {
    for (columna, encabezado) in encabezados.iter().enumerate() {
        let columna = columna as u16;
        worksheet.write_string_with_format(0, columna, *encabezado, formato_encabezado)?;
        worksheet.set_column_width(columna, anchos[columna as usize])?;
    }
    if config.congelar_encabezado {
        worksheet.set_freeze_panes(1, 0)?;
    }
    if config.autofiltro {
        worksheet.autofilter(0, 0, filas as u32, encabezados.len() as u16 - 1)?;
    }
    Ok(())
}

fn formato_decimal(decimales: usize) -> String {
    if decimales == 0 {
        "#,##0".to_owned()
    } else {
        format!("#,##0.{}", "0".repeat(decimales))
    }
}

fn escribir_texto_opcional(
    worksheet: &mut Worksheet,
    row: u32,
    col: u16,
    value: Option<&str>,
) -> Result<(), XlsxError> {
    if let Some(value) = value {
        worksheet.write_string(row, col, value)?;
    }
    Ok(())
}

fn escribir_decimal_opcional(
    worksheet: &mut Worksheet,
    row: u32,
    col: u16,
    value: Option<rust_decimal::Decimal>,
    format: &Format,
) -> Result<(), XlsxError> {
    if let Some(value) = value {
        worksheet.write_with_format(row, col, value, format)?;
    }
    Ok(())
}
