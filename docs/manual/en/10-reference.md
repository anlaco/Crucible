# Reference

## The `crucible` command

```console
{{#include ../listings/sesiones/10-ayuda.txt}}
```

| Form | What it does |
|---|---|
| `crucible` | Starts the first bench it finds: `banco.yaml` in the current folder, then next to the executable, then the installed one. |
| `crucible <banco.yaml>` | Starts every instrument on the bench. |
| `crucible <profile.yaml> [port]` | Starts a single instrument, without a bench. Port 5025 if not given. |
| `crucible --validar <file>` | Checks a bench or a profile without opening ports. Exits with code 1 on errors. |
| `crucible --version` | The version. |
| `crucible --help` | The help. Also `-h` and `--ayuda`. |

Whether the file is a bench or a profile is decided by its content, not by an
option: it has `banco:` or it has `dispositivo:`.

Any error makes `crucible` exit with code 1. With no arguments —usually a
double-click on Windows— it also waits for you to press Enter, so the window
does not close before you can read why.

## `banco.yaml`

```yaml
banco:
  nombre: <text>                    # optional
  dispositivos:
    - id: <text>                    # required
      perfil: <path>                # required; relative to banco.yaml
      transporte:
        tipo: tcp                   # required; today only tcp
        puerto: <number>            # required; different for each device
        host: <address>             # optional; 127.0.0.1 by default
      estado_inicial:               # optional
        <variable>: <value>
```

`host` decides which interface the device listens on. By default `127.0.0.1`,
this computer only. With `host: 0.0.0.0` it listens on all of them, and another
computer on the network can connect; in that case the start-up table shows
`TCPIP0::0.0.0.0::…`, which is not an address you can connect to: use the IP of
the computer running Crucible. Access from another computer is **not verified**
in this manual.

## Profile

```yaml
dispositivo:
  modelo: <text>                    # required
  idn: <text>                       # answer to *IDN?
protocolo: scpi                     # required

estado:
  <variable>: <number | true | false | text>

comandos:
  - patron: <header>                # required; no '?', no arguments, no '*'
    query: <true | false>           # false by default
    args: [<name>, ...]
    muta: { <variable>: <value | "<arg>"> }
    respuesta: <text with {variable} and <arg>>   # queries
    modelo: <model name>                          # queries

modelos:
  <name>:
    tipo: formula                   # required
    expr: <formula>                 # required
    cuando: { <variable>: <value> }
    fallback: <number | text>       # 0.0 by default
    formato: <entero | fijo:N | cientifico:N>
```

## Formulas

| Element | Examples |
|---|---|
| Variables | `tension`, `carga_ohm` |
| Numbers | `10`, `0.5`, `1e-3`, `2.5E+2` |
| Operators | `+ - * /`, parentheses, unary minus |
| Functions | `gauss(mean, sigma)`, `abs(x)`, `sqrt(x)`, `min(a, b)`, `max(a, b)` |

## SCPI errors you will see

| Error | When |
|---|---|
| `0,"No error"` | The queue is empty. |
| `-113,"Undefined header;…"` | The header is not in the profile, or not as a command or query. |
| `-200,"Execution error;…"` | A formula could not be evaluated, e.g. a variable that is not in `estado`. |
