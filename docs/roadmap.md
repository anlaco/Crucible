# Roadmap

Hitos de Crucible, de lo más cercano a lo más lejano. Cada hito marca su
alcance. Lo **ya hecho** se marca; el resto es **propuesta** de orden, no
compromiso de fecha.

El estándar agarra cuando tiene **formato claro + runtime de referencia
usable + 3-4 perfiles reales de instrumentos comunes**. Ese es el
objetivo de los primeros hitos, no "cubrir todos los instrumentos".

## H0 — Proyecto + spec del formato ✅ (hecho)

- Repo inicializado, licencia Apache-2.0, docs de diseño.
- Spec del **formato de perfil de instrumento** (con ejemplo Keithley
  2400) y de la **topología de banco**.
- ADR-0001: estándar declarativo, Apache, separado de Anvil.

→ [diseno/arquitectura.md](diseno/arquitectura.md),
[diseno/formato-de-perfil.md](diseno/formato-de-perfil.md),
[diseno/topologia-de-banco.md](diseno/topologia-de-banco.md),
[adr/0001-...](adr/0001-estandar-declarativo-apache-separado-de-anvil.md)

## H1 — Runtime de referencia mínimo + un instrumento · MVP-parcial

- Decidir el lenguaje del runtime de referencia (propuesta: Rust).
- **Runtime mínimo**: carga un perfil YAML, mantiene el estado, hace
  match de comandos SCPI (con `<x>`), evalúa modelos `formula` con
  determinismo (semilla fija), y **sirve el instrumento por SCPI/TCP en
  loopback**.
- **Perfil real**: Keithley 2400 (`*IDN?`, `SOUR:VOLT`, `OUTP ON/OFF`,
  `MEAS:VOLT?`/`MEAS:CURR?`, modelo de medición con ruido determinista).
- Modo **determinista** primero (sin red flakiness).

→ [diseno/formato-de-perfil.md](diseno/formato-de-perfil.md)

## H2 — Anvil corre una secuencia contra el gemelo · MVP-parcial

- Con el runtime de H1 levantado, **Anvil** ejecuta `ejemplos/scpi.yaml`
  (paso `medir_voltaje_scpi`) contra el Keithley simulado y ve la medida.
- **Cero cambios en Anvil**: ya tiene el paso SCPI/TCP. Esto valida el
  desacoplamiento (Anvil no distingue real de simulado).
- Smoke end-to-end documentado.

## H3 — Topología de banco + 2-3 perfiles reales · MVP-parcial

- **Topología de banco**: varios instrumentos en un YAML, cada uno en su
  puerto. `estado_inicial` override. Conexiones **informativas** (el
  runtime no propaga aún).
- **Perfiles reales** de instrumentos comunes: una fuente (Rigol DP832 o
  similar), un osciloscopio (Keysight DSOX1204, con `tipo: waveform` para
  la traza), reutilizando el Keithley.
- El banco se sirve como **un conjunto de puertos** SCPI/TCP.

→ [diseno/topologia-de-banco.md](diseno/topologia-de-banco.md)

## H4 — Propagación de estado entre instrumentos · MVP-parcial

- El runtime **propaga** el estado por las conexiones (la fuente fija V,
  el multímetro mide ese V; el DUT es una resistencia). No es solver de
  circuito todavía; es propagación de modelos.
- Modelo de DUT `builtin` (resistencia, carga) parametrizable.

## H5 — Record/replay · MVP-parcial

- Grabar respuestas de un instrumento real y replayarlas; fallar si la
  realidad difiere de lo grabado (`ReplayMismatchError`). Para detectar
  regresiones de comunicación.
- Requiere haber tenido un instrumento real (o un ESP32 que emule) para
  grabar la sesión.

## H6 — ESP32 como instrumento físico emulado · MVP-parcial / post

- Un ESP32 expone un servidor SCPI/TCP con un modelo simple (voltaje
  controlado por un potenciómetro, un LED de pass/fail). Valida la **pila
  de red física** (no loopback) y sirve de **demo tangible**.
- Por la restricción de loopback de Anvil (ADR-0011), el ESP32 en la LAN
  se accede vía un mini-proxy `127.0.0.1:5025 → ESP32` (no se toca Anvil).

## Post-MVP

- **Solver de circuito** (Kirchhoff DC, transitorios) — sólo si un caso
  real lo pide.
- **Perfiles de DUT** complejos (IV curve, carga activa) definidos por el
  usuario.
- **Auto-descubrimiento** de perfiles desde un directorio; **versionado**
  de perfiles y migración.
- **Modelos en `plugin`** (Rust/Python) para instrumentos con física no
  expresable en `formula`.
- **Catálogo de perfiles** de instrumentos comunes (la "librería" que
  hace al estándar útil de inmediato).
- **UI** mínima para ver el banco y los instrumentos en vivo.

## Cómo se gestiona el alcance

- Cada hito se vincula a issues cuando arranque su implementación.
- Un alcance que se sale del MVP se mueve explícitamente a post-MVP con
  un ADR si cambia una decisión de fondo.
- Regla rectora: **no reinventar Simulink**; modelar instrumentos como
  máquinas de estado con respuestas y un modelo de comportamiento; añadir
  física sólo cuando un caso real lo exija.