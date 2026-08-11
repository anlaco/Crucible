# Herencia de Crucible

Crucible era un proyecto hermano —un **estándar abierto para describir y simular bancos de
instrumentos SCPI**— que se cerró el 2026-08-11 y se absorbió aquí. Nunca pasó de
documentación: un commit, cero código.

Se cerró porque hacía lo mismo que InstruSim con las mismas horas, y InstruSim ya tenía el
motor. Pero la parte que Crucible sí tenía resuelta —**la ambición de estándar**— no debía
morir con el repositorio, y es lo que guardan estos documentos.

## Qué se conserva y por qué

InstruSim ya nació declarativo (`docs/PLAN.md`: catálogo en YAML, comportamiento en plugin),
así que la tesis de fondo no se pierde. Lo que aportan estos cuatro documentos es lo que
InstruSim no tenía escrito:

| Documento | Qué aporta que no estuviera aquí |
|---|---|
| `0001-...-apache-separado-de-anvil.md` | **El razonamiento estratégico.** Por qué esto es un estándar y no una feature de Anvil, por qué Apache y no AGPL, y las cuatro alternativas rechazadas |
| `topologia-de-banco.md` | El **banco entero** —instrumentos + DUT + conexiones— como dato, no solo el instrumento aislado. Es el hueco que PyVISA-sim no cubre |
| `formato-de-perfil.md` | Propuesta de formato del perfil de instrumento |
| `arquitectura.md` | Visión general del runtime de referencia |

## Estado: propuestas, no especificación

**Ninguno de estos documentos describe lo que InstruSim hace hoy.** Son propuestas escritas
antes de existir el código, y el motor de InstruSim tomó decisiones propias por su cuenta. Los
formatos YAML que aparecen aquí son ilustrativos; el catálogo real vive en `docs/PLAN.md` y en
`crates/`.

Leerlos como especificación sería el error. Están aquí como material de partida para cuando
toque publicar el formato de verdad.

## Qué queda por decidir

1. **Si InstruSim se posiciona como estándar o como herramienta.** La diferencia no es
   cosmética: un estándar necesita el formato versionado, documentado aparte del motor, y una
   suite de conformidad que permita a otro escribir un runtime alternativo. Una herramienta
   solo necesita funcionar.
2. **Topología de banco.** InstruSim tiene `rack.rs` y un modelo de señales; falta ver cuánto
   de `topologia-de-banco.md` sobrevive al contacto con ese diseño.
3. **Licencia.** Crucible era Apache-2.0 puro por razones de adopción (ver el ADR). InstruSim
   es MIT/Apache dual, que sirve igual de bien.

## Contexto

El consumidor previsto era Anvil, que ya tiene un paso SCPI/TCP (`pasos_scpi`, ADR-0017) y hoy
solo está probado contra un mock. Ese sigue siendo el caso de uso que justifica InstruSim:
correr secuencias reales de Anvil contra instrumentos simulados, sin hardware y sin
*flakiness*, incluido en CI.
