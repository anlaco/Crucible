# Repository Guidelines

> **Lee esto primero: el repositorio tiene dos linajes sin fusionar.**
>
> Crucible absorbió InstruSim el 2026-08-11 (ver
> [ADR-0003](docs/adr/0003-absorcion-de-instrusim.md)). En el árbol conviven los
> crates `crucible-*` (formato declarativo, tres capas, runtime TCP) y los
> `instrusim-*` (motor de señales, SCPI a fondo).
>
> **El SCPI ya es uno solo**, en `instrusim-scpi`. No escribas otro: si un
> dispositivo necesita hablar SCPI, implementa `ScpiDevice` y deja que
> `device::handle_message` haga el despacho. Los comandos comunes de IEEE 488.2
> y `SYSTem:ERRor?` salen gratis y **no deben reimplementarse**.
>
> **Siguen conviviendo dos runtimes**, uno por linaje. Es deuda declarada, no un
> descuido.
>
> Las convenciones de abajo describen el linaje `instrusim-*`, que es el que
> tiene el dominio. Al tocar `crucible-*`, respeta su diseño de tres capas
> (dispositivo / protocolo / transporte) del
> [ADR-0002](docs/adr/0002-separacion-de-capas-transporte-protocolo-dispositivo.md).
>
> **No renombres `instrusim-*` a `crucible-*`** hasta que la consolidación del
> ADR-0003 esté hecha: el prefijo distinto es lo que hace visible la deuda.

## Project Structure & Module Organization

Cargo workspace, `members = ["crates/*"]`, edition 2024. Las dependencias entre
crates van en una sola dirección y conviene no romperla:

```
instrusim-core  ← instrusim-model ← instrusim-net ← instrusim-cli
instrusim-scpi  ←

crucible-core   ← crucible                    (linaje del formato)
```

`instrusim-core` (motor: `SimTime`, `Clock`, `Signal`, `World`, `TriggerBus`) e
`instrusim-scpi` (parser, patrones de cabecera, cola de errores, registros
IEEE 488.2) **no dependen de nada, ni siquiera entre sí**. `instrusim-model` es
el único que junta ambos.

Cuatro invariantes que el código respeta en todas partes:

- **Los nodos de `World` guardan una `Signal`, no un número.** Se evalúa en el
  instante que pida quien la lea, así que muestrear a 1 GS/s no obliga al motor a
  correr más rápido que su reloj de 1 kHz.
- **Un instrumento nunca calcula el valor que devuelve**: lo lee de `World` por
  sus `Terminal`. Es lo que permitirá enchufar el análisis nodal sin tocar los
  modelos.
- **Un solo hilo posee el `Rack`.** Las conexiones le envían `Request` por canal.
  No hay cerrojos ni estado compartido en el proyecto.
- **Cero dependencias externas.** Solo `instrusim-config` podrá tenerlas.

Un instrumento nuevo implementa `Instrument`
(`crates/instrusim-model/src/instrument.rs`) y declara su árbol con
`CommandTable`; los comandos comunes de IEEE 488.2 y `SYSTem:ERRor?` los
resuelve `handle_message` para todos.

## Build, Test, and Development Commands

```bash
cargo test --workspace                              # 177 tests (ambos linajes)
cargo test -p instrusim-model dmm::tests::mide_la_tension_que_hay_en_sus_bornes
cargo run --release --bin instrusim                 # 127.0.0.1:5025 y :5026
python3 scripts/demo.py                             # demostración de punta a punta
```

## Coding Style & Naming Conventions

`cargo fmt --all` por defecto y `cargo clippy --all-targets --workspace -- -D
warnings`: CI rechaza cualquier aviso.

API pública en inglés, siguiendo el dominio (SCPI, clases IVI). **Comentarios,
documentación y nombres de test en castellano**, explicando el *porqué* y no el
*qué*. Los tests se nombran como la propiedad que afirman:
`la_integracion_reduce_el_ruido`.

Regla del futuro catálogo YAML: **nunca un campo `bool` donde pueda aparecer un
mnemónico SCPI**, o `ON`, `OFF`, `NO` e `Y` se volverán booleanos. Ver
`docs/PLAN.md`.

## Testing Guidelines

Tests unitarios en `mod tests` dentro del propio fichero. Use `VirtualClock` y no
`WallClock`: reproducible al bit y sin esperas.

Los de `instrusim-net` levantan servidores reales en `127.0.0.1:0` para que el
sistema elija puerto. Sincronice con una consulta encadenada (`"VOLT 7;:VOLT?"`)
en lugar de confiar en el orden de las escrituras: dos clientes escriben desde
hilos distintos y la carrera es real.

## Commit & Pull Request Guidelines

Asunto en minúscula, con prefijo de área y **sin tildes**: `core:`, `docs:`,
`fase 2:`. Cuerpo en castellano con la decisión de diseño y su motivo, no el
listado de ficheros. Un commit por concepto.

CI (`.github/workflows/ci.yml`) ejecuta tests en Linux y Windows, formato,
clippy y auditoría de licencias con `cargo-deny` (`deny.toml`): el proyecto es
Apache-2.0 (ADR-0001) y una dependencia copyleft rompe la compilación.
