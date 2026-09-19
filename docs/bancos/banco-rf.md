# Especificación: banco de RF con DUT y BITE

> **Estado**: borrador de trabajo, 19/09/2026. Recoge el inventario que ha
> facilitado el cliente y lo contrasta con lo que Crucible sabe hacer hoy.
> Las decisiones marcadas **(por confirmar)** necesitan respuesta del cliente
> antes de comprometer alcance.
>
> El cliente no se nombra: este repositorio es público (Apache-2.0, ADR-0001).
> Si la especificación tuviera que llevar su nombre, referencias de proyecto o
> el detalle del DUT, mueve este fichero a un repositorio privado.
>
> **El orden de ejecución, los costes y los riesgos están en
> [plan-banco-rf.md](plan-banco-rf.md).** Aquí solo está el inventario y qué es
> cada equipo.

## 1. Alcance

Simular el banco completo con Crucible, de modo que el software de test del
cliente no distinga el banco simulado del real: mismos recursos VISA, mismo
SCPI, mismos errores. El DUT y su BITE quedan fuera de la primera entrega en
todo lo que dependa del chasis NI (ver §3.5).

### El entorno del cliente

Condiciona todo lo demás:

- Su software es **LabVIEW + TestStand**, y manda **SCPI directamente**: no
  usan drivers IVI ni plug&play. Esto es importante y es una buena noticia —
  significa que un perfil puede declarar solo los comandos que ellos usan, sin
  tener que satisfacer a un driver genérico.
- Configuran los equipos en **NI-MAX con identificadores**, y **su código abre
  por alias VISA**, no por descriptor literal.
- Crucible correrá en la **misma máquina Windows** que LabVIEW y NI-MAX: todo
  por loopback.
- Su criterio de aceptación: **arrancar Crucible, ver los equipos en NI-MAX y
  poder configurarlos.**

Que abran por alias tiene una consecuencia grande: **el transporte que hay bajo
el alias es decisión nuestra.** Un equipo que en el banco real es USB puede
simularse por TCP sin que su software lo note, siempre que el alias apunte al
sitio correcto.

## 2. Inventario

| # | Equipo | Función | Conexión declarada | Protocolo real | Crucible hoy |
|---|---|---|---|---|---|
| 1 | Keysight **N5171B**-503 | Generador RF, 9 kHz – 3 GHz | TCP | SCPI sobre socket 5025 | ✅ cubierto |
| 2 | Keysight **N5767A** | Fuente DC, 0–60 V / 0–25 A / 1500 W | TCP | SCPI sobre socket 5025 | ✅ cubierto |
| 3 | Keysight **U2061XA**-UK6 ×3 | Sensor de potencia, 10 MHz – 6 GHz | USB | SCPI sobre **USBTMC** | ✅ **por TCP**, vía alias (§3.3); USBTMC aplazado |
| 4 | NI **USB-6343** (OEM) | DAQ multifunción | USB | **NI-DAQmx** (no SCPI) | ✅ **simulado de NI-MAX** (§3.4); el nuestro, aplazado |
| 5 | NI **783496-01** | Ver §3.5 — **no es un cRIO** | TCP | **NI-RIO** (no SCPI) | ❌ fuera de alcance |
| 6 | DUT — **todo el DUT** | Consola y operación | **Serie por FTDI** | ASCII a medida (por confirmar) | ⚠️ falta transporte serie |
| 7 | DUT — BITE | Built-In Test Equipment | Vía equipo #5 | Por confirmar | ❌ depende de #5 |

Cinco de siete quedan cubiertos con lo que hay o con funciones que ya trae
NI-MAX. **La única brecha cara que queda es el transporte serie**, y es
crítica: el DUT entero pasa por ahí.

## 3. Equipo por equipo

### 3.1 Generador RF — Keysight N5171B (opción 503)

Verificado contra el datasheet oficial y la referencia SCPI (ver §7).

- **Frecuencia**: 9 kHz – 3 GHz. Resolución 0,001 Hz.
- **Amplitud**: rango ajustable +19 a −144 dBm (estándar; +30 con opción 1EA).
  Resolución 0,01 dB. Potencia máxima nivelada +18 dBm típica entre 10 MHz y
  3 GHz.
- **Modulación**: AM, FM, ΦM y pulso, con fuente interna o externa.

Comandos que el perfil debe cubrir, con la sintaxis exacta del manual:

```
[:SOURce]:FREQuency[:CW] <value><unit>        :FREQ 2.4 GHZ
[:SOURce]:FREQuency[:CW]?
[:SOURce]:FREQuency:MODE CW|FIXed|LIST
[:SOURce]:POWer[:LEVel][:IMMediate][:AMPLitude] <value><unit>
[:SOURce]:POWer[:LEVel][:IMMediate][:AMPLitude]?
[:SOURce]:POWer[:LEVel][:IMMediate]:OFFSet <value><unit>
:OUTPut[:STATe] ON|OFF|1|0
:OUTPut[:STATe]?
:OUTPut:MODulation[:STATe] ON|OFF|1|0
:OUTPut:MODulation[:STATe]?
```

Más los comunes de IEEE 488.2 y `:SYSTem:ERRor?`, que Crucible ya resuelve solo.

> **Este equipo se programa con unidades**: `FREQ 2.4 GHZ`, `POW -10 DBM`.
> Era la brecha B2 y era bloqueante: Crucible guardaba «2.4 GHZ» como cadena, o
> en el mejor caso lo leía como 2,4 Hz. Cerrada el 19/09/2026 (§8).

### 3.2 Fuente DC — Keysight N5767A

Serie N5700. Verificado contra la guía de usuario oficial: **0–60 V, 0–25 A**,
1500 W. LAN/GPIB/USB de serie, LXI clase C. SCPI por socket en el **5025**
(telnet en el 5024).

```
[SOURce:]VOLTage[:LEVel][:IMMediate][:AMPLitude] <value>|MIN|MAX
[SOURce:]VOLTage[:LEVel][:IMMediate][:AMPLitude]? [MIN|MAX]
[SOURce:]CURRent[:LEVel][:IMMediate][:AMPLitude] <value>|MIN|MAX
[SOURce:]CURRent[:LEVel][:IMMediate][:AMPLitude]? [MIN|MAX]
[SOURce:]VOLTage:PROTection:LEVel <value>|MIN|MAX      OVP
[SOURce:]VOLTage:LIMit:LOW <value>|MIN|MAX             UVL
[SOURce:]CURRent:PROTection:STATe ON|OFF               OCP
OUTPut[:STATe] ON|OFF
OUTPut[:STATe]?
OUTPut:PROTection:CLEar
MEASure[:SCALar]:VOLTage[:DC]?
MEASure[:SCALar]:CURRent[:DC]?
STATus:QUEStionable:CONDition?
STATus:OPERation:CONDition?
```

> ⚠️ **Casi todos los comandos de esta fuente aceptan `MIN`/`MAX`.** Crucible
> todavía no los resuelve —hace falta declarar los rangos, brecha B3— pero ya
> no corrompe el estado: contesta `-224` y conserva el valor anterior (§8).

El comportamiento que hay que modelar, y que no es solo guardar números:

- **Cruce CV/CC**: la fuente limita a `CURR` y baja la tensión. El bit de modo
  vive en `STAT:OPER:COND?`.
- **OVP**: si la tensión de salida supera `VOLT:PROT:LEV`, la salida se
  dispara, se apaga, y solo `OUTP:PROT:CLE` la recupera. Es un camino de error
  que el software del cliente casi seguro ejercita.
- **Rango**: `VOLT 70` debe contestar `-222,"Data out of range"`, no aceptarse.

### 3.3 Sensores de potencia — Keysight U2061XA-UK6 (tres unidades)

Verificado: **10 MHz – 6 GHz**, sensor USB de potencia media y de pico, rango
dinámico amplio. `UK6` es la opción de certificado de calibración comercial con
datos de medida — **no afecta al comportamiento remoto**, solo a la
documentación de calibración que acompaña al equipo.

El sensor habla SCPI, pero por **USBTMC**: su recurso VISA es de la forma
`USB0::0x2A8D::<pid>::<nº serie>::INSTR`, no `TCPIP0::…::SOCKET`. Crucible no
tiene transporte USB y no va a tenerlo.

**Para este encargo no hace falta.** Como su código abre por alias de NI-MAX
(§1), basta con apuntar el alias del sensor a un recurso TCP servido por
Crucible. El VI que hace `VISA Open("SensorPot1")` no sabe ni le importa qué
hay debajo. Lo que era la brecha más cara del análisis anterior se esquiva por
un dato de integración, no por trabajo de ingeniería.

> **Esquivar no es resolver.** El cliente pidió que los sensores fueran USB y
> después aceptó TCP para agilizar la entrega. El transporte USBTMC es
> **compromiso de producto** de Crucible por decisión de su dueño, registrado
> en [roadmap.md](../roadmap.md) §H7: un simulador de instrumentos que solo
> sabe hablar TCP no cubre el parque real. Lo que cambia es **cuándo**, no
> **si**.

> Esto se cae si sus VIs usan el **driver IVI de Keysight** del sensor en vez
> de SCPI a pelo: un driver puede exigir la interfaz USB y, sobre todo, manda
> muchos más comandos de los que declara un perfil. Es la pregunta abierta más
> importante del encargo (§5.4 del plan).

Los tres no miden lo mismo, y eso decide el modelo de cada uno:

| Unidad | Papel en el banco |
|---|---|
| 1 | **Potencia de salida** del camino de RF |
| 2 | **SWR** — relación de onda estacionaria |
| 3 | **Calibración del generador** en varios puntos del banco |

Se distinguen por **alias de NI-MAX**, que es como los tiene configurados el
cliente. En esta entrega se sirven como recursos **TCP**: el cliente los pidió
USB y aceptó TCP para agilizar.

> **Los tres devuelven potencia en bruto**; el procesado —incluido el cálculo
> de la SWR— lo hace su software. Eso simplifica mucho el modelo: los tres
> perfiles son el mismo salvo por la identidad y por el punto del banco donde
> miden. No hay que modelar SWR, solo potencia.
>
> Queda en pie la consecuencia de la unidad 3, que mide «en varios puntos del
> banco»: **algo conmuta el camino**, y son los SPDT que controla la DAQ
> (§3.4). Mientras no se modele el camino de RF, el punto de medida de cada
> sensor se fija por configuración en vez de deducirse del banco.

**Identidad**: no hace falta que el cliente nos dé los números de serie. Nos
han dicho que **ellos editan los YAML** para que cuadren con lo que tienen
configurado en NI-MAX. Eso tiene una consecuencia de producto: los perfiles no
son solo entrada del runtime, son **material que el cliente lee y modifica**.
Conviene que el `idn` esté arriba, comentado y fácil de encontrar, y que el
manual explique qué tocar. Es el primer caso real de la promesa del formato
declarativo: que el usuario pueda ajustar el gemelo sin pedirnos nada.

Comandos previstos (familia X-Series; **por confirmar** contra el manual
concreto del U2061XA, que no he podido descargar):

```
INITiate[:IMMediate]        ABORt            FETCh[:SCALar][:POWer][:AC]?
READ[:SCALar][:POWer][:AC]?                  MEASure[:SCALar][:POWer][:AC]?
SENSe:FREQuency[:FIXed] <freq>               corrección por frecuencia
SENSe:AVERage:COUNt / :AUTO                  promediado
SENSe:MRATe NORMal|DOUBle|FAST               velocidad de medida
SENSe:DETector:FUNCtion AVERage|NORMal       media vs. pico
CALibration:ZERO:AUTO ONCE                   puesta a cero
CALibration[:ALL]?                           calibración
UNIT:POWer DBM|W
TRIGger:SOURce IMMediate|BUS|EXTernal
```

Modelo físico mínimo para que el banco sea útil: la potencia leída debe salir
de la que programa el generador (#1), menos las pérdidas del camino, más ruido.
Sin eso, tres sensores devolviendo un número inventado no validan nada.

**Tres unidades ⇒ tres recursos VISA distintos.** Hay que decidir cómo se
identifican (número de serie) y a qué punto del circuito mira cada una
**(por confirmar)**.

### 3.4 DAQ — NI USB-6343 OEM

Verificado: X Series, **32 entradas analógicas** (16 diferenciales) a 16 bits y
500 kS/s agregados, **4 salidas analógicas** a 16 bits, **48 líneas digitales**,
**4 contadores/temporizadores** de 32 bits. La variante OEM es la placa desnuda,
sin caja.

### ⚠️ Para qué la usan, y por qué eso lo cambia todo

El cliente la usa para cinco cosas:

1. Generar los **pulsos de trigger de las sondas** de potencia.
2. Generar el **pulso del generador de RF**.
3. **Controlar los SPDT del banco** — los conmutadores del camino de RF.
4. Leer **tensión con una sonda**.
5. Leer la **continua de un bias-T**.

Esto no es un periférico al margen: **la DAQ es el nexo del banco.** Los puntos
2 y 3 la ponen dentro del camino de señal de RF, y el 1 dentro del camino de
disparo de las medidas.

En particular, **son los SPDT los que deciden a qué punto del banco está
conectado cada sensor de potencia** (§3.3). Cualquier modelo que quiera que el
sensor lea lo que el generador emite necesita saber en qué posición están esos
conmutadores — y esa información vive en las líneas digitales de la DAQ.

**Esto no es un instrumento SCPI y no se puede describir con un perfil de
Crucible.** No hay cabeceras ni mensajes de texto: se programa con **NI-DAQmx**,
una API en C/.NET/LabVIEW que habla un protocolo propietario por USB.

**Para desbloquear el encargo no tenemos que simularla nosotros: NI ya lo
hace.** NI-MAX crea
*NI-DAQmx Simulated Devices* de cualquier modelo soportado, y el USB-6343 lo
está. El cliente lo crea con un par de clics y sus tareas de DAQmx funcionan
sin hardware. Aparece en NI-MAX con icono amarillo en vez de verde.

Lo que costaba semanas es una casilla que marcan ellos.

> **La limitación, ahora que sabemos para qué la usan**: el simulado devuelve
> datos sintéticos —típicamente una senoide— **sin relación con el resto del
> banco**. Con los cinco usos de arriba eso significa que, con el simulado de
> NI-MAX:
>
> - las lecturas de la sonda y del bias-T darán una senoide, no lo que haya;
> - **los SPDT no conmutarán nada**, porque nadie escucha esas líneas;
> - los triggers no dispararán nada.
>
> Las secuencias **correrán sin error**, que es lo que hace falta para la
> primera entrega. Pero el banco no será coherente: el sensor no podrá leer lo
> que el generador emite, porque el camino entre los dos pasa por conmutadores
> que nadie está simulando.
>
> Por eso el simulador propio sube de prioridad respecto al análisis anterior:
> no es un séptimo equipo, es **la pieza que cierra el lazo**. Sigue
> **aplazado** por decisión de producto, pero conviene saber que sin él la
> propagación entre instrumentos (B4) llega hasta donde llega.

### 3.5 ⚠️ NI 783496-01 — corrección de inventario

**El 783496-01 no es un controlador cRIO: es un chasis NI-9147**, de expansión
Ethernet RIO, 4 slots, FPGA Zynq-7020, para módulos de la serie C.

La diferencia importa mucho:

- Un cRIO controlador (cRIO-904x, cRIO-905x…) tiene procesador con Linux
  Real-Time; puede correr un servidor propio y exponer lo que se quiera —
  incluso un socket SCPI de fabricación casera.
- **El NI-9147 no ejecuta código de aplicación propio.** Es I/O remota: se
  accede desde un host (Windows o un RT) por **NI-RIO**, en modo Scan Engine o
  contra la FPGA, con LabVIEW. No hay protocolo de texto que simular, y el
  protocolo que hay es propietario y no documentado.

Esto tiene dos lecturas, y hay que preguntar al cliente cuál es la suya
**(por confirmar)**:

- **(a)** El software habla con el 9147 por NI-RIO. Entonces «TCP» describe el
  cable, no un protocolo que podamos simular, y esto se parece al problema de
  la DAQ (§3.4) más que al de un instrumento.
- **(b)** Hay un host intermedio (un PC o un cRIO de verdad) que expone un
  protocolo propio sobre TCP, y el BITE del DUT se consulta por ahí. **Entonces
  sí es simulable**, y lo que hace falta es la especificación de *ese*
  protocolo, no el modelo del chasis.

La (b) es lo más probable si el cliente dice que «se comunican por TCP». La
respuesta cambia por completo el coste de esta parte. El cliente ya la sitúa
fuera de la primera entrega, así que hay tiempo para aclararlo — pero conviene
aclararlo pronto, porque **el BITE del DUT depende de esto**.

### 3.6 DUT por serie (FTDI) — **la brecha crítica**

El DUT **va enteramente por serie**, sobre un puente USB-serie FTDI. No es solo
la consola de mantenimiento: es cómo se opera. En Windows aparece como `COMx`,
y su recurso VISA es `ASRL3::INSTR` o similar.

**El DUT sale de la primera entrega** por decisión del cliente: con poder
conectar los equipos les vale. Lo que sigue queda pendiente para después.

Parámetros de línea confirmados por el cliente: **115200 baudios, «estándar»**
—que se entiende 8N1 sin control de flujo, **a confirmar**—. Lo que sigue sin
haber, y es lo que de verdad hace falta, es **el juego de órdenes de la consola
y su terminador de línea**: saber los baudios no es saber el protocolo.

Crucible no tiene transporte serie, y esta es ahora **la única brecha cara que
queda en pie**. Sin ella no hay DUT, y sin DUT no hay test.

Como todo corre en la misma máquina Windows, la solución es un **par de puertos
COM virtuales** (com0com): Crucible abre un extremo, LabVIEW el otro, y NI-MAX
ve el segundo como un `ASRL` corriente. El detalle y sus riesgos están en la
Fase 3 del plan.

Falta la especificación del protocolo de la consola **(por confirmar)**:
velocidad, bits, paridad, control de flujo, terminador de línea y juego de
órdenes. El protocolo del DUT no es SCPI, así que además hará falta el codec
`serial_ascii` que el formato ya declara y nadie ha implementado.

> **Sinergia que conviene no dejar pasar.** `wasi-visa`, el cuarto proyecto del
> eje, **ya tiene el lado cliente de `ASRL`** por la interfaz de host
> `anlaco:visa/serial`. Lo que falta es el lado servidor en Crucible.

## 4. Lo que Crucible cubre hoy, verificado

No es teoría: el banco de ejemplo se ha levantado y se ha interrogado con
`wasi-visa` desde un guest WASM.

- Motor SCPI-99 / IEEE 488.2 completo: formas larga y corta, nodos opcionales,
  mensajes compuestos con cabecera relativa, cola de errores, registros de
  estado. Los comandos comunes y `SYSTem:ERRor?` salen gratis.
- Perfiles declarativos en YAML, bancos multi-instrumento, un puerto TCP por
  instrumento, `estado_inicial` por unidad.
- Modelos de medida por fórmula, con ruido gaussiano de semilla fija:
  reproducible al bit, apto para CI.
- Formato de salida realista (`cientifico:6`, `fijo:4`, `entero`).

## 5. Brechas, ordenadas por lo que cuestan

| id | Brecha | Bloquea | Coste |
|---|---|---|---|
| **B1** | Sufijos de canal ignorados en el camino declarativo | Cualquier equipo multicanal | Bajo — el motor SCPI ya los entrega; se descartan en `crucible-core/src/protocolo/scpi.rs:121` |
| ~~**B2**~~ | ~~Sufijos de unidad y tipos no respetados~~ | ~~#1 y #2~~ | ✅ **cerrada** — ver §8 |
| **B3** | Sin rangos ni límites declarables (no hay `-222`) | Simular fallos de #1 y #2 | Medio — pide tipar `estado:` |
| **B4** | Sin propagación entre instrumentos | Que los sensores midan lo que emite el generador | Medio-alto — es H4 del roadmap |
| **B5** | Sin transporte USBTMC | **#3** en el banco real | **Aplazada, no descartada** — los alias la esquivan en este encargo (§3.3), pero Crucible la necesita como producto |
| **B6** | Sin transporte serie | **#6** (el DUT entero) | Medio-alto — **la brecha crítica**; com0com + codec `serial_ascii` |
| **B7** | Sin NI-DAQmx | **#4** con correlación | **Aplazada** — el simulado de NI-MAX desbloquea el encargo (§3.4); hacerlo nosotros sigue siendo deseable |
| **B8** | Sin NI-RIO | **#5, #7** | Muy alto o irrelevante, según §3.5 |
| **B9** | El lenguaje de fórmulas no tiene comparaciones | CV/CC, OVP, compliance | Bajo — el parser ya es de descenso recursivo |

B2 y B3 son las que convierten el banco de «responde algo» en «sirve para
validar software de test», y son las dos más baratas de las que importan.

### B2 — cerrada el 19/09/2026

El sufijo de unidad se interpreta con su multiplicador y el tipo declarado en
`estado:` se respeta. Detalle en §8.

### B9 — comparaciones en las fórmulas

Un modelo solo puede hacer aritmética (`+ - * /`, `min`, `max`, `abs`, `sqrt`).
No hay `>`, `<` ni condicional, y `cuando:` solo compara una variable con un
literal, nunca dos variables entre sí.

Eso deja fuera justo lo que hace creíble a una fuente o a un generador:

- el indicador CV/CC, que es «¿`I_lim*R` supera a `V_prog`?»;
- el disparo automático de OVP, que es «¿la salida supera `ovp_nivel`?»;
- cualquier límite de rango, que es «¿el valor cae fuera de `[min, max]`?» —
  o sea, **B3 también depende de esto**.

En el perfil de la fuente hay un apaño aritmético para el CV/CC, comentado en
su sitio, que funciona pero es mal ejemplo para quien copie el perfil. Añadir
operadores de comparación al parser es barato y desbloquea B3 de paso.

## 8. Muestra construida

`bancos/rf/` contiene el banco servible hoy, con los dos equipos de §2 que
hablan SCPI sobre TCP:

```
bancos/rf/banco.yaml
bancos/rf/perfiles/n5171b.yaml
bancos/rf/perfiles/n5767a.yaml
```

```bash
crucible --validar bancos/rf/banco.yaml
crucible bancos/rf/banco.yaml
```

Verificado con `wasi-visa` desde un guest WASM, contra el banco levantado:

```
*IDN?             -> Keysight Technologies,N5171B,MY53050001,B.01.50
:FREQ 2.4 GHZ     ·  :FREQ?  -> +2.400000E+09
:POW -10 DBM      ·  :POW?   -> -10.00
:POW MAX          ·  SYST:ERR? -> -224,"Illegal parameter value;MAX"   (y :POW? sigue en -10.00)

*IDN?             -> Agilent Technologies,N5767A,US00000001,A.01.08
VOLT 12 · CURR 5  ·  MEAS:VOLT? -> 12.0028 · MEAS:CURR? -> 2.9994 · STAT:OPER:COND? -> 1  (CV)
CURR 1.5          ·  MEAS:VOLT? ->  5.9982 · MEAS:CURR? -> 1.5020 · STAT:OPER:COND? -> 0  (CC)
VOLT 500 MV       ·  VOLT?      -> 0.500
```

### Lo que hubo que arreglar para que la muestra funcionase

El primer comando real del generador —`:FREQ 2.4 GHZ`— dejaba el instrumento
inservible. Dos defectos encadenados, los dos corregidos:

1. **`instrusim-scpi` descartaba el sufijo de unidad en vez de interpretarlo**,
   así que `2.4 GHZ` valía 2,4 Hz: mil millones de veces menos, en silencio.
   Ahora hay un módulo `unit` que lo resuelve, con la regla real de los
   instrumentos para la `M` ambigua (mega en `MHZ`, mili en `MA`/`MV`/`MS`).
2. **El camino declarativo no miraba el tipo de la variable**, y guardaba
   cadenas donde había números. Ahora el tipo que declara `estado:` se respeta:
   una variable numérica acepta números con unidad y rechaza lo demás con
   `-224`, sin tocar el valor anterior, y una mutación que falla no escribe
   nada — el instrumento no queda medio configurado.

`MIN`/`MAX` siguen sin resolverse (hace falta B3, que necesita B9), pero ahora
**fallan a la vista** en lugar de corromper el estado.

## 6. Preguntas para el cliente

La lista viva, con lo ya contestado y el contexto de coste, está en §7 del
[plan](plan-banco-rf.md). Lo que sigue abierto, por orden de lo que bloquea:

1. **SWR** (§3.3): ¿el sensor devuelve la SWR ya calculada, o su software lee
   directa y reflejada y la calcula?
2. **Modelo y números de serie** de los tres sensores, para que `*IDN?` y el
   descriptor cuadren con lo que tienen configurado.
3. **Los SPDT** (§3.4): cuántos son, qué conmutan y qué líneas los mandan.
4. **Juego de órdenes de la consola serie del DUT** y su terminador. Ya no es
   urgente; sin esto el trabajo del DUT no se puede empezar cuando toque.

## 7. Fuentes

Especificaciones verificadas contra documentación del fabricante:

- [Datasheet EXG X-Series N5171B/N5172B](https://www.interlligent.co.uk/wp-content/uploads/2020/10/exg-x-series-signal-generators-n5171b-analog-n5172b-vector-data-sheet.pdf) — rangos de frecuencia y amplitud
- [Referencia SCPI X-Series (N5180-90057)](https://anlage.umd.edu/N5173B%20SCPI%20Command%20Reference%20N5180-90057.pdf) — sintaxis exacta de los comandos
- [Guía de usuario Serie N5700](https://ridl.cfd.rit.edu/products/manuals/agilent/power%20supplies/cd1/Model/N5700usr.pdf) — ratings del N5767A, resumen SCPI, puertos 5024/5025
- [U2061XA, ficha de producto](https://www.keysight.com/us/en/product/U2061XA/10mhz-6ghz-usb-wide-dynamic-range-average-peak-power-sensor.html) — rango 10 MHz–6 GHz
- [USB-6343, especificaciones](https://www.ni.com/docs/en-US/bundle/usb-6343-specs/page/specs.html) — canales y velocidades
- [NI-9147, ficha de producto](https://www.ni.com/en/shop/hardware/compactrio-chassis/model-ni-9147) y [guía de inicio](https://download.ni.com/support/manuals/376331c.pdf) — identificación del 783496-01
- [Chasis Ethernet RIO](https://www.ni.com/docs/en-US/bundle/ni-compactrio/page/enet-rio-chassis.html) — modos de programación
