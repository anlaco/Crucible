# First start

Crucible ships an example bench with three instruments: a DC power supply, a
multimeter and a Keithley 2400. It is there to see it working before you
describe anything.

## Start the bench

- **Linux:** run `crucible` with no arguments.
- **Windows:** Start menu → Crucible → *Arrancar banco*.

```console
{{#include ../listings/sesiones/03-arranque.txt}}
```

Each row is an instrument listening on its port (the columns: ID, model, VISA
resource). While the window stays open they are "switched on"; every incoming
connection is logged below. `Ctrl+C` switches them off (*Listo* means *ready*).

With no arguments, `crucible` uses the first bench it finds from this list:

1. `banco.yaml` in the folder you run it from.
2. `banco/banco.yaml` next to the executable (Windows installation).
3. `/usr/share/crucible/banco/banco.yaml` (Linux installation).

To start a specific one: `crucible path/to/banco.yaml`.

## Ask each instrument who it is

In **another** terminal, with the bench running:

```console
{{#include ../listings/sesiones/03-idn.txt}}
```

Each port is a different instrument, and each one answers with its own `*IDN?`.

> **`-q1` matters.** Without it, Debian's and Ubuntu's `nc` does not close the
> connection after sending, and the command waits forever.

That is all you need to get going: they are TCP sockets speaking SCPI
terminated by a newline. The next chapter connects a real program.
