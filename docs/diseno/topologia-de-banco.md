# Diseño: Topología de banco

> **Prioridad:** fundacional. **Propuesta** para discusión antes de
> implementar.

Un **banco** es **varios dispositivos + un DUT**, interconectados. La
topología describe qué hay, por dónde se conecta cada dispositivo, y cómo
se relacionan entre sí, igual que un esquema de banco en una hoja de
test. Es **el hueco que nadie cubre hoy**: el gemelo del *banco entero*,
no del dispositivo aislado (que es lo que hace PyVISA-sim).

## Qué describe una topología

1. **Dispositivos**: instancias de perfiles (uno o varios, pueden repetir
   modelo con distintos transportes).
2. **Transporte**: qué transporte usa cada dispositivo — TCP, GPIB, USB,
   serial, PXI — y sus parámetros (puerto TCP, dirección GPIB, VID/PID
   USB, baudrate serial, etc.). **El transporte es del banco, no del
   perfil**: el mismo Keithley 2400 puede ir por TCP en un banco y por
   GPIB en otro.
3. **Conexiones**: qué se conecta a qué (la fuente alimenta al DUT, el
   multímetro mide un nodo del DUT, el osciloscopio mira otro nodo). Son
   relaciones **lógicas** que el runtime usa para que los modelos se
   influyan entre sí. Independientes del transporte.
4. **DUT**: el dispositivo bajo test. Modelo (simple al principio; una
   caja configurable de "estímulo → respuesta").

## Tipos de transporte y sus parámetros

Cada dispositivo de la topología declara su transporte. El formato
soporta los mismos transportes que VISA:

| Transporte | Parámetros | Ejemplo |
|---|---|---|
| **tcp** | `host`, `puerto` | `transporte: { tipo: tcp, host: 127.0.0.1, puerto: 5025 }` |
| **gpib** | `board`, `direccion` | `transporte: { tipo: gpib, board: 0, direccion: 5 }` |
| **usb** | `vid`, `pid`, `serial` | `transporte: { tipo: usb, vid: 0x0957, pid: 0x1B07, serial: "MY56430012" }` |
| **serial** | `puerto`, `baudrate`, `data_bits`, `parity`, `stop_bits` | `transporte: { tipo: serial, puerto: COM3, baudrate: 9600, data_bits: 8, parity: none, stop_bits: 1 }` |
| **pxi** | `bus`, `device`, `funcion` | `transporte: { tipo: pxi, bus: 2, device: 0, funcion: 0 }` |

> **El runtime de referencia sirve solo por TCP (loopback) en el MVP.**
> El formato soporta todos los transportes; el runtime crece. Ver
> ADR-0002. En el MVP, los bancos declaran `transporte: tcp` para todos
> sus dispositivos. Post-MVP, el runtime aprende serial, GPIB, USB.

## Ejemplo: fuente + multímetro + cámara térmica + DUT

```yaml
# Topología de un banco mixto (gemelo digital). Propuesta de formato.
banco:
  nombre: banco_dc_con_camara

  dispositivos:
    - id: fuente
      perfil: ./perfiles/keithley_2400.yaml
      transporte: { tipo: tcp, host: 127.0.0.1, puerto: 5025 }
      estado_inicial: { modo: voltage }

    - id: multimetro
      perfil: ./perfiles/keithley_2400.yaml   # reutiliza el perfil
      transporte: { tipo: tcp, host: 127.0.0.1, puerto: 5026 }
      # Override: este ejemplar mide, no fuentea.
      estado_inicial: { modo: voltage }

    - id: camara
      perfil: ./perfiles/camara_termica.yaml
      transporte: { tipo: tcp, host: 127.0.0.1, puerto: 502 }  # Modbus TCP

  dut:
    id: dut
    modelo: resistencia          # modelo simple del DUT
    parametros: { r: 1000.0 }    # 1 kΩ

  conexiones:
    - { desde: fuente.output,  a: dut.terminales }
    - { desde: dut.terminales, a: multimetro.entrada }
    # la cámara no se "conecta" eléctricamente; observa la temperatura
    # del DUT. Es una conexión lógica de observación.
    - { desde: dut.temperatura, a: camara.sensor }
```

Con esta topología, el runtime sabe que el voltaje que la fuente produce
cae en el DUT y el multímetro lo mide; y que la cámara observa la
temperatura del DUT. Un test que pida `SOUR:VOLT 5; OUTP ON` a la fuente
y luego `MEAS:VOLT?` al multímetro verá ~5 V (menos la caída por el
modelo de carga). Un test que lea el registro 30001 de la cámara verá la
temperatura del DUT. La secuencia del consumidor no cambia respecto a un
banco real: sólo apunta cada paso al transporte correcto.

## Semántica (propuesta)

- **`dispositivos[]`**: instancias. `perfil` es un path relativo al
  archivo del banco. `transporte` fija el transporte y sus parámetros.
  `estado_inicial` overridea el `estado` del perfil (ejemplares que
  arrancan distinto).
- **`dut`**: el dispositivo bajo test. `modelo` nombra un modelo de DUT
  (resistencia, carga activa, una caja "estímulo→respuesta" configurable).
  El DUT es **lo difícil de modelar**; empezamos con modelos triviales y
  dejamos crecer.
- **`conexiones[]`**: relaciones lógicas `desde → a` entre *puntos*
  (`<dispositivo>.<nodo>`). El runtime las usa para que los modelos se
  influyan (la fuente fija un voltaje que el DUT y el multímetro ven; la
  cámara lee la temperatura del DUT). No es simulación de circuito
  completa al principio; es **propagación de estado** entre modelos.
- **Determinismo**: el banco es reproducible (semillas fijas) salvo que
  se pida lo contrario. Para CI, determinismo estricto.

## Resource strings de VISA

Cada dispositivo de la topología se puede identificar con un *resource
string* de VISA, para que el consumidor lo encuentre como encontraría un
dispositivo real:

| Transporte | Resource string |
|---|---|
| TCP (socket) | `TCPIP0::127.0.0.1::5025::SOCKET` |
| TCP (VXI-11/HiSLIP) | `TCPIP0::127.0.0.1::inst0::INSTR` |
| GPIB | `GPIB0::5::INSTR` |
| USB | `USB0::0x0957::0x1B07::MY56430012::INSTR` |
| Serial | `ASRL3::INSTR` |
| PXI | `PXI0::2::0::0::INSTR` |

El runtime puede exponer esta información para que el consumidor descubra
los dispositivos simulados como descubriría los reales.

## Evolución: del "dispositivo aislado" al "circuito"

- **Hito inicial**: dispositivos independientes, cada uno en su
  transporte; el DUT es una caja trivial. Las conexiones son informativas
  (el runtime no propaga aún). Esto ya sirve para validar un secuenciador
  contra dispositivos simulados.
- **Después**: el runtime **propaga** el estado entre dispositivos por
  las conexiones (la fuente fija V, el multímetro mide ese V; la cámara
  lee la temperatura del DUT). El DUT gana modelos (resistencia, IV curve,
  carga activa, térmica simple).
- **Más después**: un solver de circuito simple (leyes de Kirchhoff para
  DC, o un integrador para transitorios) — sólo si un caso real lo pide.
  No se empieza por aquí.

## Decisiones de diseño abiertas (para mañana)

- **¿Un puerto por dispositivo TCP, o un multiplexor (un puerto,
  encamina por cabecera)?** Propuesta: un puerto por dispositivo al
  principio (simple, como dispositivos reales en la red).
- **¿Las conexiones viven en el runtime o en el formato?** En el formato
  (para que el banco sea portable y visible), pero la *propagación* la
  decide el runtime según su madurez.
- **¿Modelo de DUT como perfil o como tipo builtin?** Propuesta:
  `builtin` (resistencia, fuente-de-carga, térmica simple) al principio;
  perfiles de DUT después.
- **¿Cómo se modelan conexiones no eléctricas?** (cámara que observa
  temperatura, motion que mueve una sonda). Propuesta: las conexiones son
  lógicas genéricas (`desde → a`); el tipo de relación se infiere del
  nodo (`.temperatura`, `.posicion`, `.output`).

## Fuera de la topología (post-MVP)

- Solver de circuito real (Kirchhoff/transitorios).
- DUT modelado por el usuario con su propio perfil complejo.
- Topología con dispositivos que se descubren dinámicamente.
- Transportes más allá de TCP en el runtime (serial, GPIB, USB, PXI).