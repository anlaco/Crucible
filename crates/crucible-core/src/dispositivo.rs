use crate::error::Result;
use crate::estado::Estado;
use crate::perfil::Perfil;
use crate::protocolo::{self, Protocolo};

pub struct Dispositivo {
    pub perfil: Perfil,
    pub estado: Estado,
    protocolo: Box<dyn Protocolo + Send>,
}

impl Dispositivo {
    pub fn from_perfil(perfil: Perfil) -> Self {
        let estado = Estado::from_hashmap(&perfil.estado);
        let proto = protocolo::crear_protocolo(&perfil.protocolo);
        Self {
            perfil,
            estado,
            protocolo: proto,
        }
    }

    pub fn procesar(&mut self, mensaje: &str) -> Result<String> {
        self.protocolo
            .procesar(mensaje, &self.perfil, &mut self.estado)
    }

    pub fn modelo(&self) -> &str {
        &self.perfil.dispositivo.modelo
    }

    pub fn clonar(&self) -> Dispositivo {
        Dispositivo {
            perfil: self.perfil.clone(),
            estado: self.estado.clone(),
            protocolo: protocolo::crear_protocolo(&self.perfil.protocolo),
        }
    }
}