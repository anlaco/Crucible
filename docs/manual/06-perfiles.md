# Describir un instrumento

Un **perfil** describe un modelo de instrumento: quién es, qué recuerda, qué
comandos entiende y qué hace con cada uno. Tiene cinco secciones:

```yaml
dispositivo:   # quién es
protocolo:     # cómo se habla; hoy, siempre scpi
estado:        # sus variables internas y su valor al encender
comandos:      # qué entiende y qué hace con cada cosa
modelos:       # cómo calcula lo que "mide" (capítulo 7)
```

Este es el perfil de la fuente del banco de ejemplo. Merece la pena leerlo
entero: el resto del capítulo lo va desmontando.

```yaml
{{#include ../../banco/perfiles/fuente_dc.yaml}}
```

## `dispositivo`

```yaml
dispositivo:
  modelo: DMM-ABC                        # obligatorio; sale en la tabla del arranque
  idn: "ACME,DMM-ABC,SN12345,FW2.01"     # lo que contesta *IDN?
```

Si tu software comprueba el `*IDN?` para reconocer el instrumento, copia aquí
**exactamente** la respuesta del aparato real.

## `protocolo`

`scpi`. El formato reserva otros valores (`modbus_tcp`, `serial_ascii`…) para lo
que vendrá, pero hoy un perfil que no sea `scpi` no arranca.

## `estado`

Las variables que el instrumento recuerda, con su valor al encender y al que
vuelven con `*RST`:

```yaml
estado:
  tension: 0.0
  salida: false
  funcion: VOLT
```

Admite números, `true`/`false` y texto. El estado es **uno por dispositivo y lo
comparten todas las conexiones**: el capítulo 8 explica qué implica.

## `comandos`

Cada entrada es **una orden** o **una consulta**. Si un comando tiene las dos
formas, como `VOLT 5` y `VOLT?`, se declara dos veces:

```yaml
comandos:
  # Orden: VOLT 5  →  guarda 5 en la variable 'tension'
  - patron: "[SOURce:]VOLTage[:LEVel]"
    args: [v]
    muta: { tension: "<v>" }

  # Consulta: VOLT?  →  contesta con el valor de 'tension'
  - patron: "[SOURce:]VOLTage[:LEVel]"
    query: true
    respuesta: "{tension}"

  # Consulta calculada: MEAS:VOLT?  →  la contesta el modelo 'lectura'
  - patron: "MEASure:VOLTage[:DC]"
    query: true
    modelo: lectura
```

| Campo | Qué es |
|---|---|
| `patron` | La cabecera en notación de manual (abajo). **Sin `?` y sin argumentos.** |
| `query` | `true` si es la consulta. Si no se pone, es una orden. |
| `args` | Nombres para los argumentos que llegan, en orden: con `args: [v]`, en `VOLT 5` `<v>` vale `5`. |
| `muta` | Variables del estado que cambia: `{ tension: "<v>" }`, o un valor fijo, `{ funcion: VOLT }`. |
| `respuesta` | (consultas) El texto que se contesta. `{variable}` se sustituye por el estado y `<arg>` por el argumento. |
| `modelo` | (consultas) El modelo de `modelos` que calcula la respuesta. |

Toda consulta necesita `respuesta` o `modelo`. Una orden puede llevar solo
`muta`, o nada si basta con que se acepte sin error.

### La notación del patrón

Es la de los manuales de los fabricantes:

- **Las mayúsculas** son la forma corta obligatoria y las minúsculas, lo que
  completa la forma larga. `VOLTage` acepta `VOLT` y `VOLTAGE`, en mayúsculas o
  minúsculas.
- **Los corchetes** marcan nodos que se pueden omitir. `[SOURce:]VOLTage[:LEVel]`
  acepta `VOLT`, `SOUR:VOLT`, `volt:lev`, `SOURCE:VOLTAGE:LEVEL`…

Un patrón cubre así todas las formas que admite el instrumento real, sin
enumerarlas. Contra la fuente del ejemplo:

```console
{{#include listings/sesiones/06-patrones.txt}}
```

Las órdenes no contestan nada, como en un instrumento real: por eso tras
`VOLT 5` no aparece ninguna línea.

> Copia los patrones del manual del fabricante (`MEASure:VOLTage[:DC]`) y no la
> forma corta que usa tu software (`MEAS:VOLT`). Así el perfil aceptará lo que
> envíe cualquier otro programa.

### Argumentos

- `ON`, `OFF`, `1` y `0` se guardan como verdadero o falso, y los modelos los
  tratan igual.
- Si un argumento declarado no llega (`CONF:VOLT` sin rango), la variable **se
  queda como estaba**.
- Sin nombres en `args`, los argumentos se referencian por posición: `<0>`,
  `<1>`.
- Una referencia que no existe, como `{variable_que_no_hay}`, se contesta
  literal, con llaves incluidas. Si ves llaves en una respuesta, hay una errata
  en el perfil.

### Instrumentos de varios canales

Un patrón por canal, con el número dentro:

```yaml
  - patron: "SOURce1:VOLTage"
    args: [v]
    muta: { tension_1: "<v>" }
  - patron: "SOURce2:VOLTage"
    args: [v]
    muta: { tension_2: "<v>" }
```

`SOUR1:VOLT 3` va al canal 1 y `SOUR2:VOLT 7` al 2. `SOUR:VOLT`, sin número,
**no** se toma como canal 1: si tu software lo manda así, declara también el
patrón sin número. Una lista de canales como `MEAS:VOLT? (@2)` llega como un
argumento con el texto `(@2)`.

## Cuando un comando no está en el perfil

No se corta la conexión ni se contesta nada: se anota en la cola de errores,
como haría el instrumento. `VOLT:PROT` no está en el perfil de la fuente:

```console
{{#include listings/sesiones/06-errores.txt}}
```

`SYST:ERR?` es la primera herramienta para completar un perfil: dice qué
cabecera no reconoció. Cada lectura saca un error de la cola; cuando se vacía,
contesta `0,"No error"`.

## Lo que no hay que declarar

`*IDN?`, `*RST`, `SYST:ERR?` y el resto de comandos comunes los resuelve
Crucible para todos los instrumentos (lista completa en el capítulo 8).
Declararlos es un error:

```console
{{#include listings/sesiones/06-comun-declarado.txt}}
```
