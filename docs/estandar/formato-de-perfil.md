# Diseño: Formato de perfil de instrumento

> **Prioridad:** fundacional. **Propuesta** para discusión antes de
> implementar. El ejemplo (Keithley 2400) es ilustrativo; la sintaxis
> exacta se cierra mañana.

Un **perfil de instrumento** describe el comportamiento de **un**
instrumento SCPI, igual que en Simulink describes el comportamiento de un
motor. Es declarativo (YAML) y lo ejecuta el runtime de referencia, que
lo sirve por SCPI/TCP.

## Qué describe un perfil

Cuatro cosas:

1. **Identidad**: modelo, cadena de `*IDN?`, puerto SCPI/TCP por defecto.
2. **Estado interno**: las variables que el instrumento recuerda
   (voltaje de la fuente, límite de corriente, output on/off, modo…). Se
   mutan con comandos y las lee el modelo.
3. **Comandos SCPI**: cada comando (o patrón con parámetros) produce un
   efecto — una **mutación de estado** y/o una **respuesta**. Es la
   "firma" del instrumento.
4. **Modelos de comportamiento**: qué produce cada medición. Declarativos
   por defecto (fórmulas/condiciones sobre el estado); caen a **código**
   (un plugin) cuando el comportamiento es complejo.

## Ejemplo: Keithley 2400 SourceMeter

```yaml
# Perfil del Keithley 2400 (gemelo digital). Propuesta de formato.
instrumento:
  modelo: KEITHLEY-2400
  idn: "Keithley,2400,1234567,A1.2"
  puerto: 5025            # puerto SCPI/TCP que el runtime expone

# Estado interno: se muta con comandos y lo lee el modelo.
estado:
  voltaje_fuente: 0.0     # V, último SOUR:VOLT fijado
  corriente_limite: 0.105 # A, último SOUR:CURR fijado
  output: false           # OUTP ON/OFF
  modo: voltage           # voltage | current

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
    cuando: { output: true }            # sólo mide con el output encendido
    expr: "voltaje_fuente + gauss(0, 0.001)"   # ideal + ruido
    fallback: "0.0"                    # output off
  medir_corriente:
    tipo: formula
    cuando: { output: true }
    expr: "voltaje_fuente / 1000.0"    # simula una carga de 1 kΩ
    fallback: "0.0"
```

Este perfil, cargado por el runtime, responde por SCPI/TCP exactamente
como se espera: `*IDN?` → la cadena; `SOUR:VOLT 5; OUTP ON; MEAS:VOLT?`
→ ~5 V con un pelín de ruido. El consumidor (Anvil) no distingue esto de
un Keithley real.

## Semántica (propuesta)

- **`comandos[].patron`**: string SCPI; `<x>` captura un argumento.
  El runtime hace *match* en orden; el primer patrón que encaja gana.
- **`muta`**: asigna al estado (`{ clave: valor }`). Los valores pueden
  referenciar argumentos capturados (`"<x>"`) o ser literales.
- **`respuesta`**: string literal que se devuelve (sin modelo).
- **`modelo`**: nombre de un modelo en `modelos` que produce la
  respuesta. El modelo evalúa sobre el estado actual.
- **`modelos[].tipo: formula`**: `expr` evalúa sobre el estado; `cuando`
  es una guarda (si no se cumple, se usa `fallback`). Funciones
  permitidas: aritmética, `gauss(mu,sigma)` (ruido determinista vía
  semilla), `uniforme`, etc. Determinismo por defecto (semilla fija).
- **Extensión a código**: un modelo puede ser `tipo: plugin` y apuntar a
  un módulo/trait que implemente la lógica compleja (una waveform real,
  un filtro…). El formato lo admite; el runtime lo carga. Datos primero,
  código cuando hace falta.

## Decisiones de diseño abiertas (para mañana)

- **Determinismo del ruido**: `gauss` con semilla fija por defecto
  (reproducible) ¿o semilla por cada sesión? Propuesta: fija por defecto,
  sobreescribible en la topología.
- **Tipos de respuesta**: ¿sólo escalares (número/string), o también
  arrays/waveforms (osciloscopio)? Propuesta: empezar con escalares;
  añadir `tipo: waveform` para osciloscopios.
- **Matcheado de comandos**: ¿prefijo SCPI estándar (abreviable:
  `MEAS:VOLT?` == `MEASure:VOLTage:DC?`)? Propuesta: sí, SCPI permite
  abreviaturas; el runtime las normaliza.
- **Validación al cargar**: el perfil se valida al cargar (comandos
  referencian modelos que existen, tipos coherentes) — fail-fast, igual
  que el cargador de Anvil.

## Fuera del formato (post-MVP)

- Modelos de física continua (control loops, térmica) — usar `plugin`.
- Auto-descubrimiento de perfiles desde un directorio.
- Versionado de perfiles y migración.