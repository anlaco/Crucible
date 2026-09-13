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
    /// Patrón SCPI en notación del estándar: mayúsculas para la abreviatura,
    /// corchetes para lo omitible. `SOURce:VOLTage[:LEVel]` acepta
    /// `SOUR:VOLT`, `source:voltage:level` y todas las combinaciones legales.
    ///
    /// **No lleva los argumentos ni el `?`**: de eso se encargan `args` y
    /// `query`.
    pub patron: String,

    /// Nombres de los argumentos posicionales, para poder referenciarlos como
    /// `<nombre>` en `muta` y en `respuesta`. Sin esto se usan `<0>`, `<1>`.
    #[serde(default)]
    pub args: Vec<String>,

    /// Si esta entrada responde a la forma consulta (`SOUR:VOLT?`) en vez de a
    /// la orden (`SOUR:VOLT 5`). El mismo patrón puede declararse dos veces,
    /// una de cada.
    #[serde(default)]
    pub query: bool,

    #[serde(default, deserialize_with = "mapa_de_textos")]
    pub muta: Option<HashMap<String, String>>,
    /// Respuesta literal, o plantilla: `{variable}` toma del estado y `<arg>`
    /// del argumento.
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
    #[serde(default, deserialize_with = "mapa_de_textos")]
    pub cuando: Option<HashMap<String, String>>,
    #[serde(default)]
    pub expr: Option<String>,
    #[serde(default, deserialize_with = "texto_opcional")]
    pub fallback: Option<String>,
    /// Cómo se escribe el número de la respuesta: `entero`, `fijo:N` o
    /// `cientifico:N`. Sin él, el formato de siempre (`1.0`, `4.5013…`).
    #[serde(default)]
    pub formato: Option<String>,
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
            if let Some(modelo) = &cmd.modelo
                && !self.modelos.contains_key(modelo)
            {
                return Err(CrucibleError::PerfilInvalido(format!(
                    "comando '{}' referencia modelo '{}' que no existe",
                    cmd.patron, modelo
                )));
            }
            cmd.validar_patron()?;
            if cmd.query && cmd.respuesta.is_none() && cmd.modelo.is_none() {
                return Err(CrucibleError::PerfilInvalido(format!(
                    "la consulta '{}' no tiene 'respuesta' ni 'modelo': no sabría qué contestar",
                    cmd.patron
                )));
            }
        }
        for (nombre, modelo) in &self.modelos {
            if modelo.tipo != "formula" {
                return Err(CrucibleError::PerfilInvalido(format!(
                    "modelo '{nombre}': tipo '{}' no soportado; el único que existe es 'formula'",
                    modelo.tipo
                )));
            }
            let Some(expr) = &modelo.expr else {
                return Err(CrucibleError::PerfilInvalido(format!(
                    "modelo '{nombre}': falta 'expr'"
                )));
            };
            if let Some(f) = &modelo.formato {
                crate::modelo::Formato::parse(f).map_err(|e| {
                    CrucibleError::PerfilInvalido(format!("modelo '{nombre}': {e}"))
                })?;
            }
            crate::modelo::validar_formula(expr)
                .map_err(|e| CrucibleError::PerfilInvalido(format!("modelo '{nombre}': {e}")))?;
        }
        Ok(())
    }
}

impl Comando {
    /// Rechaza los patrones del formato anterior a la unificación del SCPI.
    ///
    /// Antes el patrón era la línea entera —`"*IDN?"`, `"SOUR:VOLT <x>"`— porque
    /// el codec la comparaba tal cual. Ahora el patrón es solo la cabecera, en
    /// notación SCPI, y el resto va en `args` y `query`. Un perfil antiguo
    /// cargaría sin quejarse y luego no reconocería ni un comando, así que
    /// conviene que falle aquí y diga por qué (ADR-0003).
    fn validar_patron(&self) -> Result<()> {
        let p = &self.patron;

        if p.contains('<') {
            return Err(CrucibleError::PerfilInvalido(format!(
                "el patrón '{p}' lleva un argumento incrustado. El formato \
                 cambió: pon solo la cabecera y declara los argumentos en \
                 'args'. Ejemplo: patron: \"SOURce:VOLTage\"  args: [v]"
            )));
        }

        if p.ends_with('?') {
            return Err(CrucibleError::PerfilInvalido(format!(
                "el patrón '{p}' acaba en '?'. El formato cambió: quita el \
                 signo y marca 'query: true'"
            )));
        }

        if p.starts_with('*') {
            return Err(CrucibleError::PerfilInvalido(format!(
                "el patrón '{p}' es un comando común de IEEE 488.2. Ya no se \
                 declaran en el perfil: los resuelve el motor. '*IDN?' sale de \
                 'dispositivo.idn'"
            )));
        }

        if p.trim().is_empty() {
            return Err(CrucibleError::PerfilInvalido("hay un patrón vacío".into()));
        }

        Ok(())
    }
}

impl ValorRaw {
    /// El escalar tal como lo escribiría quien edita el YAML.
    pub fn a_texto(&self) -> String {
        match self {
            ValorRaw::Float(f) => f.to_string(),
            ValorRaw::Int(i) => i.to_string(),
            ValorRaw::Bool(b) => b.to_string(),
            ValorRaw::Str(s) => s.clone(),
        }
    }

    pub fn to_valor(&self) -> crate::estado::Valor {
        match self {
            ValorRaw::Float(f) => crate::estado::Valor::Float(*f),
            ValorRaw::Int(i) => crate::estado::Valor::Float(*i as f64),
            ValorRaw::Bool(b) => crate::estado::Valor::Bool(*b),
            ValorRaw::Str(s) => crate::estado::Valor::Str(s.clone()),
        }
    }
}

/// Acepta cualquier escalar donde el formato guarda texto.
///
/// Quien escribe un perfil pone `fallback: 0.0` o `cuando: { output: true }`
/// sin comillas, y es razonable. Exigir `String` a serde hacía fallar la carga
/// con un «invalid type: floating point» que no dice qué campo corregir.
fn texto_opcional<'de, D>(d: D) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<ValorRaw>::deserialize(d)?.map(|v| v.a_texto()))
}

pub(crate) fn mapa_de_textos<'de, D>(
    d: D,
) -> std::result::Result<Option<HashMap<String, String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<HashMap<String, ValorRaw>>::deserialize(d)?
        .map(|m| m.into_iter().map(|(k, v)| (k, v.a_texto())).collect()))
}
