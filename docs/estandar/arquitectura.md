# Diseño: Arquitectura de Crucible

> **Prioridad:** fundacional. Propuesta para discusión antes de
> implementar.

Crucible es un **estándar abierto para describir y simular bancos de
instrumentos SCPI** — el gemelo digital del banco de test. La meta:
probar software de test sin hardware, con la misma confianza que da un
instrumento real (dentro de lo razonable).

## Tres piezas, no una

Lo que se estandariza no es "un simulador", es **un formato + un contrato
+ un runtime de referencia**:

1. **Perfil de instrumento** (declarativo, YAML): describe el
   comportamiento de **un** instrumento — su identidad, los comandos SCPI
   que acepta, su estado interno y cómo muta, y un modelo de
   comportamiento (qué produce cada medición). Es lo "descriptible como un
   motor en Simulink". Ver [`formato-de-perfil.md`](formato-de-perfil.md).

2. **Topología de banco** (declarativo, YAML): describe **el banco
   entero** — qué instrumentos hay, cómo se conectan entre sí y al DUT, y
   qué puertos de red exponen. Esto es **el hueco que nadie cubre hoy**:
   el gemelo del *banco*, no del instrumento aislado. Ver
   [`topologia-de-banco.md`](topologia-de-banco.md).

3. **Runtime de referencia**: carga perfiles + topología, mantiene el
   estado de cada instrumento, simula cómo se influyen entre sí (la
   fuente alimenta al DUT, el multímetro mide al DUT, el osciloscopio mira
   un nodo), y sirve el banco por **SCPI sobre TCP**. El consumidor
   (Anvil, TestStand, OpenTAP…) se conecta a ese runtime como a un banco
   real.

El **estándar** es el formato (1 y 2) + el contrato SCPI/TCP. El runtime
es la **implementación de referencia**; puede haber otros.

## El contrato: SCPI sobre TCP

Un instrumento simulado se sirve por SCPI sobre TCP en loopback, igual
que un instrumento real en la red. El consumidor no sabe que es simulado:
el motor de test invoca un paso que abre un `TcpStream`, manda comandos
SCPI y parsea la respuesta. **El instrumento vive detrás del paso,
opaco al motor** (igual que en Anvil, ADR-0003). Cero cambios en el
consumidor para pasar de real a simulado.

## Por qué no reinventar Simulink

Simulink simula física continua (décadas de ingeniería). Un instrumento
SCPI es, en el 90 % de los casos, una **máquina de estado con
respuestas**: recibe un comando, muta un estado interno y devuelve un
valor. Empezamos modelando eso. La física se añade cuando un caso real la
pida (y aun entonces, como un *modelo* del instrumento, no como un motor
de simulación universal).

El formato se diseña para que el modelo sea **declarativo por defecto**
(tablas, fórmulas, condiciones) pero pueda **caer a código** (un
plugin/trait) cuando el comportamiento es complejo. Datos primero, código
cuando hace falta — el mismo patrón que Anvil.

## Desacoplamiento del consumidor

Crucible **no sabe quién lo consume**. Se sirve por SCPI/TCP; el
consumidor hace TCP. Eso lo hace:

- **Reusable**: sirve a Anvil, a TestStand, a OpenTAP, a un script
  PyVISA, a cualquier cosa que hable SCPI por TCP.
- **Estándar de facto**: si el formato es claro y el runtime fácil de
  consumir, se adopta sin comité.

## Precedentes y el hueco

- **PyVISA-sim**: perfiles declarativos YAML de **un** instrumento
  aislado. El ejemplo más cercano; punto de partida natural (imitar y
  **extender**). No modela el banco ni las interacciones.
- **pytestlab**: modelos de instrumento + `SimBackend` + record/replay.
  Muy en la línea, pero más un framework que un estándar.
- **IVI**: el estándar "oficial" de drivers tipados (era NI). Pesado,
  acoplado a Windows/COM. No es el modelo a seguir.

El hueco de Crucible: **estándar abierto, declarativo, del banco entero
(topología + interacciones), servido por SCPI/TCP, consumible por
cualquier secuenciador.** Eso no existe.

## Tres modos de simulación (cubren los casos de uso)

- **Determinista**: el modelo devuelve respuestas predecibles (fórmulas
  fijas, tablas). Sin red ni flakiness. El modo perfecto para CI estricto
  y tests automáticos.
- **TCP/SCPI en vivo**: el runtime sirve el modelo por TCP, igual que un
  instrumento. Para validar la pila de red de verdad y para demos (un
  ESP32 que emule un instrumento es este modo, en hardware).
- **Record/replay**: graba lo que un instrumento real respondió y lo
  replaya; falla si la realidad difiere de lo grabado
  (`ReplayMismatchError`). Para detectar regresiones de comunicación.
  Necesita haber tenido un real alguna vez → se pospone.

El modo determinista es el más barato y el más valioso para empezar. El
TCP es el que valida la pila real. El record/replay es el sofisticado,
para el final.

## Riesgos a vigilar desde el diseño

- **El DUT es lo difícil de modelar**, no los instrumentos. El DUT es lo
  que se está diseñando, no algo conocido. Modelamos bien los
  *instrumentos* (conocidos) y dejamos el DUT como una caja configurable;
  en muchos tests, el DUT es "estímulo → respuesta" simple.
- **No acoplar el estándar a Anvil ni a WASM.** Si sólo sirve a Anvil, no
  es estándar.
- **No bloquear el estándar por quererlo perfecto.** Un estándar agarra
  con: formato claro + runtime de referencia usable + 3-4 perfiles
  reales de instrumentos comunes. Ese es el primer hito, no "cubrir todo".