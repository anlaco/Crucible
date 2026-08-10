//! Tabla de comandos: patrón compilado a acción.
//!
//! Cada instrumento declara qué cabeceras entiende y qué significa cada una.
//! Los patrones se compilan una sola vez al construir el instrumento, no en
//! cada mensaje.
//!
//! Es deliberadamente genérica sobre la acción (`T`) porque en la fase 3 la
//! tabla dejará de escribirse en Rust y se cargará de un fichero TOML. Lo único
//! que cambiará entonces es de dónde salen los pares; el mecanismo de búsqueda
//! será el mismo.

use crate::pattern::Pattern;

/// Un conjunto de patrones asociados a acciones.
#[derive(Debug, Clone)]
pub struct CommandTable<T> {
    entries: Vec<(Pattern, T)>,
}

impl<T> Default for CommandTable<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<T> CommandTable<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construye la tabla a partir de pares patrón/acción.
    pub fn from_pairs<'a, I>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, T)>,
    {
        Self {
            entries: pairs
                .into_iter()
                .map(|(p, v)| (Pattern::parse(p), v))
                .collect(),
        }
    }

    pub fn add(&mut self, pattern: &str, value: T) {
        self.entries.push((Pattern::parse(pattern), value));
    }

    /// Busca la acción que corresponde a una cabecera.
    ///
    /// Devuelve también los sufijos numéricos, para instrumentos con canales.
    /// Gana el primer patrón que case, así que el orden de declaración importa
    /// cuando dos patrones se solapan: lo específico va antes que lo genérico.
    pub fn lookup(&self, header: &str) -> Option<(&T, Vec<u32>)> {
        self.entries
            .iter()
            .find_map(|(p, v)| p.matches(header).map(|sufijos| (v, sufijos)))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Clone, Copy)]
    enum Accion {
        Medir,
        Configurar,
        Rango,
    }

    fn tabla() -> CommandTable<Accion> {
        CommandTable::from_pairs([
            ("MEASure[:VOLTage][:DC]", Accion::Medir),
            ("CONFigure[:VOLTage][:DC]", Accion::Configurar),
            ("[SENSe:]VOLTage[:DC]:RANGe", Accion::Rango),
        ])
    }

    #[test]
    fn encuentra_la_accion_por_forma_corta_y_larga() {
        let t = tabla();
        assert_eq!(t.lookup("MEAS").map(|(a, _)| *a), Some(Accion::Medir));
        assert_eq!(
            t.lookup("MEAS:VOLT:DC").map(|(a, _)| *a),
            Some(Accion::Medir)
        );
        assert_eq!(
            t.lookup("MEASURE:VOLTAGE").map(|(a, _)| *a),
            Some(Accion::Medir)
        );
    }

    #[test]
    fn una_cabecera_desconocida_no_encuentra_nada() {
        assert!(tabla().lookup("MEAS:TEMP").is_none());
    }

    #[test]
    fn devuelve_los_sufijos_de_canal() {
        let t = CommandTable::from_pairs([("OUTPut[:STATe]", Accion::Medir)]);
        assert_eq!(t.lookup("OUTP3:STAT").map(|(_, s)| s), Some(vec![3]));
    }
}
