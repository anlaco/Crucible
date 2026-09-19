# Plan de ejecución: banco de RF bajo LabVIEW / TestStand / NI-MAX

> **Estado**: propuesta, 19/09/2026. Acompaña a
> [banco-rf.md](banco-rf.md), que es el inventario y la especificación de los
> equipos. Este documento es el **cómo y en qué orden**, con lo que cuesta cada
> cosa y lo que puede salir mal.
>
> Nada de esto está implementado salvo lo que §6 marca como hecho.

## 1. Lo que cambió respecto al primer análisis

Cuatro datos nuevos del cliente, y los cuatro mueven el plan:

1. **Usan LabVIEW + TestStand**, y mandan **SCPI directamente**: nada de
   drivers IVI ni plug&play. Esto cierra el que era el mayor riesgo del
   proyecto (§5.4).
2. **Configuran los equipos en NI-MAX con identificadores** (alias VISA), y
   **su código abre por alias**, no por descriptor literal.
3. El criterio de aceptación que han dado es: **«arrancar Crucible, ver esos
   equipos en NI-MAX y configurarlos»**. Con eso les vale.
4. **El DUT va enteramente por serie**, no solo para mantenimiento.

Y una respuesta de topología: **Crucible correrá en la misma máquina Windows**
que LabVIEW, TestStand y NI-MAX. Todo por loopback.

### Ajustes del 19/09/2026, segunda ronda

1. **El DUT sale de la primera entrega.** «Con que nos entreguéis algo que
   pueda conectar los equipos, nos vale.» El trabajo de serie y com0com pasa a
   la Fase 3.
2. **Sus equipos aparecen en NI-MAX como `::INSTR`**, no como `::SOCKET`. Eso
   convierte VXI-11 (Fase 2) en **obligatorio**, no en mejora: es lo que hace
   que el mismo alias valga para el banco real y para el simulado.
3. **Piden que los sensores sean USB, no TCP.** Es el punto caro y tiene
   análisis propio en §5.6.
4. **«Ya veremos cómo continuar con vosotros o no.»** La primera entrega es un
   piloto con el que deciden si seguimos. Eso manda en el orden: primero lo que
   se ve y se puede enseñar.

### Por qué el punto 2 es el más importante

Si su software abre los instrumentos por alias, **el transporte que hay debajo
del alias es asunto nuestro**. Un alias `SensorPot1` puede apuntar a
`USB0::0x2A8D::…::INSTR` en el banco real y a
`TCPIP0::127.0.0.1::5027::SOCKET` en el simulado, y el VI que lo abre no nota
la diferencia.

Eso **saca USBTMC (brecha B5) del camino crítico de este encargo**, que era la
segunda cosa más cara del análisis anterior. Los tres sensores de potencia
pasan a ser perfiles SCPI corrientes servidos por TCP.

Conviene no confundir las dos cosas: **esquivar no es resolver**. USBTMC sigue
siendo necesario para Crucible como producto —el truco del alias depende de
que el cliente abra por alias, y hay parque que no— pero deja de bloquear la
entrega. Decisión tomada: **aplazado al roadmap, no descartado** (§8).

### Y el punto 3 baja el listón más de lo que parece

«Ver los equipos en NI-MAX» tiene tres niveles de ambición muy distintos:

| Nivel | Qué ve el cliente | Coste |
|---|---|---|
| **A. Receta manual** | Añade cada equipo a mano en NI-MAX (*Manual Entry of Raw Socket*), le pone su alias, y funciona | ~0 — ya funciona hoy |
| **B. Configuración generada** | Ejecuta algo una vez y NI-MAX aparece poblado con los siete alias | Bajo |
| **C. Autodescubrimiento** | Arranca Crucible y los equipos **aparecen solos** en NI-MAX | Medio-alto, y en loopback es dudoso (§5.1) |

**Confirmado con el cliente: quieren el nivel C.** Su razonamiento es que hoy,
con equipos reales, despliegan la rama de dispositivos en NI-MAX y los ven
aparecer solos —«al abrir la pestaña de LAN casi siempre aparecen los de
TCP»—, y quieren que el proceso simulado se parezca lo más posible.

Es una petición razonable y cambia el plan: el descubrimiento deja de ser un
extra y pasa a ser la **Fase 2** (§4). Lo que hay que implementar y lo que
puede salir mal está en §5.1.

## 2. Cómo queda el banco visto desde NI-MAX

```
 NI-MAX  (misma máquina Windows)
 │
 ├─ Network Devices / VISA Resources
 │    Generador    → TCPIP0::127.0.0.1::5025::SOCKET   ─┐
 │    Fuente       → TCPIP0::127.0.0.1::5026::SOCKET    │   un proceso
 │    SensorPot1   → TCPIP0::127.0.0.1::5027::SOCKET    ├── crucible
 │    SensorPot2   → TCPIP0::127.0.0.1::5028::SOCKET    │   banco.yaml
 │    SensorPot3   → TCPIP0::127.0.0.1::5029::SOCKET   ─┘
 │
 ├─ Serial & Parallel
 │    DUT          → ASRL/COM11::INSTR  ── par com0com ── COM10 ── crucible
 │
 └─ NI-DAQmx Devices
      Dev1 (USB-6343)  → dispositivo simulado NATIVO de NI-MAX, no nuestro
```

Seis de los siete equipos del inventario, con dos piezas que no escribimos
nosotros: el par de puertos COM virtuales y el simulado de DAQmx.

## 3. El reparto, revisado

| # | Equipo | Antes | Ahora | Por qué |
|---|---|---|---|---|
| 1 | N5171B | ✅ hecho | ✅ hecho | — |
| 2 | N5767A | ✅ hecho | ✅ hecho | — |
| 3 | U2061XA ×3 | ❌ falta USBTMC (coste alto) | ✅ **perfil SCPI por TCP** | Abren por alias; el transporte es transparente. USBTMC → roadmap |
| 4 | USB-6343 | ❌ NI-DAQmx (coste muy alto) | ✅ **simulado nativo de NI-MAX** | Desbloquea la entrega; el simulador propio → roadmap |
| 5 | NI-9147 | ❌ NI-RIO | ❌ fuera de alcance | Sin cambios; el cliente ya lo excluye |
| 6 | DUT serie | ⚠️ falta transporte | ⚠️ **com0com + transporte serie** | Misma máquina Windows ⇒ COM virtuales viables |
| 7 | BITE | ❌ depende de #5 | ❌ fuera de alcance | — |

**Dos brechas caras salen del camino crítico** (B5 USBTMC, B7 NI-DAQmx) sin
dejar de existir, y una sigue en pie de verdad: el **transporte serie**, que
ahora es crítico porque el DUT entero pasa por ahí.

## 4. Fases

Cada fase tiene un entregable que se puede enseñar. El orden está pensado para
que lo que puede tumbar el proyecto se descubra el primer día, no el último.

**La primera entrega son las fases 0, 1 y 2**: los cinco instrumentos visibles
en NI-MAX al descubrir dispositivos y conectables desde LabVIEW. Es el piloto
con el que el cliente decide si seguimos, así que prima lo que se ve.
El resto viene después.

### Fase 0 — Prueba de concepto de la cadena completa · **en curso**

> **Estado (19/09/2026)**: preparado todo lo que se puede hacer sin la máquina
> Windows. Falta ejecutarlo allí.
>
> - `crucible.exe` y `spike-portmapper.exe` compilados para Windows
>   (`cargo xwin`, target MSVC). **No se han podido ejecutar en Windows
>   todavía**: aquí no hay wine, así que el primer arranque es parte de la
>   prueba.
> - El spike está **validado contra un `GETPORT` sintético** en Linux: responde
>   el puerto correcto para el programa 395183, responde 0 para cualquier otro,
>   y detecta la conexión TCP posterior. Eso importa: si el spike estuviera mal,
>   el silencio de NI-MAX no nos diría nada.
> - Kit con el protocolo de prueba en `fase0-kit/` (no versionado: son
>   binarios). Seis pruebas con hueco para anotar resultados.

Un solo instrumento, la cadena entera, sin construir nada nuevo:

1. Arrancar Crucible con la fuente N5767A (perfil ya hecho).
2. Añadirla a mano en NI-MAX: *Add Network Device → VISA TCP/IP Resource →
   Manual Entry of Raw Socket*, host `127.0.0.1`, puerto `5026`, alias `Fuente`.
3. Abrirla desde el panel de test de NI-MAX y mandar `*IDN?`.
4. Un VI de LabVIEW que abra **por el alias** y lea `MEAS:VOLT?`.
5. Un paso de TestStand que use ese VI.

Y un sexto paso, que es un **spike y no una implementación**: un responder de
portmapper mínimo en UDP 111 —del orden de cien líneas, desechable— solo para
ver si NI-MAX le pregunta cuando se pulsa *Find Network Instruments*.

**Si esto no funciona, el resto del plan sobra.** Es un día de trabajo y
responde a la vez a cinco incógnitas: si NI-MAX acepta recursos en `127.0.0.1`,
si el alias funciona, si el terminador de línea que espera VISA es el que
mandamos, si TestStand añade algún requisito propio, y —la más cara de todas—
**si el descubrimiento es viable en esta topología** (§5.1).

> **Necesita una máquina Windows con NI-MAX, LabVIEW y TestStand.**
> Confirmada: hay máquina para pruebas, aunque no para compilar. Crucible se
> compila en Linux y se lleva el binario; el repositorio ya tiene instalador de
> Windows y CI que compila para esa plataforma, así que el circuito existe.

### Fase 1 — Los cinco instrumentos · **el grueso de la entrega**

Perfiles de los tres sensores U2061XA y banco de cinco instrumentos.

> **Resuelto**: los sensores se sirven por TCP. El cliente pidió USB y luego
> aceptó TCP para agilizar (§5.6). El compromiso de servir USBTMC algún día es
> del producto, no de esta entrega, y vive en el roadmap.

- Confirmar el juego de comandos del U2061XA contra su manual (no he podido
  descargarlo; §3.3 de banco-rf.md lleva la lista prevista de la familia).
- Decidir qué mide cada sensor y con qué número de serie se identifica.
- Modelo de medida: por ahora, potencia fija configurable por unidad más ruido.
  La correlación con el generador es la Fase 4.

**Entregable**: `crucible bancos/rf/banco.yaml` levanta cinco instrumentos que
responden `*IDN?`, se configuran y miden.

### Fase 2 — Descubrimiento: que aparezcan solos en NI-MAX

Es lo que ha pedido el cliente (§1) y es la fase con más incógnita técnica del
plan.

NI-MAX busca instrumentos de red mandando un **broadcast UDP al puerto 111**
—el portmapper de ONC RPC— con una llamada `GETPORT` que pregunta por el
programa **395183**, el canal core de VXI-11. El instrumento contesta con el
puerto TCP donde atiende, NI-MAX abre ahí un `create_link` y normalmente pide
`*IDN?` para etiquetarlo.

Implementarlo significa un subconjunto de ONC RPC:

1. **Codificación XDR** — big-endian, alineado a 4 bytes. Sencillo.
2. **Responder de portmapper** en UDP 111, contestando al `GETPORT`.
3. **Canal core de VXI-11** sobre TCP: `create_link`, `device_write`,
   `device_read`, `device_clear`, `device_trigger`, `destroy_link`. El resto
   del estándar no hace falta.

Estimación: del orden de mil líneas con sus tests. Es una fase seria, pero
acotada y sin dependencias externas, que encaja con la regla de cero
dependencias del proyecto.

**Un efecto secundario que conviene aprovechar**: con VXI-11 el recurso deja de
ser `TCPIP0::…::SOCKET` y pasa a ser `TCPIP0::…::INSTR`, que es lo que exponen
los equipos LXI reales. Si sus N5171B y N5767A están hoy en NI-MAX como
`::INSTR`, esto no es un extra estético sino un requisito para que el alias sea
intercambiable entre banco real y simulado (§7.2.1).

Alternativa evaluada y descartada como primera opción: **mDNS/DNS-SD**
(anunciar `_lxi._tcp` más un XML en `/lxi/identification`). Es algo menos
trabajo, pero en Windows el puerto 5353 suele estar ocupado por Bonjour —que
NI instala— y eso convierte un problema acotado en uno de convivencia. VXI-11
no tiene ese conflicto y es el mecanismo que NI-MAX usa por defecto.

**Riesgo principal en §5.1: todo esto supone una red, y aquí es loopback.**

### Fase 2b — Poblar los alias sin trabajo manual · *opcional*

Con la Fase 2 los equipos aparecen solos, pero **los alias los sigue poniendo
el cliente a mano**, que es como trabajan hoy. Si resultan ser muchos o se
repite la instalación, se puede automatizar.

`visaconf.ini` es un INI de texto (`C:\ProgramData\National
Instruments\NIvisa\visaconf.ini`) con esta forma:

```ini
[ALIASES]
NumAliases=5
Alias0="'Generador','TCPIP0::127.0.0.1::5025::SOCKET'"
Alias1="'Fuente','TCPIP0::127.0.0.1::5026::SOCKET'"

[TCPIP-RSRCS]
NumRsrcs=5
Name0="TCPIP0::127.0.0.1::5025::SOCKET"
```

Tres variantes: **(a)** `crucible --exportar-visaconf` genera el fragmento;
**(b)** generar un `.nce` importable, que es el camino que NI bendice pero cuyo
formato es binario; **(c)** la NI System Configuration API, lo más limpio pero
mete una dependencia de NI.

Recomendación: **(a)**, y solo si el cliente lo pide. Riesgo en §5.2.

### Fase 3 — Transporte serie para el DUT · *fuera de la primera entrega*

El cliente la ha sacado del alcance inmediato: «con que nos entreguéis algo que
pueda conectar los equipos, nos vale». Sigue siendo la brecha cara del
proyecto, y el DUT entero depende de ella, pero ya no bloquea la entrega.

Eso baja también la urgencia del trámite de com0com (§5.3), aunque conviene
plantearlo pronto porque no depende de nosotros.

1. **Transporte serie en Crucible**: abrir un puerto COM y servir el mismo
   perfil que hoy se sirve por TCP. En el banco sería
   `transporte: { tipo: serie, puerto: "COM10", baudios: 115200, ... }`.
2. **Par de puertos virtuales**: com0com crea `COM10` ↔ `COM11`. Crucible abre
   el primero; LabVIEW, el segundo. NI-MAX ve `COM11` como `ASRL`.
3. **Codec del DUT**: su consola **no habla SCPI**, así que hace falta el codec
   `serial_ascii` que el formato ya declara y nadie ha implementado.

Parámetros confirmados: 115200 baudios. **Falta el juego de órdenes y el
terminador**, que es lo que de verdad bloquea esta fase (§7.2.2).

Nota de diseño: el formato ya separa transporte de protocolo (ADR-0002), así
que esto es un transporte nuevo, no un dispositivo nuevo.

**Riesgo alto**: com0com es un driver de kernel y el cliente no lo conoce
(§5.3). El trámite de aprobación debería empezar ya.

### Fase 4 — Que el banco sea creíble, no solo hablador

Hasta aquí los instrumentos contestan lo que se les programó. Esto es lo que
convierte el gemelo en algo con lo que validar software de test:

- **B9 — comparaciones en las fórmulas.** Sin `>` ni `<` no hay indicador
  CV/CC honesto, ni disparo de OVP, ni límites. Barato: el parser ya es de
  descenso recursivo.
- **B3 — rangos declarables.** `VOLT 70` debe contestar `-222,"Data out of
  range"`. Depende de B9. Con esto, `MIN`/`MAX` se resuelven solos.
- **B4 — propagación entre instrumentos.** Que los sensores lean la potencia
  que el generador emite, menos las pérdidas. Sin esto, tres sensores
  devolviendo un número inventado no validan nada.

B9 y B3 son baratas y de alto retorno. B4 es el hito H4 del roadmap y es el más
caro de los tres.

### Fase 5 — Documentación para el cliente

Capítulo del manual: instalar com0com, arrancar el banco, poblar NI-MAX, crear
el simulado de DAQmx, y la tabla de alias. En los dos idiomas, como el resto.

## 5. Riesgos

### 5.1 ⚠️ El descubrimiento supone una red, y aquí es loopback

Es el riesgo número uno del plan, porque el cliente ha pedido el nivel C.

El descubrimiento de NI-MAX es un **broadcast UDP**. Con Crucible y NI-MAX en
la misma máquina y los instrumentos en `127.0.0.1`, la pregunta es si ese
broadcast llega a un proceso local. En Windows, un datagrama de difusión
enviado por un proceso normalmente **sí** se entrega a sockets locales que
escuchen en `0.0.0.0`, pero depende de la interfaz, del firewall y de cómo NI-MAX
elija la dirección de destino.

Dos mitigaciones, las dos baratas:

- **Escuchar en `0.0.0.0` y no en `127.0.0.1`**, y anunciar los instrumentos en
  la IP real de la máquina. Así el banco se parece a un equipo de red de verdad
  en lugar de a algo escondido en loopback. El formato de banco ya permite fijar
  el `host`, así que no cuesta nada probarlo.
- **Regla de firewall** para el puerto 111 UDP, que casi seguro hará falta.

**Esto hay que medirlo antes de comprometer la Fase 2**, y se puede medir sin
escribir el VXI-11 entero: basta un responder de portmapper de un centenar de
líneas que conteste al `GETPORT` y ver si NI-MAX llama. Ese spike va dentro de
la Fase 0 (§4), justamente para que la incógnita se despeje el primer día.

Si resultara que no hay forma de que NI-MAX descubra en la propia máquina, el
plan de respaldo es la Fase 3 (alias generados) más la receta manual, y hay que
decírselo al cliente cuanto antes porque es su requisito.

### 5.2 NI-MAX puede pisar `visaconf.ini`

NI-MAX no siempre refleja las modificaciones manuales del fichero, y puede
sobrescribirlo al cerrarse. La receta tendrá que decir explícitamente que se
escribe con NI-MAX cerrado, y la Fase 0 debe comprobar que el cambio sobrevive
a un reinicio de NI-MAX. Hay informes conocidos de recursos TCP/IP que
desaparecen de MAX tras reiniciar.

### 5.3 com0com necesita driver de kernel

Instalar un driver en modo kernel en un PC corporativo puede chocar con
políticas, con el arranque seguro o con permisos de administrador. La versión
firmada para Windows 10/11 existe.

**Estado: resuelto.** El cliente confirma que **puede instalarlo para las
pruebas**, y que siendo software libre no tienen inconveniente por esa parte.

Deja de ser un riesgo del plan. Queda la reserva razonable de que «para las
pruebas» no es lo mismo que «en producción»: si algún día el banco simulado
tuviera que vivir en una máquina de planta, habría que repetir el trámite. No
afecta a esta entrega, sobre todo ahora que el DUT ha salido de ella.

### 5.4 ✅ Drivers IVI — descartado

Era el riesgo mayor: un driver genérico manda decenas de comandos de
inicialización y aborta al primero que no reconoce, lo que habría multiplicado
el tamaño de los perfiles.

**El cliente manda SCPI directamente.** El principio de «declarar solo lo que
usa tu software» se mantiene, y con él la estimación de las fases 1 y 3.

### 5.5 ⚠️ La DAQ es el nexo del banco, y el simulado de NI-MAX no lo cierra

Preguntado al cliente, y la respuesta cambia el papel de este equipo. Lo usan
para generar los **triggers de las sondas** y el **pulso del generador**,
**controlar los SPDT** del camino de RF, y leer una **sonda de tensión** y la
**continua de un bias-T**.

O sea: la DAQ está **dentro del camino de señal**, no al margen. Y en concreto,
**son los SPDT los que deciden a qué punto del banco mira cada sensor de
potencia** — incluido el tercero, que el cliente usa para calibrar el generador
«en varios puntos del banco».

Con el simulado de NI-MAX las secuencias **correrán**, que es lo que hace falta
para la primera entrega. Pero los SPDT no conmutarán nada, los triggers no
dispararán nada y las entradas analógicas darán una senoide. El banco no será
coherente.

Consecuencia para el plan: **la Fase 5 (propagación, B4) llega hasta donde
llegue la DAQ.** Se puede modelar que el sensor lea lo que emite el generador
solo si fijamos el camino de RF por configuración, en vez de por el estado real
de los conmutadores. Es una limitación que conviene decirle al cliente antes de
que la descubra él.

### 5.6 ⚠️ «Que los sensores sean USB»: barato de pedir, caro de cumplir

El cliente ha pedido que los tres sensores de potencia aparezcan como USB y no
como TCP. Es comprensible —en el banco real son USB— pero **es el requisito más
caro de todo el encargo**, y conviene que sepan lo que cuesta antes de fijarlo.

El problema no es hablar USBTMC: es que **Windows tiene que enumerar un
dispositivo USB** para que NI-VISA le dé un `USB0::0x2A8D::…::INSTR`. No hay
forma de inventarse eso desde un programa de usuario. Las opciones reales:

| Camino | Qué implica | Veredicto |
|---|---|---|
| **Driver UDE de Windows** | Driver en modo kernel con la USB Device Emulation de Windows 10+. Keysight lo hizo así internamente. Exige WDK, certificado EV y atestación de Microsoft, y mantenerlo | Muy caro; y no está demostrado que NI-VISA lo acepte |
| **USB/IP software** | `usbip-vudc` (controlador de dispositivo USB virtual del kernel Linux) + gadget USBTMC en FunctionFS + `usbip-win2` en Windows | 100 % software, pero muchas piezas frágiles: módulo de kernel a medida, driver VHCI de terceros y firma |
| **Puente hardware** | Una placa tipo Raspberry Pi en modo *USB gadget* presenta un USBTMC de verdad al PC; Crucible corre detrás | El más robusto: Windows ve un USBTMC nativo, sin drivers raros. Pero **mete hardware en un producto que hoy es solo software**, y hace falta uno por sensor |
| **Negociar TCP** | Alias a un recurso TCP, como estaba previsto | Coste cero, funciona hoy |

**Antes de gastar un euro hay que preguntarles por qué lo necesitan.** Si su
código abre por alias —y nos han confirmado que sí—, un alias apuntando a TCP
funciona igual. Las razones que pueden estar detrás:

- *Que el procedimiento del operario sea idéntico*: entonces es una
  preferencia, y se negocia enseñándoles que el alias tapa la diferencia.
- *Algún VI usa el descriptor literal, no el alias*: entonces es un requisito
  duro, y hay que verlo.
- *Comprueban el tipo de interfaz en algún sitio*: idem.

**Decidido el 19/09/2026**: el cliente acepta TCP —«nos vale si agiliza las
cosas»— y los sensores se entregan como recursos TCP. La Fase 1 queda
desbloqueada y el requisito USB sale del encargo.

> **Pero no sale del producto.** Decisión del dueño de Crucible, en la misma
> conversación: **Crucible tendrá que servir USBTMC**. Se concede la excepción
> para esta entrega porque hay prisa, no porque el requisito desaparezca. Está
> registrado en [docs/roadmap.md](../roadmap.md) §H7 como compromiso y no como
> condicional, que es donde tiene que vivir: el plan de un cliente es efímero y
> el roadmap del producto no.
>
> Cuando toque, el camino a proponer es el **puente hardware**, que es el único
> robusto de los tres.

## 6. Estado actual

Hecho y verificado con `wasi-visa` (detalle en §8 de banco-rf.md):

- Perfiles del N5171B y el N5767A, con cruce CV/CC funcionando.
- `bancos/rf/banco.yaml` arranca los dos y valida.
- Interpretación de sufijos de unidad (`FREQ 2.4 GHZ`) y respeto del tipo
  declarado en `estado:`, con `-224` en vez de corromper el estado.

> El último punto **toca el motor** (`instrusim-scpi` y `crucible-core`) y se
> hizo sin consultar. Está sin commitear, con 203 tests verdes y clippy limpio,
> pendiente de revisión antes de entrar.

## 7. Decisiones y preguntas

### 7.1 Contestadas (19/09/2026)

| Pregunta | Respuesta | Efecto |
|---|---|---|
| ¿Drivers IVI o SCPI directo? | **SCPI directo** | Cierra el mayor riesgo del plan (§5.4) |
| ¿Máquina Windows para probar? | **Sí, para pruebas; no para compilar** | La Fase 0 es viable |
| ¿Nivel de integración con NI-MAX? | **Nivel C: que aparezcan al descubrir** | El descubrimiento pasa a ser la Fase 2 |
| ¿`::INSTR` o `::SOCKET` en su NI-MAX? | **`::INSTR`** | VXI-11 pasa de mejora a **obligatorio** |
| ¿El DUT en la primera entrega? | **No**, con conectar los equipos basta | La fase serie sale del alcance inmediato |
| ¿Los sensores por USB o TCP? | **TCP vale** si agiliza | Desbloquea la Fase 1; USBTMC pasa al roadmap del producto |
| ¿Pueden instalar com0com? | **Sí**, para pruebas; y es software libre | §5.3 deja de ser riesgo |
| Parámetros de la línea serie | **115200, «estándar»** | Falta lo importante: el juego de órdenes (§7.2.2) |
| ¿Qué hacen con la DAQ? | Triggers, **SPDT**, sonda de tensión, bias-T | La DAQ es el nexo del banco (§5.5) |
| ¿Los tres sensores? | Salida, SWR, calibración; con alias | — |
| ¿SWR calculada por el sensor? | **No: devuelven potencia bruta** | Los tres perfiles son el mismo modelo |
| ¿Números de serie? | **Los ajustan ellos en el YAML** | No los necesitamos; el perfil debe ser fácil de editar |
| ¿El cRIO? | Por LabVIEW directo; **fuera de la fase 1** | Confirmado fuera de alcance |

### 7.2 Abiertas, por orden de lo que bloquean

1. **Los SPDT**: cuántos hay, qué conmutan y qué líneas digitales los mandan.
   No bloquea la entrega, pero hace falta para el gemelo del banco
   ([camino-de-rf.md](../diseno/camino-de-rf.md)).
2. **Juego de órdenes de la consola serie del DUT** y su terminador. Ya no es
   urgente, pero sin esto la Fase 3 no se puede ni empezar cuando toque.

**Ya no bloquea nada de la Fase 1.** Se puede empezar.

### 7.3 Nuestras

- Revisar el cambio de motor de §6 y decidir si entra.
- Preparar el binario de Windows para llevar a su máquina en la Fase 0.

## 8. Lo que este plan deja fuera, a propósito

- **El NI-9147 y el BITE del DUT** (#5 y #7). El cliente ya los excluye de la
  primera entrega, y hasta responder §7.2.6 no se puede ni estimar.
- **Autodescubrimiento VXI-11 / mDNS**, por §5.1.

### Aplazado al roadmap de Crucible, no descartado

Decisión de producto del 19/09/2026. Las dos cosas siguen siendo necesarias;
lo que se decide es que **no bloquean esta entrega**:

- **Transporte USBTMC.** El cliente lo pidió y luego aceptó TCP para agilizar,
  así que sale de esta entrega. **No sale del producto**: por decisión del
  dueño de Crucible es un compromiso, ya registrado en
  [docs/roadmap.md](../roadmap.md) §H7. Es el transporte más caro de todos
  porque exige que el sistema operativo enumere un dispositivo USB (§5.6).
- **Simulador propio de NI-DAQmx.** El simulado de NI-MAX desbloquea la
  entrega, pero devuelve una senoide sin relación con el banco (§5.5). Hacerlo
  nosotros es la única forma de que la DAQ lea de verdad lo que la fuente
  programa. El camino es el (1) de §3.4 de banco-rf.md: una capa que suplante
  a NI-DAQmx y hable con Crucible por detrás.

Conviene llevar las dos a `docs/roadmap.md` cuando cerremos el alcance con el
cliente, para que no se queden viviendo solo aquí.
- **Consolidar los dos runtimes** del repositorio. Es deuda declarada
  (ADR-0003) y no bloquea nada de esto; mezclarlo con el encargo del cliente
  solo añadiría riesgo.

## 9. Fuentes

- [Create Simulated NI-DAQmx Devices in NI MAX](https://knowledge.ni.com/KnowledgeArticleDetails?id=kA03q000000x0PxCAI&l=en-US) — el USB-6343 se simula de forma nativa
- [Where Are NI-VISA Aliases Stored?](https://knowledge.ni.com/KnowledgeArticleDetails?id=kA00Z0000019L7jSAE) y [Measurement and Automation Explorer (MAX) Configuration Files Location](https://knowledge.ni.com/KnowledgeArticleDetails?id=kA00Z0000004B0ySAE) — `visaconf.ini`
- [Export/Import the System Configuration in NI MAX](https://knowledge.ni.com/KnowledgeArticleDetails?id=kA03q000000YHceCAG&l=en-US) — ficheros `.nce`
- [VISA TCP/IP Resources Do Not Show Up in MAX after a Reboot](https://knowledge.ni.com/KnowledgeArticleDetails?id=kA00Z0000019L7ASAU) — riesgo §5.2
- [LXI mDNS DNS-SD vs. VXI-11 para descubrimiento en NI-MAX](https://forums.ni.com/t5/Instrument-Control-GPIB-Serial/LXI-mDNS-DNS-SD-implemntation-vs-VXI-11-for-its-discovery-in-NI/td-p/3636539) — mecanismos de descubrimiento
- [com0com — null-modem emulator](https://com0com.sourceforge.net/) y [versión firmada para Windows 10/11](https://com0com.com/) — puertos COM virtuales
- [Virtual COM Ports and Third-Party USB Serial Adapters in NI-VISA](https://knowledge.ni.com/KnowledgeArticleDetails?id=kA00Z0000019Le9SAE&l=en-US) — cómo los ve NI-VISA
