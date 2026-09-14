# Qué es Crucible

Crucible sirve para **probar software de test sin el hardware**. En vez de
necesitar la fuente, el multímetro y la SMU encima de la mesa para desarrollar
tu secuencia, describes cómo se comportan en unos ficheros de texto y Crucible
los pone a escuchar en la red.

Tu software no nota la diferencia porque no la hay en lo que ve: abre un
recurso VISA, manda `*IDN?`, configura `VOLT 5`, pregunta `MEAS:CURR?` y recibe
respuestas con el formato del aparato real. Con los mismos errores, además: un
comando mal escrito deja su entrada en `SYST:ERR?`, como en un instrumento de
verdad.

## Qué simula y qué no

Un instrumento de test es, casi siempre, **una máquina de estados con
respuestas**: recibe un comando, cambia algo por dentro y contesta. Crucible
modela eso:

- **Estado**: la tensión programada, si la salida está encendida, el rango…
- **Comandos**: qué entiende el instrumento y qué cambia cada uno.
- **Modelos**: cómo calcula lo que "mide", con una fórmula sobre el estado. La
  corriente de una fuente puede ser la ley de Ohm con un poco de ruido.

No simula física continua ni circuitos: la fuente no alimenta de verdad al
multímetro. Si tu prueba necesita que un valor pase de un instrumento a otro,
tu script lo fija (el capítulo 8 enseña cómo).

## Las palabras

| Palabra | Qué significa aquí |
|---|---|
| **banco** | Los instrumentos que se simulan a la vez. Un fichero `banco.yaml`. |
| **perfil** | La descripción de **un modelo** de instrumento: su `*IDN?`, su estado, sus comandos y sus modelos. Un fichero YAML por modelo. |
| **dispositivo** | Una **unidad** del banco: un perfil servido en un puerto. Dos multímetros iguales son dos dispositivos con el mismo perfil. |
| **estado** | Las variables que recuerda el instrumento. Es uno por dispositivo y lo comparten todas las conexiones. |
| **modelo** | Una fórmula que calcula la respuesta de una consulta a partir del estado. |
| **recurso VISA** | La dirección con la que tu software abre el instrumento: `TCPIP0::127.0.0.1::5025::SOCKET`. |

## Qué necesitas

- **Linux x86-64 o Windows.** Todo lo de este manual se ha ejecutado en Linux.
  En Windows las órdenes son las mismas; la instalación y la prueba de
  PowerShell del capítulo 4 las ejecuta la integración continua en cada
  versión, el resto está **no verificado** en Windows.
- Para probarlo sin tu software: `nc` (netcat) en Linux, o PowerShell en
  Windows.
- Para el ejemplo de Python: Python 3 con `pyvisa` y `pyvisa-py`.

Ningún instrumento, ninguna licencia y ningún driver VISA especial.
