# Crucible

> Un **estándar abierto para describir y simular bancos de test** — el
> gemelo digital del banco.

Crucible nace para una idea simple: **poder probar software de test sin
el hardware**. En vez de depender de un multímetro, una fuente, un
osciloscopio, una cámara térmica y un DAQ físicos para desarrollar y
validar tu secuenciador, describes **el comportamiento** de esos
dispositivos en un fichero y un *runtime* los simula, hablando el
protocolo correcto por el transporte correcto, como un banco real.

La analogía es Simulink: ahí describes *cómo se comporta un motor* y un
runtime lo simula. Aquí describes *cómo se comporta un Keithley 2400* — o
una cámara térmica, o un fixture custom — y el runtime lo sirve. La
diferencia: un dispositivo de test es, en el 90 % de los casos, una
**máquina de estado con respuestas** (recibe un comando o una escritura a
un registro, muta su estado, devuelve algo) — no física continua.
Empezamos por ahí.

## Tres capas, no una

Un banco de test no es solo instrumentos SCPI por TCP. Es una mezcla
heterogénea de dispositivos, protocolos y transportes. Crucible separa
las tres capas que el diseño inicial confundía:

- **Dispositivo** (perfil): QUÉ hace — comportamiento, estado, modelos.
- **Protocolo**: CÓMO se habla — SCPI, Modbus, serial custom, etc.
- **Transporte**: POR DÓNDE — TCP, GPIB, USB, serial, PXI.

Un Keithley 2400 es el mismo dispositivo hable SCPI por GPIB, TCP o
USB-TMC. Una cámara térmica habla Modbus, no SCPI. Un fixture custom
habla un protocolo serial ad-hoc. El formato los describe a todos; el
runtime de referencia los sirve.

## Qué es y qué no es

- **Es un estándar**: un **formato declarativo** (perfil de dispositivo +
  topología de banco) + un **contrato** (protocolo sobre transporte) + un
  **runtime de referencia** que lo ejecuta. Cualquier herramienta de test
  —Anvil, TestStand, OpenTAP— lo consume sin saber que es simulado.
- **No es un driver ni un VISA**: no reemplaza la capa de transporte; se
  alinea con VISA (mismos transportes, resource strings) pero va más
  allá: simula el comportamiento del dispositivo, no solo lo transporta.
- **No es Simulink**: no simula física continua compleja. Modela
  dispositivos como máquinas de estado con respuestas y un modelo de
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
  describe un dispositivo (con ejemplos SCPI, Modbus y serial custom).
- [`topologia-de-banco.md`](docs/diseno/topologia-de-banco.md) — cómo se
  compone un banco (varios dispositivos + DUT, multi-transporte).

Decisiones de fondo en [`docs/adr/`](docs/adr/):
- [`0001`](docs/adr/0001-estandar-declarativo-apache-separado-de-anvil.md)
  — estándar declarativo, Apache, separado de Anvil.
- [`0002`](docs/adr/0002-separacion-de-capas-transporte-protocolo-dispositivo.md)
  — separación de capas transporte / protocolo / dispositivo.

## Licencia

Apache-2.0. Elegida a propósito: que se linken y adopten como referencia
(sin el copyleft del AGPL del producto Anvil). Ver
[`docs/adr/0001-estandar-declarativo-apache-separado-de-anvil.md`](docs/adr/0001-estandar-declarativo-apache-separado-de-anvil.md).