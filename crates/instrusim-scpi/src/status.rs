//! Registros de estado de IEEE 488.2.
//!
//! Es la parte que los simuladores caseros se saltan y que hace que fallen con
//! clientes serios. La norma define una jerarquía de registros que el
//! instrumento va encendiendo y que el cliente consulta para saber si algo fue
//! mal sin tener que preguntar por la cola de errores en cada paso.
//!
//! El registro de sucesos estándar (ESR) acumula qué familias de problema han
//! ocurrido. El byte de estado (STB) resume el conjunto: su bit 5 se enciende si
//! algún suceso *habilitado* del ESR está activo, y su bit 6 se enciende si
//! cualquier resumen habilitado está activo. Esa cascada es la que permite a un
//! cliente comprobar un solo byte y saber si merece la pena mirar más.

/// Bits del registro de sucesos estándar (ESR).
pub mod esr {
    /// Operación completada, respuesta a `*OPC`.
    pub const OPC: u8 = 0;
    /// Error de consulta: se pidió una respuesta que no existe.
    pub const QUERY_ERROR: u8 = 2;
    /// Error propio del instrumento.
    pub const DEVICE_ERROR: u8 = 3;
    /// Error de ejecución: el comando era válido pero no se pudo cumplir.
    pub const EXECUTION_ERROR: u8 = 4;
    /// Error de comando: el mensaje no se entendió.
    pub const COMMAND_ERROR: u8 = 5;
    /// El instrumento se ha encendido desde la última lectura.
    pub const POWER_ON: u8 = 7;
}

/// Bits del byte de estado (STB).
pub mod stb {
    /// Hay errores en la cola.
    pub const ERROR_QUEUE: u8 = 2;
    /// Hay una respuesta esperando a ser leída.
    pub const MESSAGE_AVAILABLE: u8 = 4;
    /// Resumen del registro de sucesos estándar.
    pub const EVENT_SUMMARY: u8 = 5;
    /// Resumen general: alguna de las condiciones habilitadas está activa.
    pub const MASTER_SUMMARY: u8 = 6;
}

/// Los registros de estado de un instrumento.
#[derive(Debug, Default)]
pub struct StatusModel {
    /// Sucesos ocurridos. Se limpia al leerlo.
    event: u8,
    /// Qué sucesos se propagan al resumen. Lo fija el cliente con `*ESE`.
    event_enable: u8,
    /// Qué resúmenes provocan petición de servicio. Lo fija `*SRE`.
    service_enable: u8,
}

impl StatusModel {
    pub fn new() -> Self {
        let mut s = Self::default();
        // La norma exige que el instrumento arranque marcando el encendido.
        s.set_event(esr::POWER_ON);
        s
    }

    /// Enciende un bit del registro de sucesos.
    pub fn set_event(&mut self, bit: u8) {
        self.event |= 1 << bit;
    }

    /// Lee el registro de sucesos y lo limpia, que es lo que hace `*ESR?`.
    ///
    /// La lectura destructiva es deliberada en la norma: garantiza que cada
    /// suceso se comunica una sola vez y que dos clientes no se pisan.
    pub fn read_event(&mut self) -> u8 {
        std::mem::take(&mut self.event)
    }

    pub fn event_enable(&self) -> u8 {
        self.event_enable
    }

    pub fn set_event_enable(&mut self, mask: u8) {
        self.event_enable = mask;
    }

    pub fn service_enable(&self) -> u8 {
        self.service_enable
    }

    pub fn set_service_enable(&mut self, mask: u8) {
        self.service_enable = mask;
    }

    /// El byte de estado, respuesta a `*STB?`.
    ///
    /// Se calcula al vuelo a partir del resto, en vez de guardarse: así es
    /// imposible que quede desincronizado, que es el fallo clásico de las
    /// implementaciones que lo mantienen a mano.
    pub fn status_byte(&self, hay_errores: bool, hay_respuesta: bool) -> u8 {
        let mut stb = 0u8;

        if hay_errores {
            stb |= 1 << stb::ERROR_QUEUE;
        }
        if hay_respuesta {
            stb |= 1 << stb::MESSAGE_AVAILABLE;
        }
        // Bit 5: algún suceso habilitado está activo.
        if self.event & self.event_enable != 0 {
            stb |= 1 << stb::EVENT_SUMMARY;
        }
        // Bit 6: alguno de los bits anteriores está además habilitado para
        // pedir servicio. Es el resumen del resumen.
        if stb & self.service_enable != 0 {
            stb |= 1 << stb::MASTER_SUMMARY;
        }

        stb
    }

    /// `*CLS`: borra los sucesos, pero no las máscaras de habilitación.
    pub fn clear(&mut self) {
        self.event = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn al_arrancar_se_marca_el_encendido() {
        let mut s = StatusModel::new();
        assert_eq!(s.read_event() & (1 << esr::POWER_ON), 1 << esr::POWER_ON);
    }

    #[test]
    fn leer_los_sucesos_los_borra() {
        let mut s = StatusModel::new();
        s.read_event(); // consumir el bit de encendido

        s.set_event(esr::COMMAND_ERROR);
        assert_eq!(s.read_event(), 1 << esr::COMMAND_ERROR);
        assert_eq!(s.read_event(), 0, "la segunda lectura debe salir limpia");
    }

    #[test]
    fn los_sucesos_se_acumulan_hasta_que_se_leen() {
        let mut s = StatusModel::new();
        s.read_event();

        s.set_event(esr::COMMAND_ERROR);
        s.set_event(esr::EXECUTION_ERROR);

        let esperado = (1 << esr::COMMAND_ERROR) | (1 << esr::EXECUTION_ERROR);
        assert_eq!(s.read_event(), esperado);
    }

    #[test]
    fn un_suceso_no_habilitado_no_llega_al_resumen() {
        let mut s = StatusModel::new();
        s.read_event();

        s.set_event(esr::COMMAND_ERROR);
        // Sin habilitar nada, el bit de resumen sigue apagado.
        assert_eq!(s.status_byte(false, false) & (1 << stb::EVENT_SUMMARY), 0);
    }

    #[test]
    fn un_suceso_habilitado_enciende_el_resumen() {
        let mut s = StatusModel::new();
        s.read_event();

        s.set_event_enable(1 << esr::COMMAND_ERROR);
        s.set_event(esr::COMMAND_ERROR);

        let stb = s.status_byte(false, false);
        assert_ne!(stb & (1 << stb::EVENT_SUMMARY), 0);
    }

    /// La cascada completa: un error de comando habilitado en el ESR enciende
    /// el bit 5 del STB, y si además está habilitado en el SRE enciende el 6.
    #[test]
    fn la_cascada_de_resumenes_llega_hasta_el_bit_maestro() {
        let mut s = StatusModel::new();
        s.read_event();

        s.set_event_enable(1 << esr::COMMAND_ERROR);
        s.set_service_enable(1 << stb::EVENT_SUMMARY);
        s.set_event(esr::COMMAND_ERROR);

        let stb = s.status_byte(false, false);
        assert_ne!(stb & (1 << stb::EVENT_SUMMARY), 0, "bit 5");
        assert_ne!(stb & (1 << stb::MASTER_SUMMARY), 0, "bit 6");
    }

    #[test]
    fn la_cola_de_errores_y_la_respuesta_pendiente_tienen_su_bit() {
        let s = StatusModel::new();
        assert_ne!(s.status_byte(true, false) & (1 << stb::ERROR_QUEUE), 0);
        assert_ne!(
            s.status_byte(false, true) & (1 << stb::MESSAGE_AVAILABLE),
            0
        );
    }

    #[test]
    fn cls_borra_los_sucesos_pero_respeta_las_mascaras() {
        let mut s = StatusModel::new();
        s.set_event_enable(0xFF);
        s.set_event(esr::COMMAND_ERROR);

        s.clear();

        assert_eq!(s.read_event(), 0);
        assert_eq!(s.event_enable(), 0xFF, "*CLS no debe tocar *ESE");
    }
}
