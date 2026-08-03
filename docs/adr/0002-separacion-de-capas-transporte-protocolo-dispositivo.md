# ADR-0002: Separación de capas transporte / protocolo / dispositivo

- **Estado:** Aceptada
- **Fecha:** 2026-08-03

## Contexto

El diseño inicial de Crucible (ADR-0001, arquitectura.md) asumía que el
estándar era "SCPI sobre TCP". Eso es correcto para un recorte del MVP,
pero **incorrecto como modelo del estándar**. La realidad de un banco de
test es:

- **SCPI es un protocolo, no un transporte.** Un Keithley 2400 habla SCPI
  igual por GPIB, TCP, USB-TMC o RS-232. Confundir protocolo con
  transporte hace que el formato no pueda expresar un banco real.
- **Muchos elementos del banco no hablan SCPI.** Una cámara térmica habla
  Modbus RTU. Un DAQ de NI usa DAQmx (binario propietario). Un fixture
  custom usa un protocolo serial ad-hoc. Un PLC expone registros por
  Modbus TCP o OPC UA. Una cámara industrial habla GigE Vision. Si el
  estándar sólo modela SCPI, no puede describir el banco — sólo la
  fracción de instrumentos de banco que hablan SCPI por TCP.
- **El transporte es del banco, no del dispositivo.** El mismo modelo de
  instrumento puede estar conectado por TCP en un banco y por GPIB en
  otro. Fijar el transporte en el perfil lo hace no portable entre
  bancos.

VISA (el estándar de facto de I/O) ya separa transporte (TCPIP, GPIB,
USB, ASRL, PXI) de protocolo (VXI-11, HiSLIP, USB-TMC, raw socket) y lo
expresa en el *resource string*. Crucible se alinea con ese modelo pero
va más allá: simula el comportamiento del dispositivo, no solo lo
transporta.

## Decisión

El estándar separa **tres capas**:

1. **Dispositivo (perfil)**: describe QUÉ hace el elemento del banco —
   identidad, estado, comandos/registros, modelos de comportamiento. Es
   independiente del transporte y, en gran medida, del protocolo. El
   perfil declara su protocolo (porque el protocolo define cómo se
   codifican los comandos), pero **no declara transporte**.

2. **Protocolo**: describe CÓMO se habla con el dispositivo — SCPI,
   Modbus RTU/TCP, serial custom, DAQmx, VXI-11, HiSLIP. Es una
   propiedad del perfil. El runtime implementa un *codec* por protocolo.

3. **Transporte**: describe POR DÓNDE se conecta el consumidor al
   dispositivo — TCP, GPIB, USB, serial, PXI. Es una propiedad de la
   **topología** (del banco), no del perfil. El runtime implementa un
   *transport driver* por transporte.

```
┌──────────────────────────────┐
│      DISPOSITIVO (perfil)     │  ← QUÉ hace
├──────────────────────────────┤
│      PROTOCOLO                │  ← CÓMO se habla
├──────────────────────────────┤
│      TRANSPORTE               │  ← POR DÓNDE
└──────────────────────────────┘
```

### Recorte del MVP

- **Formato**: soporta las tres capas desde el inicio. Los perfiles
  declaran `protocolo`; las topologías declaran `transporte`. No hay que
  refactorizar el formato después.
- **Runtime de referencia**: implementa solo **TCP (loopback)** como
  transporte, y **SCPI + Modbus** como protocolos. Añadir serial, GPIB,
  USB o un protocolo nuevo es añadir un driver/codec al runtime, sin
  tocar el formato.

### Lenguaje y targets

- **Lenguaje: Rust.** Compila a nativo y a WASM (wasm32-wasip2). Es el
  único lenguaje que hace las dos cosas bien y es coherente con Anvil.
- **Dos targets de compilación desde el inicio**:
  - **Nativo** (`x86_64-pc-windows-msvc`, y Linux/macOS): usa `tokio` /
    `std::net` para TCP. Para desarrollo, CI, demos. Binario standalone.
  - **WASM** (`wasm32-wasip2`): usa `wasi-sockets` (Phase 3, incluido en
    WASI 0.3) para TCP. Se carga en `wasmtime` o en el runtime de Anvil.
- **El valor de WASM aquí no es "correr en navegador"**: es que el
  runtime de Crucible se cargue como componente WASM dentro del runtime
  de Anvil (o cualquier host WASI), con aislamiento de sandbox y
  portabilidad. Cuando **wasi-VISA** (en desarrollo en ANLACO) exponga
  GPIB/USB-TMC/serial/PXI a componentes WASM, el mismo código de
  Crucible servirá todos los transportes vía WASM, sin reescribir nada.
- **MVP**: target nativo con TCP. El target WASM se valida cuando
  wasi-sockets esté estable en el host objetivo (wasmtime o Anvil).
- **Abstracción de transporte**: un trait `Transport` con
  implementaciones por target — `tokio` (nativo) y `wasi-sockets`
  (WASM). El codec de protocolo (SCPI, Modbus) es shared, independiente
  del transporte.

## Por qué esta forma

- **Fiel a la realidad**: un banco de test mezcla transportes y
  protocolos. El estándar tiene que poder describirlo todo, no solo la
  fracción SCPI-sobre-TCP.
- **Portable**: el perfil de un Keithley 2400 es el mismo sin importar si
  el banco lo conecta por TCP, GPIB o USB. El transporte es del banco.
- **Extensible sin rotura**: añadir un transporte o protocolo nuevo no
  cambia los perfiles existentes ni las topologías existentes (salvo
  querer usar el nuevo).
- **Alineado con VISA**: usa los mismos transportes y *resource strings*
  que VISA, lo que facilita la adopción (los consumidores ya conocen
  `TCPIP0::host::port::SOCKET`, `GPIB0::addr::INSTR`, etc.).
- **MVP barato**: el runtime solo necesita TCP loopback para servir
  cualquier dispositivo cuyo transporte declarado sea TCP. El formato es
  completo desde día 1; el runtime crece por detrás.

## Alternativas rechazadas

- **"SCPI sobre TCP" (diseño inicial)**: correcto para un recorte del
  MVP, pero incorrecto como modelo. No puede describir cámaras
  térmicas (Modbus), DAQs (DAQmx), PLCs (OPC UA), fixtures custom
  (serial). Confunde protocolo con transporte. **Rechazado como modelo
  del estándar**; se mantiene como recorte del runtime MVP.
- **Transporte fijo en el perfil**: hace que un perfil no sea portable
  entre bancos (un Keithley por GPIB no sirve para un banco que lo quiere
  por TCP). **Rechazado**: el transporte va en la topología.
- **Formatos separados por tipo de dispositivo** (un schema para SCPI,
  otro para Modbus, otro para DAQ): fragmenta el estándar y complica la
  topología (un banco mixto necesita varios parsers). **Rechazado**: un
  solo formato con `protocolo` como campo; el codec del protocolo hace
  la diferencia.
- **VISA como runtime**: VISA es una capa de I/O, no un simulador. No
  modela comportamiento ni estado. Crucible se alinea con el modelo de
  VISA (mismos transportes, resource strings) pero va más allá.

## Consecuencias

- El formato de perfil gana un campo `protocolo` y pierde el campo
  `puerto` (que pasa a la topología como parámetro del transporte).
- El formato de topología gana un campo `transporte` por dispositivo, con
  sus parámetros.
- El runtime de referencia implementa transport drivers (MVP: solo TCP) y
  protocol codecs (MVP: SCPI + Modbus) como módulos independientes.
- Los perfiles existentes (ejemplo Keithley 2400) se actualizan para
  declarar `protocolo: scpi` y quitarse el `puerto`.
- El roadmap se actualiza: el MVP ahora cubre formato multi-protocolo +
  runtime TCP-only + SCPI y Modbus como primeros protocolos.

## Relaciona

- [arquitectura.md](../diseno/arquitectura.md)
- [formato-de-perfil.md](../diseno/formato-de-perfil.md)
- [topologia-de-banco.md](../diseno/topologia-de-banco.md)
- [roadmap.md](../roadmap.md)
- [ADR-0001](0001-estandar-declarativo-apache-separado-de-anvil.md)