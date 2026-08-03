use crate::error::{CrucibleError, Result};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Perfil {
    pub dispositivo: DispositivoInfo,
    pub protocolo: ProtocoloTipo,
    #[serde(default)]
    pub estado: HashMap<String, ValorRaw>,
    #[serde(default)]
    pub comandos: Vec<Comando>,
    #[serde(default)]
    pub registros: Option<RegistrosModbus>,
    #[serde(default)]
    pub modelos: HashMap<String, ModeloDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DispositivoInfo {
    pub modelo: String,
    #[serde(default)]
    pub tipo: Option<String>,
    #[serde(default)]
    pub idn: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProtocoloTipo {
    Scpi,
    ModbusRtu,
    ModbusTcp,
    SerialAscii,
    SerialBinario,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Comando {
    pub patron: String,
    #[serde(default)]
    pub muta: Option<HashMap<String, String>>,
    #[serde(default)]
    pub respuesta: Option<String>,
    #[serde(default)]
    pub modelo: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegistrosModbus {
    #[serde(default)]
    pub holding: Vec<RegistroModbus>,
    #[serde(default)]
    pub input: Vec<RegistroModbus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegistroModbus {
    pub direccion: u16,
    #[serde(default)]
    pub muta: Option<HashMap<String, String>>,
    #[serde(default)]
    pub modelo: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModeloDef {
    #[serde(rename = "tipo")]
    pub tipo: String,
    #[serde(default)]
    pub cuando: Option<HashMap<String, String>>,
    #[serde(default)]
    pub expr: Option<String>,
    #[serde(default)]
    pub fallback: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ValorRaw {
    Float(f64),
    Int(i64),
    Bool(bool),
    Str(String),
}

impl Perfil {
    pub fn from_yaml(text: &str) -> Result<Self> {
        let perfil: Perfil = serde_yaml::from_str(text)?;
        perfil.validar()?;
        Ok(perfil)
    }

    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::from_yaml(&text)
    }

    fn validar(&self) -> Result<()> {
        if self.dispositivo.modelo.is_empty() {
            return Err(CrucibleError::PerfilInvalido(
                "dispositivo.modelo no puede estar vacío".into(),
            ));
        }
        for cmd in &self.comandos {
            if let Some(modelo) = &cmd.modelo {
                if !self.modelos.contains_key(modelo) {
                    return Err(CrucibleError::PerfilInvalido(format!(
                        "comando '{}' referencia modelo '{}' que no existe",
                        cmd.patron, modelo
                    )));
                }
            }
        }
        Ok(())
    }
}

impl ValorRaw {
    pub fn to_valor(&self) -> crate::estado::Valor {
        match self {
            ValorRaw::Float(f) => crate::estado::Valor::Float(*f),
            ValorRaw::Int(i) => crate::estado::Valor::Float(*i as f64),
            ValorRaw::Bool(b) => crate::estado::Valor::Bool(*b),
            ValorRaw::Str(s) => crate::estado::Valor::Str(s.clone()),
        }
    }
}