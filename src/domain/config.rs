use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AppConfig {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub extraccion: ExtraccionConfig,
    #[serde(default)]
    pub clasificacion: ClasificacionConfig,
    #[serde(default)]
    pub excel: ExcelConfig,
    #[serde(default)]
    pub tabla: TablaConfig,
    #[serde(default)]
    pub apariencia: AparienciaConfig,
    #[serde(default)]
    pub atajos: AtajosConfig,
    #[serde(default)]
    pub avanzado: AvanzadoConfig,
}

impl AppConfig {
    pub fn default_config() -> Self {
        Self::default()
    }

    pub fn is_valid(&self) -> bool {
        self.validar().is_empty()
    }

    pub fn validar(&self) -> Vec<ErrorConfig> {
        let mut errors = Vec::new();

        if let Some(rfc) = &self.general.rfc_propio
            && !rfc.trim().is_empty()
        {
            let normalized = rfc.trim();
            if !validar_rfc(normalized) {
                errors.push(ErrorConfig::new(
                    "general.rfc_propio",
                    "El RFC propio no es válido. Usa 12 o 13 caracteres alfanuméricos.",
                ));
            }
        }

        if let Some(error) = validar_hoja("CFDI", &self.excel.nombre_hoja_cfdi) {
            errors.push(ErrorConfig::new("excel.nombre_hoja_cfdi", error));
        }
        if let Some(error) = validar_hoja("Resumen", &self.excel.nombre_hoja_resumen) {
            errors.push(ErrorConfig::new("excel.nombre_hoja_resumen", error));
        }
        if let Some(error) = validar_hoja("Clasificación", &self.excel.nombre_hoja_clasificacion) {
            errors.push(ErrorConfig::new("excel.nombre_hoja_clasificacion", error));
        }
        if let Some(error) = validar_hoja("Errores", &self.excel.nombre_hoja_errores) {
            errors.push(ErrorConfig::new("excel.nombre_hoja_errores", error));
        }
        let included_sheet_names = [
            (
                self.excel.hojas_incluidas.cfdi,
                &self.excel.nombre_hoja_cfdi,
            ),
            (
                self.excel.hojas_incluidas.resumen,
                &self.excel.nombre_hoja_resumen,
            ),
            (
                self.excel.hojas_incluidas.clasificacion,
                &self.excel.nombre_hoja_clasificacion,
            ),
            (
                self.excel.hojas_incluidas.errores,
                &self.excel.nombre_hoja_errores,
            ),
        ];
        for (index, (included, name)) in included_sheet_names.iter().enumerate() {
            if *included
                && included_sheet_names[..index]
                    .iter()
                    .any(|(earlier_included, earlier_name)| {
                        *earlier_included && earlier_name.eq_ignore_ascii_case(name)
                    })
            {
                errors.push(ErrorConfig::new(
                    "excel.hojas_incluidas",
                    "Los nombres de las hojas incluidas deben ser distintos.",
                ));
            }
        }

        if !(28..=44).contains(&self.tabla.altura_fila) {
            errors.push(ErrorConfig::new(
                "tabla.altura_fila",
                "La altura de fila debe estar entre 28 y 44 px.",
            ));
        }
        if !self.tabla.overscan_px.is_finite() || self.tabla.overscan_px < 0.0 {
            errors.push(ErrorConfig::new(
                "tabla.overscan_px",
                "El overscan debe ser un número positivo.",
            ));
        }

        if !self.apariencia.tamaño_fuente.is_finite()
            || !(10.0..=20.0).contains(&self.apariencia.tamaño_fuente)
        {
            errors.push(ErrorConfig::new(
                "apariencia.tamaño_fuente",
                "El tamaño de fuente debe estar entre 10 y 20 px.",
            ));
        }

        if self.excel.decimales_monto > 6 {
            errors.push(ErrorConfig::new(
                "excel.decimales_monto",
                "Los decimales de monto deben estar entre 0 y 6.",
            ));
        }

        if self.excel.decimales_tipo_cambio > 8 {
            errors.push(ErrorConfig::new(
                "excel.decimales_tipo_cambio",
                "Los decimales del tipo de cambio deben estar entre 0 y 8.",
            ));
        }

        if self.excel.columnas_visibles_cfdi.is_empty() {
            errors.push(ErrorConfig::new(
                "excel.columnas_visibles_cfdi",
                "Debe haber al menos una columna visible en la hoja CFDI.",
            ));
        }
        let valid_columns = [
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
        if self
            .excel
            .columnas_visibles_cfdi
            .iter()
            .any(|column| !valid_columns.contains(&column.as_str()))
        {
            errors.push(ErrorConfig::new(
                "excel.columnas_visibles_cfdi",
                "Hay una columna CFDI que no existe.",
            ));
        }
        for (index, column) in self.excel.columnas_visibles_cfdi.iter().enumerate() {
            if self.excel.columnas_visibles_cfdi[..index]
                .iter()
                .any(|earlier| earlier == column)
            {
                errors.push(ErrorConfig::new(
                    "excel.columnas_visibles_cfdi",
                    "No repitas columnas CFDI.",
                ));
                break;
            }
        }

        if !(self.excel.hojas_incluidas.cfdi
            || self.excel.hojas_incluidas.resumen
            || self.excel.hojas_incluidas.clasificacion
            || self.excel.hojas_incluidas.errores)
        {
            errors.push(ErrorConfig::new(
                "excel.hojas_incluidas",
                "Debe incluirse al menos una hoja.",
            ));
        }

        if self.excel.formato_nombre_archivo.trim().is_empty()
            || self.excel.formato_nombre_archivo.chars().any(|character| {
                matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            })
        {
            errors.push(ErrorConfig::new(
                "excel.formato_nombre_archivo",
                "El formato del nombre no puede estar vacío ni incluir caracteres de ruta.",
            ));
        }

        if self.apariencia.color_acento.is_empty() {
            errors.push(ErrorConfig::new(
                "apariencia.color_acento",
                "Selecciona un color de acento.",
            ));
        }

        let duplicates = self.atajos.detect_conflicts();
        if !duplicates.is_empty() {
            errors.push(ErrorConfig::new(
                "atajos",
                format!("Hay atajos duplicados: {}.", duplicates.join(", ")),
            ));
        }

        errors
    }

    pub fn reordenar_version(&mut self) {
        self.version = CONFIG_VERSION;
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            general: GeneralConfig::default(),
            extraccion: ExtraccionConfig::default(),
            clasificacion: ClasificacionConfig::default(),
            excel: ExcelConfig::default(),
            tabla: TablaConfig::default(),
            apariencia: AparienciaConfig::default(),
            atajos: AtajosConfig::default(),
            avanzado: AvanzadoConfig::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct GeneralConfig {
    pub rfc_propio: Option<String>,
    pub ultima_carpeta_zip: Option<PathBuf>,
    pub ultima_carpeta_exportacion: Option<PathBuf>,
    pub recordar_rfc: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            rfc_propio: None,
            ultima_carpeta_zip: None,
            ultima_carpeta_exportacion: None,
            recordar_rfc: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct ExtraccionConfig {
    pub repetir_impuestos_en_cada_concepto: bool,
    pub incluir_tipos: TipoComprobanteConfig,
    pub columna_origen_emitida_recibida: bool,
}

impl Default for ExtraccionConfig {
    fn default() -> Self {
        Self {
            repetir_impuestos_en_cada_concepto: true,
            incluir_tipos: TipoComprobanteConfig::todos(),
            columna_origen_emitida_recibida: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct TipoComprobanteConfig {
    pub i: bool,
    pub e: bool,
    pub p: bool,
    pub n: bool,
    pub t: bool,
}

impl TipoComprobanteConfig {
    pub fn todos() -> Self {
        Self {
            i: true,
            e: true,
            p: true,
            n: true,
            t: true,
        }
    }
}

impl Default for TipoComprobanteConfig {
    fn default() -> Self {
        Self::todos()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct ClasificacionConfig {
    pub reglas_honorarios: Vec<ReglaClasificacion>,
    pub reglas_inversion: Vec<ReglaClasificacion>,
    pub regla_nomina_tipo_n: bool,
    pub regla_ingreso_gasto_por_rfc: bool,
    pub categoria_por_defecto: String,
}

impl Default for ClasificacionConfig {
    fn default() -> Self {
        Self {
            reglas_honorarios: vec![ReglaClasificacion {
                categoria: "Honorarios".to_owned(),
                palabras_clave: vec![
                    "honorarios".to_owned(),
                    "servicios profesionales".to_owned(),
                    "asesoría".to_owned(),
                    "consultoría".to_owned(),
                ],
                activa: true,
            }],
            reglas_inversion: vec![ReglaClasificacion {
                categoria: "Inversión".to_owned(),
                palabras_clave: vec![
                    "equipo de cómputo".to_owned(),
                    "maquinaria".to_owned(),
                    "vehículo".to_owned(),
                    "mobiliario".to_owned(),
                    "terreno".to_owned(),
                    "edificio".to_owned(),
                    "activo fijo".to_owned(),
                ],
                activa: true,
            }],
            regla_nomina_tipo_n: true,
            regla_ingreso_gasto_por_rfc: true,
            categoria_por_defecto: "Otros".to_owned(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ReglaClasificacion {
    pub categoria: String,
    pub palabras_clave: Vec<String>,
    pub activa: bool,
}

impl Default for ReglaClasificacion {
    fn default() -> Self {
        Self {
            categoria: "Honorarios".to_owned(),
            palabras_clave: vec!["honorarios".to_owned()],
            activa: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct ExcelConfig {
    pub nombre_hoja_cfdi: String,
    pub nombre_hoja_resumen: String,
    pub nombre_hoja_clasificacion: String,
    pub nombre_hoja_errores: String,
    pub hojas_incluidas: HojasIncluidasConfig,
    pub columnas_visibles_cfdi: Vec<String>,
    pub decimales_monto: usize,
    pub decimales_tipo_cambio: usize,
    pub formato_nombre_archivo: String,
    pub congelar_encabezado: bool,
    pub autofiltro: bool,
    pub ancho_automatico: bool,
    pub abrir_carpeta_al_exportar: bool,
}

impl Default for ExcelConfig {
    fn default() -> Self {
        Self {
            nombre_hoja_cfdi: "CFDI".to_owned(),
            nombre_hoja_resumen: "Resumen por producto".to_owned(),
            nombre_hoja_clasificacion: "Clasificación sugerida".to_owned(),
            nombre_hoja_errores: "Errores".to_owned(),
            hojas_incluidas: HojasIncluidasConfig::default(),
            columnas_visibles_cfdi: vec![
                "UUID".to_owned(),
                "Fecha".to_owned(),
                "RFC Emisor".to_owned(),
                "Nombre Emisor".to_owned(),
                "RFC Receptor".to_owned(),
                "Tipo de Comprobante".to_owned(),
                "Total".to_owned(),
                "ClaveProdServ".to_owned(),
                "Descripción".to_owned(),
                "Importe".to_owned(),
                "Archivo".to_owned(),
            ],
            decimales_monto: 2,
            decimales_tipo_cambio: 6,
            formato_nombre_archivo: "cfdi_{YYYYMMDD}_{HHmm}.xlsx".to_owned(),
            congelar_encabezado: true,
            autofiltro: true,
            ancho_automatico: true,
            abrir_carpeta_al_exportar: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct HojasIncluidasConfig {
    pub cfdi: bool,
    pub resumen: bool,
    pub clasificacion: bool,
    pub errores: bool,
}

impl Default for HojasIncluidasConfig {
    fn default() -> Self {
        Self {
            cfdi: true,
            resumen: true,
            clasificacion: true,
            errores: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct TablaConfig {
    pub altura_fila: i32,
    pub mostrar_hover: bool,
    pub overscan_px: f32,
}

impl Default for TablaConfig {
    fn default() -> Self {
        Self {
            altura_fila: 32,
            mostrar_hover: true,
            overscan_px: 240.0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct AparienciaConfig {
    pub tema: TemaApp,
    pub color_acento: String,
    pub densidad: DensidadApp,
    pub tamaño_fuente: f32,
}

impl Default for AparienciaConfig {
    fn default() -> Self {
        Self {
            tema: TemaApp::Sistema,
            color_acento: "Indigo".to_owned(),
            densidad: DensidadApp::Normal,
            tamaño_fuente: 13.0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum TemaApp {
    #[default]
    Sistema,
    Claro,
    Oscuro,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum DensidadApp {
    #[default]
    Normal,
    Compacta,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct AtajosConfig {
    pub abrir_zip: String,
    pub procesar: String,
    pub exportar: String,
    pub pestaña_1: String,
    pub pestaña_2: String,
    pub pestaña_3: String,
    pub pestaña_4: String,
    pub abrir_configuracion: String,
}

impl AtajosConfig {
    fn detect_conflicts(&self) -> Vec<String> {
        let mut seen = Vec::new();
        let mut duplicates = Vec::new();
        let values = vec![
            ("abrir_zip", self.abrir_zip.clone()),
            ("procesar", self.procesar.clone()),
            ("exportar", self.exportar.clone()),
            ("pestaña_1", self.pestaña_1.clone()),
            ("pestaña_2", self.pestaña_2.clone()),
            ("pestaña_3", self.pestaña_3.clone()),
            ("pestaña_4", self.pestaña_4.clone()),
            ("abrir_configuracion", self.abrir_configuracion.clone()),
        ];

        for (name, value) in values {
            if value.trim().is_empty() {
                continue;
            }
            if seen.iter().any(|item: &(String, String)| item.1 == value) {
                if !duplicates.iter().any(|entry| entry == name) {
                    duplicates.push(name.to_owned());
                }
            } else {
                seen.push((name.to_owned(), value));
            }
        }

        duplicates
    }
}

impl Default for AtajosConfig {
    fn default() -> Self {
        Self {
            abrir_zip: "Ctrl+O".to_owned(),
            procesar: "Ctrl+P".to_owned(),
            exportar: "Ctrl+E".to_owned(),
            pestaña_1: "Ctrl+1".to_owned(),
            pestaña_2: "Ctrl+2".to_owned(),
            pestaña_3: "Ctrl+3".to_owned(),
            pestaña_4: "Ctrl+4".to_owned(),
            abrir_configuracion: "Ctrl+,".to_owned(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct AvanzadoConfig {
    pub hilos_de_procesamiento: String,
    pub mostrar_errores_detallados: bool,
    pub registro_de_depuracion: bool,
}

impl Default for AvanzadoConfig {
    fn default() -> Self {
        Self {
            hilos_de_procesamiento: "Auto".to_owned(),
            mostrar_errores_detallados: false,
            registro_de_depuracion: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorConfig {
    pub campo: &'static str,
    pub mensaje: String,
}

impl ErrorConfig {
    pub fn new(campo: &'static str, mensaje: impl Into<String>) -> Self {
        Self {
            campo,
            mensaje: mensaje.into(),
        }
    }
}

fn validar_rfc(value: &str) -> bool {
    let normalized = value.trim();
    if normalized.is_empty() {
        return true;
    }
    let chars: Vec<char> = normalized.chars().collect();
    if !(12..=13).contains(&chars.len()) {
        return false;
    }
    chars
        .iter()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, 'Ñ' | '&'))
}

fn validar_hoja(nombre: &str, value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(format!(
            "El nombre de la hoja {} no puede estar vacío.",
            nombre
        ));
    }
    if trimmed.chars().count() > 31 {
        return Some(format!(
            "El nombre de la hoja {} puede tener máximo 31 caracteres.",
            nombre
        ));
    }
    let invalid = [':', '\\', '/', '?', '*', '[', ']'];
    if trimmed.chars().any(|char| invalid.contains(&char)) {
        return Some(format!(
            "El nombre de la hoja {} contiene caracteres no permitidos.",
            nombre
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn valida_rfc_propio_opcional_y_longitudes_de_persona_fisica_y_moral() {
        let mut config = AppConfig::default();
        assert!(config.is_valid());

        config.general.rfc_propio = Some("AAA010101AAA".to_owned());
        assert!(config.is_valid());
        config.general.rfc_propio = Some("ABCD010101AA1".to_owned());
        assert!(config.is_valid());
        config.general.rfc_propio = Some("RFC-CORTO".to_owned());
        assert!(
            config
                .validar()
                .iter()
                .any(|error| error.campo == "general.rfc_propio")
        );
    }

    #[test]
    fn rechaza_columnas_desconocidas_y_nombres_de_archivo_con_ruta() {
        let mut config = AppConfig::default();
        config.excel.columnas_visibles_cfdi = vec!["Columna inventada".to_owned()];
        config.excel.formato_nombre_archivo = "../fuera.xlsx".to_owned();

        let fields = config
            .validar()
            .into_iter()
            .map(|error| error.campo)
            .collect::<Vec<_>>();
        assert!(fields.contains(&"excel.columnas_visibles_cfdi"));
        assert!(fields.contains(&"excel.formato_nombre_archivo"));
    }
}
