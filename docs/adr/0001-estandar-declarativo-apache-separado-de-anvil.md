# ADR-0001: Estándar declarativo, Apache-2.0, separado de Anvil

- **Estado:** Aceptada
- **Fecha:** 2026-08-03 (inicialización del proyecto)

## Contexto

Anvil (el secuenciador, punta de lanza de ANLACO) necesita validarse y
demostrarse **sin hardware**. El paso SCPI de Anvil está probado sólo
contra un mock. La idea natural —un simulador de instrumentos— se elevó a
algo más ambicioso: un **estándar abierto para describir y simular bancos
de instrumentos SCPI**, un *gemelo digital* del banco de test, estilo
Simulink ("describe cómo se comporta un instrumento y un runtime lo
simula").

La pregunta de fondo: ¿esto es una feature de Anvil, o un proyecto
aparte? Y si es aparte, ¿con qué licencia y qué forma?

## Decisión

**Crucible es un proyecto independiente** de Anvil, con tres rasgos:

1. **Estándar declarativo**: el comportamiento se describe en **YAML**
   (perfil de instrumento + topología de banco), no en código. Un runtime
   de referencia lo ejecuta y lo sirve por **SCPI sobre TCP**. El modelo
   es declarativo por defecto y cae a código (plugin) cuando hace falta
   — el patrón "datos primero, código cuando hace falta", igual que Anvil.

2. **Licencia Apache-2.0**, no AGPL. El estándar y el runtime de referencia
   quieren **adoptarse como referencia** y linkarse en código ajeno. El
   AGPL (que protege a Anvil como producto) asustaría a quien quiera
   consumir el estándar. Apache lo deja libre sin ese peso.

3. **Separado de Anvil**: Anvil es el primer **cliente**, no el dueño.
   Crucible se sirve por SCPI/TCP; cualquier secuenciador (Anvil, TestStand,
   OpenTAP) o script (PyVISA) lo consume sin acoplarse. Si sólo sirviera a
   Anvil, no sería estándar.

## Por qué esta forma

- **Reusable y posicionable**: un estándar abierto del *banco entero*
  (topología + interacciones) no existe hoy (PyVISA-sim modela
  instrumentos aislados; pytestlab es un framework, no un estándar; IVI es
  pesado y de la era NI). El hueco es real.
- **Desacoplado del consumidor**: servirse por SCPI/TCP significa cero
  cambios en el consumidor para pasar de real a simulado. Demuestra que el
  instrumento vive detrás del paso, opaco al motor (coherente con Anvil
  ADR-0003).
- **Determinismo para CI**: el modo determinista del runtime permite
  tests reproducibles sin hardware ni flakiness — el valor para Anvil.
- **Apache fomenta la adopción**: que otros linken el runtime o escriban
  runtimes alternativos para el mismo formato es *bueno* para el estándar.

## Alternativas rechazadas

- **Simulador ad hoc dentro de Anvil (un mock ampliado)**: sirve a Anvil
  pero no a nadie más; no es estándar; no crece a un gemelo del banco.
- **Simulador acoplado a Anvil (crate dentro del repo Anvil)**: lo ata al
  producto y a su licencia AGPL; repele a terceros.
- **Reinventar Simulink (motor de simulación física continua)**: décadas
  de ingeniería, alcance desmesurado. Un instrumento SCPI es una máquina
  de estado con respuestas; empezar por ahí.
- **IVI / drivers tipados**: estándar "oficial" pero pesado, acoplado a
  Windows/COM, de la era NI. No es el modelo.

## Recortes iniciales (MVP-parcial del estándar)

- **No física continua**: máquinas de estado + modelos (fórmulas/tablas),
  caen a plugin si hace falta.
- **No record/replay en el primer hito**: se pospone (requiere haber
  tenido un instrumento real para grabar).
- **DUT como caja trivial al principio**: lo difícil de modelar es el
  dispositivo bajo test, no los instrumentos; se empieza con modelos
  simples (resistencia) y crece.
- **Lenguaje del runtime de referencia**: decidir mañana. Propuesta:
  **Rust** (coherente con Anvil, compila a WASM, librería Apache).

## Consecuencias

- Crucible nace como **proyecto hermanado** de Anvil (mismo directorio
  padre, `01-PRODUCTOS/`), repositorio y licencia propios.
- El primer hito del roadmap es: runtime de referencia mínimo + perfil
  de un Keithley 2400 + banco ejemplo + servir por SCPI/TCP en loopback,
  para que Anvil corra una secuencia contra el gemelo.
- Anvil **no cambia** para consumirlo: ya tiene un paso SCPI/TCP
  (`pasos_scpi`). Crucible valida la apuesta de desacoplamiento de Anvil.

## Relaciona

- [arquitectura.md](../diseno/arquitectura.md)
- [formato-de-perfil.md](../diseno/formato-de-perfil.md)
- [topologia-de-banco.md](../diseno/topologia-de-banco.md)
- [roadmap.md](../roadmap.md)