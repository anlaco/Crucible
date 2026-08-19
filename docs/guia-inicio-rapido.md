# Guía de inicio: Anvil mide contra un instrumento de Crucible

De cero a un veredicto. Al final de esta guía una secuencia de
[Anvil](https://github.com/anlaco/anvil) mide el voltaje de un Keithley 2400 que
**no existe** y el veredicto refleja la medida del gemelo digital que sirve
Crucible — no un número fabricado de la nada.

Está pensada para alguien que no ha visto el repo: un agente de betatesting o
una persona externa por igual. No instala nada que no venga en un `cargo build`
ni supone conocimiento previo del código.

## El «mock SCPI en 5025» soy yo

La [guía de inicio rápido de Anvil](https://github.com/anlaco/anvil/blob/main/docs/guia-inicio-rapido.md)
—sección *«Medir contra un instrumento que no existe»*— manda a su paso
`medir_voltaje_scpi` abrir un socket a `127.0.0.1:5025` y mandar
`MEASURE:VOLTAGE?`. Al otro lado del socket, lo que responde, ahí va esta guía.

> Ese «mock SCPI en 5025» es **Crucible**. No es un mock de mentiras: es el
> gemelo digital del banco, descrito en un YAML y servido por su protocolo real.

Esta guía no repite qué hace el paso de Anvil ni cómo se arranca su motor: eso
ya está en la guía de Anvil. Aquí va la mitad que falta: **cómo se levanta
Crucible, qué perfil se elige y por qué**.

## 1. Levantar el runtime

Necesitas Rust estable y `cargo`. Desde la raíz de este repo:

```sh
cargo build
./target/debug/crucible perfiles/keithley_2400_demo.yaml
```

El runtime imprime una línea y se queda escuchando:

```
Crucible: KEITHLEY-2400 en 127.0.0.1:5025
```

El binario es `crucible`. Toma la ruta del perfil como argumento posicional y,
opcional, el puerto:

```
crucible <perfil.yaml> [puerto]      # un dispositivo
crucible --banco <banco.yaml>        # varios dispositivos (cada uno en su puerto)
```

Por defecto escucha en `127.0.0.1:5025`, que es justo el `ANVIL_SCPI_ADDR` que
usa Anvil. Si el puerto está ocupado, arranca en otro:

```sh
./target/debug/crucible perfiles/keithley_2400_demo.yaml 5027
```

y apunta Anvil allí con `ANVIL_SCPI_ADDR=127.0.0.1:5027` (ver §5).

## 2. Comprobar que responde

Con `netcat`, contra el runtime levantado en el paso anterior:

```sh
printf '*IDN?\n'      | nc 127.0.0.1 5025   # → Keithley,2400,1234567,A1.2
printf 'MEAS:VOLT?\n' | nc 127.0.0.1 5025   # → 4.501385029307777
```

La segunda consulta la puedes lanzar desde **otra** conexión, después de cerrar
la primera:

```sh
printf 'MEAS:VOLT?\n' | nc 127.0.0.1 5025   # → 4.501385029307777 (sigue)
```

Sigue dando ~4,5 V. No es casualidad: el dispositivo es **uno solo y lo
comparten todas las conexiones**, no se clona por cliente. Lo que configures en
una sesión sobrevive a la reconexión, igual que un aparato de verdad que no se
reinicia entre quien lo maneja. Si a alguien se le ocurre reconfigurarlo por
SCPI (`SOUR:VOLT 5.0`, por ejemplo), el cambio queda; un `*RST` lo devuelve al
estado del perfil.

> Si aquí te sale `0.0` en vez de ~4,5 V, estás en el perfil equivocado: ver §3.

## 3. Qué perfil usar (y por qué no el de referencia)

En `perfiles/` hay dos, y solo uno sirve para esta guía:

| Perfil | Arranca con | `MEAS:VOLT?` |
|---|---|---|
| `keithley_2400.yaml` (referencia) | `output: false`, `voltaje_fuente: 0.0` | **`0.0`** |
| `keithley_2400_demo.yaml` (demo) | `output: true`, `voltaje_fuente: 4.5` | **~4,501 V** |

Son el mismo instrumento; solo cambia el **estado inicial**. El de referencia
arranca en reposo, como un Keithley recién encendido. El `_demo` arranca con la
salida ya puesta a 4,5 V, como si alguien lo hubiera dejado configurado antes
de que llegue el secuenciador.

La guía usa el `_demo` por una razón concreta: el paso `medir_voltaje_scpi` de
Anvil **no configura nada antes** — abre su conexión y manda solo
`MEASURE:VOLTAGE?`. Contra el perfil en reposo la respuesta es `0.0`, que cae
fuera del límite `4.0–5.0` de `ejemplos/scpi.yaml`, y el paso sale en `fallo`.
Contra el `_demo`, la primera medida ya entrega ~4,5 V dentro de rango.

El porqué está en el propio perfil. El modelo de medida es una fórmula con
guarda:

```yaml
modelos:
  medir_voltaje:
    tipo: formula
    cuando: { output: "true" }        # solo mide con la salida encendida
    expr: "voltaje_fuente + gauss(0, 0.001)"
    fallback: "0.0"                    # si output es false, responde 0.0
```

El perfil de referencia hace bien en arrancar apagado: es lo que haría el
aparato real. Lo que cambia para el smoke es el **escenario**, no el modelo del
aparato. De ahí un fichero aparte.

> Selección: el perfil es un **argumento posicional** al arrancar. No hay
> `--profile` ni variable de entorno. `crucible perfiles/<cual>.yaml`.

## 4. La medida del gemelo, no un número fabricado

El `4.501385029307777` no es un número puesto a mano. Viene de evaluar
`voltaje_fuente + gauss(0, 0.001)` contra el estado del dispositivo: 4,5 V de
fondo más un ruido gaussiano de milésimas. Es lo que devolvería un Keithley real
midiendo una fuente estable.

Tiene ruido, pero **no varía entre corridas**: la semilla es fija (`42`), así
que dos ejecuciones del mismo runtime dan exactamente el mismo `4.50138…`. Es
deliberado: permite meter el smoke en CI sin que parpadee. Si reconfiguras
`voltaje_fuente` por SCPI, la medida se mueve con él; si apagas la salida, cae a
`0.0` (el `fallback`).

Y no es el `4.2` fijo que devuelve el paso `medir_voltaje` de demo de Anvil
(que no toca la red). El paso de esta guía se llama `medir_voltaje_scpi`, abre
un socket de verdad y el número que devuelve lo lee del gemelo. El veredicto
`paso` nace de que `4.501 ∈ [4.0, 5.0]`, evaluado contra esa medida real.

## 5. Correr la secuencia de Anvil contra Crucible

Crucible ya está en `127.0.0.1:5025`. Para la mitad de Anvil, sigue la [guía de
inicio rápido de Anvil](https://github.com/anlaco/anvil/blob/main/docs/guia-inicio-rapido.md)
para construir los guests (`cargo build --target wasm32-wasip2 -p ejecutor_pasos
-p motor`) y tener `wasmtime`. Después, dos terminales más, desde la raíz del
repo de Anvil:

```sh
# Terminal 2 — el ejecutor de pasos (escucha en 9100)
wasmtime -S cli -S tcp=y -S inherit-network=y \
  target/wasm32-wasip2/debug/ejecutor_pasos.wasm
```

```sh
# Terminal 3 — el motor, contra la secuencia y contra Crucible
ANVIL_SCPI_ADDR=127.0.0.1:5025 \
wasmtime -S cli -S tcp=y -S inherit-network=y --dir=. \
  target/wasm32-wasip2/debug/anvil-guest.wasm ejemplos/scpi.yaml
```

`ANVIL_SCPI_ADDR` apunta el paso a Crucible. Como el default ya es
`127.0.0.1:5025`, aquí es redundante; se deja escrito para que se vea dónde se
apunta. Si arrancaste Crucible en otro puerto (§1), cambia solo este valor.

Salida esperada, y código de salida `0`:

```
=== scpi_demo: paso ===
  [paso] medir_voltaje_scpi: SCPI medido: 4.501385029307777 V
```

El `paso` de la cabecera es el veredicto de la secuencia; el `4.50138…` es la
medida que el gemelo devolvió por el socket. Si sales en `fallo` o `error`, ver
§6.

> Nota sobre el `ANVIL_SCPI_ADDR`: este override **solo se respeta en el camino
> `wasmtime` CLI** (el guest hereda el entorno del shell). El binario único
> `anvil` —cuando exista compilado— no se lo pasa al guest y va siempre al
> default `5025`; para apuntarlo a otro puerto habría que declarar el ejecutor.
> Aquí no importa: Crucible ya escucha en `5025`.

## 6. Si algo falla

- **`connection refused` en Anvil** — Crucible no está levantado, o no está en
  `5025`. Comprueba con `printf '*IDN?\n' | nc 127.0.0.1 5025` antes de nada.
  Si arrancaste en otro puerto, ajusta `ANVIL_SCPI_ADDR`.

- **`0.0` o veredicto `fallo`** — arrancaste el perfil de referencia
  (`keithley_2400.yaml`) en vez del `_demo`. El de referencia arranca en reposo
  y la primera medida es `0.0`, fuera del límite `4.0–5.0`. Arranca con
  `perfiles/keithley_2400_demo.yaml`.

- **Valor fuera de rango o raro** — alguien reconfiguró el instrumento por SCPI
  y el estado sobrevivió. Un `*RST` lo devuelve al estado del perfil:
  `printf '*RST\n' | nc 127.0.0.1 5025`. O reinicia el runtime.

- **`SCPI sin respuesta` / `no numérica`** — hay algo escuchando en `5025` que
  no es Crucible. Para: `ss -ltn | grep :5025` debería mostrar `crucible`, no
  otra cosa.

---

Verificado ejecutando el 19/08/2026: `4.501385029307777 V`, veredicto `paso`,
exit `0`, estado que sobrevive a la reconexión. La medida es determinista por
semilla fija.