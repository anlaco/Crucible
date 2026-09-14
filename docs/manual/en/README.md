# Crucible Manual

Crucible simulates a bench of measurement instruments. You describe each
instrument in a YAML file and Crucible serves it over the network speaking
**real SCPI**, just like the physical device. Your test software —PyVISA,
LabVIEW, a sequencer such as [Anvil](https://anlaco.github.io/anvil/)— connects
to it as if the bench were plugged in, without knowing it is not.

This manual is also [in Spanish](../es/): the **ES** button in the top bar
takes you to the same page in the other language.

This manual explains how to use it: install it, start a bench, connect your
software and describe your own instruments.

> **Version 0.1.0.** Written and checked against **Crucible 0.1.0 on Linux
> x86-64**. Crucible is young: the chapter [What it does not do
> (yet)](11-limitations.md) says plainly where it falls short.

## Where to start

- **I want to see it working:** chapters 2 to 4, in order.
- **I want to simulate my bench:** then 5 to 9. [Chapter 9](09-recipe.md) is the
  step-by-step recipe.
- **I am looking for a field or an option:** [Reference](10-reference.md).
- **Something does not answer:** [Troubleshooting](12-troubleshooting.md).

## Conventions

Commands start with `$` and are run from **your bench folder**, a copy of the
example bench (chapter 5 shows how to make it), with the bench running in
another terminal, unless the text says otherwise. The lines after a command are
what it printed.

Terminal examples use `nc` (netcat) because every Linux has it. On Windows,
chapter 4 gives the PowerShell equivalent.

## Crucible speaks Spanish, for now

Crucible 0.1.0 prints its messages in Spanish, and the file format uses Spanish
keys: a bench is `banco`, a profile has `estado`, `comandos` and `modelos`. This
manual shows everything exactly as Crucible prints and reads it, and explains
what it means. The keys are not translatable: write them as shown.

| Key | Meaning |
|---|---|
| `banco`, `dispositivos` | bench, devices |
| `perfil`, `transporte`, `puerto` | profile, transport, port |
| `estado`, `estado_inicial` | state, initial state |
| `comandos`, `patron`, `muta`, `respuesta` | commands, pattern, mutates, response |
| `modelos`, `cuando`, `formato` | models, when, format |

## How this manual stays true

Nothing a command prints was typed by hand. Every terminal session is a file in
[`listings/sesiones/`](https://github.com/anlaco/Crucible/tree/main/docs/manual/listings/sesiones),
shared by the Spanish and English editions and included as it is, and
[`check.sh`](https://github.com/anlaco/Crucible/blob/main/docs/manual/check.sh)
runs them all again against the binary and fails if any no longer matches. The
example bench and profiles shown in these pages are the repository files, not
copies.

```console
$ docs/manual/check.sh target/debug/crucible
```

What could not be run here is marked **not verified**, in those words.

For how Crucible is built inside —the format as a standard, the design
decisions— see the
[repository documentation](https://github.com/anlaco/Crucible/tree/main/docs)
(in Spanish).
