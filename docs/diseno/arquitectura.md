# Diseño: Arquitectura de Crucible

> **Prioridad:** fundacional. Propuesta para discusión antes de
> implementar.

Crucible es un **estándar abierto para describir y simular bancos de
test** — el gemelo digital del banco. La meta: probar software de test
sin hardware, con la misma confianza que da un instrumento real (dentro
de lo razonable).

## Qué se simula: el banco entero, no sólo el instrumento

Un banco de test no es solo instrumentos SCPI por TCP. Es **una mezcla
heterogénea** de elementos:

- Un **multímetro Keithley 2400** que habla SCPI por GPIB.
- Una **fuente Rigol DP832** que habla SCPI por USB-TMC.
- Una **cámara térmica Espec** que habla Modbus RTU por RS-485.
- Un **DAQ NI USB-6212** controlado por NI-DAQmx (binario propietario).
- Un **PLC** que expone registros por Modbus TCP.
- Un **programador JTAG** accesible por TCP (OpenOCD).
- Un **fixture custom** con un FTDI y un protocolo serial ad-hoc.

El error de base del diseño inicial era asumir "SCPI sobre TCP" como lo
único. **SCPI es un protocolo de comandos, no un transporte**. Un
Keithley 2400 habla SCPI igual por GPIB, TCP, USB-TMC o RS-232. Y muchos
elementos del banco **no hablan SCPI en absoluto**: hablan Modbus, DAQmx,
protocolos binarios propietarios, o comandos seriales custom.

## Tres capas, no una

El estándar separa tres capas que el diseño inicial confundía:

```
┌──────────────────────────────┐
│      DISPOSITIVO (perfil)     │  ← QUÉ hace
│                               │     (comportamiento, estado, modelos)
├──────────────────────────────┤
│      PROTOCOLO                │  ← CÓMO se habla
│                               │     (SCPI, Modbus, binario, API)
├──────────────────────────────┤
│      TRANSPORTE               │  ← POR DÓNDE
│                               │     (TCP, GPIB, USB, Serial, PXI)
└──────────────────────────────┘
```

### Capa 1: Dispositivo (perfil)

Un **perfil** describe el comportamiento de **un** elemento del banco —
su identidad, su estado interno y cómo muta, y un modelo de
comportamiento (qué produce cada medición u observación). Es lo
"descriptible como un motor en Simulink". El perfil es **independiente
del transporte** y, en gran medida, del protocolo: describe *qué* hace el
dispositivo, no *cómo* se lo preguntan. Ver
[`formato-de-perfil.md`](formato-de-perfil.md).

### Capa 2: Protocolo

El protocolo define **cómo se codifica la comunicación** con el
dispositivo: el formato de los mensajes, las reglas de
comando/respuesta, la codificación de los datos. Un mismo dispositivo
puede hablar varios protocolos (un Keithley 2400 habla SCPI; una cámara
térmica habla Modbus RTU). El protocolo es una propiedad del perfil,
porque define cómo se expresan los comandos/registros que mutan el
estado. Protocolos soportados por el formato:

| Protocolo | Tipo | Dispositivos típicos |
|---|---|---|
| **SCPI** | Texto ASCII, comando/respuesta | Multímetros, fuentes, osciloscopios, cargas |
| **Modbus RTU** | Binario con CRC, registros | Cámaras, PLCs, módulos I/O, fuentes industriales |
| **Modbus TCP** | Binario sobre TCP, registros | Mismos que RTU pero en red |
| **Serial custom** | Texto o binario ad-hoc | Fuentes baratas, fixtures, sondas |
| **DAQmx-style** | API binaria propietaria | DAQs (NI, Advantech, MCC) |
| **VXI-11 / HiSLIP** | RPC sobre TCP | Instrumentos LXI |

El **runtime de referencia** implementa SCPI y Modbus en el MVP. Los
demás son soportados por el formato y se añaden al runtime cuando un
caso real lo pide.

### Capa 3: Transporte

El transporte define **por dónde** se conecta el consumidor al
dispositivo simulado. Es una propiedad de la **topología** (del banco),
no del perfil: el banco decide "este Keithley va por TCP en puerto 5025,
este por GPIB en dirección 5". Transportes soportados por el formato:

| Transporte | Parámetros | VISA resource string |
|---|---|---|
| **TCP/IP** | `host`, `puerto` | `TCPIP0::host::puerto::SOCKET` |
| **GPIB** | `board`, `direccion` | `GPIB0::direccion::INSTR` |
| **USB** | `vid`, `pid`, `serial` | `USB0::vid::pid::serial::INSTR` |
| **Serial** | `puerto`, `baudrate`, `parity`, `data_bits`, `stop_bits` | `ASRLn::INSTR` |
| **PXI** | `bus`, `device`, `funcion` | `PXI0::bus::device::funcion::INSTR` |

El **runtime de referencia** sirve solo por **TCP (loopback)** en el
MVP. La arquitectura no lo impide a futuro: añadir serial, GPIB o USB es
añadir un *transport driver* al runtime, sin tocar el formato. Ver
ADR-0002.

## Tres piezas, no una

Lo que se estandariza no es "un simulador", es **un formato + un
contrato + un runtime de referencia**:

1. **Perfil de dispositivo** (declarativo, YAML): describe el
   comportamiento de **un** elemento del banco — su identidad, su
   protocolo, los comandos/registros que acepta, su estado interno y
   cómo muta, y un modelo de comportamiento. Es lo "descriptible como un
   motor en Simulink". Ver [`formato-de-perfil.md`](formato-de-perfil.md).

2. **Topología de banco** (declarativo, YAML): describe **el banco
   entero** — qué elementos hay, qué transporte usa cada uno, cómo se
   conectan entre sí y al DUT, y qué direcciones/puertos exponen. Esto es
   **el hueco que nadie cubre hoy**: el gemelo del *banco*, no del
   elemento aislado. Ver [`topologia-de-banco.md`](topologia-de-banco.md).

3. **Runtime de referencia**: carga perfiles + topología, mantiene el
   estado de cada elemento, simula cómo se influyen entre sí (la fuente
   alimenta al DUT, el multímetro mide al DUT, el osciloscopio mira un
   nodo), y sirve el banco **por el transporte que cada elemento
   declare**. El consumidor (Anvil, TestStand, OpenTAP…) se conecta al
   runtime como a un banco real.

El **estándar** es el formato (1 y 2) + el contrato de
transporte/protocolo. El runtime es la **implementación de referencia**;
puede haber otros.

## El contrato: transparente al consumidor

Un elemento simulado se sirve por el mismo transporte y protocolo que un
elemento real. El consumidor no sabe que es simulado: el motor de test
invoca un paso que abre una conexión, manda comandos y parsea la
respuesta. **El dispositivo vive detrás del paso, opaco al motor** (igual
que en Anvil, ADR-0003). Cero cambios en el consumidor para pasar de
real a simulado.

Para SCPI sobre TCP, el consumidor abre un `TcpStream` y manda texto. Para
Modbus TCP, abre un `TcpStream` al puerto 502 y manda tramas Modbus. Para
serial, abre el puerto serie. El runtime de referencia sirve todo por
TCP loopback en el MVP — el consumidor que usa TCP no nota diferencia; el
que usa serial o GPIB necesita el runtime que soporte ese transporte
(post-MVP).

## Por qué no reinventar Simulink

Simulink simula física continua (décadas de ingeniería). Un instrumento
es, en el 90 % de los casos, una **máquina de estado con respuestas**:
recibe un comando (o una escritura a un registro), muta un estado interno
y devuelve un valor. Empezamos modelando eso. La física se añade cuando
un caso real la pida (y aun entonces, como un *modelo* del dispositivo,
no como un motor de simulación universal).

El formato se diseña para que el modelo sea **declarativo por defecto**
(tablas, fórmulas, condiciones) pero pueda **caer a código** (un
plugin/trait) cuando el comportamiento es complejo. Datos primero,
código cuando hace falta — el mismo patrón que Anvil.

## Desacoplamiento del consumidor

Crucible **no sabe quién lo consume**. Se sirve por el transporte y
protocolo que el banco declara; el consumidor habla ese protocolo sobre
ese transporte. Eso lo hace:

- **Reusable**: sirve a Anvil, a TestStand, a OpenTAP, a un script
  PyVISA, a un cliente Modbus, a cualquier cosa que hable el protocolo
  correcto.
- **Estándar de facto**: si el formato es claro y el runtime fácil de
  consumir, se adopta sin comité.

## Precedentes y el hueco

- **PyVISA-sim**: perfiles declarativos YAML de **un** instrumento
  aislado. Soporta varios *resource types* (TCPIP, USB, GPIB, ASRL) como
  strings de VISA, pero la simulación es solo texto SCPI — sin Modbus, sin
  binario, sin estado entre dispositivos. El ejemplo más cercano; punto
  de partida natural (imitar y **extender**). No modela el banco ni las
  interacciones.
- **pytestlab**: modelos de instrumento + `SimBackend` + record/replay.
  Muy en la línea, pero más un framework que un estándar. SCPI-céntrico.
- **IVI**: el estándar "oficial" de drivers tipados (era NI). Pesado,
  acoplado a Windows/COM. No es el modelo a seguir.
- **VISA**: abstrae múltiples transportes (TCP, GPIB, USB, Serial, PXI)
  bajo una API unificada y un *resource string* estándar. No es un
  simulador — es la capa de I/O. Crucible **se alinea con el modelo de
  VISA** (reconoce los mismos transportes y resource strings) pero va
  más allá: simula el comportamiento del dispositivo, no solo lo
  transporta.

El hueco de Crucible: **estándar abierto, declarativo, multi-transporte y
multi-protocolo, del banco entero (topología + interacciones), servido
por el transporte y protocolo que cada elemento declare, consumible por
cualquier secuenciador.** Eso no existe.

## Tres modos de simulación (cubren los casos de uso)

- **Determinista**: el modelo devuelve respuestas predecibles (fórmulas
  fijas, tablas). Sin red ni flakiness. El modo perfecto para CI estricto
  y tests automáticos.
- **En vivo**: el runtime sirve el modelo por el transporte real (TCP,
  serial, etc.), igual que un dispositivo. Para validar la pila de
  comunicación de verdad y para demos (un ESP32 que emule un
  instrumento es este modo, en hardware).
- **Record/replay**: graba lo que un dispositivo real respondió y lo
  replaya; falla si la realidad difiere de lo grabado
  (`ReplayMismatchError`). Para detectar regresiones de comunicación.
  Necesita haber tenido un real alguna vez → se pospone.

El modo determinista es el más barato y el más valioso para empezar. El
en vivo es el que valida la pila real. El record/replay es el
sofisticado, para el final.

## Riesgos a vigilar desde el diseño

- **El DUT es lo difícil de modelar**, no los instrumentos. El DUT es lo
  que se está diseñando, no algo conocido. Modelamos bien los
  *instrumentos* (conocidos) y dejamos el DUT como una caja configurable;
  en muchos tests, el DUT es "estímulo → respuesta" simple.
- **No acoplar el estándar a Anvil ni a WASM.** Si sólo sirve a Anvil, no
  es estándar.
- **No bloquear el estándar por quererlo perfecto.** Un estándar agarra
  con: formato claro + runtime de referencia usable + 3-4 perfiles
  reales de dispositivos comunes. Ese es el primer hito, no "cubrir todo".
- **No asumir que todo es SCPI.** El formato nace multi-protocolo; el
  runtime MVP implementa SCPI + Modbus (los más comunes) y crece.