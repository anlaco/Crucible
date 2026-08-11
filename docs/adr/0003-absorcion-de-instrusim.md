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

1. ~~**Un solo SCPI.**~~ ✅ **Hecho (2026-08-11).** Ver §«La fusión del SCPI» más abajo.
2. **SCPI pasa a ser un protocolo entre varios**, detrás de la abstracción del ADR-0002,
   no el caso privilegiado. Es lo que desbloquea Modbus.
3. **Un solo runtime y un solo binario.** El de InstruSim tiene más dominio; el de
   Crucible tiene la carga del perfil. Sobra uno.
4. **El motor de señales sube al modelo declarativo**: hoy los dispositivos de InstruSim
   son Rust (`dmm.rs`, `psu.rs`); tienen que poder describirse en el perfil, cayendo a
   plugin solo cuando haga falta.
5. **Renombrar `instrusim-*` a `crucible-*`** cuando lo anterior esté hecho. Antes no:
   el prefijo distinto es útil mientras haya dos linajes, porque hace visible la deuda.

## La fusión del SCPI (paso 1, hecho el 2026-08-11)

Había dos implementaciones. La de `instrusim-scpi` entiende el protocolo: formas larga y
corta, nodos opcionales, sufijos de canal, mensajes compuestos con cabeceras relativas,
`MIN`/`MAX`/`DEF`, unidades, cola de errores y registros de IEEE 488.2. La de
`crucible-core` comparaba la línea recibida con el patrón carácter a carácter: funcionaba
con los patrones que ella misma escribía y fallaba con todo lo demás.

**Ganó el motor; ganó el formato.** No es un empate diplomático: son cosas distintas y
cada una aportó la suya.

**Qué se movió.** El despacho de lo obligatorio —comandos comunes, `SYSTem:ERRor?`,
mensajes compuestos, anotación de errores— vivía en `instrusim-model`, atado al motor de
simulación. Ahora vive en `instrusim-scpi::device`, detrás del contrato `ScpiDevice`, que
**no conoce el mundo simulado**. Lo cumplen los dos linajes:

- Los instrumentos en Rust, mediante un puente que empaqueta `(&mut dyn Instrument, &mut
  World)` durante el despacho. `Rack` y la capa de red no se enteraron.
- `DispositivoScpi`, que traduce el perfil YAML a una `CommandTable` y un `execute`.

**Qué cambió en el formato.** El patrón dejó de ser la línea entera y pasó a ser la
cabecera en notación SCPI:

```yaml
# antes                            # ahora
- patron: "SOUR:VOLT <x>"          - patron: "SOURce:VOLTage[:LEVel]"
  muta: { voltaje_fuente: "<x>" }    args: [v]
                                     muta: { voltaje_fuente: "<v>" }
```

Los argumentos se declaran en `args`, la forma consulta se marca con `query: true` en vez
del `?`, y los comandos comunes desaparecen del perfil: los resuelve el motor, y `*IDN?`
sale de `dispositivo.idn`. Es **incompatible con los perfiles anteriores**, así que
`Perfil::validar` los detecta y explica la migración en el mensaje de error en lugar de
cargar un perfil que no reconocería ni un comando.

**Lo que ahora funciona y antes no:** `source:voltage:level 5.0` igual que `SOUR:VOLT 5.0`;
`*IDN?;:SOUR:VOLT?` devolviendo las dos respuestas unidas por `;`; un comando desconocido
anotándose en la cola en vez de cortar la conversación; `SYST:ERR?`, `*ESR?`, `*STB?` y
`*RST` sin declararlos.

**Dos cosas más que cayeron por el camino:**

- El bucle de red decidía si contestar mirando si la línea acababa en `?`. Con mensajes
  compuestos eso es falso: `SOUR:VOLT?;:OUTP ON` lleva consulta y no acaba en `?`. Ahora
  lo dice el motor, que es quien lo sabe.
- `Banco::cargar_dispositivos` ignoraba en silencio los perfiles que no cargaban. Un banco
  al que le falta la fuente no es un banco degradado: es uno que va a dar resultados
  falsos. Ahora falla entero y dice cuál.

**Estado:** 176 tests verdes, `clippy` limpio en todo el workspace. Queda un solo SCPI.

**Lo que costó tiempo, para la próxima:** dos entradas con el mismo patrón —la orden y la
consulta— se tapaban entre sí, porque `CommandTable::lookup` devuelve la primera
coincidencia. Se resolvió con dos tablas, una por forma. El síntoma fue un test colgado en
vez de un fallo, porque el cliente esperaba una respuesta que nunca llegaba; el helper de
los tests de humo lleva ahora un tope de cinco segundos para que eso falle en vez de
bloquear CI.

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
