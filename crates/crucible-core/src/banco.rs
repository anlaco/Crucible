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

    pub fn cargar_dispositivos(&self, base_dir: &std::path::Path) -> Vec<(InstanciaDispositivo, Dispositivo)> {
        let mut out = Vec::new();
        for inst in &self.banco.dispositivos {
            let path = base_dir.join(&inst.perfil);
            if let Ok(perfil) = Perfil::from_file(&path) {
                let mut disp = Dispositivo::from_perfil(perfil);
                if let Some(overrides) = &inst.estado_inicial {
                    for (k, v) in overrides {
                        if let Ok(f) = v.parse::<f64>() {
                            disp.estado.set(k, crate::estado::Valor::Float(f));
                        } else if let Ok(b) = v.parse::<bool>() {
                            disp.estado.set(k, crate::estado::Valor::Bool(b));
                        } else {
                            disp.estado.set(k, crate::estado::Valor::Str(v.clone()));
                        }
                    }
                }
                out.push((inst.clone(), disp));
            }
        }
        out
    }
}