# Problemas frecuentes

## «no puede escuchar en 127.0.0.1:5025»

```console
{{#include ../listings/sesiones/12-puerto-ocupado.txt}}
```

Ya hay otro Crucible abierto, u otro programa, en ese puerto. Cierra la otra
ventana o cambia el puerto en `banco.yaml`. Para ver quién lo tiene:

- Linux: `ss -ltnp | grep :5025`
- Windows: `netstat -ano | findstr :5025`

## «no encuentro ningún banco»

```console
{{#include ../listings/sesiones/12-sin-banco.txt}}
```

Has ejecutado `crucible` sin argumentos en una carpeta sin `banco.yaml`, y no
hay banco de ejemplo instalado donde lo busca. Indica la ruta:
`crucible ~/mi_banco/banco.yaml`. (Sin argumentos, Crucible espera a que pulses
Enter también en una terminal.)

## La ventana de Windows se cierra al hacer doble clic

Si hay un error, la ventana espera a que pulses Enter. Si aun así se cierra,
ábrela desde una terminal (`cmd`) para leer el mensaje.

## `nc` se queda esperando

Falta `-q1`: el `nc` de Debian y Ubuntu no cierra la conexión al acabar de
enviar. `printf '*IDN?\n' | nc -q1 127.0.0.1 5025`.

## Mi software da *timeout*

- **Falta la terminación** `\n` al escribir: Crucible no procesa la línea hasta
  el salto de línea. Con recursos `::SOCKET`, configúrala en VISA (capítulo 4).
- **Es una orden**, sin `?`: no contesta nada, como un instrumento real. No
  leas después de una orden.
- **La consulta no está en el perfil**, o una fórmula falló: no llega respuesta
  y el error queda en la cola. Pregunta `SYST:ERR?`.

## `SYST:ERR?` devuelve `-113,"Undefined header;…"`

Esa cabecera no está en el perfil. Revisa el patrón, y si es orden o consulta
(`query: true`).

## Un comando encadenado no hace nada

`SOUR:VOLT 3;OUTP OFF` deja un `-113` con `SOUR:OUTP`. Es la regla de SCPI:
tras `;`, la cabecera es relativa al nodo anterior. Escribe `;:OUTP OFF`
(capítulo 8).

## `MEAS:…?` devuelve siempre `0.0`

No se cumple el `cuando` del modelo, por ejemplo porque la salida está apagada.
Comprueba el estado con su consulta (`OUTP?`).

## Una respuesta sale con llaves: `{tensoin}`

La variable no existe en el estado: hay una errata en `respuesta`.

## Una unidad no arranca con el valor de `estado_inicial`

El nombre de la variable está mal escrito: `estado_inicial` no avisa de las
variables que no existen (capítulo 5). O algo mandó `*RST`, que vuelve a los
valores del perfil.

## He cambiado un YAML y no noto el cambio

Crucible lee los ficheros al arrancar. Para con `Ctrl+C` y vuelve a arrancar.

## El valor configurado «se ha quedado» de una ejecución anterior

El estado es compartido y dura mientras Crucible esté abierto (capítulo 8).
Manda `*RST` al empezar tus pruebas o reinicia Crucible.
