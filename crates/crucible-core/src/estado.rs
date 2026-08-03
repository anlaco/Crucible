use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Float(f64),
    Int(i64),
    Bool(bool),
    Str(String),
}

impl Valor {
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Valor::Float(f) => Some(*f),
            Valor::Int(i) => Some(*i as f64),
            Valor::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Valor::Str(s) => s.parse().ok(),
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Valor::Bool(b) => Some(*b),
            Valor::Float(f) => Some(*f != 0.0),
            Valor::Int(i) => Some(*i != 0),
            Valor::Str(s) => {
                if s.eq_ignore_ascii_case("true") || s == "1" {
                    Some(true)
                } else if s.eq_ignore_ascii_case("false") || s == "0" {
                    Some(false)
                } else {
                    None
                }
            }
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            Valor::Float(f) => format_float(*f),
            Valor::Int(i) => i.to_string(),
            Valor::Bool(b) => b.to_string(),
            Valor::Str(s) => s.clone(),
        }
    }
}

fn format_float(f: f64) -> String {
    if f == f.trunc() && f.abs() < 1e16 {
        format!("{:.1}", f)
    } else {
        format!("{}", f)
    }
}

#[derive(Debug, Clone)]
pub struct Estado {
    vars: HashMap<String, Valor>,
}

impl Estado {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    pub fn from_hashmap(map: &HashMap<String, crate::perfil::ValorRaw>) -> Self {
        let mut estado = Self::new();
        for (k, v) in map {
            estado.set(k, v.to_valor());
        }
        estado
    }

    pub fn get(&self, nombre: &str) -> Option<&Valor> {
        self.vars.get(nombre)
    }

    pub fn set(&mut self, nombre: &str, valor: Valor) {
        self.vars.insert(nombre.to_string(), valor);
    }

    pub fn get_float(&self, nombre: &str) -> Option<f64> {
        self.vars.get(nombre).and_then(|v| v.as_float())
    }

    pub fn get_bool(&self, nombre: &str) -> Option<bool> {
        self.vars.get(nombre).and_then(|v| v.as_bool())
    }
}

impl Default for Estado {
    fn default() -> Self {
        Self::new()
    }
}