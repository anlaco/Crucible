//! Sufijos de unidad de un parámetro numérico.
//!
//! SCPI permite escribir el valor con su unidad, y los clientes reales lo hacen
//! constantemente: el manual del generador de RF que simulamos documenta sus
//! ejemplos como `:FREQ 2.4 GHZ` y `:POW -10 DBM`, no como `:FREQ 2400000000`.
//!
//! El sufijo **no es decorado**: lleva el multiplicador. Descartarlo y quedarse
//! con la mantisa —que es lo que se hacía aquí antes— convierte `2.4 GHZ` en
//! 2,4 Hz, mil millones de veces menos, sin que nadie se entere. Un error de
//! parámetro es molesto; un valor silenciosamente equivocado por 10⁹ es un
//! resultado de test falso.
//!
//! ## La ambigüedad de la `M`
//!
//! SCPI-99 reserva `MA` para «mega» justamente porque `M` ya era «mili», de
//! modo que un megahercio se escribe `MAHZ`. En la práctica no lo respeta
//! nadie: todos los instrumentos —y el propio manual de Keysight— aceptan
//! `MHZ` como megahercio, y nadie ha pedido jamás un milihercio.
//!
//! La regla que se aplica aquí es la de los instrumentos reales, no la de la
//! letra del estándar:
//!
//! - con `HZ`, la `M` es **mega** (`MHZ` = 10⁶ Hz), y `MAHZ` también;
//! - con cualquier otra unidad, la `M` es **mili** (`MA` = 10⁻³ A, `MV`, `MS`).
//!
//! El prefijo `A` de «atto» no se reconoce: colisionaría con el amperio, y
//! entre interpretar `A` como 10⁻¹⁸ o como la unidad de corriente, la segunda
//! es la única que alguien ha escrito alguna vez.

/// Unidades base que se reconocen al final de un sufijo.
///
/// Se prueban en este orden, así que las más largas van primero: si no, `DBM`
/// se leería como el prefijo `D` sobre `BM`, o `DB` dejaría una `M` suelta.
const UNIDADES: &[&str] = &[
    "DBUV", "DBM", "DBW", "DB", "OHM", "PCT", "DEG", "RAD", "HZ", "V", "A", "W", "S",
];

/// Prefijos multiplicadores, de más largo a más corto por el mismo motivo:
/// `MA` (mega) tiene que probarse antes que `M` (mili).
const PREFIJOS: &[(&str, f64)] = &[
    ("MA", 1e6),
    ("EX", 1e18),
    ("PE", 1e15),
    ("T", 1e12),
    ("G", 1e9),
    ("K", 1e3),
    ("M", 1e-3),
    ("U", 1e-6),
    ("N", 1e-9),
    ("P", 1e-12),
    ("F", 1e-15),
];

/// Lee un número decimal con su sufijo de unidad opcional y lo devuelve en la
/// unidad base del SI (Hz, V, A, W, s…).
///
/// Devuelve `None` si la mantisa no es un número o si el sufijo no se reconoce.
/// Rechazar el sufijo desconocido es deliberado: aceptarlo callando fue lo que
/// permitió que `2.4 GHZ` valiera 2,4.
///
/// `dBm` y `dB` son logarítmicas y no admiten multiplicador; se devuelven tal
/// cual, que es lo que espera quien programa una amplitud.
pub fn parse_decimal(raw: &str) -> Option<f64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    let corte = fin_de_mantisa(raw);
    let mantisa: f64 = raw[..corte].parse().ok()?;

    let sufijo = raw[corte..].trim();
    if sufijo.is_empty() {
        return Some(mantisa);
    }

    Some(mantisa * multiplicador(&sufijo.to_ascii_uppercase())?)
}

/// Dónde acaba la parte numérica.
///
/// La `E` del exponente es el caso delicado: en `1E3` forma parte del número y
/// en `1EX` es el prefijo «exa». Se distingue mirando lo que viene detrás, que
/// en un exponente es siempre un dígito o un signo seguido de dígito.
fn fin_de_mantisa(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = 0;

    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
        i += 1;
    }

    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            i = j;
        }
    }

    i
}

/// El factor por el que hay que multiplicar la mantisa, o `None` si el sufijo
/// no es una unidad conocida.
fn multiplicador(sufijo: &str) -> Option<f64> {
    let unidad = UNIDADES.iter().find(|u| sufijo.ends_with(**u))?;
    let prefijo = &sufijo[..sufijo.len() - unidad.len()];

    if prefijo.is_empty() {
        return Some(1.0);
    }

    // Las unidades logarítmicas no se escalan: «mdBm» no significa nada.
    if unidad.starts_with("DB") {
        return None;
    }

    let (_, factor) = PREFIJOS.iter().find(|(p, _)| *p == prefijo)?;

    // La excepción de la `M`, explicada en la cabecera del módulo.
    if *unidad == "HZ" && prefijo == "M" {
        return Some(1e6);
    }

    Some(*factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compara con margen: `100 * 1e-6` no da exactamente `1e-4` en binario, y
    /// el test no está para comprobar cómo redondea el hardware.
    fn casi(a: Option<f64>, b: f64) {
        let a = a.expect("se esperaba un número");
        assert!(
            (a - b).abs() <= b.abs() * 1e-12,
            "se esperaba {b}, se obtuvo {a}"
        );
    }

    #[test]
    fn un_numero_sin_sufijo_se_lee_tal_cual() {
        assert_eq!(parse_decimal("10"), Some(10.0));
        assert_eq!(parse_decimal("-3.5"), Some(-3.5));
        assert_eq!(parse_decimal("1e-3"), Some(1e-3));
        assert_eq!(parse_decimal("+2.5E3"), Some(2500.0));
    }

    /// El fallo que motivó este módulo: la mantisa sola daba 2,4 Hz.
    #[test]
    fn el_sufijo_de_frecuencia_aporta_el_multiplicador() {
        casi(parse_decimal("2.4 GHZ"), 2.4e9);
        casi(parse_decimal("2.4GHZ"), 2.4e9);
        casi(parse_decimal("100 MHZ"), 100e6);
        casi(parse_decimal("100 KHZ"), 100e3);
        assert_eq!(parse_decimal("9 HZ"), Some(9.0));
    }

    /// `MHZ` es megahercio y `MA` es miliamperio: la misma letra, dos cosas,
    /// y las dos aparecen en los manuales de este banco.
    #[test]
    fn la_eme_es_mega_en_hercios_y_mili_en_lo_demas() {
        casi(parse_decimal("1 MHZ"), 1e6);
        casi(parse_decimal("1 MAHZ"), 1e6);
        casi(parse_decimal("1 MA"), 1e-3);
        casi(parse_decimal("1 MV"), 1e-3);
        casi(parse_decimal("1 MS"), 1e-3);
    }

    #[test]
    fn las_unidades_electricas_admiten_prefijo() {
        casi(parse_decimal("10 V"), 10.0);
        casi(parse_decimal("500 MV"), 0.5);
        casi(parse_decimal("2 KV"), 2000.0);
        casi(parse_decimal("25 A"), 25.0);
        casi(parse_decimal("100 UA"), 100e-6);
        casi(parse_decimal("1500 W"), 1500.0);
    }

    /// Una amplitud en dBm es logarítmica: el número es el número.
    #[test]
    fn las_unidades_logaritmicas_no_se_escalan() {
        assert_eq!(parse_decimal("-10 DBM"), Some(-10.0));
        assert_eq!(parse_decimal("-144DBM"), Some(-144.0));
        assert_eq!(parse_decimal("3 DB"), Some(3.0));
        // Un prefijo sobre una unidad logarítmica no significa nada.
        assert_eq!(parse_decimal("1 MDBM"), None);
    }

    /// `1E3` es mil y `1EX` es un trillón: la `E` se decide por lo que sigue.
    #[test]
    fn la_e_del_exponente_no_se_confunde_con_el_prefijo_exa() {
        assert_eq!(parse_decimal("1E3"), Some(1000.0));
        assert_eq!(parse_decimal("1E3HZ"), Some(1000.0));
    }

    /// Antes cualquier cosa detrás del número se ignoraba en silencio.
    #[test]
    fn un_sufijo_desconocido_es_un_error_y_no_un_descarte() {
        assert_eq!(parse_decimal("2 XYZZY"), None);
        assert_eq!(parse_decimal("MAX"), None);
        assert_eq!(parse_decimal(""), None);
        assert_eq!(parse_decimal("ON"), None);
    }
}
