//! Un dispositivo simulado, montado a partir de su perfil.
//!
//! Es una fachada fina sobre el protocolo que declare el perfil. Hoy solo hay
//! SCPI; cuando entren Modbus y los seriales del ADR-0002, esta es la capa que
//! elige cuál, y la única que hay que tocar.

use crate::error::Result;
use crate::estado::Estado;
use crate::perfil::Perfil;
use crate::protocolo::scpi::DispositivoScpi;

pub struct Dispositivo {
    scpi: DispositivoScpi,
}

impl Dispositivo {
    pub fn from_perfil(perfil: Perfil) -> Result<Self> {
        Ok(Self {
            scpi: crate::protocolo::scpi::desde_perfil(perfil)?,
        })
    }

    /// Procesa una línea recibida y devuelve la respuesta, si la hay.
    ///
    /// Devuelve `None` cuando el mensaje no contenía ninguna consulta —incluido
    /// el caso de un comando erróneo, que se anota en la cola en vez de cortar
    /// la conversación. El cliente lo recupera con `SYSTem:ERRor?`, igual que
    /// en un instrumento real.
    pub fn procesar(&mut self, mensaje: &str) -> Option<String> {
        self.scpi.procesar(mensaje)
    }

    pub fn modelo(&self) -> &str {
        &self.scpi.perfil().dispositivo.modelo
    }

    pub fn estado(&self) -> &Estado {
        &self.scpi.estado
    }

    pub fn estado_mut(&mut self) -> &mut Estado {
        &mut self.scpi.estado
    }

    /// Arranca una copia independiente con el mismo perfil y el mismo estado.
    ///
    /// No se usa para aislar conexiones TCP entre sí: un dispositivo servido
    /// por `crucible` es uno solo, compartido, porque modela un aparato físico
    /// único. Esto es para levantar un segundo dispositivo equivalente en otro
    /// sitio (otro puerto, otro test), no una copia por cliente.
    pub fn clonar(&self) -> Result<Dispositivo> {
        let mut nuevo = Dispositivo::from_perfil(self.scpi.perfil().clone())?;
        *nuevo.estado_mut() = self.scpi.estado.clone();
        Ok(nuevo)
    }
}
