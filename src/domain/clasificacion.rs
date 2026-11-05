//! Reglas puras para sugerencias de clasificación contable.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::domain::{
    cfdi::CfdiRow,
    config::{ClasificacionConfig, ReglaClasificacion},
};

pub const CLAVES_HONORARIOS: &[&str] = &[
    "honorarios",
    "servicios profesionales",
    "asesoría",
    "consultoría",
];

pub const CLAVES_INVERSION: &[&str] = &[
    "equipo de cómputo",
    "maquinaria",
    "vehículo",
    "mobiliario",
    "terreno",
    "edificio",
    "activo fijo",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Categoria {
    Ingreso,
    Gasto,
    Honorarios,
    Nomina,
    Inversion,
    Otros,
}

impl Categoria {
    pub fn nombre(self) -> &'static str {
        match self {
            Self::Ingreso => "Ingreso",
            Self::Gasto => "Gasto",
            Self::Honorarios => "Honorarios",
            Self::Nomina => "Nómina",
            Self::Inversion => "Inversión",
            Self::Otros => "Otros",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Clasificacion {
    pub categoria: Categoria,
    pub criterio: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FilaClasificada {
    pub fila: CfdiRow,
    pub clasificacion: Clasificacion,
}

pub fn clasificar(fila: &CfdiRow, rfc_propio: Option<&str>) -> Clasificacion {
    clasificar_con_config(fila, rfc_propio, &ClasificacionConfig::default())
}

pub fn clasificar_con_config(
    fila: &CfdiRow,
    rfc_propio: Option<&str>,
    config: &ClasificacionConfig,
) -> Clasificacion {
    if fila
        .tipo_comprobante
        .as_deref()
        .is_some_and(|tipo| tipo.eq_ignore_ascii_case("N"))
        && config.regla_nomina_tipo_n
    {
        return sugerencia(Categoria::Nomina, "Tipo N");
    }

    let descripcion = fila
        .descripcion
        .as_deref()
        .map(normalizar)
        .unwrap_or_default();

    if let Some(sugerencia) = buscar_regla(&descripcion, &config.reglas_honorarios) {
        return sugerencia;
    }

    if let Some(sugerencia) = buscar_regla(&descripcion, &config.reglas_inversion) {
        return sugerencia;
    }

    if fila
        .tipo_comprobante
        .as_deref()
        .is_some_and(|tipo| tipo.eq_ignore_ascii_case("I"))
        && config.regla_ingreso_gasto_por_rfc
    {
        let rfc_normalizado = rfc_propio.map(normalizar_rfc).filter(|rfc| !rfc.is_empty());
        if let Some(rfc_normalizado) = rfc_normalizado {
            if fila
                .rfc_emisor
                .as_deref()
                .is_some_and(|rfc| normalizar_rfc(rfc) == rfc_normalizado)
            {
                return sugerencia(Categoria::Ingreso, "RFC emisor coincide con RFC propio");
            }
            if fila
                .rfc_receptor
                .as_deref()
                .is_some_and(|rfc| normalizar_rfc(rfc) == rfc_normalizado)
            {
                return sugerencia(Categoria::Gasto, "RFC receptor coincide con RFC propio");
            }
            return sugerencia(
                categoria_desde_nombre(&config.categoria_por_defecto),
                "Tipo I sin coincidencia con RFC propio",
            );
        }

        return sugerencia(
            categoria_desde_nombre(&config.categoria_por_defecto),
            "Tipo I sin RFC propio",
        );
    }

    let criterio = match fila.tipo_comprobante.as_deref() {
        Some(tipo) => format!("Tipo {tipo} sin regla"),
        None => "Tipo de comprobante ausente sin regla".to_owned(),
    };
    sugerencia(
        categoria_desde_nombre(&config.categoria_por_defecto),
        criterio,
    )
}

fn buscar_regla(descripcion: &str, reglas: &[ReglaClasificacion]) -> Option<Clasificacion> {
    reglas
        .iter()
        .filter(|regla| regla.activa)
        .find_map(|regla| {
            regla
                .palabras_clave
                .iter()
                .find(|clave| descripcion.contains(&normalizar(clave)))
                .map(|clave| {
                    sugerencia(
                        categoria_desde_nombre(&regla.categoria),
                        format!("Descripción contiene palabra clave: {clave}"),
                    )
                })
        })
}

fn categoria_desde_nombre(nombre: &str) -> Categoria {
    match normalizar(nombre).trim() {
        "ingreso" => Categoria::Ingreso,
        "gasto" => Categoria::Gasto,
        "honorarios" => Categoria::Honorarios,
        "nomina" => Categoria::Nomina,
        "inversion" => Categoria::Inversion,
        _ => Categoria::Otros,
    }
}

fn sugerencia(categoria: Categoria, criterio: impl Into<String>) -> Clasificacion {
    Clasificacion {
        categoria,
        criterio: criterio.into(),
    }
}

fn normalizar_rfc(rfc: &str) -> String {
    normalizar(rfc)
        .chars()
        .filter(|caracter| !caracter.is_whitespace())
        .collect()
}

fn normalizar(texto: &str) -> String {
    texto
        .nfd()
        .filter(|caracter| !is_combining_mark(*caracter))
        .flat_map(char::to_lowercase)
        .collect()
}
