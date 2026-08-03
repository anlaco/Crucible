use crate::error::{CrucibleError, Result};
use crate::estado::Estado;
use crate::perfil::{Comando, Perfil};
use std::collections::HashMap;

pub mod scpi;

pub trait Protocolo {
    fn procesar(&mut self, mensaje: &str, perfil: &Perfil, estado: &mut Estado) -> Result<String>;
}

pub fn crear_protocolo(tipo: &crate::perfil::ProtocoloTipo) -> Box<dyn Protocolo + Send> {
    match tipo {
        crate::perfil::ProtocoloTipo::Scpi => Box::new(scpi::CodecScpi::new()),
        _ => Box::new(scpi::CodecScpi::new()),
    }
}

pub fn aplicar_mutacion(
    muta: &HashMap<String, String>,
    estado: &mut Estado,
    args: &HashMap<String, String>,
) {
    for (clave, expr) in muta {
        let valor = resolver_expr(expr, args);
        if let Some(f) = valor.parse::<f64>().ok() {
            estado.set(clave, crate::estado::Valor::Float(f));
        } else if let Some(b) = valor.parse::<bool>().ok() {
            estado.set(clave, crate::estado::Valor::Bool(b));
        } else {
            estado.set(clave, crate::estado::Valor::Str(valor));
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

pub fn encontrar_comando<'a>(
    comandos: &'a [Comando],
    mensaje: &str,
) -> Option<(usize, HashMap<String, String>)> {
    scpi::match_comando(comandos, mensaje)
}

pub fn respuesta_comando(
    cmd: &Comando,
    perfil: &Perfil,
    estado: &Estado,
    evaluador: &mut crate::modelo::EvaluadorModelos,
) -> Result<String> {
    if let Some(resp) = &cmd.respuesta {
        return Ok(resp.clone());
    }
    if let Some(modelo_nombre) = &cmd.modelo {
        let modelo = perfil
            .modelos
            .get(modelo_nombre)
            .ok_or_else(|| CrucibleError::ModeloNoEncontrado(modelo_nombre.clone()))?;
        let resultado = evaluador.evaluar(modelo, estado)?;
        return Ok(resultado);
    }
    Ok(String::new())
}