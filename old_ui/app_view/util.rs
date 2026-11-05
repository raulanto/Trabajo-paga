//! Funciones puras de formateo, validación y manejo de rutas.

use std::path::{Path, PathBuf};

use chrono::Local;
use rust_decimal::Decimal;

pub fn validar_rfc(value: &str) -> Option<&'static str> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let length = value.chars().count();
    if !(12..=13).contains(&length) {
        return Some("El RFC debe tener 12 o 13 caracteres");
    }
    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, 'Ñ' | '&'))
    {
        return Some("Usa solo letras, números, Ñ o &");
    }
    None
}

pub fn format_decimal(value: Decimal, decimals: usize) -> String {
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

pub fn format_integer(value: usize) -> String {
    let mut result = String::new();
    for (index, character) in value.to_string().chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            result.push(',');
        }
        result.push(character);
    }
    result.chars().rev().collect()
}

pub(super) fn format_filename_preview(template: &str) -> String {
    let now = Local::now();
    template
        .replace("{YYYYMMDD}", &now.format("%Y%m%d").to_string())
        .replace("{HHmm}", &now.format("%H%M").to_string())
        .replace("{YYYY}", &now.format("%Y").to_string())
        .replace("{MM}", &now.format("%m").to_string())
        .replace("{DD}", &now.format("%d").to_string())
}

pub(super) fn normalizar_caja_rfc(value: &str) -> String {
    value.chars().flat_map(char::to_uppercase).collect()
}

pub(super) fn es_zip(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}

pub(super) fn ensure_extension(mut path: PathBuf, extension: &str) -> PathBuf {
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case(extension))
    {
        path.set_extension(extension);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validar_rfc_acepta_12_y_13_caracteres_y_avisa_sin_bloquear() {
        assert_eq!(validar_rfc("ASF180425991"), None);
        assert_eq!(validar_rfc("CAR060613CDA"), None);
        assert_eq!(validar_rfc(""), None);
        assert!(validar_rfc("CORTO").is_some());
        assert!(validar_rfc("ASF18042599!").is_some());
        assert_eq!(normalizar_caja_rfc("asf180425991"), "ASF180425991");
    }

    #[test]
    fn format_decimal_usa_miles_y_dos_decimales_sin_float() {
        assert_eq!(format_decimal("1248.5".parse().unwrap(), 2), "1,248.50");
        assert_eq!(
            format_decimal("-1000000.00".parse().unwrap(), 2),
            "-1,000,000.00"
        );
        assert_eq!(format_decimal("0".parse().unwrap(), 2), "0.00");
    }

    #[test]
    fn extensiones_se_normalizan_sin_duplicar_el_punto() {
        assert!(es_zip(Path::new("comprobantes.ZIP")));
        assert!(!es_zip(Path::new("comprobantes.xml")));
        assert_eq!(
            ensure_extension(PathBuf::from("reporte"), "xlsx"),
            PathBuf::from("reporte.xlsx")
        );
        assert_eq!(
            ensure_extension(PathBuf::from("reporte.XLSX"), "xlsx"),
            PathBuf::from("reporte.XLSX")
        );
    }
}
