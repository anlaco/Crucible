# Describing an instrument

A **profile** describes an instrument model: who it is, what it remembers,
which commands it understands and what it does with each one. It has five
sections:

```yaml
dispositivo:   # who it is
protocolo:     # how it is spoken to; today, always scpi
estado:        # its internal variables and their power-on value
comandos:      # what it understands and what it does with each thing
modelos:       # how it computes what it "measures" (chapter 7)
```

This is the profile of the supply on the example bench. It is worth reading in
full (the comments are in Spanish): the rest of the chapter takes it apart.

```yaml
{{#include ../../../banco/perfiles/fuente_dc.yaml}}
```

## `dispositivo`

```yaml
dispositivo:
  modelo: DMM-ABC                        # required; shown in the start-up table
  idn: "ACME,DMM-ABC,SN12345,FW2.01"     # what *IDN? answers
```

If your software checks `*IDN?` to recognise the instrument, copy the real
device's answer here **exactly**.

## `protocolo`

`scpi`. The format reserves other values (`modbus_tcp`, `serial_ascii`…) for
what is coming, but today a profile that is not `scpi` does not start.

## `estado`

The variables the instrument remembers, with their power-on value, the one
they return to on `*RST`:

```yaml
estado:
  tension: 0.0
  salida: false
  funcion: VOLT
```

Numbers, `true`/`false` and text are accepted. The state is **one per device,
shared by every connection**: chapter 8 explains what that implies.

## `comandos`

Each entry is **a command** or **a query**. If a command has both forms, like
`VOLT 5` and `VOLT?`, it is declared twice:

```yaml
comandos:
  # Command: VOLT 5  →  stores 5 in the 'tension' variable
  - patron: "[SOURce:]VOLTage[:LEVel]"
    args: [v]
    muta: { tension: "<v>" }

  # Query: VOLT?  →  answers with the value of 'tension'
  - patron: "[SOURce:]VOLTage[:LEVel]"
    query: true
    respuesta: "{tension}"

  # Computed query: MEAS:VOLT?  →  answered by the 'lectura' model
  - patron: "MEASure:VOLTage[:DC]"
    query: true
    modelo: lectura
```

| Field | What it is |
|---|---|
| `patron` | The header in manual notation (below). **No `?` and no arguments.** |
| `query` | `true` if it is the query. Otherwise, it is a command. |
| `args` | Names for incoming arguments, in order: with `args: [v]`, in `VOLT 5` `<v>` is `5`. |
| `muta` | State variables it changes: `{ tension: "<v>" }`, or a fixed value, `{ funcion: VOLT }`. |
| `respuesta` | (queries) The text to answer. `{variable}` is replaced by the state and `<arg>` by the argument. |
| `modelo` | (queries) The model in `modelos` that computes the answer. |

Every query needs `respuesta` or `modelo`. A command may carry only `muta`, or
nothing if being accepted without error is enough.

### Pattern notation

It is the notation of manufacturers' manuals:

- **Upper case** is the required short form, and lower case completes the long
  form. `VOLTage` accepts `VOLT` and `VOLTAGE`, in upper or lower case.
- **Brackets** mark nodes that can be left out. `[SOURce:]VOLTage[:LEVel]`
  accepts `VOLT`, `SOUR:VOLT`, `volt:lev`, `SOURCE:VOLTAGE:LEVEL`…

One pattern thus covers every form the real instrument accepts, without listing
them. Against the example supply:

```console
{{#include ../listings/sesiones/06-patrones.txt}}
```

Commands answer nothing, as on a real instrument: that is why no line appears
after `VOLT 5`.

> Copy patterns from the manufacturer's manual (`MEASure:VOLTage[:DC]`), not the
> short form your software uses (`MEAS:VOLT`). That way the profile accepts
> whatever any other program sends.

### Arguments

- `ON`, `OFF`, `1` and `0` are stored as true or false, and models treat them
  the same.
- If a declared argument does not arrive (`CONF:VOLT` with no range), the
  variable **stays as it was**.
- Without names in `args`, arguments are referred to by position: `<0>`, `<1>`.
- A reference that does not exist, such as `{no_such_variable}`, is answered
  literally, braces included. If you see braces in an answer, there is a typo in
  the profile.

### Multi-channel instruments

One pattern per channel, with the number inside:

```yaml
  - patron: "SOURce1:VOLTage"
    args: [v]
    muta: { tension_1: "<v>" }
  - patron: "SOURce2:VOLTage"
    args: [v]
    muta: { tension_2: "<v>" }
```

`SOUR1:VOLT 3` goes to channel 1 and `SOUR2:VOLT 7` to channel 2. `SOUR:VOLT`,
with no number, is **not** taken as channel 1: if your software sends it that
way, declare the unnumbered pattern too. A channel list such as
`MEAS:VOLT? (@2)` arrives as an argument with the text `(@2)`.

## When a command is not in the profile

The connection is not dropped and nothing is answered: the error goes into the
error queue, as the instrument would do. `VOLT:PROT` is not in the supply's
profile:

```console
{{#include ../listings/sesiones/06-errores.txt}}
```

`SYST:ERR?` is the first tool for completing a profile: it tells you which
header it did not recognise. Each read takes one error off the queue; when it is
empty, it answers `0,"No error"`.

## What you do not declare

`*IDN?`, `*RST`, `SYST:ERR?` and the other common commands are handled by
Crucible for every instrument (full list in chapter 8). Declaring them is an
error:

```console
{{#include ../listings/sesiones/06-comun-declarado.txt}}
```

The message says `*RST` is an IEEE 488.2 common command, which the engine
resolves, and that `*IDN?` comes from `dispositivo.idn`.
