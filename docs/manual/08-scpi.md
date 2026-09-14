# Lo que todo instrumento sabe hacer

Hay cosas que todo instrumento SCPI hace igual, y Crucible las da hechas. No se
declaran en el perfil.

## Comandos comunes

| Comando | Qué hace |
|---|---|
| `*IDN?` | Devuelve `dispositivo.idn`. |
| `*RST` | Devuelve el estado a los valores del perfil. |
| `*CLS` | Vacía la cola de errores y el registro de estado. |
| `*OPC`, `*OPC?`, `*WAI` | Sincronización. Siempre listo: nada tarda. |
| `*ESE`, `*ESE?`, `*ESR?`, `*SRE`, `*SRE?`, `*STB?`, `*TST?` | Registros de IEEE 488.2. |
| `SYSTem:ERRor[:NEXT]?` | El siguiente error de la cola, p. ej. `-113,"Undefined header;FOO"`. |
| `SYSTem:VERSion?` | La versión de SCPI. |

## Mensajes compuestos

Varios comandos separados por `;` se ejecutan en orden, y las respuestas de las
consultas salen en una línea, separadas también por `;` (lo viste en el
capítulo 7: `0.99999;1`).

Hay una regla de SCPI que conviene tener clara. Tras un `;`, la cabecera
siguiente es **relativa al nodo de la anterior**, salvo que empiece por `:`:

```console
{{#include listings/sesiones/08-compuestos.txt}}
```

- `VOLT 5;OUTP ON` funciona: `VOLT` cuelga de la raíz, así que `OUTP` también.
- `SOUR:VOLT 3;OUTP OFF` no: el nodo actual es `SOUR`, y `OUTP` se lee como
  `SOUR:OUTP`, que no existe. La salida sigue encendida.
- Con `:` delante (`;:OUTP OFF`) se vuelve a la raíz y funciona siempre.

Un instrumento real hace exactamente lo mismo. Si dudas, pon `;:`.

## El estado es compartido

Cada dispositivo tiene **un solo estado**, y lo comparten todas las conexiones.
No se clona por cliente ni se reinicia al desconectar: igual que un aparato real
no se resetea cuando cambia quien lo maneja.

El multímetro del ejemplo tiene un comando que el real no tiene,
`SIMulate:INPut`, para fijar la tensión en sus bornes desde un script. Cada
orden de esta sesión es una conexión distinta:

```console
{{#include listings/sesiones/08-estado-compartido.txt}}
```

Los 12 V fijados por una conexión los mide la siguiente. Y tras `*RST` vuelve a
0 V —el valor del **perfil**—, no a los 3,3 V del `estado_inicial` con el que el
banco lo arrancó.

Consecuencias prácticas:

- Si tu software configura algo y termina, la siguiente ejecución lo encuentra
  configurado. Manda `*RST` al empezar, o reinicia Crucible, si quieres partir
  de cero.
- `SIMulate:INPut` es la manera de **encadenar instrumentos**: los dispositivos
  no se ven entre sí, así que tu script lee la fuente y se lo pasa al multímetro.
  Puedes añadir comandos así a cualquier perfil.

## Los errores no cortan la conexión

Un comando desconocido, un argumento que falta o una fórmula que no se puede
evaluar dejan su error en la cola y la conexión sigue abierta. Tu software lo ve
como en el instrumento real: preguntando `SYST:ERR?`.
