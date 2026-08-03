# Diseño: Formato de perfil de dispositivo

> **Prioridad:** fundacional. **Propuesta** para discusión antes de
> implementar. Los ejemplos son ilustrativos; la sintaxis exacta se cierra
> mañana.

Un **perfil de dispositivo** describe el comportamiento de **un** elemento
del banco — un instrumento SCPI, un DAQ, una cámara térmica, un fixture
custom. Igual que en Simulink describes el comportamiento de un motor. Es
declarativo (YAML) y lo ejecuta el runtime de referencia.

## Qué describe un perfil

Cinco cosas:

1. **Identidad**: modelo, cadena de `*IDN?` (o equivalente), tipo de
   dispositivo.
2. **Protocolo**: cómo se habla con el dispositivo — SCPI, Modbus RTU,
   serial custom, etc. Define cómo se codifican los comandos/registros.
3. **Estado interno**: las variables que el dispositivo recuerda
   (voltaje de la fuente, límite de corriente, output on/off, modo,
   setpoint de temperatura…). Se mutan con comandos y las lee el modelo.
4. **Comandos/registros**: cada comando (o patrón con parámetros, o
   registro Modbus) produce un efecto — una **mutación de estado** y/o
   una **respuesta**. Es la "firma" del dispositivo, expresada en el
   protocolo que habla.
5. **Modelos de comportamiento**: qué produce cada medición u
   observación. Declarativos por defecto (fórmulas/condiciones sobre el
   estado); caen a **código** (un plugin) cuando el comportamiento es
   complejo.

> **El transporte NO está en el perfil.** El transporte (TCP, GPIB, USB,
> serial, PXI) es una propiedad de la **topología** — el banco decide por
> dónde se conecta a cada dispositivo. Un Keithley 2400 es el mismo
> dispositivo hable SCPI por GPIB, TCP o USB-TMC. Ver
> [`topologia-de-banco.md`](topologia-de-banco.md).

## Ejemplo 1: Keithley 2400 (SCPI)

```yaml
# Perfil del Keithley 2400 (gemelo digital). Propuesta de formato.
dispositivo:
  modelo: KEITHLEY-2400
  tipo: instrumento            # instrumento | daq | camara | fixture | plc | dut
  idn: "Keithley,2400,1234567,A1.2"

protocolo: scpi                # el protocolo define cómo se codifican los comandos

# Estado interno: se muta con comandos y lo lee el modelo.
estado:
  voltaje_fuente: 0.0          # V, último SOUR:VOLT fijado
  corriente_limite: 0.105      # A, último SOUR:CURR fijado
  output: false                # OUTP ON/OFF
  modo: voltage                # voltage | current

# Comandos SCPI: patrón -> efecto (mutación y/o respuesta).
# <x> captura un argumento numérico.
comandos:
  - patron: "*IDN?"
    respuesta: "Keithley,2400,1234567,A1.2"
  - patron: "OUTP ON"
    muta: { output: true }
  - patron: "OUTP OFF"
    muta: { output: false }
  - patron: "SOUR:VOLT <x>"
    muta: { voltaje_fuente: "<x>" }
  - patron: "SOUR:CURR <x>"
    muta: { corriente_limite: "<x>" }
  - patron: "MEAS:VOLT?"
    modelo: medir_voltaje
  - patron: "MEAS:CURR?"
    modelo: medir_corriente

# Modelos de comportamiento (declarativos; caen a código si hace falta).
modelos:
  medir_voltaje:
    tipo: formula
    cuando: { output: true }                  # sólo mide con el output encendido
    expr: "voltaje_fuente + gauss(0, 0.001)"  # ideal + ruido
    fallback: "0.0"                           # output off
  medir_corriente:
    tipo: formula
    cuando: { output: true }
    expr: "voltaje_fuente / 1000.0"           # simula una carga de 1 kΩ
    fallback: "0.0"
```

## Ejemplo 2: cámara térmica (Modbus RTU)

```yaml
# Perfil de una cámara térmica genérica (gemelo digital).
dispositivo:
  modelo: CAMARA-TERMICA-GEN
  tipo: camara

protocolo: modbus_rtu          # registros Modbus, no comandos de texto

estado:
  setpoint: 25.0               # °C, consigna
  temperatura_actual: 25.0     # °C, lectura del sensor
  running: false               # cámara en marcha
  alarma: false

# Registros Modbus: dirección -> efecto.
# holding = lectura/escritura, input = solo lectura.
registros:
  holding:
    - direccion: 40001         # setpoint (°C × 10)
      muta: { setpoint: "<valor> / 10.0" }
    - direccion: 40002         # run/stop
      muta: { running: "<valor> == 1" }
  input:
    - direccion: 30001         # temperatura actual (°C × 10)
      modelo: leer_temperatura
    - direccion: 30002         # alarma
      modelo: leer_alarma

modelos:
  leer_temperatura:
    tipo: formula
    cuando: { running: true }
    expr: "temperatura_actual + (setpoint - temperatura_actual) * 0.01"
    fallback: "temperatura_actual"
  leer_alarma:
    tipo: formula
    expr: "setpoint > 150.0"
```

## Ejemplo 3: fixture custom (serial ASCII)

```yaml
# Perfil de un fixture custom con un FTDI y un protocolo serial ad-hoc.
dispositivo:
  modelo: FIXTURE-LED-01
  tipo: fixture

protocolo: serial_ascii        # protocolo de texto propietario sobre serial

estado:
  led_rojo: false
  led_verde: false
  boton: false
  fallo: false

# Comandos: mismo esquema que SCPI, pero la codificación la decide el
# protocolo serial_ascii (texto terminado en \n, comando y respuesta).
comandos:
  - patron: "LED:RED ON"
    muta: { led_rojo: true }
    respuesta: "OK"
  - patron: "LED:RED OFF"
    muta: { led_rojo: false }
    respuesta: "OK"
  - patron: "LED:GREEN ON"
    muta: { led_verde: true }
    respuesta: "OK"
  - patron: "LED:GREEN OFF"
    muta: { led_verde: false }
    respuesta: "OK"
  - patron: "BTN?"
    modelo: leer_boton
  - patron: "FAIL?"
    modelo: leer_fallo

modelos:
  leer_boton:
    tipo: formula
    expr: "boton"
  leer_fallo:
    tipo: formula
    expr: "fallo"
```

## Semántica (propuesta)

### Dispositivo

- **`dispositivo.modelo`**: identificador del modelo.
- **`dispositivo.tipo`**: `instrumento` | `daq` | `camara` | `fixture` |
  `plc` | `dut`. Es informativo (ayuda a catalogar y validar), no cambia
  la semántica del perfil.
- **`dispositivo.idn`**: cadena de identificación (para `*IDN?` en SCPI,
  o equivalente en otros protocolos).

### Protocolo

- **`protocolo`**: `scpi` | `modbus_rtu` | `modbus_tcp` | `serial_ascii` |
  `serial_binario` | `daqmx` | `vxi11` | `hislip`. Define:
  - Cómo se codifican los comandos/registros.
  - Cómo se parsea una trama entrante y se mapea a un comando/registro.
  - Cómo se codifica la respuesta.
- El runtime implementa un *codec* por protocolo. El perfil declara el
  protocolo; el codec hace el resto.

### Estado

- **`estado`**: variables que el dispositivo recuerda. Se mutan con
  comandos/registros y las leen los modelos. Tipos: `float`, `int`,
  `bool`, `string`, `enum`.

### Comandos (protocolos texto: SCPI, serial_ascii)

- **`comandos[].patron`**: string de comando; `<x>` captura un argumento.
  El runtime hace *match* en orden; el primer patrón que encaja gana.
- **`muta`**: asigna al estado (`{ clave: valor }`). Los valores pueden
  referenciar argumentos capturados (`"<x>"`) o ser literales.
- **`respuesta`**: string literal que se devuelve (sin modelo).
- **`modelo`**: nombre de un modelo en `modelos` que produce la
  respuesta. El modelo evalúa sobre el estado actual.

### Registros (protocolos binarios: Modbus)

- **`registros.holding[]`**: registros de lectura/escritura. Una
  escritura a la dirección muta el estado.
- **`registros.input[]`**: registros de solo lectura. Una lectura
  devuelve el resultado de un modelo.
- **`direccion`**: dirección del registro Modbus.
- **`muta`**: cómo se transforma el valor escrito en estado.
- **`modelo`**: nombre del modelo que produce el valor de lectura.

### Modelos de comportamiento

- **`modelos[].tipo: formula`**: `expr` evalúa sobre el estado; `cuando`
  es una guarda (si no se cumple, se usa `fallback`). Funciones
  permitidas: aritmética, `gauss(mu,sigma)` (ruido determinista vía
  semilla), `uniforme`, etc. Determinismo por defecto (semilla fija).
- **`modelos[].tipo: tabla`**: lookup table con interpolación. Para
  curvas IV, tablas de calibración, etc.
- **`modelos[].tipo: plugin`**: apunta a un módulo/trait que implemente
  la lógica compleja (una waveform real, un filtro, un modelo térmico).
  El formato lo admite; el runtime lo carga. Datos primero, código
  cuando hace falta.

## Decisiones de diseño abiertas (para mañana)

- **Determinismo del ruido**: `gauss` con semilla fija por defecto
  (reproducible) ¿o semilla por cada sesión? Propuesta: fija por defecto,
  sobreescribible en la topología.
- **Tipos de respuesta**: ¿sólo escalares (número/string), o también
  arrays/waveforms (osciloscopio)? Propuesta: empezar con escalares;
  añadir `tipo: waveform` para osciloscopios.
- **Matcheado de comandos SCPI**: ¿prefijo SCPI estándar (abreviable:
  `MEAS:VOLT?` == `MEASure:VOLTage:DC?`)? Propuesta: sí, SCPI permite
  abreviaturas; el runtime las normaliza.
- **Registros Modbus**: ¿soportar coils (1-bit) y discrete inputs
  además de holding/input registers? Propuesta: sí, el formato Modbus
  completo.
- **Validación al cargar**: el perfil se valida al cargar (comandos
  referencian modelos que existen, registros en rango, tipos coherentes)
  — fail-fast, igual que el cargador de Anvil.

## Fuera del formato (post-MVP)

- Modelos de física continua (control loops, térmica) — usar `plugin`.
- Auto-descubrimiento de perfiles desde un directorio.
- Versionado de perfiles y migración.
- Perfiles que declaran múltiples protocolos (un dispositivo que habla
  SCPI por un lado y Modbus por otro).