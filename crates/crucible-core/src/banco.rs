use crate::dispositivo::Dispositivo;
use crate::perfil::Perfil;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Banco {
    #[serde(default)]
    pub banco: BancoDef,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BancoDef {
    #[serde(default)]
    pub nombre: String,
    #[serde(default)]
    pub dispositivos: Vec<InstanciaDispositivo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstanciaDispositivo {
    pub id: String,
    pub perfil: String,
    pub transporte: Transporte,
    #[serde(default)]
    pub estado_inicial: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Transporte {
    pub tipo: String,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub puerto: Option<u16>,
}

impl Banco {
    pub fn from_yaml(text: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(text)
    }

    /// Carga los dispositivos del banco.
    ///
    /// Un perfil que no cargue tumba **el banco entero**, en vez de arrancar a
    /// medias: un banco al que le falta la fuente no es un banco degradado, es
    /// un banco que va a dar resultados falsos. Antes se ignoraba en silencio.
    pub fn cargar_dispositivos(
        &self,
        base_dir: &std::path::Path,
    ) -> crate::error::Result<Vec<(InstanciaDispositivo, Dispositivo)>> {
        let mut out = Vec::new();
        for inst in &self.banco.dispositivos {
            let path = base_dir.join(&inst.perfil);
            let perfil = Perfil::from_file(&path).map_err(|e| {
                crate::error::CrucibleError::PerfilInvalido(format!(
                    "dispositivo '{}' ({}): {e}",
                    inst.id,
                    path.display()
                ))
            })?;
            let mut disp = Dispositivo::from_perfil(perfil)?;
            if let Some(overrides) = &inst.estado_inicial {
                let estado = disp.estado_mut();
                for (k, v) in overrides {
                    if let Ok(f) = v.parse::<f64>() {
                        estado.set(k, crate::estado::Valor::Float(f));
                    } else if let Ok(b) = v.parse::<bool>() {
                        estado.set(k, crate::estado::Valor::Bool(b));
                    } else {
                        estado.set(k, crate::estado::Valor::Str(v.clone()));
                    }
                }
            }
            out.push((inst.clone(), disp));
        }
        Ok(out)
    }
}
