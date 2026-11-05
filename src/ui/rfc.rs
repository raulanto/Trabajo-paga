pub(crate) fn normalizar_rfc(value: &str) -> String {
    value.chars().flat_map(char::to_uppercase).collect()
}

pub(crate) fn aviso_rfc(value: &str) -> Option<&'static str> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if !(12..=13).contains(&value.chars().count()) {
        return Some("El RFC suele tener 12 o 13 caracteres.");
    }

    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, 'Ñ' | '&'))
    {
        return Some("Usa letras, números, Ñ o &.");
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_rfc_a_mayusculas() {
        assert_eq!(normalizar_rfc("aaa010101aaa"), "AAA010101AAA");
    }

    #[test]
    fn valida_longitud_y_caracteres_sin_bloquear_el_vacio() {
        assert_eq!(aviso_rfc(""), None);
        assert_eq!(aviso_rfc("  "), None);
        assert_eq!(aviso_rfc("AAA010101AAA"), None);
        assert_eq!(aviso_rfc("ABC010101AAAB"), None);
        assert!(aviso_rfc("CORTO").is_some());
        assert!(aviso_rfc("AAA010101AA!").is_some());
    }
}
