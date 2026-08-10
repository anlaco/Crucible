# InstruSim — Plan de arranque

Motor de simulación de instrumentación. A corto plazo, equipos virtuales que hablan SCPI por red
para que otro software pruebe su código sin hardware. A largo plazo, un rack completo
virtualizado: instrumentos acoplados por señales reales y disparados entre sí.

## Principio de diseño

**No se soportan clientes, se implementan protocolos.** Un instrumento real no sabe quién se le
conecta: expone socket raw y HiSLIP hablando SCPI, y a partir de ahí funciona cualquier cosa
—pyvisa, LabVIEW, MATLAB, C#, `netcat`— sin código específico para ninguna.

**El motor es el producto; los instrumentos son plugins.** Se construye primero el núcleo y los
equipos se apoyan en él, no al revés.

## Decisiones cerradas

| Decisión | Elección |
|---|---|
| Lenguaje | Rust — binario único, sin runtime, sin instalación, sin administrador |
| Plataformas | Linux y Windows |
| Enfoque | **Motor primero**, instrumentos después |
| Modelo del mundo | Nodos que contienen **señales evaluables en el tiempo**, no escalares |
| Triggers | Bus de eventos en el núcleo desde el primer commit |
| Reloj | Servicio conmutable: tiempo real blando (1 kHz) o tiempo virtual |
| Transporte | Socket raw (5025) primero, HiSLIP (4880) después |
| Protocolo | SCPI-99 conforme por clase + comandos obligatorios IEEE 488.2 |
| Catálogo v0.1 | Clases IVI genéricas: DMM, fuente DC, matriz de conmutación |
| Definición | Declarativa (TOML) + comportamiento en plugin de código |
| Interfaz | Ficheros + web embebida |
| Producto | Open source, licencia dual MIT / Apache-2.0 |
| Desarrollo | Íntegramente Linux + pyvisa; Windows como aceptación |

**Fuera de alcance:** emulación USBTMC y GPIB (imposible sin drivers privilegiados), HAL de
DAQmx, clonado de SCPI de fabricantes concretos.

## Las cuatro invariantes del motor

Baratas ahora, carísimas después. Se respetan desde el primer commit:

1. **El mundo existe siempre**, aunque esté vacío. Ningún modelo se escribe sin `&mut World`.
2. **Los instrumentos declaran sus bornes** aunque no haya nada conectado.
3. **Separación estricta entre estado SCPI y estado físico.** La capa SCPI nunca calcula un
   valor: pregunta al modelo, que pregunta al mundo. Es lo único que separa un mock de un motor.
4. **El reloj es un servicio, no un bucle.** Permite tiempo virtual para CI determinista y
   aceleración de la simulación sin tocar los modelos.

## El núcleo

### Señales evaluables — la decisión que define el techo

Un motor ingenuo guarda un número por nodo y lo refresca a 1 kHz. Con eso nunca habrá
osciloscopios: muestrear a 1 GS/s exigiría correr el motor a 1 GHz.

Aquí cada nodo contiene una **función del tiempo**. El generador coloca "senoide de 1 MHz"; el
osciloscopio pide 10.000 muestras separadas 1 ns y el motor las **evalúa**. El reloj de 1 kHz
solo gobierna cambios de configuración y transitorios lentos.

```rust
pub enum Signal {
    Constant(f64),
    Sine { amp: f64, freq: f64, phase: f64, offset: f64 },
    Ramp { from: f64, to: f64, t0: SimTime, dur: Duration },
    Pulse { .. },
    Noise { rms: f64, seed: u64 },      // determinista por semilla
    Sum(Vec<Signal>),
    Scaled(Box<Signal>, f64),
    Sampled(Arc<Waveform>),             // tabla + interpolación
    Script(ScriptRef),                  // Rhai, para lo que no cabe en datos
}

impl Signal {
    fn eval(&self, t: SimTime) -> f64;
    fn eval_block(&self, t0: SimTime, dt: Duration, out: &mut [f64]);  // clave para adquisición
}
```

`eval_block` es lo que hace viable una captura de osciloscopio sin matar el rendimiento.

**Consecuencia inesperada y muy buena:** cuando llegue el netlist, la red será lineal y
resistiva, así que por superposición el potencial de un nodo es una **combinación lineal de las
señales de las fuentes**. El solver nodal produce ganancias, y el nodo resultante se expresa con
`Sum` y `Scaled` sobre las señales originales. Es decir, **la física compartida y la fidelidad en
alta frecuencia son compatibles sin resolver el circuito a alta frecuencia.** El tipo `Signal`
elegido hoy es justo el que hace posible el rack virtual completo mañana.

### Mundo, bornes y triggers

```rust
pub struct World {
    pub t: SimTime,
    nodes: SlotMap<NodeId, Node>,       // Node { potential: Signal }
    triggers: TriggerBus,               // líneas y eventos con marca de tiempo
}

pub trait Instrument: Send {
    fn identity(&self) -> &Identity;
    fn terminals(&self) -> &[Terminal];
    fn execute(&mut self, cmd: Command, world: &mut World)
        -> Result<Option<Response>, ScpiError>;
    fn step(&mut self, world: &mut World, dt: Duration);
}
```

El bus de triggers habilita desde el día 1 `*TRG`, `INIT`, `ABORt` y el modelo de disparo de
SCPI, y más adelante las líneas físicas entre equipos de un rack.

### Concurrencia

Un único hilo de simulación posee el mundo. Los front-ends de red son tareas `tokio` que envían
comandos por canal y reciben la respuesta por `oneshot`. **No hay locks ni estado compartido
mutable.** El hilo de simulación atiende la cola de comandos de forma continua y hace tick del
reloj cuando toca, así que la latencia de un comando es de microsegundos, no del periodo del
reloj. Como efecto colateral, bajo reloj virtual el sistema es completamente determinista.

## Estructura

```
instrusim/
  crates/
    instrusim-core/     # SimTime, Clock, Signal, World, TriggerBus, bucle de simulación
    instrusim-model/    # trait Instrument + clases IVI genéricas
    instrusim-scpi/     # envoltura de `scpi` + despacho desde tabla declarativa
    instrusim-net/      # socket raw, HiSLIP, anuncio mDNS
    instrusim-config/   # carga de TOML de instrumento y de escenario
    instrusim-web/      # axum: estado en vivo
    instrusim-cli/      # binario
  models/               # catálogo declarativo (*.toml)
  scenarios/            # racks de ejemplo
```

## Dependencias

- `scpi` 1.0 + `scpi-contrib` (MIT, `no_std`) — parser SCPI e IEEE 488.2 con comandos
  obligatorios y registros de estado.
- `tokio` — red asíncrona. `axum` — web embebida.
- `rhai` — scripting en Rust puro. **Preferido sobre Lua**: `mlua` exige toolchain de C y
  rompería la compilación limpia en Windows.
- `mdns-sd` — anuncio DNS-SD sin privilegios (fase HiSLIP).
- `nalgebra` o `faer` — solo cuando llegue el netlist.

**Aviso de licencia:** `lxi-rs` implementa HiSLIP en Rust pero su implementación principal es
**GPLv3** y está enfocada a Unix. Incompatible con MIT/Apache. Se usa **solo como referencia de
lectura**; HiSLIP se implementa de cero.

## Fases

**F1 — Núcleo del motor.** `SimTime`, `Clock` (real y virtual), `Signal` con `eval` y
`eval_block`, `World` con nodos y bornes, `TriggerBus`, y el bucle de simulación con cola de
comandos. Sin red y sin SCPI. Entregable: batería de tests unitarios.
*Hito:* evaluar en bloque una senoide de 1 MHz a 1 GS/s con el reloj corriendo a 1 kHz, y un
evento de trigger propagándose entre dos participantes con marca de tiempo correcta.

**F2 — Protocolo y primer instrumento.** Socket raw en 5025, parser SCPI, `*IDN?` y un DMM
genérico que lee el potencial de su borne.
*Hito:* `pyvisa` abre `TCPIP0::127.0.0.1::5025::SOCKET`, pregunta `*IDN?` y lee `MEAS:VOLT:DC?`.

**F3 — Clases y catálogo.** DMM, fuente DC y matriz de conmutación conformes a su clase IVI.
Carga declarativa desde TOML, escenarios con varios instrumentos, cola de errores y máquina de
estados IEEE 488.2 completa, modelo de disparo sobre el bus de triggers.

**F4 — HiSLIP y descubrimiento.** Servidor HiSLIP con varios dispositivos lógicos y anuncio
mDNS. Los instrumentos pasan a ser indistinguibles de equipos LXI reales.

**F5 — Observabilidad y realismo.** Web embebida con estado en vivo y traza de comandos.
Inyección de fallos: timeouts, errores de comando, fuera de rango, deriva.

**F6 — Física compartida.** Netlist con análisis nodal en continua. El solver produce ganancias
y los nodos se expresan por superposición sobre las señales de las fuentes. Aquí lo que mide el
DMM pasa a ser consecuencia de lo que aplica la fuente: el salto a gemelo digital.

**F7 — El rack completo.** Osciloscopio y generador (ya posibles gracias a `Signal`), líneas de
trigger entre equipos, VXI-11, modelos de equipos comerciales, DUT importado como FMU 3.0.

## Riesgos

| Riesgo | Mitigación |
|---|---|
| Motor primero retrasa el primer resultado visible | F1 entrega tests, no humo; F2 llega en días |
| `Signal` se convierte en un lenguaje completo | Variantes cerradas; lo raro va a `Script` |
| Superposición solo vale para redes lineales | Es el alcance declarado; no linealidades por script |
| HiSLIP desde cero consume de más | Va en F4, con el producto ya útil |
| Contaminación de licencia con GPLv3 | Prohibido copiar de `lxi-rs`; auditoría en CI |
| Divergencia Windows/Linux | CI en ambos desde el primer commit; cero dependencias con C |

## Verificación

1. **Unitaria:** `Signal` contra soluciones analíticas; determinismo del ruido por semilla;
   orden y marca de tiempo de los eventos de trigger; parser SCPI contra la batería obligatoria
   de IEEE 488.2 con sintaxis larga y corta y códigos de error.
2. **Integración:** suite `pyvisa` que levanta un escenario, se conecta, configura y mide.
   Corre en cada commit.
3. **Determinismo:** el mismo escenario bajo reloj virtual produce resultados idénticos.
4. **Aceptación:** un cliente ajeno al proyecto ejecuta una secuencia de medida sin adaptaciones.
5. **CI:** compilación y tests en Linux y Windows, más auditoría de licencias.

## Siguiente paso

F1: el workspace de Cargo y el núcleo del motor, con los tests como entregable.
