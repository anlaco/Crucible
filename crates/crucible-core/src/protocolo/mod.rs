//! La capa de protocolo: cómo se habla con un dispositivo.
//!
//! El perfil describe *qué* hace el dispositivo; el protocolo, *cómo* se le
//! dice. Hoy solo está implementado SCPI; Modbus y los seriales a medida son el
//! siguiente paso del ADR-0002, y por eso el despacho está detrás de esta
//! frontera en vez de suponer SCPI en todas partes.

use crate::estado::{Estado, Valor};
use instrusim_scpi::{ErrorCode, ScpiError};
use std::collections::HashMap;

pub mod scpi;

/// Sustituye en el texto las referencias a variables de estado y a argumentos.
///
/// `"{voltaje_fuente}"` toma el valor del estado; `"<v>"` toma el argumento que
/// el perfil nombró `v`. Un texto sin ninguna de las dos formas se devuelve tal
/// cual, que es el caso más común (una respuesta fija).
pub fn resolver_plantilla(texto: &str, estado: &Estado, args: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(texto.len());
    let mut resto = texto;

    while let Some(ini) = resto.find(['{', '<']) {
        let abre = resto.as_bytes()[ini] as char;
        let cierra = if abre == '{' { '}' } else { '>' };

        let Some(fin) = resto[ini..].find(cierra).map(|p| ini + p) else {
            break;
        };

        out.push_str(&resto[..ini]);
        let nombre = &resto[ini + 1..fin];

        let valor = if abre == '{' {
            estado.get(nombre).map(|v| v.as_str())
        } else {
            args.get(nombre).cloned()
        };

        match valor {
            Some(v) => out.push_str(&v),
            // Referencia a algo que no existe: se deja literal, para que se vea
            // en la respuesta en vez de desaparecer en silencio.
            None => out.push_str(&resto[ini..=fin]),
        }
        resto = &resto[fin + 1..];
    }

    out.push_str(resto);
    out
}

/// Aplica las mutaciones de estado que declara un comando.
///
/// El tipo de cada variable **ya está declarado**: es el del valor que le dio
/// el bloque `estado:` del perfil. Una variable que arrancó como número solo
/// acepta números, y una que arrancó como booleano solo acepta `ON`/`OFF`/`1`/
/// `0`. Antes no se miraba: `VOLT MAX` guardaba la cadena «MAX» donde había un
/// número, el instrumento seguía contestando como si nada, y el error no salía
/// hasta dos comandos después, cuando una fórmula intentaba leerla. Ahora sale
/// aquí, como `-224`, que es lo que contestaría el aparato real.
///
/// Se valida todo antes de escribir nada: un comando que falla no debe dejar el
/// instrumento configurado a medias.
pub fn aplicar_mutacion(
    muta: &HashMap<String, String>,
    estado: &mut Estado,
    args: &HashMap<String, String>,
) -> Result<(), ScpiError> {
    let mut pendientes = Vec::with_capacity(muta.len());

    for (clave, expr) in muta {
        // Un argumento opcional que no llegó (`CONF:VOLT` sin rango) deja la
        // variable como estaba. Antes se guardaba el texto «<rango>» tal cual y
        // la siguiente medida fallaba sin que se viera por qué.
        let e = expr.trim();
        if e.starts_with('<') && e.ends_with('>') && !args.contains_key(&e[1..e.len() - 1]) {
            continue;
        }
        let valor = resolver_expr(expr, args);
        pendientes.push((clave, convertir(&valor, estado.get(clave))?));
    }

    for (clave, valor) in pendientes {
        estado.set(clave, valor);
    }
    Ok(())
}

/// Convierte el texto recibido al tipo que ya tenía la variable.
fn convertir(valor: &str, actual: Option<&Valor>) -> Result<Valor, ScpiError> {
    let fallo = || ScpiError::with_detail(ErrorCode::IllegalParameterValue, valor);

    match actual {
        // Un número admite el sufijo de unidad con su multiplicador: la fuente
        // acepta `VOLT 500 MV` y el generador `FREQ 2.4 GHZ`.
        Some(Valor::Float(_) | Valor::Int(_)) => instrusim_scpi::parse_decimal(valor)
            .map(Valor::Float)
            .ok_or_else(fallo),
        Some(Valor::Bool(_)) => booleano(valor).map(Valor::Bool).ok_or_else(fallo),
        // Una variable de texto guarda mnemónicos (`CW`, `LIST`), y ahí `ON` es
        // la palabra «ON», no un booleano. Es la regla del catálogo: nunca un
        // `bool` donde pueda aparecer un mnemónico SCPI.
        Some(Valor::Str(_)) => Ok(Valor::Str(valor.to_string())),
        // Variable que el perfil no declaró: no hay tipo que respetar, así que
        // se infiere. Sigue siendo lo mejor que se puede hacer, pero un perfil
        // que declare su estado no pasa por aquí.
        None => Ok(inferir(valor)),
    }
}

fn booleano(valor: &str) -> Option<bool> {
    match valor.trim().to_ascii_uppercase().as_str() {
        "ON" | "1" | "TRUE" => Some(true),
        "OFF" | "0" | "FALSE" => Some(false),
        _ => None,
    }
}

fn inferir(valor: &str) -> Valor {
    if let Some(f) = instrusim_scpi::parse_decimal(valor) {
        Valor::Float(f)
    } else if let Some(b) = booleano(valor) {
        Valor::Bool(b)
    } else {
        Valor::Str(valor.to_string())
    }
}

fn resolver_expr(expr: &str, args: &HashMap<String, String>) -> String {
    let expr = expr.trim();
    if expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2 {
        let inner = &expr[1..expr.len() - 1];
        if let Some(val) = args.get(inner) {
            return val.clone();
        }
        return inner.to_string();
    }
    if expr.starts_with('<') && expr.ends_with('>') && expr.len() >= 2 {
        let nombre = &expr[1..expr.len() - 1];
        if let Some(val) = args.get(nombre) {
            return val.clone();
        }
    }
    if let Some(val) = args.get(expr) {
        return val.clone();
    }
    expr.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::estado::Valor;

    #[test]
    fn la_plantilla_toma_variables_del_estado() {
        let mut e = Estado::new();
        e.set("v", Valor::Float(5.0));
        assert_eq!(resolver_plantilla("V={v}", &e, &HashMap::new()), "V=5.0");
    }

    #[test]
    fn una_referencia_inexistente_se_queda_literal() {
        assert_eq!(
            resolver_plantilla("{nada}", &Estado::new(), &HashMap::new()),
            "{nada}"
        );
    }

    /// Muta `clave` con el texto `valor`, como haría el comando del perfil.
    fn mutar(estado: &mut Estado, clave: &str, valor: &str) -> Result<(), ScpiError> {
        let args = HashMap::from([("x".to_string(), valor.to_string())]);
        aplicar_mutacion(
            &HashMap::from([(clave.to_string(), "<x>".to_string())]),
            estado,
            &args,
        )
    }

    #[test]
    fn on_y_off_se_guardan_como_booleanos() {
        let mut e = Estado::new();
        e.set("output", Valor::Bool(false));
        mutar(&mut e, "output", "ON").unwrap();
        assert_eq!(e.get_bool("output"), Some(true));
    }

    /// El generador de RF se programa así, y antes esto guardaba 2,4 Hz.
    #[test]
    fn una_variable_numerica_acepta_el_sufijo_de_unidad() {
        let mut e = Estado::new();
        e.set("frecuencia", Valor::Float(0.0));
        mutar(&mut e, "frecuencia", "2.4 GHZ").unwrap();
        assert_eq!(e.get_float("frecuencia"), Some(2.4e9));
    }

    /// `MAX` en una variable numérica es un error de parámetro, no una cadena
    /// que envenena el estado hasta el siguiente `*RST`.
    #[test]
    fn una_variable_numerica_rechaza_lo_que_no_es_un_numero() {
        let mut e = Estado::new();
        e.set("tension", Valor::Float(1.0));
        let err = mutar(&mut e, "tension", "MAX").unwrap_err();
        assert_eq!(err.code, ErrorCode::IllegalParameterValue);
        assert_eq!(
            e.get_float("tension"),
            Some(1.0),
            "el valor anterior debe sobrevivir al comando fallido"
        );
    }

    #[test]
    fn una_variable_booleana_rechaza_lo_que_no_es_booleano() {
        let mut e = Estado::new();
        e.set("salida", Valor::Bool(false));
        assert!(mutar(&mut e, "salida", "MAYBE").is_err());
        assert_eq!(e.get_bool("salida"), Some(false));
    }

    /// Una variable de texto guarda mnemónicos, y ahí `ON` es la palabra «ON».
    #[test]
    fn una_variable_de_texto_guarda_el_mnemonico() {
        let mut e = Estado::new();
        e.set("modo", Valor::Str("CW".into()));
        mutar(&mut e, "modo", "LIST").unwrap();
        assert_eq!(e.get("modo"), Some(&Valor::Str("LIST".into())));
    }

    /// Si una clave de la mutación falla, ninguna se escribe: el instrumento no
    /// puede quedarse medio configurado.
    #[test]
    fn una_mutacion_que_falla_no_escribe_nada() {
        let mut e = Estado::new();
        e.set("a", Valor::Float(1.0));
        e.set("b", Valor::Float(2.0));
        let args = HashMap::from([("x".to_string(), "no_es_un_numero".to_string())]);
        let muta = HashMap::from([
            ("a".to_string(), "7".to_string()),
            ("b".to_string(), "<x>".to_string()),
        ]);
        assert!(aplicar_mutacion(&muta, &mut e, &args).is_err());
        assert_eq!(e.get_float("a"), Some(1.0));
        assert_eq!(e.get_float("b"), Some(2.0));
    }
}
