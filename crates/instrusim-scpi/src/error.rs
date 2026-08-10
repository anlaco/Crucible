//! Errores SCPI y la cola donde se acumulan.
//!
//! SCPI-99 no comunica los fallos abortando: el instrumento acepta el comando,
//! anota el error en una cola y sigue funcionando. El cliente los recupera con
//! `SYSTem:ERRor?`, que devuelve `-113,"Undefined header"` y va vaciando la cola
//! hasta que sale `0,"No error"`.
//!
//! Reproducirlo bien importa más de lo que parece: un cliente decente comprueba
//! la cola después de cada configuración, y un simulador que siempre conteste
//! "no error" deja pasar precisamente los bugs que se quieren cazar.

use std::collections::VecDeque;
use std::fmt;

/// Códigos de error normalizados por SCPI-99, capítulo 21.
///
/// Los negativos están definidos por el estándar; los positivos quedan para
/// errores propios del instrumento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    NoError,
    /// -100: error genérico de comando.
    CommandError,
    /// -102: el mensaje no cumple la sintaxis.
    SyntaxError,
    /// -108: se han pasado parámetros de más.
    ParameterNotAllowed,
    /// -109: falta un parámetro obligatorio.
    MissingParameter,
    /// -113: la cabecera no corresponde a ningún comando conocido.
    UndefinedHeader,
    /// -151: cadena mal formada.
    InvalidStringData,
    /// -200: error genérico de ejecución.
    ExecutionError,
    /// -221: el estado actual impide ejecutar el comando.
    SettingsConflict,
    /// -222: el valor está fuera del rango admitido.
    DataOutOfRange,
    /// -224: el valor no es uno de los admitidos.
    IllegalParameterValue,
    /// -350: se han perdido errores por desbordamiento de la cola.
    QueueOverflow,
    /// -410: el instrumento tenía una respuesta sin leer y se ha perdido.
    QueryInterrupted,
}

impl ErrorCode {
    pub fn code(self) -> i16 {
        match self {
            ErrorCode::NoError => 0,
            ErrorCode::CommandError => -100,
            ErrorCode::SyntaxError => -102,
            ErrorCode::ParameterNotAllowed => -108,
            ErrorCode::MissingParameter => -109,
            ErrorCode::UndefinedHeader => -113,
            ErrorCode::InvalidStringData => -151,
            ErrorCode::ExecutionError => -200,
            ErrorCode::SettingsConflict => -221,
            ErrorCode::DataOutOfRange => -222,
            ErrorCode::IllegalParameterValue => -224,
            ErrorCode::QueueOverflow => -350,
            ErrorCode::QueryInterrupted => -410,
        }
    }

    /// El texto exacto que exige el estándar. Hay clientes que lo comparan.
    pub fn message(self) -> &'static str {
        match self {
            ErrorCode::NoError => "No error",
            ErrorCode::CommandError => "Command error",
            ErrorCode::SyntaxError => "Syntax error",
            ErrorCode::ParameterNotAllowed => "Parameter not allowed",
            ErrorCode::MissingParameter => "Missing parameter",
            ErrorCode::UndefinedHeader => "Undefined header",
            ErrorCode::InvalidStringData => "Invalid string data",
            ErrorCode::ExecutionError => "Execution error",
            ErrorCode::SettingsConflict => "Settings conflict",
            ErrorCode::DataOutOfRange => "Data out of range",
            ErrorCode::IllegalParameterValue => "Illegal parameter value",
            ErrorCode::QueueOverflow => "Queue overflow",
            ErrorCode::QueryInterrupted => "Query INTERRUPTED",
        }
    }

    /// Bit del registro de sucesos estándar (ESR) que este error activa.
    ///
    /// IEEE 488.2 clasifica los errores en familias y cada una enciende su bit:
    /// 5 para errores de comando, 4 de ejecución, 3 propios del instrumento y
    /// 2 de consulta.
    pub fn esr_bit(self) -> u8 {
        match self.code() {
            0 => 0,
            c if (-199..=-100).contains(&c) => 5,
            c if (-299..=-200).contains(&c) => 4,
            c if (-399..=-300).contains(&c) => 3,
            c if (-499..=-400).contains(&c) => 2,
            _ => 3,
        }
    }
}

/// Un error concreto: su código y, opcionalmente, información añadida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScpiError {
    pub code: ErrorCode,
    /// Detalle que se añade al mensaje estándar, entre punto y coma.
    /// Por ejemplo: `-113,"Undefined header;MEAS:FOO"`.
    pub detail: Option<String>,
}

impl ScpiError {
    pub fn new(code: ErrorCode) -> Self {
        Self { code, detail: None }
    }

    pub fn with_detail(code: ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: Some(detail.into()),
        }
    }
}

impl fmt::Display for ScpiError {
    /// El formato exacto de respuesta de `SYSTem:ERRor?`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.detail {
            Some(d) => write!(f, "{},\"{};{}\"", self.code.code(), self.code.message(), d),
            None => write!(f, "{},\"{}\"", self.code.code(), self.code.message()),
        }
    }
}

/// Atajos para los errores más frecuentes.
impl From<ErrorCode> for ScpiError {
    fn from(code: ErrorCode) -> Self {
        ScpiError::new(code)
    }
}

/// Cola de errores del instrumento, primero en entrar primero en salir.
///
/// El estándar exige un mínimo de 2 posiciones y que, al llenarse, el último
/// hueco se reserve para `-350,"Queue overflow"`. Los instrumentos reales
/// suelen guardar entre 10 y 30; aquí 32.
#[derive(Debug)]
pub struct ErrorQueue {
    entries: VecDeque<ScpiError>,
    capacity: usize,
    overflowed: bool,
}

impl Default for ErrorQueue {
    fn default() -> Self {
        Self::new(32)
    }
}

impl ErrorQueue {
    pub fn new(capacity: usize) -> Self {
        assert!(
            capacity >= 2,
            "la cola de errores necesita al menos dos huecos"
        );
        Self {
            entries: VecDeque::new(),
            capacity,
            overflowed: false,
        }
    }

    /// Anota un error. Si la cola está llena, se descarta el nuevo y se marca
    /// el desbordamiento: el estándar prefiere conservar los errores antiguos,
    /// porque son los que explican la causa original del problema.
    pub fn push(&mut self, error: ScpiError) {
        if self.entries.len() + 1 >= self.capacity {
            self.overflowed = true;
            return;
        }
        self.entries.push_back(error);
    }

    /// Extrae el error más antiguo. Devuelve `NoError` cuando no queda ninguno,
    /// que es lo que exige `SYSTem:ERRor?`.
    pub fn pop(&mut self) -> ScpiError {
        if let Some(e) = self.entries.pop_front() {
            return e;
        }
        if self.overflowed {
            self.overflowed = false;
            return ScpiError::new(ErrorCode::QueueOverflow);
        }
        ScpiError::new(ErrorCode::NoError)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && !self.overflowed
    }

    /// Vacía la cola. Es lo que hace `*CLS`.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.overflowed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_formato_de_respuesta_es_el_del_estandar() {
        let e = ScpiError::new(ErrorCode::UndefinedHeader);
        assert_eq!(e.to_string(), "-113,\"Undefined header\"");
    }

    #[test]
    fn el_detalle_va_tras_un_punto_y_coma() {
        let e = ScpiError::with_detail(ErrorCode::UndefinedHeader, "MEAS:FOO");
        assert_eq!(e.to_string(), "-113,\"Undefined header;MEAS:FOO\"");
    }

    #[test]
    fn una_cola_vacia_responde_que_no_hay_error() {
        let mut cola = ErrorQueue::default();
        assert_eq!(cola.pop().code, ErrorCode::NoError);
    }

    #[test]
    fn los_errores_salen_en_el_orden_en_que_entraron() {
        let mut cola = ErrorQueue::default();
        cola.push(ScpiError::new(ErrorCode::UndefinedHeader));
        cola.push(ScpiError::new(ErrorCode::DataOutOfRange));

        assert_eq!(cola.pop().code, ErrorCode::UndefinedHeader);
        assert_eq!(cola.pop().code, ErrorCode::DataOutOfRange);
        assert_eq!(cola.pop().code, ErrorCode::NoError);
    }

    #[test]
    fn al_desbordarse_se_conservan_los_antiguos_y_se_avisa() {
        let mut cola = ErrorQueue::new(3);
        cola.push(ScpiError::new(ErrorCode::CommandError));
        cola.push(ScpiError::new(ErrorCode::SyntaxError));
        // Este ya no cabe.
        cola.push(ScpiError::new(ErrorCode::DataOutOfRange));

        assert_eq!(cola.pop().code, ErrorCode::CommandError);
        assert_eq!(cola.pop().code, ErrorCode::SyntaxError);
        assert_eq!(cola.pop().code, ErrorCode::QueueOverflow);
        assert_eq!(cola.pop().code, ErrorCode::NoError);
    }

    #[test]
    fn cada_familia_de_error_enciende_su_bit_del_registro_de_sucesos() {
        assert_eq!(ErrorCode::UndefinedHeader.esr_bit(), 5); // comando
        assert_eq!(ErrorCode::DataOutOfRange.esr_bit(), 4); // ejecución
        assert_eq!(ErrorCode::QueueOverflow.esr_bit(), 3); // instrumento
        assert_eq!(ErrorCode::QueryInterrupted.esr_bit(), 2); // consulta
    }
}
