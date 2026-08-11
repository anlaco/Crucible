//! La capa de protocolo: cómo se habla con un dispositivo.
//!
//! El perfil describe *qué* hace el dispositivo; el protocolo, *cómo* se le
//! dice. Hoy solo está implementado SCPI; Modbus y los seriales a medida son el
//! siguiente paso del ADR-0002, y por eso el despacho está detrás de esta
//! frontera en vez de suponer SCPI en todas partes.

use crate::estado::Estado;
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
pub fn aplicar_mutacion(
    muta: &HashMap<String, String>,
    estado: &mut Estado,
    args: &HashMap<String, String>,
) {
    for (clave, expr) in muta {
        let valor = resolver_expr(expr, args);
        // `ON`/`OFF` llegan así desde SCPI y son booleanos, no texto.
        let valor_norm = match valor.to_ascii_uppercase().as_str() {
            "ON" => "true".to_string(),
            "OFF" => "false".to_string(),
            _ => valor,
        };
        if let Ok(f) = valor_norm.parse::<f64>() {
            estado.set(clave, crate::estado::Valor::Float(f));
        } else if let Ok(b) = valor_norm.parse::<bool>() {
            estado.set(clave, crate::estado::Valor::Bool(b));
        } else {
            estado.set(clave, crate::estado::Valor::Str(valor_norm));
        }
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

    #[test]
    fn on_y_off_se_guardan_como_booleanos() {
        let mut e = Estado::new();
        let args = HashMap::from([("x".to_string(), "ON".to_string())]);
        aplicar_mutacion(
            &HashMap::from([("output".to_string(), "<x>".to_string())]),
            &mut e,
            &args,
        );
        assert_eq!(e.get_bool("output"), Some(true));
    }
}
