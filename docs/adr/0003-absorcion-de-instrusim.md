# ADR-0003: Absorción de InstruSim

- **Estado:** Aceptada
- **Fecha:** 2026-08-11
- **Deroga parcialmente:** nada. Complementa [ADR-0001](0001-estandar-declarativo-apache-separado-de-anvil.md) y [ADR-0002](0002-separacion-de-capas-transporte-protocolo-dispositivo.md).

## Contexto

ANLACO mantenía **dos proyectos con la misma tesis**: simular dispositivos de test
para poder validar software sin hardware.

- **Crucible** (este repositorio): partió del formato. Estándar declarativo, separación
  de tres capas (dispositivo / protocolo / transporte), alineación con VISA, WASM como
  target de primera clase. ~1.000 líneas de Rust y un runtime de referencia mínimo:
  cargador YAML, codec SCPI por patrones, evaluador de fórmulas, servidor TCP.
- **InstruSim**: partió del motor. ~5.500 líneas de Rust en cinco crates: reloj virtual
  y señales evaluables en el tiempo, SCPI a fondo (parser, patrones, formato, modelo de
  estado IEEE 488.2, cola de errores), modelos de DMM y fuente, capa de red y CLI.

Dos proyectos compitiendo por las mismas horas de una sola persona. Uno sobraba.

**La decisión inicial fue la contraria a esta** —cerrar Crucible y quedarse con
InstruSim— y se tomó sobre un dato falso: una copia local desactualizada de Crucible que
solo tenía documentación. El repositorio real tenía el runtime H1. Corregido el dato, la
dirección se invirtió.

## Decisión

**Crucible absorbe InstruSim.** El repositorio de InstruSim se cierra; su historial
completo se fusiona aquí (`git merge --allow-unrelated-histories`), no se copia.

### Por qué en esta dirección

Aunque InstruSim tenía cinco veces más código, **Crucible tenía el marco correcto**:

1. **Tres capas.** InstruSim asume SCPI sobre TCP; Crucible separa dispositivo,
   protocolo y transporte desde el diseño. Un banco real lleva Modbus y serie, no solo
   SCPI. Ese rediseño es caro de retrofitear y ya estaba hecho aquí.
2. **Posicionamiento como estándar.** El objetivo es que un tercero escriba un runtime
   alternativo para el mismo formato. Eso exige que el formato sea el centro y el motor
   un detalle, que es exactamente la relación que Crucible plantea y la inversa de
   InstruSim.
3. **WASM como target de primera clase** (ADR-0002): el runtime cargándose como
   componente dentro de Anvil, y `wasi-VISA` exponiendo GPIB/USB-TMC/serie/PXI a
   componentes WASM. InstruSim no lo contemplaba, y es la tesis de la casa.
4. **Apache-2.0 por decisión de producto**, argumentada en ADR-0001.

Lo que InstruSim aportaba —profundidad de dominio— es **portable**: un parser SCPI y un
modelo de estado IEEE 488.2 valen igual bajo cualquier marco. Al revés no: el marco de
tres capas habría que reconstruirlo entero.

## Estado tras la fusión

**Los dos linajes conviven; no están fusionados.** El árbol tiene siete crates y **dos
implementaciones de SCPI**:

| Crate | Linaje | Qué hace |
|---|---|---|
| `crucible-core` | Crucible | Perfiles YAML, codec SCPI por patrones, modelos fórmula |
| `crucible` | Crucible | CLI + runtime TCP (tokio) |
| `instrusim-core` | InstruSim | Reloj virtual, señales, mundo, disparos |
| `instrusim-scpi` | InstruSim | Parser SCPI, patrones, formato, estado IEEE 488.2, errores |
| `instrusim-model` | InstruSim | Contrato de instrumento, DMM, fuente, rack |
| `instrusim-net` | InstruSim | Capa de red |
| `instrusim-cli` | InstruSim | Binario `instrusim` |

Compila entero y pasan **166 tests**. Es un punto de partida honesto, no un final: dos
runtimes y dos SCPI en el mismo árbol son deuda, y está declarada.

Cambios de workspace: `resolver = "3"`, edición **2024** (los crates de Crucible saltan
de 2021 y compilan sin tocar código), `members = ["crates/*"]`, y licencia unificada a
**Apache-2.0** — los crates de InstruSim eran MIT/Apache dual; mismo autor y sin
contribuciones de terceros, así que la unificación es legítima.

## Plan de consolidación

En este orden, y ninguno de estos pasos está hecho:

1. **Un solo SCPI.** `instrusim-scpi` es más completo (IEEE 488.2, cola de errores,
   status model); el codec de `crucible-core` es un match de patrones más simple pero es
   el que se alimenta del perfil YAML. La unión previsible: quedarse con el motor de
   `instrusim-scpi` y alimentarlo desde el perfil declarativo. Renombrar a
   `crucible-scpi`.
2. **SCPI pasa a ser un protocolo entre varios**, detrás de la abstracción del ADR-0002,
   no el caso privilegiado. Es lo que desbloquea Modbus.
3. **Un solo runtime y un solo binario.** El de InstruSim tiene más dominio; el de
   Crucible tiene la carga del perfil. Sobra uno.
4. **El motor de señales sube al modelo declarativo**: hoy los dispositivos de InstruSim
   son Rust (`dmm.rs`, `psu.rs`); tienen que poder describirse en el perfil, cayendo a
   plugin solo cuando haga falta.
5. **Renombrar `instrusim-*` a `crucible-*`** cuando lo anterior esté hecho. Antes no:
   el prefijo distinto es útil mientras haya dos linajes, porque hace visible la deuda.

## Consecuencias

- El repositorio InstruSim se archiva; nada se pierde, el historial está aquí.
- Anvil sigue siendo el consumidor previsto: correr secuencias reales contra el gemelo,
  sin hardware y sin *flakiness*, incluido en CI. `pasos_scpi` (Anvil ADR-0017) hoy solo
  está probado contra un mock.
- Queda **un solo** proyecto de simulación en ANLACO, que era el objetivo.

## Lección

El dato que decidió esto estaba a un `git fetch` de distancia. Una copia local
desactualizada estuvo a punto de cerrar el repositorio equivocado. Antes de matar un
proyecto, sincronizar con el remoto.
