# El camino de RF: qué hace falta para vender un gemelo del banco

> **Estado**: nota de dirección, 19/09/2026. No es un diseño cerrado ni un
> compromiso de implementación. Es el análisis de **qué puertas no hay que
> cerrar ahora** para que mañana se pueda modelar un camino de RF con sus
> conmutadores, acopladores y atenuadores.
>
> Decisión de producto del dueño de Crucible: el primer cliente solo pide poder
> conectar sus equipos, pero **a la larga se le va a vender un gemelo del
> banco**. Conviene que las decisiones de esta entrega no lo impidan.

## 1. Qué cambia respecto a lo que hay

[topologia-de-banco.md](topologia-de-banco.md) ya prevé un grafo de
`conexiones[]` (`desde → a`) y una evolución de «dispositivos aislados» a
«propagación de estado» y de ahí a un solver. La dirección es la correcta. Lo
que un camino de RF añade, y que ese diseño no cubre todavía, es esto:

### 1.1 Hay cosas en el banco que no son dispositivos

Un atenuador, un acoplador direccional, un SPDT, un cable, una carga de 50 Ω:
**no tienen transporte, no hablan ningún protocolo y nadie se conecta a ellos**.
Pero están en el camino y modifican lo que llega al otro extremo.

Hoy el formato de banco solo conoce `dispositivos[]` —cada uno con su
transporte obligatorio— y un `dut`. **Falta el concepto de elemento pasivo**:
algo que forma parte del banco, tiene puertos y una función de transferencia,
y a lo que nadie se conecta por red.

### 1.2 `desde → a` no basta

Una conexión punto a punto vale para «la fuente alimenta el DUT». No vale para:

- un **SPDT**, que tiene tres puertos y **cambia de topología** según una señal
  de control;
- un **acoplador direccional**, que tiene cuatro (entrada, salida, acoplado,
  aislado) y reparte la potencia entre ellos con un factor de acoplamiento y
  una direccionalidad;
- un **circulador**, donde el camino depende del sentido.

Es decir: los elementos del camino son **multipuerto**, y su comportamiento es
una relación entre puertos, no una arista.

### 1.3 Lo que se propaga no es un número

En el mundo de DC que modela `instrusim-core`, un nodo tiene un potencial. En
RF, por cada punto del camino hay **potencia incidente y potencia reflejada**,
y ambas **dependen de la frecuencia**. De esa pareja salen las magnitudes que
el cliente mide: la potencia de salida, y la SWR que su software calcula.

`Signal` en `instrusim-core` es una función del **tiempo**. Para RF hace falta
una magnitud que sea función de la **frecuencia** y que lleve dirección. No es
el mismo tipo, aunque sí la misma idea.

### 1.4 La topología cambia en tiempo de ejecución, y la manda otro equipo

Los SPDT de este banco los controla la **DAQ**, por líneas digitales (§3.4 de
[bancos/banco-rf.md](../bancos/banco-rf.md)). Así que el grafo no es estático:
depende del estado de un dispositivo que hoy **no simulamos nosotros** —lo
hace el simulado nativo de NI-MAX, que no nos dice nada.

**Este es el nudo.** Un gemelo del camino de RF no puede estar completo
mientras la DAQ sea una caja negra ajena: sin saber en qué posición están los
conmutadores, no se puede saber qué mide cada sensor. Por eso el simulador
propio de DAQmx deja de ser un capricho de completitud y pasa a ser
**condición necesaria del gemelo**.

### 1.5 Faltan logaritmos en el lenguaje de fórmulas

Los dB son logarítmicos. Mientras todo se quede en dB, un atenuador es una
resta y un acoplador otra. Pero en cuanto haya que **combinar** potencias
—dos señales que llegan al mismo punto— hay que pasar a lineal y volver:

```
P_total_dBm = 10 * log10( 10^(P1/10) + 10^(P2/10) )
```

El evaluador de `crucible-core` tiene hoy `gauss`, `abs`, `sqrt`, `min` y
`max`. **Faltan `log10` y `pow`**, y siguen faltando las comparaciones (B9).
Las tres son baratas: el parser ya es de descenso recursivo.

## 2. Lo que ya está bien encaminado

No todo hay que inventarlo. **`instrusim-core` ya tiene la abstracción
correcta**, aunque hoy solo la use para DC:

- `World` con nodos identificados, y `Terminal` como borne de un instrumento
  conectado a un nodo.
- La invariante del proyecto: **«un instrumento nunca calcula el valor que
  devuelve: lo lee de `World` por sus `Terminal`»**.
- El propio `world.rs` dice que existe desde el primer día «aunque todavía no
  resuelva ningún circuito», porque es «la costura por la que entrará la
  física».

Esa costura es exactamente la que necesita el camino de RF. Un sensor de
potencia que lee de un nodo no se entera de si lo que hay en ese nodo lo puso
una configuración fija o una red de atenuadores y conmutadores. Que es justo lo
que hace falta para pasar de «gemelo que responde» a «gemelo que simula».

## 3. La consecuencia incómoda

**El linaje declarativo no llega a esa costura.** `crucible-core` depende de
`instrusim-scpi` y de nada más: no conoce `World`, ni nodos, ni terminales. Un
perfil YAML no puede declarar que un sensor mide en un punto del banco, porque
no hay banco que modelar — solo un `HashMap` de variables por dispositivo.

Mientras el producto fuera «cada instrumento responde lo suyo», esa separación
era deuda declarada y tolerable (ADR-0003). **Si el producto pasa a ser un
gemelo del banco, deja de serlo**: la consolidación de los dos linajes se
convierte en el camino crítico, porque es la única forma de que un perfil
declarativo pueda leer de un mundo simulado.

No hay que hacerlo ahora. Hay que saber que es lo siguiente.

## 4. Qué no cerrar en la entrega actual

Reglas concretas para las decisiones de estos días:

1. **Que la potencia que devuelve un sensor salga de una variable de estado
   con nombre propio**, no de una constante incrustada en la fórmula. Mañana
   esa variable la alimentará el mundo en vez del fichero, y el perfil no
   tendrá que cambiar.
2. **No meter el camino de RF en los perfiles de instrumento.** La pérdida de
   un cable o el acoplamiento de un acoplador son del **banco**, no del
   aparato. Si acaban dentro del perfil del sensor, luego hay que sacarlos de
   ahí. (El perfil del generador ya tiene un `perdida_cable_db` que está
   justamente en el sitio equivocado por este criterio: conviene moverlo al
   banco en cuanto haya dónde.)
3. **No dar por hecho que un elemento del banco tiene transporte.** Si se toca
   el esquema del banco, dejar sitio para entradas sin `transporte`.
4. **Al añadir funciones al evaluador, añadir `log10` y `pow`** junto con las
   comparaciones. Es el mismo trabajo en el mismo sitio.
5. **No tratar la DAQ como un equipo más.** Es el que manda la topología.

## 5. Lo que hay que preguntar cuando toque

- **Los SPDT**: cuántos, qué conmutan, qué líneas los controlan.
- **El plano del banco de RF**: qué hay entre el generador y cada sensor
  —atenuadores, acopladores, cables, circuladores— y con qué valores.
- **Márgenes esperados**: con qué exactitud tiene que coincidir la potencia
  simulada con la real para que el gemelo les sirva de algo.

## 6. Para no perder de vista

Esto no es un hito con fecha. Es la razón por la que, cuando toque elegir entre
dos caminos de implementación, conviene preferir el que deje entrar la física
más tarde sin reescribir lo de ahora.
