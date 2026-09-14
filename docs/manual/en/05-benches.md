# Building your bench

A bench is a folder shaped like this:

```text
mi_banco/
├── banco.yaml             ← which instruments there are and on which port
└── perfiles/
    ├── fuente_xyz.yaml    ← one profile per instrument MODEL
    └── dmm_abc.yaml
```

## Start by copying the example

The quickest way is to start from the example bench and change it:

- **Linux:**

  ```console
  $ cp -r /usr/share/crucible/banco ~/mi_banco
  $ cd ~/mi_banco
  ```

- **Windows:** Start menu → Crucible → *Abrir carpeta del banco*, and edit it
  there or copy the folder somewhere else.

From that folder, `crucible` with no arguments starts your bench: it is the
first place it looks.

## `banco.yaml`

This is the example's (its comments are in Spanish; the text below explains
every field):

```yaml
{{#include ../../../banco/banco.yaml}}
```

Each entry under `dispositivos` is **one unit**:

| Field | What it is |
|---|---|
| `id` | A name for you. It appears in the start-up table and in errors. |
| `perfil` | The instrument's profile, with its path **relative to `banco.yaml`**. |
| `transporte` | Where it listens: `{ tipo: tcp, puerto: 5025 }`. Today `tipo` can only be `tcp`. |
| `estado_inicial` | Optional. Values that replace those in the profile's `estado` for this unit only. |

Two units of the same model **share a profile** and use different ports, and,
if needed, a different `estado_inicial`:

```yaml
    - id: dmm_entrada
      perfil: perfiles/dmm_abc.yaml
      transporte: { tipo: tcp, puerto: 5026 }

    - id: dmm_salida
      perfil: perfiles/dmm_abc.yaml
      transporte: { tipo: tcp, puerto: 5027 }
      estado_inicial:
        tension_entrada: 12.0
```

## Validate before starting

```console
{{#include ../listings/sesiones/05-validar.txt}}
```

`--validar` loads the bench and all its profiles, checks patterns, formulas and
ports, and shows the resource table **without opening any port**, so you can
use it while the bench is running. Do it every time you edit a YAML file: the
errors say which file, which command and what is wrong.

Each instrument needs its own port. With this bench, which repeats 5025:

```yaml
{{#include ../listings/puerto_repetido.yaml}}
```

```console
{{#include ../listings/sesiones/05-puerto-repetido.txt}}
```

The error says both devices use 127.0.0.1:5025 and each instrument needs its
own port.

## Three rules worth knowing

**If one profile has an error, the whole bench does not start.** On purpose: a
bench missing an instrument would give false results without anyone noticing.

**`*RST` returns the unit to the profile's values, not to `estado_inicial`.**
The example multimeter starts with 3.3 V on its terminals, but after `*RST` it
reads 0 V (you will see it in chapter 8). If your software sends `*RST` at the
start, set the value in the profile or set it again over SCPI afterwards.

**A misspelled variable in `estado_inicial` is not an error.** Here
`tension_entarda` does not exist in the multimeter profile, and still:

```yaml
{{#include ../listings/errata.yaml}}
```

```console
{{#include ../listings/sesiones/05-errata.txt}}
```

(*es válido* means *is valid*.) The multimeter will start reading 0 V. If a unit
does not start with the value you expected, check the spelling first.
