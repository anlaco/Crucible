# Models: what the instrument measures

A query with `respuesta: "{tension}"` returns what was configured. A measurement
is something else: the current a supply delivers depends on the voltage, the
load and whether the output is on. That is a **model**.

```yaml
modelos:
  corriente_medida:
    tipo: formula
    cuando: { salida: true }
    expr: "min(tension / carga_ohm, limite_corriente) + gauss(0, 1e-5)"
    fallback: 0.0
    formato: fijo:5
```

And the query that uses it:

```yaml
  - patron: "MEASure[:SCALar]:CURRent[:DC]"
    query: true
    modelo: corriente_medida
```

## Fields

| Field | What it is |
|---|---|
| `tipo` | Always `formula`. |
| `expr` | The formula. |
| `cuando` | Conditions on the state. If any is not met, `fallback` is answered. If there are several, all must hold. |
| `fallback` | The answer when `cuando` is not met. Defaults to `0.0`. Number or text. |
| `formato` | How the number is written. Optional. |

## What goes in `expr`

- State variables, by name: `tension`, `carga_ohm`.
- Numbers: `10`, `0.5`, `1e-3`, `2.5E+2`.
- `+ - * /` with the usual precedence, parentheses and unary minus.
- Functions:

| Function | Result |
|---|---|
| `gauss(mean, sigma)` | Gaussian noise. It changes on every read, but the series repeats the same each time you start Crucible, so automated tests are reproducible. |
| `abs(x)` | Absolute value. |
| `sqrt(x)` | Square root. |
| `min(a, b)`, `max(a, b)` | Minimum and maximum. |

There are no conditionals inside the formula: `cuando` only chooses between the
formula and a fixed `fallback`.

## The supply's model, running

Against a freshly started bench:

```console
{{#include ../listings/sesiones/07-modelos.txt}}
```

Step by step:

1. With the output off `cuando` is not met, and the measurement is the
   `fallback`, written in the model's format: `0.00000`.
2. `OUTP?` answers `0`, not `false`: it is answered by the `salida_como_numero`
   model, with `formato: entero`, like a real instrument.
3. At 5 V across 10 Ω, half an ampere plus noise.
4. At 20 V it would be 2 A, but `min` clips it to the 1 A limit: the supply goes
   into constant current.
5. The multimeter writes scientific notation, like most do.

## `formato`

Your software expects numbers written the way the real instrument writes them:

| `formato` | Example answer | Typical use |
|---|---|---|
| *(not set)* | `1.0`, `4.501385029307777` | — |
| `entero` (integer) | `1`, `0`, `-3` | States (`OUTP?`) and counters. |
| `fijo:N` (fixed) | `fijo:4` → `4.5014` | Supplies that answer with fixed decimals. |
| `cientifico:N` (scientific) | `cientifico:6` → `+4.501385E+00` | Multimeters and most SCPI measurements. |

The format also applies to `fallback` when it is numeric.

Queries with `respuesta: "{variable}"` return the value as it is (`5.0`). If
your software expects another form, use a model whose `expr` is the variable,
with the right `formato`.

## Errors in formulas

Syntax errors —an unclosed parenthesis, a function that does not exist— show up
when validating or starting, with the position:

```yaml
{{#include ../listings/perfiles/formula_rota.yaml}}
```

```console
{{#include ../listings/sesiones/07-formula-rota.txt}}
```

The message says the function `gaus` is unknown, lists the ones that exist and
gives the position.

A **variable that does not exist** in the state, however, is only detected when
the query is made: no answer arrives and `SYST:ERR?` returns
`-200,"Execution error;…la variable '…' no existe en el estado…"` (*the variable
does not exist in the state*). Declare in `estado` every variable your formulas
use.
