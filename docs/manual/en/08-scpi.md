# What every instrument already does

Some things every SCPI instrument does the same way, and Crucible provides them
ready-made. They are not declared in the profile.

## Common commands

| Command | What it does |
|---|---|
| `*IDN?` | Returns `dispositivo.idn`. |
| `*RST` | Returns the state to the profile's values. |
| `*CLS` | Clears the error queue and the status register. |
| `*OPC`, `*OPC?`, `*WAI` | Synchronisation. Always ready: nothing takes time. |
| `*ESE`, `*ESE?`, `*ESR?`, `*SRE`, `*SRE?`, `*STB?`, `*TST?` | IEEE 488.2 registers. |
| `SYSTem:ERRor[:NEXT]?` | The next error in the queue, e.g. `-113,"Undefined header;FOO"`. |
| `SYSTem:VERSion?` | The SCPI version. |

## Compound messages

Several commands separated by `;` run in order, and query answers come back on
one line, also separated by `;` (you saw it in chapter 7: `0.99999;1`).

There is an SCPI rule worth being clear about. After a `;`, the next header is
**relative to the previous one's node**, unless it starts with `:`:

```console
{{#include ../listings/sesiones/08-compuestos.txt}}
```

- `VOLT 5;OUTP ON` works: `VOLT` hangs off the root, so `OUTP` does too.
- `SOUR:VOLT 3;OUTP OFF` does not: the current node is `SOUR`, and `OUTP` is
  read as `SOUR:OUTP`, which does not exist. The output stays on.
- With a leading `:` (`;:OUTP OFF`) you go back to the root, and it always
  works.

A real instrument does exactly the same. When in doubt, write `;:`.

## The state is shared

Each device has **a single state**, shared by every connection. It is not
cloned per client or reset on disconnect: just as a real device does not reset
when someone else picks it up.

The example multimeter has a command the real one does not, `SIMulate:INPut`,
to set the voltage on its terminals from a script. Every command in this session
is a separate connection:

```console
{{#include ../listings/sesiones/08-estado-compartido.txt}}
```

The 12 V set by one connection is measured by the next. And after `*RST` it goes
back to 0 V —the **profile's** value—, not to the 3.3 V of the `estado_inicial`
the bench started it with.

In practice:

- If your software configures something and exits, the next run finds it
  configured. Send `*RST` at the start, or restart Crucible, if you want to
  start from scratch.
- `SIMulate:INPut` is the way to **chain instruments**: devices do not see each
  other, so your script reads the supply and passes the value to the
  multimeter. You can add commands like it to any profile.

## Errors do not drop the connection

An unknown command, a missing argument or a formula that cannot be evaluated
leaves its error in the queue and the connection stays open. Your software sees
it as on the real instrument: by asking `SYST:ERR?`.
