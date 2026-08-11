//! Formato de las respuestas numéricas.
//!
//! SCPI no deja libertad aquí: los números en coma flotante se devuelven en
//! formato NR3, con signo siempre explícito, un dígito antes del punto y
//! exponente de dos cifras con signo. Un multímetro real contesta
//! `+5.000018E+00`, nunca `5.000018` ni `5.0e0`.
//!
//! Importa porque hay clientes que analizan la respuesta con expresiones
//! regulares estrictas, y porque el valor de desbordamiento tiene un número
//! reservado que los drivers reconocen como "fuera de rango".

/// Valor que devuelve un instrumento cuando la medida excede el rango.
///
/// Es una convención universal en instrumentación: al ver este número, el
/// cliente sabe que no es una lectura sino un desbordamiento.
pub const OVERFLOW: f64 = 9.9e37;

/// Formatea un real en NR3, el formato exigido por SCPI.
pub fn nr3(v: f64) -> String {
    if v.is_nan() || v.is_infinite() {
        return nr3(OVERFLOW);
    }

    // {:+.6E} da "+5.000018E0"; el exponente hay que rellenarlo a dos cifras.
    let s = format!("{v:+.6E}");
    let (mantisa, exponente) = s.split_once('E').expect("el formato E siempre lo incluye");

    let (signo, digitos) = match exponente.strip_prefix('-') {
        Some(resto) => ('-', resto),
        None => ('+', exponente.trim_start_matches('+')),
    };

    format!("{mantisa}E{signo}{digitos:0>2}")
}

/// Formatea un entero como respuesta SCPI (formato NR1).
pub fn nr1(v: i64) -> String {
    v.to_string()
}

/// Formatea un booleano como el 1 o el 0 que espera el estándar.
pub fn boolean(v: bool) -> String {
    if v { "1".to_string() } else { "0".to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_formato_es_el_nr3_del_estandar() {
        assert_eq!(nr3(5.000018), "+5.000018E+00");
        assert_eq!(nr3(-3.3), "-3.300000E+00");
        assert_eq!(nr3(0.0), "+0.000000E+00");
    }

    #[test]
    fn el_exponente_siempre_lleva_dos_cifras_y_signo() {
        assert_eq!(nr3(1e-3), "+1.000000E-03");
        assert_eq!(nr3(1e100), "+1.000000E+100"); // tres cifras cuando hacen falta
        assert_eq!(nr3(1.5e7), "+1.500000E+07");
    }

    #[test]
    fn el_desbordamiento_tiene_su_valor_reservado() {
        assert_eq!(nr3(OVERFLOW), "+9.900000E+37");
        assert_eq!(nr3(f64::NAN), nr3(OVERFLOW));
        assert_eq!(nr3(f64::INFINITY), nr3(OVERFLOW));
    }

    #[test]
    fn los_enteros_y_booleanos_van_sin_adornos() {
        assert_eq!(nr1(-113), "-113");
        assert_eq!(boolean(true), "1");
        assert_eq!(boolean(false), "0");
    }
}
