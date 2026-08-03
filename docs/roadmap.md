# Roadmap

Hitos de Crucible, de lo más cercano a lo más lejano. Cada hito marca su
alcance. Lo **ya hecho** se marca; el resto es **propuesta** de orden, no
compromiso de fecha.

El estándar agarra cuando tiene **formato claro + runtime de referencia
usable + 3-4 perfiles reales de dispositivos comunes**. Ese es el
objetivo de los primeros hitos, no "cubrir todos los dispositivos".

## H0 — Proyecto + spec del formato ✅ (hecho)

- Repo inicializado, licencia Apache-2.0, docs de diseño.
- Spec del **formato de perfil de dispositivo** (multi-protocolo: SCPI,
  Modbus, serial custom) y de la **topología de banco** (multi-transporte:
  TCP, GPIB, USB, serial, PXI).
- ADR-0001: estándar declarativo, Apache, separado de Anvil.
- ADR-0002: separación de capas transporte / protocolo / dispositivo.

→ [diseno/arquitectura.md](diseno/arquitectura.md),
[diseno/formato-de-perfil.md](diseno/formato-de-perfil.md),
[diseno/topologia-de-banco.md](diseno/topologia-de-banco.md),
[adr/0001-...](adr/0001-estandar-declarativo-apache-separado-de-anvil.md),
[adr/0002-...](adr/0002-separacion-de-capas-transporte-protocolo-dispositivo.md)

## H1 — Runtime de referencia mínimo + dos dispositivos · MVP-parcial

- Decidir el lenguaje del runtime de referencia (propuesta: Rust).
- **Runtime mínimo**: carga un perfil YAML, mantiene el estado, hace
  match de comandos/registros, evalúa modelos `formula` con determinismo
  (semilla fija), y **sirve el dispositivo por TCP en loopback**.
- **Codecs de protocolo**: SCPI (match de patrones con `<x>`) y Modbus
  (lectura/escritura de registros holding/input).
- **Transport driver**: TCP (loopback). Los demás transportes (serial,
  GPIB, USB, PXI) se añaden post-MVP.
- **Perfiles reales**:
  - Keithley 2400 (`*IDN?`, `SOUR:VOLT`, `OUTP ON/OFF`,
    `MEAS:VOLT?`/`MEAS:CURR?`, modelo de medición con ruido determinista)
    — protocolo SCPI.
  - Cámara térmica genérica (registros Modbus: setpoint, temperatura
    actual, run/stop, alarma) — protocolo Modbus TCP.
- Modo **determinista** primero (sin red flakiness).

→ [diseno/formato-de-perfil.md](diseno/formato-de-perfil.md)

## H2 — Anvil corre una secuencia contra el gemelo · MVP-parcial

- Con el runtime de H1 levantado, **Anvil** ejecuta `ejemplos/scpi.yaml`
  (paso `medir_voltaje_scpi`) contra el Keithley simulado y ve la medida.
- **Cero cambios en Anvil**: ya tiene el paso SCPI/TCP. Esto valida el
  desacoplamiento (Anvil no distingue real de simulado).
- Smoke end-to-end documentado.

## H3 — Topología de banco + 3-4 perfiles reales · MVP-parcial

- **Topología de banco**: varios dispositivos en un YAML, cada uno con su
  transporte (MVP: todos TCP). `estado_inicial` override. Conexiones
  **informativas** (el runtime no propaga aún).
- **Perfiles reales** de dispositivos comunes:
  - Una fuente (Rigol DP832 o similar) — SCPI.
  - Un osciloscopio (Keysight DSOX1204, con `tipo: waveform` para la
    traza) — SCPI.
  - Una cámara térmica — Modbus TCP.
  - Un fixture custom (LEDs, botón) — serial ASCII sobre TCP.
  - Reutilizando el Keithley 2400.
- El banco se sirve como **un conjunto de puertos** TCP, cada uno hablando
  su protocolo.

→ [diseno/topologia-de-banco.md](diseno/topologia-de-banco.md)

## H4 — Propagación de estado entre dispositivos · MVP-parcial

- El runtime **propaga** el estado por las conexiones (la fuente fija V,
  el multímetro mide ese V; la cámara lee la temperatura del DUT). No es
  solver de circuito todavía; es propagación de modelos.
- Modelo de DUT `builtin` (resistencia, carga, térmica simple)
  parametrizable.
- Conexiones lógicas no eléctricas (cámara observa temperatura, etc.).

## H5 — Record/replay · MVP-parcial

- Grabar respuestas de un dispositivo real y replayarlas; fallar si la
  realidad difiere de lo grabado (`ReplayMismatchError`). Para detectar
  regresiones de comunicación.
- Requiere haber tenido un dispositivo real (o un ESP32 que emule) para
  grabar la sesión.

## H6 — ESP32 como dispositivo físico emulado · MVP-parcial / post

- Un ESP32 expone un servidor SCPI/TCP con un modelo simple (voltaje
  controlado por un potenciómetro, un LED de pass/fail). Valida la **pila
  de red física** (no loopback) y sirve de **demo tangible**.
- Por la restricción de loopback de Anvil (ADR-0011), el ESP32 en la LAN
  se accede vía un mini-proxy `127.0.0.1:5025 → ESP32` (no se toca Anvil).

## H7 — Más transportes en el runtime · post-MVP

- **Serial (RS-232/RS-485)**: el runtime abre puertos serie reales (o
  virtuales) y sirve los dispositivos por serial. Para cámaras, fuentes
  baratas, fixtures.
- **GPIB**: el runtime sirve dispositivos por GPIB (requiere hardware
  GPIB o un emulador). Menos prioritario (GPIB es legacy).
- **USB-TMC**: el runtime sirve dispositivos por USB (requiere driver
  USB-TMC). Más complejo por dependencia del SO.
- **PXI**: register-level. Muy específico; post-MVP lejano.

## Post-MVP

- **Solver de circuito** (Kirchhoff DC, transitorios) — sólo si un caso
  real lo pide.
- **Perfiles de DUT** complejos (IV curve, carga activa, térmica)
  definidos por el usuario.
- **Auto-descubrimiento** de perfiles desde un directorio; **versionado**
  de perfiles y migración.
- **Modelos en `plugin`** (Rust/Python) para dispositivos con física no
  expresable en `formula`.
- **Catálogo de perfiles** de dispositivos comunes (la "librería" que
  hace al estándar útil de inmediato).
- **Protocolos adicionales**: DAQmx-style, OPC UA, GigE Vision, VXI-11,
  HiSLIP — cuando un caso real lo pida.
- **UI** mínima para ver el banco y los dispositivos en vivo.

## Cómo se gestiona el alcance

- Cada hito se vincula a issues cuando arranque su implementación.
- Un alcance que se sale del MVP se mueve explícitamente a post-MVP con
  un ADR si cambia una decisión de fondo.
- Regla rectora: **no reinventar Simulink**; modelar dispositivos como
  máquinas de estado con respuestas y un modelo de comportamiento; añadir
  física sólo cuando un caso real lo exija.
- Regla rectora de capas (ADR-0002): el **formato** es multi-transporte y
  multi-protocolo desde el inicio; el **runtime** crece por detrás, sin
  tocar el formato.