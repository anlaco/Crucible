//! Patrones de cabecera SCPI: forma larga, forma corta y nodos opcionales.
//!
//! SCPI define cada comando con una notación en la que las mayúsculas marcan la
//! abreviatura admitida y los corchetes marcan lo que se puede omitir:
//!
//! ```text
//! [SENSe:]VOLTage[:DC]:RANGe
//! ```
//!
//! Ese único patrón debe aceptar `SENS:VOLT:DC:RANG`, `VOLT:RANG`,
//! `voltage:range` y una docena más de combinaciones, todas equivalentes y
//! todas legales. Un simulador que solo reconozca la forma que él escribió
//! falla en cuanto el cliente usa la otra, y ese es exactamente el tipo de fallo
//! que el usuario achacará a su propio código durante media tarde.

/// Un tramo de la cabecera: un mnemónico con sus dos formas.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Segment {
    /// Forma corta, las mayúsculas del patrón: `VOLT`.
    short: String,
    /// Forma larga completa: `VOLTAGE`.
    long: String,
    /// Si el tramo iba entre corchetes y puede omitirse.
    optional: bool,
}

impl Segment {
    fn new(word: &str, optional: bool) -> Self {
        let short: String = word
            .chars()
            .filter(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            .collect();
        let long = word.to_ascii_uppercase();
        // Un mnemónico escrito entero en mayúsculas no tiene abreviatura.
        let short = if short.is_empty() {
            long.clone()
        } else {
            short
        };
        Self {
            short,
            long,
            optional,
        }
    }

    /// ¿Este tramo acepta el mnemónico recibido?
    ///
    /// Devuelve además el sufijo numérico si lo hubiera: en `OUTP2`, el 2. Los
    /// instrumentos con varios canales lo usan para saber a cuál se refiere el
    /// comando.
    fn accepts(&self, input: &str) -> Option<Option<u32>> {
        let input = input.to_ascii_uppercase();

        // Separar el sufijo numérico del final, si lo hay.
        let corte = input
            .rfind(|c: char| !c.is_ascii_digit())
            .map(|i| i + 1)
            .unwrap_or(0);
        let (nombre, sufijo_txt) = input.split_at(corte);

        // Un mnemónico del patrón que ya acaba en dígito (como `DC`... no, pero
        // sí `Q2`) se compara entero; en ese caso no hay sufijo que extraer.
        if nombre == self.short || nombre == self.long {
            let sufijo = if sufijo_txt.is_empty() {
                None
            } else {
                sufijo_txt.parse().ok()
            };
            return Some(sufijo);
        }

        if input == self.short || input == self.long {
            return Some(None);
        }

        None
    }
}

/// Un patrón de cabecera compilado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    segments: Vec<Segment>,
}

impl Pattern {
    /// Compila la notación del estándar.
    ///
    /// Se hace una vez al arrancar y se reutiliza en cada comando, en vez de
    /// interpretar el texto en cada mensaje.
    pub fn parse(pattern: &str) -> Self {
        let mut segments = Vec::new();
        let mut actual = String::new();
        let mut dentro_de_corchete = false;
        let mut actual_opcional = false;

        for c in pattern.chars() {
            match c {
                '[' => dentro_de_corchete = true,
                ']' => dentro_de_corchete = false,
                ':' => {
                    if !actual.is_empty() {
                        segments.push(Segment::new(&actual, actual_opcional));
                        actual.clear();
                    }
                }
                _ => {
                    if actual.is_empty() {
                        // El carácter de apertura decide si el tramo entero es
                        // opcional, no el de cierre.
                        actual_opcional = dentro_de_corchete;
                    }
                    actual.push(c);
                }
            }
        }
        if !actual.is_empty() {
            segments.push(Segment::new(&actual, actual_opcional));
        }

        Self { segments }
    }

    /// ¿Casa esta cabecera con el patrón?
    ///
    /// Devuelve los sufijos numéricos encontrados, en orden. `None` significa
    /// que no casa.
    pub fn matches(&self, header: &str) -> Option<Vec<u32>> {
        let partes: Vec<&str> = header
            .trim_start_matches(':')
            .split(':')
            .filter(|s| !s.is_empty())
            .collect();

        let mut sufijos = Vec::new();
        if self.encaja_desde(0, &partes, &mut sufijos) {
            Some(sufijos)
        } else {
            None
        }
    }

    /// Recorrido con vuelta atrás sobre los tramos opcionales.
    ///
    /// Hay que probar ambas opciones en cada tramo omitible: `VOLT:RANG` con el
    /// patrón `[SENSe:]VOLTage[:DC]:RANGe` exige saltarse dos tramos, y con
    /// `SENS:VOLT:RANG` solo uno. Un recorrido lineal ávido se equivocaría en
    /// cuanto un tramo opcional coincidiese por casualidad con el siguiente.
    fn encaja_desde(&self, i: usize, partes: &[&str], sufijos: &mut Vec<u32>) -> bool {
        if i == self.segments.len() {
            return partes.is_empty();
        }

        let seg = &self.segments[i];

        // Rama 1: consumir el mnemónico.
        if let Some((primera, resto)) = partes.split_first()
            && let Some(sufijo) = seg.accepts(primera)
        {
            let marca = sufijos.len();
            if let Some(s) = sufijo {
                sufijos.push(s);
            }
            if self.encaja_desde(i + 1, resto, sufijos) {
                return true;
            }
            // No salió: deshacer lo anotado antes de probar la otra rama.
            sufijos.truncate(marca);
        }

        // Rama 2: omitirlo, si el patrón lo permite.
        if seg.optional && self.encaja_desde(i + 1, partes, sufijos) {
            return true;
        }

        false
    }

    /// La cabecera en forma larga y canónica, para mensajes y documentación.
    pub fn canonical(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.long.as_str())
            .collect::<Vec<_>>()
            .join(":")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acepta_forma_corta_y_larga() {
        let p = Pattern::parse("MEASure:VOLTage:DC");

        assert!(p.matches("MEAS:VOLT:DC").is_some());
        assert!(p.matches("MEASURE:VOLTAGE:DC").is_some());
        assert!(p.matches("MEAS:VOLTAGE:DC").is_some()); // mezcladas
    }

    #[test]
    fn no_distingue_mayusculas_de_minusculas() {
        let p = Pattern::parse("MEASure:VOLTage:DC");
        assert!(p.matches("meas:volt:dc").is_some());
        assert!(p.matches("Measure:Voltage:Dc").is_some());
    }

    #[test]
    fn rechaza_abreviaturas_que_el_estandar_no_admite() {
        let p = Pattern::parse("MEASure:VOLTage:DC");
        // "MEASUR" no es ni la forma corta ni la larga: es inválida.
        assert!(p.matches("MEASUR:VOLT:DC").is_none());
        assert!(p.matches("M:VOLT:DC").is_none());
    }

    #[test]
    fn los_tramos_opcionales_se_pueden_omitir_o_no() {
        let p = Pattern::parse("[SENSe:]VOLTage[:DC]:RANGe");

        assert!(p.matches("SENS:VOLT:DC:RANG").is_some()); // todo
        assert!(p.matches("VOLT:DC:RANG").is_some()); // sin SENSe
        assert!(p.matches("SENS:VOLT:RANG").is_some()); // sin DC
        assert!(p.matches("VOLT:RANG").is_some()); // sin ninguno
    }

    #[test]
    fn los_tramos_obligatorios_no_se_pueden_omitir() {
        let p = Pattern::parse("[SENSe:]VOLTage[:DC]:RANGe");
        assert!(p.matches("SENS:DC:RANG").is_none()); // falta VOLTage
        assert!(p.matches("VOLT:DC").is_none()); // falta RANGe
    }

    #[test]
    fn sobran_tramos_al_final() {
        let p = Pattern::parse("MEASure:VOLTage");
        assert!(p.matches("MEAS:VOLT:DC").is_none());
    }

    #[test]
    fn los_dos_puntos_iniciales_son_opcionales() {
        let p = Pattern::parse("MEASure:VOLTage");
        assert!(p.matches(":MEAS:VOLT").is_some());
    }

    #[test]
    fn extrae_el_sufijo_numerico_de_canal() {
        let p = Pattern::parse("OUTPut:STATe");

        assert_eq!(p.matches("OUTP:STAT"), Some(vec![]));
        assert_eq!(p.matches("OUTP2:STAT"), Some(vec![2]));
        assert_eq!(p.matches("OUTPUT12:STATE"), Some(vec![12]));
    }

    #[test]
    fn un_tramo_opcional_intermedio_se_salta_bien() {
        let p = Pattern::parse("SOURce[:CURRent]:CLIMit");
        assert!(p.matches("SOUR:CURR:CLIM").is_some());
        assert!(p.matches("SOUR:CLIM").is_some());
    }

    /// El caso que obliga a la vuelta atrás de verdad: el tramo opcional y el
    /// obligatorio que le sigue aceptan el mismo mnemónico. Un recorrido ávido
    /// consumiría la `B` en el tramo opcional, se quedaría sin mnemónicos para
    /// el obligatorio y daría la cabecera por inválida.
    #[test]
    fn la_vuelta_atras_deshace_una_eleccion_equivocada() {
        let p = Pattern::parse("A[:B]:B");
        assert!(p.matches("A:B").is_some());
        assert!(p.matches("A:B:B").is_some());
        assert!(p.matches("A").is_none());
    }

    #[test]
    fn la_forma_canonica_es_la_larga() {
        let p = Pattern::parse("[SENSe:]VOLTage[:DC]:RANGe");
        assert_eq!(p.canonical(), "SENSE:VOLTAGE:DC:RANGE");
    }
}
