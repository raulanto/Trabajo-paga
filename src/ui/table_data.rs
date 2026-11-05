use rust_decimal::Decimal;
use std::rc::Rc;

use ely_gpui_component::tables::{Cell, Column, FilterRule, Row, Test};
use gpui::{SharedString, rems};

use crate::{
    application::{procesar_zip::ResultadoProceso, resumen::ResumenProducto},
    domain::{cfdi::CfdiRow, clasificacion::FilaClasificada, error::ErrorArchivo},
};

use super::dimensions;

pub(crate) const CFDI_HEADERS: [&str; 21] = [
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

pub(crate) const RESUMEN_HEADERS: [&str; 4] = [
    "ClaveProdServ",
    "Descripción",
    "Importe total",
    "Número de conceptos",
];

pub(crate) const CLASIFICACION_HEADERS: [&str; 7] = [
    "Archivo",
    "UUID",
    "Tipo de Comprobante",
    "ClaveProdServ",
    "Descripción",
    "Categoría sugerida",
    "Criterio",
];

pub(crate) const ERROR_HEADERS: [&str; 2] = ["Archivo", "Motivo"];

#[derive(Clone, Copy)]
pub(crate) struct ColumnSpec {
    pub key: &'static str,
    pub title: &'static str,
    pub width: f32,
    pub numeric: bool,
    pub sortable: bool,
}

pub(crate) const CFDI_COLUMNS: [ColumnSpec; 21] = [
    col("uuid", CFDI_HEADERS[0], dimensions::WIDTH_UUID),
    col("fecha", CFDI_HEADERS[1], dimensions::WIDTH_DATE),
    col("rfc_emisor", CFDI_HEADERS[2], dimensions::WIDTH_RFC),
    col("nombre_emisor", CFDI_HEADERS[3], dimensions::WIDTH_NAME),
    col("rfc_receptor", CFDI_HEADERS[4], dimensions::WIDTH_RFC),
    col("tipo_comprobante", CFDI_HEADERS[5], dimensions::WIDTH_TYPE),
    num("subtotal", CFDI_HEADERS[6], dimensions::WIDTH_MONEY, false),
    num("descuento", CFDI_HEADERS[7], dimensions::WIDTH_MONEY, false),
    num(
        "iva_trasladado",
        CFDI_HEADERS[8],
        dimensions::WIDTH_MONEY,
        false,
    ),
    num(
        "isr_retenido",
        CFDI_HEADERS[9],
        dimensions::WIDTH_MONEY,
        false,
    ),
    num(
        "iva_retenido",
        CFDI_HEADERS[10],
        dimensions::WIDTH_MONEY,
        false,
    ),
    num("total", CFDI_HEADERS[11], dimensions::WIDTH_MONEY, false),
    col("uso_cfdi", CFDI_HEADERS[12], dimensions::WIDTH_CODE),
    col("metodo_pago", CFDI_HEADERS[13], dimensions::WIDTH_METHOD),
    col("forma_pago", CFDI_HEADERS[14], dimensions::WIDTH_METHOD),
    col("moneda", CFDI_HEADERS[15], dimensions::WIDTH_CURRENCY),
    num(
        "tipo_cambio",
        CFDI_HEADERS[16],
        dimensions::WIDTH_EXCHANGE,
        false,
    ),
    col("clave_prod_serv", CFDI_HEADERS[17], dimensions::WIDTH_CODE),
    col(
        "descripcion",
        CFDI_HEADERS[18],
        dimensions::WIDTH_DESCRIPTION,
    ),
    num("importe", CFDI_HEADERS[19], dimensions::WIDTH_MONEY, false),
    col("archivo", CFDI_HEADERS[20], dimensions::WIDTH_FILE),
];

pub(crate) const RESUMEN_COLUMNS: [ColumnSpec; 4] = [
    col(
        "clave_prod_serv",
        RESUMEN_HEADERS[0],
        dimensions::WIDTH_CODE,
    ),
    col(
        "descripcion",
        RESUMEN_HEADERS[1],
        dimensions::WIDTH_DESCRIPTION,
    ),
    num(
        "importe_total",
        RESUMEN_HEADERS[2],
        dimensions::WIDTH_MONEY,
        false,
    ),
    num(
        "numero_conceptos",
        RESUMEN_HEADERS[3],
        dimensions::WIDTH_COUNT,
        false,
    ),
];

pub(crate) const CLASIFICACION_COLUMNS: [ColumnSpec; 7] = [
    col("archivo", CLASIFICACION_HEADERS[0], dimensions::WIDTH_FILE),
    col("uuid", CLASIFICACION_HEADERS[1], dimensions::WIDTH_UUID),
    col(
        "tipo_comprobante",
        CLASIFICACION_HEADERS[2],
        dimensions::WIDTH_TYPE,
    ),
    col(
        "clave_prod_serv",
        CLASIFICACION_HEADERS[3],
        dimensions::WIDTH_CODE,
    ),
    col(
        "descripcion",
        CLASIFICACION_HEADERS[4],
        dimensions::WIDTH_DESCRIPTION,
    ),
    col(
        "categoria",
        CLASIFICACION_HEADERS[5],
        dimensions::WIDTH_CATEGORY,
    ),
    col(
        "criterio",
        CLASIFICACION_HEADERS[6],
        dimensions::WIDTH_CRITERION,
    ),
];

pub(crate) const ERROR_COLUMNS: [ColumnSpec; 2] = [
    col("archivo", ERROR_HEADERS[0], dimensions::WIDTH_FILE),
    col("motivo", ERROR_HEADERS[1], dimensions::WIDTH_ERROR),
];

pub(crate) struct PreparedTable {
    pub rows: Vec<Row>,
    values: Vec<Box<[SharedString]>>,
    keys: Vec<SharedString>,
}

pub(crate) struct PreparedTables {
    pub cfdi: PreparedTable,
    pub resumen: PreparedTable,
    pub clasificacion: PreparedTable,
    pub errores: PreparedTable,
    pub importe_total: ImporteTotal,
}

pub(crate) struct UiTables {
    pub cfdi: UiTable,
    pub resumen: UiTable,
    pub clasificacion: UiTable,
    pub errores: UiTable,
    pub importe_total: ImporteTotal,
}

pub(crate) struct UiTable {
    pub rows: Rc<Vec<Row>>,
    values: Vec<Box<[SharedString]>>,
    keys: Vec<SharedString>,
}

pub(crate) enum ImporteTotal {
    SinDatos,
    Etiqueta(SharedString),
}

impl PreparedTables {
    pub fn from_result(result: &ResultadoProceso) -> Self {
        let importe_total = match sumar_importes(&result.filas) {
            Ok(Some(total)) => ImporteTotal::Etiqueta(
                format!("Importe total: {}", format_decimal(total, 2)).into(),
            ),
            Ok(None) => ImporteTotal::SinDatos,
            Err(()) => ImporteTotal::Etiqueta("Importe total fuera del rango decimal".into()),
        };
        Self {
            cfdi: prepare_rows(
                &result.filas,
                &CFDI_COLUMNS,
                &CFDI_HEADERS,
                |row, index| format!("cfdi:{}:{index}", row.archivo_origen),
                cfdi_values,
            ),
            resumen: prepare_rows(
                &result.resumen,
                &RESUMEN_COLUMNS,
                &RESUMEN_HEADERS,
                |row, index| {
                    format!(
                        "resumen:{:?}:{:?}:{index}",
                        row.clave_prod_serv, row.descripcion
                    )
                },
                resumen_values,
            ),
            clasificacion: prepare_rows(
                &result.clasificaciones,
                &CLASIFICACION_COLUMNS,
                &CLASIFICACION_HEADERS,
                |row, index| {
                    format!(
                        "clasificacion:{}:{:?}:{index}",
                        row.fila.archivo_origen, row.fila.uuid
                    )
                },
                clasificacion_values,
            ),
            errores: prepare_rows(
                &result.errores,
                &ERROR_COLUMNS,
                &ERROR_HEADERS,
                |row, index| format!("error:{}:{index}", row.archivo),
                error_values,
            ),
            importe_total,
        }
    }

    pub fn into_ui(self) -> UiTables {
        UiTables {
            cfdi: self.cfdi.into_ui(),
            resumen: self.resumen.into_ui(),
            clasificacion: self.clasificacion.into_ui(),
            errores: self.errores.into_ui(),
            importe_total: self.importe_total,
        }
    }
}

impl PreparedTable {
    fn into_ui(self) -> UiTable {
        UiTable {
            rows: Rc::new(self.rows),
            values: self.values,
            keys: self.keys,
        }
    }
}

impl UiTable {
    pub fn to_tsv(&self, headers: &[&str], selected: &[SharedString]) -> String {
        to_tsv(&self.values, &self.keys, headers, selected)
    }

    pub fn visible_count(
        &self,
        query: &str,
        columns: &[ColumnSpec],
        filters: &[FilterRule],
        any: bool,
    ) -> usize {
        let query = query.trim().to_lowercase();
        let filters = filters
            .iter()
            .filter(|rule| rule.test == Test::Empty || !rule.value.trim().is_empty())
            .filter_map(|rule| {
                columns
                    .iter()
                    .position(|column| column.key == rule.column.as_ref())
                    .map(|column| (column, rule))
            })
            .collect::<Vec<_>>();

        self.values
            .iter()
            .filter(|row| {
                query.is_empty()
                    || row
                        .iter()
                        .any(|value| value.to_lowercase().contains(&query))
            })
            .filter(|row| {
                if filters.is_empty() {
                    return true;
                }
                let matches = |(column, rule): &(usize, &FilterRule)| {
                    let value = row[*column].as_ref();
                    let expected = rule.value.trim().to_lowercase();
                    let value = value.to_lowercase();
                    match rule.test {
                        Test::Contains => value.contains(&expected),
                        Test::Equals => value == expected,
                        Test::NotEquals => value != expected,
                        Test::Above | Test::Below => false,
                        Test::Empty => value.is_empty(),
                    }
                };
                if any {
                    filters.iter().any(matches)
                } else {
                    filters.iter().all(matches)
                }
            })
            .count()
    }
}

pub(crate) fn columns(specs: &[ColumnSpec]) -> Vec<Column> {
    specs
        .iter()
        .map(|spec| {
            let column = Column::new(spec.key, spec.title).width(rems(spec.width));
            let column = if spec.numeric { column.end() } else { column };
            if spec.sortable {
                column
            } else {
                column.unsorted()
            }
        })
        .collect()
}

fn prepare_rows<T>(
    values: &[T],
    columns: &[ColumnSpec],
    headers: &[&str],
    make_key: impl Fn(&T, usize) -> String,
    format_row: impl Fn(&T) -> Vec<String>,
) -> PreparedTable {
    let mut rows = Vec::with_capacity(values.len());
    let mut formatted_rows = Vec::with_capacity(values.len());
    let mut keys = Vec::with_capacity(values.len());
    for (index, item) in values.iter().enumerate() {
        let formatted = format_row(item)
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let key = SharedString::from(make_key(item, index));
        rows.push(Row::new(
            key.clone(),
            formatted.iter().cloned().map(Cell::Text),
        ));
        keys.push(key);
        formatted_rows.push(formatted);
    }
    debug_assert_eq!(columns.len(), headers.len());
    PreparedTable {
        rows,
        values: formatted_rows,
        keys,
    }
}

fn to_tsv(
    values: &[Box<[SharedString]>],
    keys: &[SharedString],
    headers: &[&str],
    selected: &[SharedString],
) -> String {
    let selected = selected
        .iter()
        .map(|key| key.as_ref())
        .collect::<std::collections::HashSet<_>>();
    let has_selection = !selected.is_empty();
    let mut lines = Vec::with_capacity(values.len() + 1);
    lines.push(
        headers
            .iter()
            .map(|value| clean_tsv(value))
            .collect::<Vec<_>>()
            .join("\t"),
    );
    for (index, row) in values.iter().enumerate() {
        if has_selection && !selected.contains(keys[index].as_ref()) {
            continue;
        }
        lines.push(
            row.iter()
                .map(|value| clean_tsv(value))
                .collect::<Vec<_>>()
                .join("\t"),
        );
    }
    lines.join("\n")
}

const fn col(key: &'static str, title: &'static str, width: f32) -> ColumnSpec {
    ColumnSpec {
        key,
        title,
        width,
        numeric: false,
        sortable: true,
    }
}

const fn num(key: &'static str, title: &'static str, width: f32, sortable: bool) -> ColumnSpec {
    ColumnSpec {
        key,
        title,
        width,
        numeric: true,
        sortable,
    }
}

fn clean_tsv(value: &str) -> String {
    value.replace(['\t', '\n', '\r'], " ")
}

fn cfdi_values(row: &CfdiRow) -> Vec<String> {
    vec![
        optional_text(&row.uuid),
        optional_text(&row.fecha),
        optional_text(&row.rfc_emisor),
        optional_text(&row.nombre_emisor),
        optional_text(&row.rfc_receptor),
        optional_text(&row.tipo_comprobante),
        optional_decimal(row.subtotal, 2),
        optional_decimal(row.descuento, 2),
        optional_decimal(row.iva_trasladado_002, 2),
        optional_decimal(row.isr_retenido_001, 2),
        optional_decimal(row.iva_retenido_002, 2),
        optional_decimal(row.total, 2),
        optional_text(&row.uso_cfdi),
        optional_text(&row.metodo_pago),
        optional_text(&row.forma_pago),
        optional_text(&row.moneda),
        optional_decimal(row.tipo_cambio, 6),
        optional_text(&row.clave_prod_serv),
        optional_text(&row.descripcion),
        optional_decimal(row.importe_concepto, 2),
        row.archivo_origen.clone(),
    ]
}

fn resumen_values(row: &ResumenProducto) -> Vec<String> {
    vec![
        optional_text(&row.clave_prod_serv),
        optional_text(&row.descripcion),
        format_decimal(row.importe_total, 2),
        format_integer(row.num_conceptos),
    ]
}

fn clasificacion_values(row: &FilaClasificada) -> Vec<String> {
    vec![
        row.fila.archivo_origen.clone(),
        optional_text(&row.fila.uuid),
        optional_text(&row.fila.tipo_comprobante),
        optional_text(&row.fila.clave_prod_serv),
        optional_text(&row.fila.descripcion),
        row.clasificacion.categoria.nombre().to_owned(),
        row.clasificacion.criterio.clone(),
    ]
}

fn error_values(row: &ErrorArchivo) -> Vec<String> {
    vec![row.archivo.clone(), row.motivo.clone()]
}

fn optional_text(value: &Option<String>) -> String {
    value.clone().unwrap_or_default()
}

fn optional_decimal(value: Option<Decimal>, decimals: usize) -> String {
    value.map_or_else(String::new, |value| format_decimal(value, decimals))
}

fn sumar_importes(rows: &[CfdiRow]) -> Result<Option<Decimal>, ()> {
    let mut total = None;
    for importe in rows.iter().filter_map(|row| row.importe_concepto) {
        total = match total {
            None => Some(importe),
            Some(actual) => match actual.checked_add(importe) {
                Some(suma) => Some(suma),
                None => return Err(()),
            },
        };
    }
    Ok(total)
}

fn format_decimal(value: Decimal, decimals: usize) -> String {
    let formatted = format!("{value:.decimals$}");
    let (sign, number) = formatted
        .strip_prefix('-')
        .map_or(("", formatted.as_str()), |number| ("-", number));
    let (integer, fraction) = number.split_once('.').unwrap_or((number, ""));
    let mut reversed = String::with_capacity(formatted.len() + integer.len() / 3);
    for (index, character) in integer.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            reversed.push(',');
        }
        reversed.push(character);
    }
    let integer = reversed.chars().rev().collect::<String>();
    let mut grouped = format!("{sign}{integer}");
    if decimals > 0 {
        grouped.push('.');
        grouped.push_str(fraction);
    }
    grouped
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell as Counter;
    use std::str::FromStr;

    #[test]
    fn mantiene_las_columnas_de_las_cuatro_vistas() {
        assert_eq!(CFDI_COLUMNS.len(), 21);
        assert_eq!(RESUMEN_COLUMNS.len(), 4);
        assert_eq!(CLASIFICACION_COLUMNS.len(), 7);
        assert_eq!(ERROR_COLUMNS.len(), 2);
        assert!(!CFDI_COLUMNS[6].sortable);
        assert!(!CFDI_COLUMNS[19].sortable);
        assert!(CFDI_COLUMNS[6].numeric);
        assert!(CFDI_COLUMNS[19].numeric);
    }

    #[test]
    fn formatea_decimal_y_deja_vacios_sin_valor() {
        assert_eq!(
            format_decimal(Decimal::from_str("1234567.8").unwrap(), 2),
            "1,234,567.80"
        );
        assert_eq!(optional_decimal(None, 2), "");
    }

    #[test]
    fn prepara_tsv_con_vacios_y_sanea_separadores() {
        let values = vec![
            vec![SharedString::from("uno\tdos"), SharedString::from("")].into_boxed_slice(),
            vec![SharedString::from("tres"), SharedString::from("4")].into_boxed_slice(),
        ];
        let table = PreparedTable {
            rows: Vec::new(),
            values,
            keys: vec!["test-0".into(), "test-1".into()],
        };
        assert_eq!(
            to_tsv(&table.values, &table.keys, &["Nombre", "Dato"], &[]),
            "Nombre\tDato\nuno dos\t\ntres\t4"
        );
        assert_eq!(
            to_tsv(
                &table.values,
                &table.keys,
                &["Nombre", "Dato"],
                &[SharedString::from("test-1")]
            ),
            "Nombre\tDato\ntres\t4"
        );
    }

    #[test]
    fn cuenta_filas_visibles_con_busqueda_y_filtros_de_ely() {
        let table = PreparedTable {
            rows: Vec::new(),
            values: vec![
                vec!["Uno".into(), "Válido".into()].into_boxed_slice(),
                vec!["Dos".into(), "Error".into()].into_boxed_slice(),
                vec!["Tres".into(), "".into()].into_boxed_slice(),
            ],
            keys: vec!["uno".into(), "dos".into(), "tres".into()],
        }
        .into_ui();
        let columns = [col("nombre", "Nombre", 10.0), col("estado", "Estado", 10.0)];
        assert_eq!(table.visible_count("o", &columns, &[], false), 2);

        let filters = [FilterRule {
            column: "estado".into(),
            test: Test::Contains,
            value: "error".into(),
        }];
        assert_eq!(table.visible_count("", &columns, &filters, false), 1);
        assert_eq!(table.visible_count("dos", &columns, &filters, false), 1);
        assert_eq!(table.visible_count("uno", &columns, &filters, false), 0);
    }

    #[test]
    fn formatea_cada_fila_una_sola_vez_antes_de_renderizar() {
        let values = [10, 20];
        let calls = Counter::new(0);
        let table = prepare_rows(
            &values,
            &[col("valor", "Valor", 8.0)],
            &["Valor"],
            |_, index| format!("test-{index}"),
            |value| {
                calls.set(calls.get() + 1);
                vec![value.to_string()]
            },
        );
        assert_eq!(calls.get(), values.len());
        assert_eq!(table.rows.len(), values.len());
        assert_eq!(
            to_tsv(&table.values, &table.keys, &["Valor"], &[]),
            "Valor\n10\n20"
        );
    }

    #[test]
    fn prepara_cinco_mil_filas_tabulares_sin_perder_conceptos() {
        let filas = (0..5_000)
            .map(|index| CfdiRow {
                importe_concepto: Some(Decimal::ONE),
                archivo_origen: format!("cfdi-{index}.xml"),
                ..empty_cfdi()
            })
            .collect();
        let result = ResultadoProceso {
            filas,
            resumen: Vec::new(),
            clasificaciones: Vec::new(),
            errores: Vec::new(),
            total_xml_leidos: 5_000,
            total_cfdi_ok: 5_000,
        };
        let prepared = PreparedTables::from_result(&result);
        assert_eq!(prepared.cfdi.rows.len(), 5_000);
        assert_eq!(
            to_tsv(
                &prepared.cfdi.values,
                &prepared.cfdi.keys,
                &CFDI_HEADERS,
                &[]
            )
            .lines()
            .count(),
            5_001
        );
        assert!(matches!(prepared.importe_total, ImporteTotal::Etiqueta(_)));
    }

    #[test]
    fn los_totales_solo_suman_importes_presentes_con_decimal() {
        let rows = vec![
            CfdiRow {
                subtotal: Some(Decimal::from_str("1000").unwrap()),
                total: Some(Decimal::from_str("1200").unwrap()),
                importe_concepto: Some(Decimal::from_str("0.1").unwrap()),
                ..empty_cfdi()
            },
            CfdiRow {
                subtotal: Some(Decimal::from_str("1000").unwrap()),
                total: Some(Decimal::from_str("1200").unwrap()),
                importe_concepto: Some(Decimal::from_str("0.2").unwrap()),
                ..empty_cfdi()
            },
            CfdiRow {
                importe_concepto: None,
                ..empty_cfdi()
            },
        ];
        match sumar_importes(&rows) {
            Ok(Some(total)) => {
                assert_eq!(total, Decimal::from_str("0.3").unwrap());
            }
            Ok(None) | Err(()) => {
                panic!("se esperaba una suma decimal exacta")
            }
        }
        assert!(matches!(sumar_importes(&[]), Ok(None)));
    }

    fn empty_cfdi() -> CfdiRow {
        CfdiRow {
            uuid: None,
            fecha: None,
            rfc_emisor: None,
            nombre_emisor: None,
            rfc_receptor: None,
            tipo_comprobante: None,
            subtotal: None,
            descuento: None,
            iva_trasladado_002: None,
            isr_retenido_001: None,
            iva_retenido_002: None,
            total: None,
            uso_cfdi: None,
            metodo_pago: None,
            forma_pago: None,
            moneda: None,
            tipo_cambio: None,
            clave_prod_serv: None,
            descripcion: None,
            importe_concepto: None,
            archivo_origen: String::new(),
        }
    }
}
