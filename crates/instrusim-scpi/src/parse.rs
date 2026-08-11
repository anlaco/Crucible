//! Análisis de un mensaje SCPI recibido por la línea.
//!
//! Un mensaje es una línea de texto que puede contener varios comandos
//! separados por punto y coma:
//!
//! ```text
//! *RST;:CONF:VOLT:DC 10;:SENS:VOLT:DC:NPLC 1;:READ?
//! ```
//!
//! Y tiene una regla que sorprende a todo el mundo la primera vez: si un
//! comando encadenado **no** empieza por dos puntos, su cabecera se interpreta
//! *relativa al nivel del comando anterior*. Así, `VOLT:DC 10;RANG 100` quiere
//! decir `VOLT:DC 10` seguido de `VOLT:RANG 100`, no de `RANG 100`. Los
//! clientes reales usan esa forma, así que hay que soportarla.

use crate::error::{ErrorCode, ScpiError};

/// Un comando ya separado del mensaje y listo para despachar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// Cabecera absoluta, sin dos puntos iniciales y en mayúsculas.
    pub header: String,
    /// Si acaba en `?` y por tanto espera respuesta.
    pub query: bool,
    /// Argumentos, ya separados por comas y sin espacios sobrantes.
    pub args: Vec<String>,
}

impl Command {
    /// ¿Es un comando común de IEEE 488.2, de los que empiezan por asterisco?
    pub fn is_common(&self) -> bool {
        self.header.starts_with('*')
    }

    /// El argumento en la posición pedida, o error de parámetro ausente.
    pub fn arg(&self, i: usize) -> Result<&str, ScpiError> {
        self.args
            .get(i)
            .map(|s| s.as_str())
            .ok_or_else(|| ScpiError::new(ErrorCode::MissingParameter))
    }

    /// Un argumento numérico.
    ///
    /// Acepta también las palabras clave que SCPI admite en lugar de un número:
    /// `MINimum`, `MAXimum` y `DEFault`, que se devuelven como [`Numeric`].
    pub fn numeric(&self, i: usize) -> Result<Numeric, ScpiError> {
        let raw = self.arg(i)?;
        let up = raw.to_ascii_uppercase();

        if "MINIMUM".starts_with(&up) && up.starts_with("MIN") {
            return Ok(Numeric::Min);
        }
        if "MAXIMUM".starts_with(&up) && up.starts_with("MAX") {
            return Ok(Numeric::Max);
        }
        if "DEFAULT".starts_with(&up) && up.starts_with("DEF") {
            return Ok(Numeric::Default);
        }

        // Los instrumentos aceptan sufijos de unidad: "10 V", "1e-3A".
        let sin_unidad: String = raw
            .chars()
            .take_while(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E'))
            .collect();

        sin_unidad
            .parse::<f64>()
            .map(Numeric::Value)
            .map_err(|_| ScpiError::with_detail(ErrorCode::IllegalParameterValue, raw))
    }

    /// Un argumento booleano: `ON`, `OFF`, `1`, `0`.
    pub fn boolean(&self, i: usize) -> Result<bool, ScpiError> {
        let raw = self.arg(i)?;
        match raw.to_ascii_uppercase().as_str() {
            "ON" | "1" => Ok(true),
            "OFF" | "0" => Ok(false),
            _ => Err(ScpiError::with_detail(
                ErrorCode::IllegalParameterValue,
                raw,
            )),
        }
    }

    /// Rechaza argumentos de más. Un comando que no los espera debe protestar,
    /// no ignorarlos en silencio.
    pub fn expect_no_args(&self) -> Result<(), ScpiError> {
        if self.args.is_empty() {
            Ok(())
        } else {
            Err(ScpiError::new(ErrorCode::ParameterNotAllowed))
        }
    }
}

/// Un parámetro numérico, que puede ser un valor o una palabra clave.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Numeric {
    Value(f64),
    Min,
    Max,
    Default,
}

impl Numeric {
    /// Resuelve la palabra clave contra los límites del parámetro concreto.
    pub fn resolve(self, min: f64, max: f64, default: f64) -> f64 {
        match self {
            Numeric::Value(v) => v,
            Numeric::Min => min,
            Numeric::Max => max,
            Numeric::Default => default,
        }
    }
}

/// Trocea una línea recibida en los comandos que contiene.
///
/// Devuelve error solo si la sintaxis es irrecuperable; que una cabecera no
/// exista se detecta después, al despachar.
pub fn parse_message(line: &str) -> Result<Vec<Command>, ScpiError> {
    let mut comandos = Vec::new();
    // Nivel del árbol en el que estamos, para las cabeceras relativas.
    let mut contexto: Vec<String> = Vec::new();

    for trozo in split_top_level(line, ';') {
        let trozo = trozo.trim();
        if trozo.is_empty() {
            continue;
        }

        // Separar cabecera de argumentos por el primer espacio en blanco.
        let (cabecera_bruta, args_brutos) = match trozo.find(char::is_whitespace) {
            Some(i) => (&trozo[..i], trozo[i..].trim()),
            None => (trozo, ""),
        };

        let query = cabecera_bruta.ends_with('?');
        let cabecera_bruta = cabecera_bruta.trim_end_matches('?');

        if cabecera_bruta.is_empty() {
            return Err(ScpiError::with_detail(ErrorCode::SyntaxError, trozo));
        }

        let header = if cabecera_bruta.starts_with('*') {
            // Los comandos comunes no participan del árbol ni lo alteran.
            cabecera_bruta.to_ascii_uppercase()
        } else if let Some(absoluta) = cabecera_bruta.strip_prefix(':') {
            contexto = niveles_superiores(absoluta);
            absoluta.to_ascii_uppercase()
        } else if contexto.is_empty() {
            contexto = niveles_superiores(cabecera_bruta);
            cabecera_bruta.to_ascii_uppercase()
        } else {
            // Cabecera relativa: cuelga del nivel del comando anterior.
            let mut completa = contexto.join(":");
            completa.push(':');
            completa.push_str(cabecera_bruta);
            let completa = completa.to_ascii_uppercase();
            contexto = niveles_superiores(&completa);
            completa
        };

        let args = if args_brutos.is_empty() {
            Vec::new()
        } else {
            split_top_level(args_brutos, ',')
                .into_iter()
                .map(|a| a.trim().to_string())
                .collect()
        };

        comandos.push(Command {
            header,
            query,
            args,
        });
    }

    Ok(comandos)
}

/// Todos los mnemónicos de una cabecera menos el último: el nivel del árbol en
/// el que quedan los comandos encadenados que vengan después.
fn niveles_superiores(header: &str) -> Vec<String> {
    let mut partes: Vec<String> = header
        .trim_start_matches(':')
        .split(':')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_uppercase())
        .collect();
    partes.pop();
    partes
}

/// Trocea respetando las comillas, para que un punto y coma o una coma dentro
/// de una cadena no partan el mensaje por la mitad.
fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let mut trozos = Vec::new();
    let mut inicio = 0;
    let mut comilla: Option<char> = None;

    for (i, c) in s.char_indices() {
        match comilla {
            Some(q) if c == q => comilla = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => comilla = Some(c),
            None if c == sep => {
                trozos.push(&s[inicio..i]);
                inicio = i + c.len_utf8();
            }
            None => {}
        }
    }
    trozos.push(&s[inicio..]);
    trozos
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uno(line: &str) -> Command {
        let mut cmds = parse_message(line).expect("debería analizarse");
        assert_eq!(cmds.len(), 1, "se esperaba un único comando");
        cmds.remove(0)
    }

    #[test]
    fn un_comando_simple() {
        let c = uno("*IDN?");
        assert_eq!(c.header, "*IDN");
        assert!(c.query);
        assert!(c.args.is_empty());
        assert!(c.is_common());
    }

    #[test]
    fn la_cabecera_se_normaliza_a_mayusculas() {
        assert_eq!(uno("meas:volt:dc?").header, "MEAS:VOLT:DC");
    }

    #[test]
    fn los_dos_puntos_iniciales_se_descartan() {
        assert_eq!(uno(":CONF:VOLT:DC 10").header, "CONF:VOLT:DC");
    }

    #[test]
    fn los_argumentos_se_separan_por_comas() {
        let c = uno("CONF:VOLT:DC 10,0.001");
        assert_eq!(c.args, vec!["10", "0.001"]);
        assert!(!c.query);
    }

    #[test]
    fn varios_comandos_en_una_linea() {
        let cmds = parse_message("*CLS;*IDN?").unwrap();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].header, "*CLS");
        assert_eq!(cmds[1].header, "*IDN");
    }

    /// La regla que más despista de SCPI: una cabecera encadenada sin dos
    /// puntos iniciales cuelga del nivel de la anterior.
    #[test]
    fn las_cabeceras_encadenadas_son_relativas() {
        let cmds = parse_message("SENS:VOLT:DC:NPLC 1;RANG 10").unwrap();
        assert_eq!(cmds[0].header, "SENS:VOLT:DC:NPLC");
        assert_eq!(cmds[1].header, "SENS:VOLT:DC:RANG");
    }

    #[test]
    fn los_dos_puntos_iniciales_vuelven_a_la_raiz() {
        let cmds = parse_message("SENS:VOLT:DC:NPLC 1;:OUTP ON").unwrap();
        assert_eq!(cmds[1].header, "OUTP");
    }

    #[test]
    fn los_comandos_comunes_no_alteran_el_contexto() {
        let cmds = parse_message("SENS:VOLT:DC:NPLC 1;*CLS;RANG 10").unwrap();
        assert_eq!(cmds[2].header, "SENS:VOLT:DC:RANG");
    }

    #[test]
    fn un_punto_y_coma_entre_comillas_no_parte_el_mensaje() {
        let c = uno("SYST:COMM:TEXT \"hola;adios\"");
        assert_eq!(c.args, vec!["\"hola;adios\""]);
    }

    #[test]
    fn los_numeros_admiten_sufijo_de_unidad() {
        assert_eq!(uno("VOLT 10 V").numeric(0).unwrap(), Numeric::Value(10.0));
        assert_eq!(uno("VOLT 1e-3").numeric(0).unwrap(), Numeric::Value(1e-3));
    }

    #[test]
    fn los_numeros_admiten_las_palabras_clave_del_estandar() {
        assert_eq!(uno("VOLT MIN").numeric(0).unwrap(), Numeric::Min);
        assert_eq!(uno("VOLT MAXimum").numeric(0).unwrap(), Numeric::Max);
        assert_eq!(uno("VOLT def").numeric(0).unwrap(), Numeric::Default);
    }

    #[test]
    fn las_palabras_clave_se_resuelven_contra_los_limites() {
        assert_eq!(Numeric::Min.resolve(0.0, 30.0, 5.0), 0.0);
        assert_eq!(Numeric::Max.resolve(0.0, 30.0, 5.0), 30.0);
        assert_eq!(Numeric::Default.resolve(0.0, 30.0, 5.0), 5.0);
        assert_eq!(Numeric::Value(7.0).resolve(0.0, 30.0, 5.0), 7.0);
    }

    #[test]
    fn los_booleanos_admiten_las_cuatro_formas() {
        assert!(uno("OUTP ON").boolean(0).unwrap());
        assert!(uno("OUTP 1").boolean(0).unwrap());
        assert!(!uno("OUTP off").boolean(0).unwrap());
        assert!(!uno("OUTP 0").boolean(0).unwrap());
    }

    #[test]
    fn un_parametro_ausente_da_el_error_del_estandar() {
        let c = uno("VOLT");
        assert_eq!(c.numeric(0).unwrap_err().code, ErrorCode::MissingParameter);
    }

    #[test]
    fn un_parametro_ilegible_da_el_error_del_estandar() {
        let c = uno("VOLT patata");
        assert_eq!(
            c.numeric(0).unwrap_err().code,
            ErrorCode::IllegalParameterValue
        );
    }

    #[test]
    fn los_argumentos_de_mas_se_rechazan() {
        assert_eq!(
            uno("*CLS 1").expect_no_args().unwrap_err().code,
            ErrorCode::ParameterNotAllowed
        );
    }
}
