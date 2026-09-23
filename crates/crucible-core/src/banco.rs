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
    #[serde(default, deserialize_with = "crate::perfil::mapa_de_textos")]
    pub estado_inicial: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub enum Transporte {
    Tcp {
        host: Option<String>,
        puerto: Option<u16>,
    },
    Vxi11 {
        host: Option<String>,
        puerto: Option<u16>,
        device: Option<String>,
    },
}

impl Transporte {
    pub fn host(&self) -> Option<&str> {
        match self {
            Transporte::Tcp { host, .. } | Transporte::Vxi11 { host, .. } => host.as_deref(),
        }
    }
    pub fn host_or_default(&self) -> &str {
        self.host().unwrap_or("127.0.0.1")
    }
    pub fn puerto(&self) -> Option<u16> {
        match self {
            Transporte::Tcp { puerto, .. } | Transporte::Vxi11 { puerto, .. } => *puerto,
        }
    }
    pub fn is_vxi11(&self) -> bool {
        matches!(self, Transporte::Vxi11 { .. })
    }
    pub fn device_name(&self) -> &str {
        match self {
            Transporte::Vxi11 { device, .. } => device.as_deref().unwrap_or("inst0"),
            Transporte::Tcp { .. } => "",
        }
    }
    /// Resource string VISA.
    pub fn resource_string(&self) -> String {
        let host = self.host_or_default();
        match self {
            Transporte::Tcp { puerto, .. } => {
                format!("TCPIP0::{}::{}::SOCKET", host, puerto.unwrap_or(5025))
            }
            Transporte::Vxi11 {
                puerto: _, device, ..
            } => {
                let dev = device.as_deref().unwrap_or("inst0");
                format!("TCPIP0::{}::{}::INSTR", host, dev)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Transporte {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(rename = "tipo")]
            tipo_raw: String,
            #[serde(default)]
            host: Option<String>,
            #[serde(default)]
            puerto: Option<u16>,
            #[serde(default)]
            device: Option<String>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let t = raw.tipo_raw.to_ascii_lowercase();
        match t.as_str() {
            "tcp" => Ok(Transporte::Tcp {
                host: raw.host,
                puerto: raw.puerto,
            }),
            "vxi11" | "vxi-11" | "vxi_11" => Ok(Transporte::Vxi11 {
                host: raw.host,
                puerto: raw.puerto,
                device: raw.device,
            }),
            other => Err(serde::de::Error::custom(format!(
                "transporte tipo desconocido '{}'; esperaba 'tcp' o 'vxi11'",
                other
            ))),
        }
    }
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
