# Crucible

Estándar abierto para describir y simular bancos de test: un perfil YAML dice
cómo se comporta cada instrumento y un runtime lo sirve por su protocolo real
(hoy SCPI sobre TCP), para que el software de test no distinga el banco simulado
del físico. Rust, workspace de Cargo, edición 2024, Apache-2.0.

## Lee esto primero: dos linajes sin fusionar

Crucible absorbió InstruSim el 2026-08-11
([ADR-0003](docs/adr/0003-absorcion-de-instrusim.md)). Conviven dos familias de
crates, y **la deuda es declarada, no un descuido**:

- `crucible-*`: el formato declarativo (perfiles y bancos YAML) y su runtime TCP.
  Respeta las tres capas dispositivo / protocolo / transporte del
  [ADR-0002](docs/adr/0002-separacion-de-capas-transporte-protocolo-dispositivo.md).
- `instrusim-*`: el motor (reloj, señales, mundo, disparos), SCPI a fondo e
  instrumentos escritos en Rust.

Reglas que se derivan:

- **El SCPI ya es uno solo, en `instrusim-scpi`. No escribas otro.** Un
  dispositivo implementa `ScpiDevice` y deja el despacho a
  `instrusim_scpi::handle_message`. Los comandos comunes de IEEE 488.2 y
  `SYSTem:ERRor?` salen gratis: **no se reimplementan ni se declaran**.
- Siguen existiendo **dos runtimes y dos binarios** (`crucible` e `instrusim`).
  No los consolides de paso en otra tarea.
- **No renombres `instrusim-*` a `crucible-*`** hasta cerrar la consolidación
  del ADR-0003: el prefijo distinto es lo que hace visible la deuda.

## Mapa del repositorio

Las dependencias entre crates van en una sola dirección; no la rompas:

```
instrusim-core ─┐
                ├─► instrusim-model ─► instrusim-net ─► instrusim-cli  (bin `instrusim`)
instrusim-scpi ─┤
                └─► crucible-core ───► crucible                        (bin `crucible`)
```

`instrusim-core` e `instrusim-scpi` no dependen de nada, ni entre sí.

- `crucible-core`: carga y valida perfiles (`perfil.rs`) y bancos (`banco.rs`);
  `protocolo/scpi.rs` traduce el perfil a una `CommandTable`; `modelo.rs` es el
  evaluador de fórmulas. `dispositivo.rs` es el único sitio que elige protocolo.
- `crucible`: CLI y servidor TCP (tokio); `lib.rs` existe para que los tests de
  `tests/smoke_scpi.rs` levanten el servidor real.
- `banco/`: banco de ejemplo que se empaqueta en los instaladores y que incluye
  el manual. `perfiles/`: Keithley 2400 de referencia y `_demo` (smoke con
  Anvil). `bancos/rf/`: banco del primer cliente.
- `docs/adr/`, `docs/diseno/`: decisiones y diseño del formato. `docs/PLAN.md`
  es el plan **histórico** de InstruSim: el crate `scpi`, TOML e
  `instrusim-config` no existen; manda el código. `docs/bancos/`: especificación
  y plan del banco de RF. `docs/manual/`: manual de usuario (mdBook, es + en).
- `spikes/`: código desechable fuera del workspace, cada uno con su
  `[workspace]` vacío.

## Comandos

```bash
cargo test --workspace                                   # todo (≈200 tests)
cargo test -p instrusim-model dmm::tests::mide_la_tension_que_hay_en_sus_bornes
cargo fmt --all
cargo clippy --all-targets --workspace -- -D warnings    # CI rechaza cualquier aviso

cargo run -p crucible -- --validar banco/banco.yaml      # valida sin abrir puertos
cargo run -p crucible -- banco/banco.yaml                # 127.0.0.1:5025-5027
cargo run -p crucible -- perfiles/keithley_2400.yaml 5030
cargo run --release --bin instrusim                      # rack demo en :5025 y :5026
python3 scripts/demo.py                                  # contra `instrusim`

cargo build -p crucible && docs/manual/check.sh          # manual contra el binario
PYTHON=/ruta/venv/bin/python docs/manual/check.sh        # incluye la sesión pyvisa
```

`check.sh` exige los puertos 5025-5027 libres. `--record` reescribe las
sesiones: solo para una sesión nueva, y revisando el diff antes de commitear.

## Invariantes

- **Linaje `instrusim-*`:**
  - Los nodos de `World` guardan una `Signal` (función del tiempo), no un
    número: así se muestrea a 1 GS/s con el motor a 1 kHz.
  - Un instrumento nunca calcula lo que devuelve: lo lee de `World` por sus
    `Terminal`. Es la costura por la que entrará el análisis nodal.
  - Un solo hilo posee el `Rack`; las conexiones le mandan `Request` por canal.
    Sin cerrojos ni estado compartido.
  - Cero dependencias externas.
- **Linaje `crucible-*`:** un dispositivo es **uno por puerto, compartido por
  todas las conexiones** (`Arc<tokio::sync::Mutex>`, bloqueado por mensaje). El
  estado sobrevive a la reconexión; `*RST` vuelve al `estado:` del perfil, no al
  `estado_inicial` del banco.
- **Un banco arranca entero o no arranca**: un perfil roto o un puerto ocupado
  tumban el banco completo, a propósito. No lo "suavices".
- Dependencias externas solo en `crucible-*` y con licencia de la lista de
  `deny.toml`. Nunca copies código de `lxi-rs` (GPLv3).

## Al escribir código

- **Instrumento en Rust:** implementa `Instrument`
  (`crates/instrusim-model/src/instrument.rs`) y declara su árbol con
  `CommandTable`.
- **Perfil YAML:** `patron` es solo la cabecera en notación SCPI
  (`SOURce:VOLTage[:LEVel]`), **sin `?`, sin argumentos y sin `*`**; los
  argumentos van en `args` y la consulta con `query: true`. `Perfil::validar`
  rechaza el formato antiguo con un mensaje de migración.
- `CommandTable::lookup` devuelve **la primera coincidencia**: declara lo
  específico antes que lo genérico. Orden y consulta del mismo patrón van en
  tablas separadas (`DispositivoScpi`); si no, la consulta queda tapada.
- **Tipos del estado:** el tipo de cada variable es el de su valor en `estado:`.
  Numérica acepta sufijos de unidad (`2.4 GHZ` vía `instrusim_scpi::parse_decimal`);
  booleana, `ON/OFF/1/0`; texto guarda el mnemónico tal cual. **Nunca un `bool`
  donde pueda aparecer un mnemónico SCPI.** Una mutación que falla no escribe nada.
- **Fórmulas:** solo `+ - * /`, paréntesis, `gauss`, `abs`, `sqrt`, `min`, `max`.
  No hay comparaciones (brecha B9). Si las añades, añade también `log10` y `pow`
  ([camino-de-rf.md](docs/diseno/camino-de-rf.md) §4).

## Tests

- Unitarios en `mod tests` dentro del propio fichero, nombrados en inglés como
  la propiedad que afirman: `integrating_reduces_the_noise`. Los de castellano
  que quedan (`la_integracion_reduce_el_ruido`) se renombran al tocarlos.
- `VirtualClock`, no `WallClock`: reproducible al bit y sin esperas.
- Los que abren sockets usan el puerto `0`. Sincroniza con una consulta
  encadenada (`"VOLT 7;:VOLT?"`) o sondeando con límite, **nunca con un `sleep`
  fijo**: fallaba en el Windows de CI.
- Toda lectura de socket en un test lleva tope de tiempo: sin él, un
  dispositivo que no contesta cuelga CI en vez de fallar.

## Trampas conocidas

- En SCPI, tras `;` la cabecera es **relativa al nodo anterior**:
  `SOUR:VOLT 1;OUTP ON` es `SOUR:OUTP`. En tests y perfiles, usa `;:`.
- **Cambiar un mensaje o una salida de `crucible` rompe el manual.** Sus sesiones
  (`docs/manual/listings/sesiones/`) se comparan con el binario y el workflow
  `manual.yml` no publica si difieren. Actualiza `es/` y `en/` a la vez y
  mantén la tabla de `docs/manual/idioma.js`.
- Una variable mal escrita en `estado_inicial` no da error: se ignora en silencio.
- **Repo público:** no escribas el nombre del cliente ni detalles de su DUT
  (ver la cabecera de `docs/bancos/banco-rf.md`).

## Estilo

- `cargo fmt` por defecto; clippy sin avisos.
- **Código en inglés**: API pública siguiendo el dominio (SCPI, clases IVI), y
  también comentarios, documentación y nombres de test, explicando el *porqué*
  y no el *qué*.
- Quedan en castellano, y no por descuido: **los mensajes que ve el usuario**
  (ayuda, errores, tabla de arranque), porque el manual los compara letra a
  letra, y **los commits**. El código antiguo sigue comentado en castellano;
  se traduce al tocarlo, no en una pasada aparte, así que **conviven los dos
  idiomas en el mismo fichero** y no hay que «arreglarlo».

## Commits y PR

- Asunto en minúscula, con prefijo de área y **sin tildes**: `core:`, `docs:`,
  `net:`, `fase 2:`.
- Cuerpo en castellano con la decisión de diseño y su motivo, no la lista de
  ficheros. Un commit por concepto.
- CI (`.github/workflows/ci.yml`): tests en Linux y Windows, `fmt --check`,
  clippy y `cargo-deny`. Una dependencia copyleft rompe la compilación
  (ADR-0001). `release.yml` solo prueba `crucible`, `crucible-core` e
  `instrusim-scpi`.
