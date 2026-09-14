# Primer arranque

Crucible trae un banco de ejemplo con tres instrumentos: una fuente de
alimentación DC, un multímetro y un Keithley 2400. Sirve para verlo funcionar
antes de describir nada.

## Arrancar el banco

- **Linux:** ejecuta `crucible` sin argumentos.
- **Windows:** menú Inicio → Crucible → *Arrancar banco*.

```console
{{#include listings/sesiones/03-arranque.txt}}
```

Cada fila es un instrumento escuchando en su puerto. Mientras la ventana siga
abierta, están "encendidos"; cada conexión que llega queda anotada debajo.
`Ctrl+C` los apaga.

Sin argumentos, `crucible` usa el primer banco que encuentra de esta lista:

1. `banco.yaml` en la carpeta desde la que lo ejecutas.
2. `banco/banco.yaml` junto al ejecutable (instalación de Windows).
3. `/usr/share/crucible/banco/banco.yaml` (instalación de Linux).

Para arrancar uno concreto: `crucible ruta/a/banco.yaml`.

## Preguntar a cada instrumento quién es

En **otra** terminal, con el banco arrancado:

```console
{{#include listings/sesiones/03-idn.txt}}
```

Cada puerto es un instrumento distinto, y cada uno contesta con su `*IDN?`.

> **`-q1` importa.** Sin él, el `nc` de Debian y Ubuntu no cierra la conexión
> al terminar de enviar y la orden se queda esperando para siempre.

Y eso es todo lo que hay que saber para empezar: son sockets TCP que hablan
SCPI terminado en salto de línea. El capítulo siguiente conecta un programa de
verdad.
