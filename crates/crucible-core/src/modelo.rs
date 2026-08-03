use crate::error::{CrucibleError, Result};
use crate::estado::Estado;
use crate::perfil::ModeloDef;
use std::collections::HashMap;

pub struct EvaluadorModelos {
    semilla: u64,
}

impl EvaluadorModelos {
    pub fn con_semilla(semilla: u64) -> Self {
        Self { semilla }
    }

    pub fn evaluar(&mut self, modelo: &ModeloDef, estado: &Estado) -> Result<String> {
        if let Some(cuando) = &modelo.cuando {
            if !evaluar_guarda(cuando, estado) {
                if let Some(fb) = &modelo.fallback {
                    return Ok(evaluar_fallback(fb));
                }
                return Ok("0.0".into());
            }
        }

        match modelo.tipo.as_str() {
            "formula" => {
                let expr = modelo
                    .expr
                    .as_ref()
                    .ok_or_else(|| CrucibleError::Evaluacion("modelo formula sin expr".into()))?;
                let val = evaluar_formula(expr, estado, self.semilla)?;
                Ok(format_resultado(val))
            }
            _ => Err(CrucibleError::Evaluacion(format!(
                "tipo de modelo no soportado: {}",
                modelo.tipo
            ))),
        }
    }
}

fn evaluar_guarda(guarda: &HashMap<String, String>, estado: &Estado) -> bool {
    for (clave, esperado) in guarda {
        let val = estado.get(clave);
        let actual = match val {
            Some(v) => v.as_str(),
            None => return false,
        };
        if !actual.eq_ignore_ascii_case(esperado.trim_matches('"')) {
            return false;
        }
    }
    true
}

fn evaluar_fallback(fb: &str) -> String {
    let fb = fb.trim_matches('"');
    if let Ok(f) = fb.parse::<f64>() {
        format_resultado(f)
    } else {
        fb.to_string()
    }
}

fn evaluar_formula(expr: &str, estado: &Estado, semilla: u64) -> Result<f64> {
    let expr = expr.trim();
    let val = evaluar_expr(expr, estado, semilla)?;
    Ok(val)
}

fn evaluar_expr(expr: &str, estado: &Estado, semilla: u64) -> Result<f64> {
    let expr = expr.trim();

    if let Some(pos) = encontrar_operador(expr, '+') {
        if pos > 0 {
            let izq = evaluar_expr(&expr[..pos], estado, semilla)?;
            let der = evaluar_expr(&expr[pos + 1..], estado, semilla)?;
            return Ok(izq + der);
        }
    }
    if let Some(pos) = encontrar_operador(expr, '-') {
        if pos > 0 {
            let izq = evaluar_expr(&expr[..pos], estado, semilla)?;
            let der = evaluar_expr(&expr[pos + 1..], estado, semilla)?;
            return Ok(izq - der);
        }
    }
    if let Some(pos) = encontrar_operador(expr, '*') {
        let izq = evaluar_expr(&expr[..pos], estado, semilla)?;
        let der = evaluar_expr(&expr[pos + 1..], estado, semilla)?;
        return Ok(izq * der);
    }
    if let Some(pos) = encontrar_operador(expr, '/') {
        let izq = evaluar_expr(&expr[..pos], estado, semilla)?;
        let der = evaluar_expr(&expr[pos + 1..], estado, semilla)?;
        if der == 0.0 {
            return Err(CrucibleError::Evaluacion("división por cero".into()));
        }
        return Ok(izq / der);
    }

    if expr.starts_with("gauss(") && expr.ends_with(')') {
        let args = &expr[6..expr.len() - 1];
        let parts: Vec<&str> = args.split(',').collect();
        if parts.len() != 2 {
            return Err(CrucibleError::Evaluacion("gauss necesita 2 args".into()));
        }
        let mu: f64 = parts[0].trim().parse().map_err(|_| {
            CrucibleError::Evaluacion(format!("gauss: mu inválido: {}", parts[0]))
        })?;
        let sigma: f64 = parts[1].trim().parse().map_err(|_| {
            CrucibleError::Evaluacion(format!("gauss: sigma inválido: {}", parts[1]))
        })?;
        let noise = pseudo_gauss(semilla);
        return Ok(mu + sigma * noise);
    }

    if let Ok(lit) = expr.parse::<f64>() {
        return Ok(lit);
    }

    let nombre = expr.trim_matches('"');
    if let Some(v) = estado.get_float(nombre) {
        return Ok(v);
    }

    Err(CrucibleError::Evaluacion(format!(
        "no puedo evaluar: {}",
        expr
    )))
}

fn encontrar_operador(expr: &str, op: char) -> Option<usize> {
    let mut depth = 0i32;
    for (i, c) in expr.char_indices().rev() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 && c == op => return Some(i),
            _ => {}
        }
    }
    None
}

fn pseudo_gauss(semilla: u64) -> f64 {
    let mut s = semilla.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    s ^= s >> 29;
    s = s.wrapping_mul(0x9E3779B97F4A7C15);
    s ^= s >> 32;
    let u = (s >> 11) as f64 / (1u64 << 53) as f64;
    let v = ((s >> 40) & 0x7FF) as f64 / 2048.0;
    let two_pi = 2.0 * std::f64::consts::PI;
    let r = (-2.0 * (1.0 - u).ln()).sqrt();
    r * (two_pi * v).cos()
}

fn format_resultado(f: f64) -> String {
    if f == f.trunc() && f.abs() < 1e16 {
        format!("{:.1}", f)
    } else {
        format!("{}", f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evalua_literal() {
        let estado = Estado::new();
        let r = evaluar_expr("5.0", &estado, 42).unwrap();
        assert!((r - 5.0).abs() < 1e-9);
    }

    #[test]
    fn evalua_variable() {
        let mut estado = Estado::new();
        estado.set("voltaje", crate::estado::Valor::Float(3.5));
        let r = evaluar_expr("voltaje", &estado, 42).unwrap();
        assert!((r - 3.5).abs() < 1e-9);
    }

    #[test]
    fn evalua_suma() {
        let mut estado = Estado::new();
        estado.set("v", crate::estado::Valor::Float(5.0));
        let r = evaluar_expr("v + 1.0", &estado, 42).unwrap();
        assert!((r - 6.0).abs() < 1e-9);
    }

    #[test]
    fn evalua_division() {
        let mut estado = Estado::new();
        estado.set("v", crate::estado::Valor::Float(5.0));
        let r = evaluar_expr("v / 1000.0", &estado, 42).unwrap();
        assert!((r - 0.005).abs() < 1e-9);
    }

    #[test]
    fn gauss_determinista() {
        let estado = Estado::new();
        let a = evaluar_expr("gauss(0, 1)", &estado, 42).unwrap();
        let b = evaluar_expr("gauss(0, 1)", &estado, 42).unwrap();
        assert!((a - b).abs() < 1e-9, "gauss debe ser determinista con misma semilla");
    }
}