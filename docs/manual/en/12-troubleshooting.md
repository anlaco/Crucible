# Troubleshooting

## «no puede escuchar en 127.0.0.1:5025»

*Cannot listen on 127.0.0.1:5025.*

```console
{{#include ../listings/sesiones/12-puerto-ocupado.txt}}
```

Another Crucible, or another program, already has that port. Close the other
window or change the port in `banco.yaml`. To see who has it:

- Linux: `ss -ltnp | grep :5025`
- Windows: `netstat -ano | findstr :5025`

## «no encuentro ningún banco»

*I cannot find any bench.*

```console
{{#include ../listings/sesiones/12-sin-banco.txt}}
```

You ran `crucible` with no arguments in a folder without `banco.yaml`, and there
is no example bench installed where it looks (the message lists the places it
searched). Give the path: `crucible ~/mi_banco/banco.yaml`. (With no arguments,
Crucible waits for Enter —*Pulsa Enter para cerrar*— in a terminal too.)

## The Windows window closes on double-click

On an error, the window waits for you to press Enter. If it still closes, open
it from a terminal (`cmd`) to read the message.

## `nc` hangs

`-q1` is missing: Debian's and Ubuntu's `nc` does not close the connection after
sending. `printf '*IDN?\n' | nc -q1 127.0.0.1 5025`.

## My software times out

- **Missing termination** `\n` on writes: Crucible does not process the line
  until the newline. With `::SOCKET` resources, set it in VISA (chapter 4).
- **It is a command**, with no `?`: it answers nothing, like a real instrument.
  Do not read after a command.
- **The query is not in the profile**, or a formula failed: no answer arrives
  and the error is queued. Ask `SYST:ERR?`.

## `SYST:ERR?` returns `-113,"Undefined header;…"`

That header is not in the profile. Check the pattern, and whether it is a
command or a query (`query: true`).

## A chained command does nothing

`SOUR:VOLT 3;OUTP OFF` leaves a `-113` with `SOUR:OUTP`. It is the SCPI rule:
after `;`, the header is relative to the previous node. Write `;:OUTP OFF`
(chapter 8).

## `MEAS:…?` always returns `0.0`

The model's `cuando` is not met, for instance because the output is off. Check
the state with its query (`OUTP?`).

## An answer comes out with braces: `{tensoin}`

The variable does not exist in the state: there is a typo in `respuesta`.

## A unit does not start with its `estado_inicial` value

The variable name is misspelled: `estado_inicial` does not warn about variables
that do not exist (chapter 5). Or something sent `*RST`, which returns to the
profile's values.

## I changed a YAML file and nothing changed

Crucible reads the files at start-up. Stop it with `Ctrl+C` and start it again.

## A configured value "stuck" from a previous run

The state is shared and lasts while Crucible is running (chapter 8). Send `*RST`
at the start of your tests or restart Crucible.
