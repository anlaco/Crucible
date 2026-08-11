# Diseño: Topología de banco

> **Prioridad:** fundacional. **Propuesta** para discusión antes de
> implementar.

Un **banco** es **varios instrumentos + un DUT**, interconectados. La
topología describe qué hay y cómo se relacionan, igual que un esquema de
banco en una hoja de test. Es **el hueco que nadie cubre hoy**: el gemelo
del *banco entero*, no del instrumento aislado (que es lo que hace
PyVISA-sim).

## Qué describe una topología

1. **Instrumentos**: instancias de perfiles (uno o varios, pueden repetir
   modelo en distintos puertos).
2. **Conexiones**: qué se conecta a qué (la fuente alimenta al DUT, el
   multímetro mide un nodo del DUT, el osciloscopio mira otro nodo). Son
   relaciones **lógicas** que el runtime usa para que los modelos se
   influyan entre sí.
3. **DUT**: el dispositivo bajo test. Modelo (simple al principio; una
   caja configurable de "estímulo → respuesta").
4. **Exposición de red**: qué puertos SCPI/TCP abre el runtime (uno por
   instrumento, o un multiplexor).

## Ejemplo: fuente + multímetro + DUT resistivo

```yaml
# Topología de un banco mínimo (gemelo digital). Propuesta de formato.
banco:
  nombre: banco_dc_basico

  instrumentos:
    - id: fuente
      perfil: ./perfiles/keithley_2400.yaml
      puerto: 5025
    - id: multimetro
      perfil: ./perfiles/keithley_2400.yaml   # reutiliza el perfil
      puerto: 5026
      # Override: este ejemplar mide, no fuentea.
      estado_inicial: { modo: voltage }

  dut:
    id: dut
    modelo: resistencia        # modelo simple del DUT
    parametros: { r: 1000.0 }  # 1 kΩ

  conexiones:
    - { desde: fuente.output,  a: dut.terminales }
    - { desde: dut.terminales, a: multimetro.entrada }
```

Con esta topología, el runtime sabe que el voltaje que la fuente produce
cae en el DUT y el multímetro lo mide. Un test que pida `SOUR:VOLT 5;
OUTP ON` a la fuente y luego `MEAS:VOLT?` al multímetro verá ~5 V (menos
la caída por el modelo de carga). La secuencia del consumidor no cambia
respecto a un banco real: sólo apunta cada paso al puerto correcto.

## Semántica (propuesta)

- **`instrumentos[]`**: instancias. `perfil` es un path relativo al
  archivo del banco; `puerto` es el SCPI/TCP que el runtime abre para ese
  ejemplar. `estado_inicial` overridea el `estado` del perfil (ejemplares
  que arrancan distinto).
- **`dut`**: el dispositivo bajo test. `modelo` nombra un modelo de DUT
  (resistencia, carga activa, una caja "estímulo→respuesta" configurable).
  El DUT es **lo difícil de modelar**; empezamos con modelos triviales y
  dejamos crecer.
- **`conexiones[]`**: relaciones lógicas `desde → a` entre *puntos*
  (`<instrumento>.<nodo>`). El runtime las usa para que los modelos se
  influyan (la fuente fija un voltaje que el DUT y el multímetro ven). No
  es simulación de circuito completa al principio; es **propagación de
  estado** entre modelos.
- **Determinismo**: el banco es reproducible (semillas fijas) salvo que
  se pida lo contrario. Para CI, determinismo estricto.

## Evolución: del "instrumento aislado" al "circuito"

- **Hito inicial**: instrumentos independientes, cada uno en su puerto;
  el DUT es una caja trivial. Las conexiones son informativas (el runtime
  no propaga aún). Esto ya sirve para validar un secuenciador contra
  instrumentos simulados.
- **Después**: el runtime **propaga** el estado entre instrumentos por
  las conexiones (la fuente fija V, el multímetro mide ese V). El DUT gana
  modelos (resistencia, IV curve, carga activa).
- **Más después**: un solver de circuito simple (leyes de Kirchhoff para
  DC, o un integrador para transitorios) — sólo si un caso real lo pide.
  No se empieza por aquí.

## Decisiones de diseño abiertas (para mañana)

- **¿Un puerto por instrumento, o un multiplexor (un puerto, encamina
  por cabecera)?** Propuesta: un puerto por instrumento al principio
  (simple, como instrumentos reales en la red).
- **¿Las conexiones viven en el runtime o en el formato?** En el formato
  (para que el banco sea portable y visible), pero la *propagación* la
  decide el runtime según su madurez.
- **¿Modelo de DUT como perfil o como tipo builtin?** Propuesta:
  `builtin` (resistencia, fuente-de-carga…) al principio; perfiles de DUT
  después.

## Fuera de la topología (post-MVP)

- Solver de circuito real (Kirchhoff/transitorios).
- DUT modelado por el usuario con su propio perfil complejo.
- Topología con instrumentos que se descubren dinámicamente.