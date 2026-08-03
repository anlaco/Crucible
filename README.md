# Crucible

> Un **estándar abierto para describir y simular bancos de instrumentos
> SCPI** — el gemelo digital del banco de test.

Crucible nace para una idea simple: **poder probar software de test sin
el hardware**. En vez de depender de un multímetro, una fuente y un
osciloscopio físicos para desarrollar y validar tu secuenciador, describes
**el comportamiento** de esos instrumentos en un fichero y un *runtime*
los simula, hablando SCPI por TCP como un banco real.

La analogía es Simulink: ahí describes *cómo se comporta un motor* y un
runtime lo simula. Aquí describes *cómo se comporta un Keithley 2400* y el
runtime lo sirve por SCPI/TCP. La diferencia: un instrumento SCPI es, en
el 90 % de los casos, una **máquina de estado con respuestas** (recibe un
comando, muta su estado, devuelve algo) — no física continua. Empezamos
por ahí.

## Qué es y qué no es

- **Es un estándar**: un **formato declarativo** (perfil de instrumento +
  topología de banco) + un **contrato** (SCPI sobre TCP) + un **runtime de
  referencia** que lo ejecuta. Cualquier herramienta de test —Anvil,
  TestStand, OpenTAP— lo consume sin saber que es simulado.
- **No es un driver ni un VISA**: no reemplaza la capa de transporte; se
  sirve por SCPI/TCP, igual que muchos instrumentos modernos.
- **No es Simulink**: no simula física continua compleja. Modela
  instrumentos como máquinas de estado con respuestas y un modelo de
  comportamiento (al principio tablas/fórmulas; después, si hace falta,
  código).
- **No es un adorno de Anvil**: es un proyecto aparte, licencia
  Apache-2.0, consumible por cualquier secuenciador. Anvil será su primer
  cliente, no su dueño.

## Estado

Inicialización (repo + spec del formato). Ver [`docs/roadmap.md`](docs/roadmap.md).
El diseño del formato está en [`docs/diseno/`](docs/diseno/):
- [`arquitectura.md`](docs/diseno/arquitectura.md) — visión general.
- [`formato-de-perfil.md`](docs/diseno/formato-de-perfil.md) — cómo se
  describe un instrumento (con un Keithley 2400 de ejemplo).
- [`topologia-de-banco.md`](docs/diseno/topologia-de-banco.md) — cómo se
  compone un banco (varios instrumentos + DUT).

Decisiones de fondo en [`docs/adr/`](docs/adr/).

## Licencia

Apache-2.0. Elegida a propósito: que se linken y adopten como referencia
(sin el copyleft del AGPL del producto Anvil). Ver
[`docs/adr/0001-estandar-declarativo-apache-separado-de-anvil.md`](docs/adr/0001-estandar-declarativo-apache-separado-de-anvil.md).