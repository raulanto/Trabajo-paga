use std::io::{Cursor, Write};

use calamine::{Data, DataType, Reader, open_workbook_auto};
use cfdi_zip_processor::{
    application::{
        ports::{CfdiParser, ExcelExporter},
        procesar_zip::{ProcesarZip, ResultadoProceso},
        resumen::{ErrorResumen, ResumenProducto, resumir},
    },
    domain::{
        cfdi::CfdiRow,
        clasificacion::{Categoria, Clasificacion, clasificar},
        config::AppConfig,
    },
    infrastructure::{
        xlsx_exporter::XlsxExporter, xml_parser::XmlCfdiParser, zip_reader::ZipXmlReader,
    },
};
use rust_decimal::Decimal;
use tempfile::NamedTempFile;
use zip::{
    CompressionMethod,
    write::{SimpleFileOptions, ZipWriter},
};

fn decimal(value: &str) -> Decimal {
    value.parse().expect("test decimal is valid")
}

fn fila(
    tipo_comprobante: Option<&str>,
    descripcion: Option<&str>,
    clave_prod_serv: Option<&str>,
    importe_concepto: Option<&str>,
) -> CfdiRow {
    CfdiRow {
        uuid: None,
        fecha: None,
        rfc_emisor: None,
        nombre_emisor: None,
        rfc_receptor: None,
        tipo_comprobante: tipo_comprobante.map(str::to_owned),
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
        clave_prod_serv: clave_prod_serv.map(str::to_owned),
        descripcion: descripcion.map(str::to_owned),
        importe_concepto: importe_concepto.map(decimal),
        archivo_origen: "prueba.xml".to_owned(),
    }
}

fn fila_ganado() -> CfdiRow {
    XmlCfdiParser
        .parsear(
            "cfdi40_ganado_iva_tasa0.xml",
            include_bytes!("fixtures/cfdi40_ganado_iva_tasa0.xml"),
        )
        .expect("ganado CFDI fixture parses")
        .into_iter()
        .next()
        .expect("fixture has a concept")
}

#[test]
fn clasificacion_tipo_i_usa_rfc_propio_ignorando_mayusculas_y_espacios() {
    let fila = fila_ganado();

    assert_eq!(
        clasificar(&fila, Some("ASF180425991")).categoria,
        Categoria::Ingreso
    );
    assert_eq!(
        clasificar(&fila, Some("CAR060613CDA")).categoria,
        Categoria::Gasto
    );
    assert_eq!(
        clasificar(&fila, None),
        Clasificacion {
            categoria: Categoria::Otros,
            criterio: "Tipo I sin RFC propio".to_owned(),
        }
    );
    assert_eq!(
        clasificar(&fila, Some(" asf180425991 ")).categoria,
        Categoria::Ingreso
    );
    assert_eq!(
        clasificar(&fila, Some(" rfc-no-relacionado ")).criterio,
        "Tipo I sin coincidencia con RFC propio"
    );
}

#[test]
fn clasificacion_honorarios_ignora_mayusculas_y_acentos() {
    let fila = fila(
        Some("I"),
        Some("ASESORÍA y CONSULTORÍA especializada"),
        None,
        None,
    );
    let clasificacion = clasificar(&fila, None);

    assert_eq!(clasificacion.categoria, Categoria::Honorarios);
    assert_eq!(
        clasificacion.criterio,
        "Descripción contiene palabra clave: asesoría"
    );
}

#[test]
fn clasificacion_inversion_ignora_mayusculas_y_acentos() {
    let fila = fila(Some("I"), Some("EQUIPO DE COMPUTO industrial"), None, None);
    let clasificacion = clasificar(&fila, None);

    assert_eq!(clasificacion.categoria, Categoria::Inversion);
    assert_eq!(
        clasificacion.criterio,
        "Descripción contiene palabra clave: equipo de cómputo"
    );
}

#[test]
fn clasificacion_prioriza_tipo_n_sobre_honorarios_e_inversion() {
    let fila = fila(
        Some("N"),
        Some("Honorarios por equipo de cómputo"),
        None,
        None,
    );
    let clasificacion = clasificar(&fila, Some("ASF180425991"));

    assert_eq!(
        clasificacion,
        Clasificacion {
            categoria: Categoria::Nomina,
            criterio: "Tipo N".to_owned(),
        }
    );
}

#[test]
fn clasificacion_prioriza_honorarios_sobre_inversion() {
    let fila = fila(
        Some("I"),
        Some("Honorarios por adquisición de maquinaria"),
        None,
        None,
    );

    assert_eq!(clasificar(&fila, None).categoria, Categoria::Honorarios);
}

#[test]
fn palabras_clave_priorizan_sobre_coincidencia_rfc() {
    let mut honorarios = fila(Some("I"), Some("Servicios profesionales"), None, None);
    honorarios.rfc_emisor = Some("ASF180425991".to_owned());
    let mut inversion = fila(Some("I"), Some("Maquinaria"), None, None);
    inversion.rfc_emisor = Some("ASF180425991".to_owned());

    assert_eq!(
        clasificar(&honorarios, Some("ASF180425991")).categoria,
        Categoria::Honorarios
    );
    assert_eq!(
        clasificar(&inversion, Some("ASF180425991")).categoria,
        Categoria::Inversion
    );
}

#[test]
fn clasificacion_otros_explica_tipo_sin_regla() {
    let fila = fila(Some("T"), Some("Traslado de mercancía"), None, None);
    let clasificacion = clasificar(&fila, None);

    assert_eq!(clasificacion.categoria, Categoria::Otros);
    assert_eq!(clasificacion.criterio, "Tipo T sin regla");
}

#[test]
fn resumen_agrupa_y_suma_decimales_exactos_contando_importes_ausentes() {
    let filas = vec![
        fila(Some("I"), Some("Mismo producto"), Some("A"), Some("0.10")),
        fila(Some("I"), Some("Mismo producto"), Some("A"), Some("0.20")),
        fila(Some("I"), Some("Mismo producto"), Some("A"), None),
        fila(Some("I"), Some("Otro producto"), Some("B"), Some("1.00")),
        fila(Some("I"), Some("Solo sin importe"), Some("C"), None),
    ];

    let resumen = resumir(&filas).expect("small decimal sums fit");

    assert_eq!(
        resumen,
        vec![
            ResumenProducto {
                clave_prod_serv: Some("B".to_owned()),
                descripcion: Some("Otro producto".to_owned()),
                importe_total: decimal("1.00"),
                num_conceptos: 1,
            },
            ResumenProducto {
                clave_prod_serv: Some("A".to_owned()),
                descripcion: Some("Mismo producto".to_owned()),
                importe_total: decimal("0.30"),
                num_conceptos: 3,
            },
            ResumenProducto {
                clave_prod_serv: Some("C".to_owned()),
                descripcion: Some("Solo sin importe".to_owned()),
                importe_total: Decimal::ZERO,
                num_conceptos: 1,
            },
        ]
    );
}

#[test]
fn resumen_reporta_desbordamiento_decimal() {
    let filas = vec![
        fila(
            Some("I"),
            Some("Producto"),
            Some("A"),
            Some("79228162514264337593543950335"),
        ),
        fila(Some("I"), Some("Producto"), Some("A"), Some("1")),
    ];

    assert!(matches!(
        resumir(&filas),
        Err(ErrorResumen::ImporteFueraDeRango { .. })
    ));
}

fn crear_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (nombre, bytes) in entries {
        writer
            .start_file(*nombre, options)
            .expect("ZIP entry starts");
        writer.write_all(bytes).expect("ZIP entry writes");
    }

    writer.finish().expect("ZIP finishes").into_inner()
}

fn escribir_temporal(bytes: &[u8]) -> NamedTempFile {
    let mut archivo = NamedTempFile::new().expect("temporary file is created");
    archivo.write_all(bytes).expect("temporary file is written");
    archivo
}

#[test]
fn procesar_zip_cuenta_xml_validos_y_continua_tras_error() {
    let archive = crear_zip(&[
        (
            "ganado/cfdi40_ganado_iva_tasa0.xml",
            include_bytes!("fixtures/cfdi40_ganado_iva_tasa0.xml"),
        ),
        (
            "arbitrario/cfdi_33.XML",
            include_bytes!("fixtures/cfdi_33.xml"),
        ),
        (
            "subcarpeta/corrupto.xml",
            include_bytes!("fixtures/xml_corrupto.xml"),
        ),
        ("notas/leer.txt", b"no XML"),
    ]);
    let zip_file = escribir_temporal(&archive);
    let processor = ProcesarZip::new(ZipXmlReader, XmlCfdiParser);
    let mut progress = Vec::new();
    let result = processor.ejecutar_con_progreso(zip_file.path(), None, |done, total| {
        progress.push((done, total));
    });

    assert_eq!(result.total_xml_leidos, 3);
    assert_eq!(result.total_cfdi_ok, 2);
    assert_eq!(result.filas.len(), 2);
    assert_eq!(result.resumen.len(), 2);
    assert_eq!(result.clasificaciones.len(), 2);
    assert_eq!(result.errores.len(), 1);
    assert!(
        result.errores[0]
            .archivo
            .ends_with("subcarpeta/corrupto.xml")
    );
    assert!(progress.contains(&(0, 3)));
    assert!(progress.contains(&(1, 3)));
    assert!(progress.contains(&(2, 3)));
    assert!(progress.contains(&(3, 3)));
}

#[test]
fn procesar_zip_procesa_5000_xml_sin_perder_filas() {
    let fixture = include_bytes!("fixtures/cfdi40_ganado_iva_tasa0.xml");
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for index in 0..5_000 {
        writer
            .start_file(format!("lote/{index}.xml"), options)
            .expect("ZIP entry starts");
        writer.write_all(fixture).expect("fixture writes to ZIP");
    }

    let archive = writer.finish().expect("ZIP finishes").into_inner();
    let zip_file = escribir_temporal(&archive);
    let result = ProcesarZip::new(ZipXmlReader, XmlCfdiParser).ejecutar(zip_file.path(), None);

    assert_eq!(result.total_xml_leidos, 5_000);
    assert_eq!(result.total_cfdi_ok, 5_000);
    assert_eq!(result.filas.len(), 5_000);
    assert!(result.errores.is_empty());
}

#[test]
fn procesar_zip_reporta_zip_corrupto_sin_detener_el_caso_de_uso() {
    let zip_file = escribir_temporal(b"not a zip");
    let result = ProcesarZip::new(ZipXmlReader, XmlCfdiParser).ejecutar(zip_file.path(), None);

    assert_eq!(result.total_xml_leidos, 0);
    assert_eq!(result.total_cfdi_ok, 0);
    assert!(result.filas.is_empty());
    assert_eq!(result.errores.len(), 1);
}

#[test]
fn procesar_zip_respeta_tipos_incluidos_y_no_repite_impuestos_si_se_configura() {
    let fixture = crear_zip(&[(
        "cfdi40_ganado_iva_tasa0.xml",
        include_bytes!("fixtures/cfdi40_ganado_iva_tasa0.xml"),
    )]);
    let zip_file = escribir_temporal(&fixture);
    let processor = ProcesarZip::new(ZipXmlReader, XmlCfdiParser);

    let mut config = AppConfig::default();
    config.extraccion.incluir_tipos.i = false;
    let filtrado = processor.ejecutar_con_config(zip_file.path(), None, &config, |_, _| {});
    assert_eq!(filtrado.total_cfdi_ok, 1);
    assert!(filtrado.filas.is_empty());

    config.extraccion.incluir_tipos.i = true;
    config.extraccion.repetir_impuestos_en_cada_concepto = false;
    let sin_impuestos_repetidos =
        processor.ejecutar_con_config(zip_file.path(), None, &config, |_, _| {});
    assert!(!sin_impuestos_repetidos.filas.is_empty());
    assert!(
        sin_impuestos_repetidos
            .filas
            .iter()
            .all(|fila| fila.iva_trasladado_002.is_none()
                && fila.isr_retenido_001.is_none()
                && fila.iva_retenido_002.is_none())
    );
}

#[test]
fn exportar_xlsx_conserva_hojas_numeros_y_celdas_vacias() {
    let fixture = crear_zip(&[(
        "cfdi40_ganado_iva_tasa0.xml",
        include_bytes!("fixtures/cfdi40_ganado_iva_tasa0.xml"),
    )]);
    let zip_file = escribir_temporal(&fixture);
    let resultado: ResultadoProceso =
        ProcesarZip::new(ZipXmlReader, XmlCfdiParser).ejecutar(zip_file.path(), None);
    let xlsx_file = NamedTempFile::new().expect("temporary xlsx is created");
    let mut config = AppConfig::default();
    config.excel.columnas_visibles_cfdi = [
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
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();

    XlsxExporter
        .exportar(
            &resultado.filas,
            &resultado.resumen,
            &resultado.clasificaciones,
            &resultado.errores,
            &config,
            xlsx_file.path(),
        )
        .expect("XLSX exports");

    let mut workbook = open_workbook_auto(xlsx_file.path()).expect("XLSX reopens");
    assert_eq!(
        workbook.sheet_names(),
        [
            "CFDI",
            "Resumen por producto",
            "Clasificación sugerida",
            "Errores",
        ]
    );

    let cfdi = workbook.worksheet_range("CFDI").expect("CFDI sheet opens");
    assert_eq!(cfdi.height(), 2);
    assert_eq!(cfdi.get((1, 8)).and_then(DataType::get_float), Some(0.0));
    for col in [7, 9, 10, 13, 16] {
        assert_eq!(
            cfdi.get((1, col)),
            Some(&Data::Empty),
            "column {col} should remain empty"
        );
    }

    let resumen = workbook
        .worksheet_range("Resumen por producto")
        .expect("summary sheet opens");
    assert_eq!(resumen.height(), 2);
    let clasificacion = workbook
        .worksheet_range("Clasificación sugerida")
        .expect("classification sheet opens");
    assert_eq!(clasificacion.height(), 2);
    let errores = workbook
        .worksheet_range("Errores")
        .expect("errors sheet opens");
    assert_eq!(errores.height(), 1);
}

#[test]
fn exportar_xlsx_respeta_nombres_hojas_inclusion_y_columnas_configuradas() {
    let fixture = crear_zip(&[(
        "cfdi40_ganado_iva_tasa0.xml",
        include_bytes!("fixtures/cfdi40_ganado_iva_tasa0.xml"),
    )]);
    let zip_file = escribir_temporal(&fixture);
    let resultado = ProcesarZip::new(ZipXmlReader, XmlCfdiParser).ejecutar(zip_file.path(), None);
    let xlsx_file = NamedTempFile::new().expect("temporary xlsx is created");
    let mut config = AppConfig::default();
    config.excel.nombre_hoja_cfdi = "Comprobantes".to_owned();
    config.excel.hojas_incluidas.resumen = false;
    config.excel.hojas_incluidas.clasificacion = false;
    config.excel.hojas_incluidas.errores = false;
    config.excel.columnas_visibles_cfdi = vec!["Importe".to_owned(), "Archivo".to_owned()];

    XlsxExporter
        .exportar(
            &resultado.filas,
            &resultado.resumen,
            &resultado.clasificaciones,
            &resultado.errores,
            &config,
            xlsx_file.path(),
        )
        .expect("configured XLSX exports");

    let mut workbook = open_workbook_auto(xlsx_file.path()).expect("XLSX reopens");
    assert_eq!(workbook.sheet_names(), ["Comprobantes"]);
    let cfdi = workbook
        .worksheet_range("Comprobantes")
        .expect("configured sheet opens");
    assert_eq!(
        cfdi.get((0, 0)).and_then(DataType::get_string),
        Some("Importe")
    );
    assert_eq!(
        cfdi.get((0, 1)).and_then(DataType::get_string),
        Some("Archivo")
    );
}

#[test]
fn exportar_xlsx_escribe_encabezado_de_errores_sin_registros() {
    let xlsx_file = NamedTempFile::new().expect("temporary xlsx is created");
    XlsxExporter
        .exportar(&[], &[], &[], &[], &AppConfig::default(), xlsx_file.path())
        .expect("empty XLSX exports");

    let mut workbook = open_workbook_auto(xlsx_file.path()).expect("XLSX reopens");
    let errores = workbook
        .worksheet_range("Errores")
        .expect("errors sheet opens");

    assert_eq!(errores.height(), 1);
    assert_eq!(
        errores.get((0, 0)).and_then(DataType::get_string),
        Some("Archivo")
    );
    assert_eq!(
        errores.get((0, 1)).and_then(DataType::get_string),
        Some("Motivo")
    );
}
