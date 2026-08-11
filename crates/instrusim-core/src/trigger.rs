//! Bus de disparo: las líneas por las que los instrumentos se sincronizan.
//!
//! En un rack real los equipos no viven aislados: la fuente avisa de que ha
//! establecido la tensión, el multímetro dispara la medida, el generador arranca
//! una ráfaga. Eso viaja por líneas de trigger, físicas o por bus.
//!
//! Está en el núcleo desde el primer día aunque casi nadie lo use todavía,
//! porque añadirlo después obligaría a tocar todos los instrumentos ya escritos.
//! Sirve tanto para las líneas físicas de un rack como para el modelo de disparo
//! de SCPI (`*TRG`, `INIT`, `ABORt`), que se apoyará en él sin cambios.

use std::collections::BTreeMap;

use crate::SimTime;

/// Identificador de una línea de disparo.
///
/// Un *tuple struct*: una `struct` cuyos campos no tienen nombre y se acceden
/// por posición. Es el patrón habitual para envolver un identificador y evitar
/// que se confunda con cualquier otro número que ande cerca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineId(pub u16);

impl LineId {
    /// Línea reservada para el disparo por software: `*TRG` y equivalentes.
    pub const SOFTWARE: LineId = LineId(0);
}

/// Flanco de un evento de disparo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Rising,
    Falling,
}

/// Un disparo concreto: qué línea, cuándo y en qué flanco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerEvent {
    pub line: LineId,
    pub at: SimTime,
    pub edge: Edge,
}

/// Las líneas de disparo del rack y los eventos pendientes de consumir.
///
/// Los eventos se guardan en un `BTreeMap` indexado por instante, que es un
/// diccionario **ordenado** por su clave, como el `TreeMap` de Java. Así los
/// eventos salen siempre en orden cronológico sin tener que ordenarlos, que es
/// justo lo que necesita un modelo de disparo: un instrumento debe reaccionar a
/// los flancos en el orden en que ocurrieron, nunca en el orden en que otros
/// instrumentos resultaron ejecutarse dentro del tic.
///
/// La clave incluye un contador además del instante, porque en un mismo
/// nanosegundo puede haber varios eventos y un mapa no admite claves repetidas.
/// El contador preserva el orden de emisión dentro del mismo instante.
#[derive(Debug, Default)]
pub struct TriggerBus {
    pending: BTreeMap<(SimTime, u64), TriggerEvent>,
    /// Se incrementa con cada emisión. Desempata eventos simultáneos.
    seq: u64,
}

impl TriggerBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Emite un flanco en una línea.
    pub fn emit(&mut self, line: LineId, at: SimTime, edge: Edge) {
        let evento = TriggerEvent { line, at, edge };
        self.pending.insert((at, self.seq), evento);
        self.seq += 1;
    }

    /// Atajo para el caso normal: un flanco de subida.
    pub fn pulse(&mut self, line: LineId, at: SimTime) {
        self.emit(line, at, Edge::Rising);
    }

    /// Atajo para el disparo por software, el que usará `*TRG`.
    pub fn software_trigger(&mut self, at: SimTime) {
        self.pulse(LineId::SOFTWARE, at);
    }

    /// Cuántos eventos quedan sin consumir.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Extrae, en orden cronológico, los eventos de una línea ocurridos hasta
    /// `until` inclusive. Los de otras líneas se quedan donde están.
    ///
    /// Es la operación que llamará cada instrumento en su `step`: "dame lo que
    /// ha pasado en mi línea desde la última vez".
    pub fn take_until(&mut self, line: LineId, until: SimTime) -> Vec<TriggerEvent> {
        // Se recogen primero las claves y luego se borran, porque no se puede
        // modificar un mapa mientras se recorre. El compilador lo impide: el
        // recorrido mantiene un préstamo inmutable y borrar exigiría uno mutable
        // al mismo tiempo. En Java esto compilaría y explotaría en ejecución con
        // un ConcurrentModificationException; aquí no llega a compilar.
        let claves: Vec<(SimTime, u64)> = self
            .pending
            .range(..=(until, u64::MAX))
            .filter(|(_, ev)| ev.line == line)
            .map(|(clave, _)| *clave)
            .collect();

        claves
            .into_iter()
            .map(|clave| {
                self.pending
                    .remove(&clave)
                    .expect("la clave venía del mapa")
            })
            .collect()
    }

    /// Extrae todos los eventos ocurridos hasta `until`, de cualquier línea.
    pub fn take_all_until(&mut self, until: SimTime) -> Vec<TriggerEvent> {
        // `split_off` parte el mapa en dos por una clave. Nos quedamos con la
        // parte posterior y devolvemos la anterior, ya ordenada.
        let posteriores = self.pending.split_off(&(until, u64::MAX));
        let hasta_ahora = std::mem::replace(&mut self.pending, posteriores);
        hasta_ahora.into_values().collect()
    }

    /// Descarta todo lo pendiente. Es lo que hace `ABORt`.
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(nanos: u64) -> SimTime {
        SimTime::from_nanos(nanos)
    }

    #[test]
    fn un_bus_recien_creado_esta_vacio() {
        let bus = TriggerBus::new();
        assert!(bus.is_empty());
    }

    #[test]
    fn los_eventos_salen_en_orden_cronologico_aunque_entren_desordenados() {
        let mut bus = TriggerBus::new();
        let linea = LineId(1);

        // Emitidos a propósito en desorden: es lo que pasaría si dos
        // instrumentos programan disparos futuros dentro del mismo tic.
        bus.pulse(linea, t(300));
        bus.pulse(linea, t(100));
        bus.pulse(linea, t(200));

        let eventos = bus.take_until(linea, t(1000));
        let instantes: Vec<u64> = eventos.iter().map(|e| e.at.as_nanos()).collect();

        assert_eq!(instantes, vec![100, 200, 300]);
    }

    #[test]
    fn los_eventos_simultaneos_conservan_el_orden_de_emision() {
        let mut bus = TriggerBus::new();
        let linea = LineId(1);

        bus.emit(linea, t(100), Edge::Rising);
        bus.emit(linea, t(100), Edge::Falling);

        let eventos = bus.take_until(linea, t(100));
        assert_eq!(eventos.len(), 2);
        assert_eq!(eventos[0].edge, Edge::Rising);
        assert_eq!(eventos[1].edge, Edge::Falling);
    }

    #[test]
    fn no_se_entregan_eventos_del_futuro() {
        let mut bus = TriggerBus::new();
        let linea = LineId(1);

        bus.pulse(linea, t(100));
        bus.pulse(linea, t(5_000)); // aún no ha llegado su momento

        let eventos = bus.take_until(linea, t(1_000));
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].at.as_nanos(), 100);

        // El del futuro sigue ahí esperando.
        assert_eq!(bus.len(), 1);
    }

    #[test]
    fn cada_linea_ve_solo_lo_suyo() {
        let mut bus = TriggerBus::new();

        bus.pulse(LineId(1), t(100));
        bus.pulse(LineId(2), t(100));

        let de_la_uno = bus.take_until(LineId(1), t(1000));
        assert_eq!(de_la_uno.len(), 1);
        assert_eq!(de_la_uno[0].line, LineId(1));

        // El de la línea 2 no se ha tocado.
        assert_eq!(bus.len(), 1);
    }

    #[test]
    fn los_eventos_consumidos_no_se_repiten() {
        let mut bus = TriggerBus::new();
        let linea = LineId(1);

        bus.pulse(linea, t(100));
        assert_eq!(bus.take_until(linea, t(1000)).len(), 1);
        assert_eq!(bus.take_until(linea, t(1000)).len(), 0);
    }

    #[test]
    fn el_disparo_por_software_va_por_su_linea_reservada() {
        let mut bus = TriggerBus::new();
        bus.software_trigger(t(42));

        let eventos = bus.take_until(LineId::SOFTWARE, t(100));
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].edge, Edge::Rising);
    }

    #[test]
    fn abortar_descarta_lo_pendiente() {
        let mut bus = TriggerBus::new();
        bus.pulse(LineId(1), t(100));
        bus.pulse(LineId(2), t(200));

        bus.clear();
        assert!(bus.is_empty());
    }

    #[test]
    fn se_pueden_recoger_todas_las_lineas_de_una_vez() {
        let mut bus = TriggerBus::new();
        bus.pulse(LineId(2), t(200));
        bus.pulse(LineId(1), t(100));
        bus.pulse(LineId(3), t(9_000));

        let eventos = bus.take_all_until(t(1_000));
        let instantes: Vec<u64> = eventos.iter().map(|e| e.at.as_nanos()).collect();

        assert_eq!(instantes, vec![100, 200]);
        assert_eq!(bus.len(), 1);
    }
}
