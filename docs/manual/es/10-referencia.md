# Referencia

## La orden `crucible`

```console
{{#include ../listings/sesiones/10-ayuda.txt}}
```

| Forma | Qué hace |
|---|---|
| `crucible` | Arranca el primer banco que encuentra: `banco.yaml` en la carpeta actual, luego junto al ejecutable, luego el de la instalación. |
| `crucible <banco.yaml>` | Arranca todos los instrumentos del banco. |
| `crucible <perfil.yaml> [puerto]` | Arranca un solo instrumento, sin banco. Puerto 5025 si no se indica. |
| `crucible --validar <fichero>` | Comprueba un banco o un perfil sin abrir puertos. Sale con código 1 si hay errores. |
| `crucible --version` | La versión. |
| `crucible --help` | La ayuda. También `-h` y `--ayuda`. |

Si el fichero es un banco o un perfil lo decide su contenido, no una opción:
tiene `banco:` o tiene `dispositivo:`.

Cualquier error hace que `crucible` salga con código 1. Sin argumentos
—normalmente un doble clic en Windows— espera además a que pulses Enter, para
que la ventana no se cierre antes de leer el motivo.

## `banco.yaml`

```yaml
banco:
  nombre: <texto>                   # opcional
  dispositivos:
    - id: <texto>                   # obligatorio
      perfil: <ruta>                # obligatorio; relativa a banco.yaml
      transporte:
        tipo: tcp                   # obligatorio; hoy solo tcp
        puerto: <número>            # obligatorio; distinto en cada dispositivo
        host: <dirección>           # opcional; 127.0.0.1 por defecto
      estado_inicial:               # opcional
        <variable>: <valor>
```

`host` decide en qué interfaz escucha el dispositivo. Por defecto `127.0.0.1`,
solo el propio equipo. Con `host: 0.0.0.0` escucha en todas, y otro equipo de la
red puede conectarse; en ese caso la tabla del arranque muestra
`TCPIP0::0.0.0.0::…`, que no es una dirección válida para conectar: usa la IP del
equipo que ejecuta Crucible. El acceso desde otro equipo está **no verificado**
en este manual.

## Perfil

```yaml
dispositivo:
  modelo: <texto>                   # obligatorio
  idn: <texto>                      # respuesta de *IDN?
protocolo: scpi                     # obligatorio

estado:
  <variable>: <número | true | false | texto>

comandos:
  - patron: <cabecera>              # obligatorio; sin '?' ni argumentos, sin '*'
    query: <true | false>           # false por defecto
    args: [<nombre>, ...]
    muta: { <variable>: <valor | "<arg>"> }
    respuesta: <texto con {variable} y <arg>>   # consultas
    modelo: <nombre de modelo>                  # consultas

modelos:
  <nombre>:
    tipo: formula                   # obligatorio
    expr: <fórmula>                 # obligatorio
    cuando: { <variable>: <valor> }
    fallback: <número | texto>      # 0.0 por defecto
    formato: <entero | fijo:N | cientifico:N>
```

## Fórmulas

| Elemento | Ejemplos |
|---|---|
| Variables | `tension`, `carga_ohm` |
| Números | `10`, `0.5`, `1e-3`, `2.5E+2` |
| Operadores | `+ - * /`, paréntesis, signo negativo |
| Funciones | `gauss(media, sigma)`, `abs(x)`, `sqrt(x)`, `min(a, b)`, `max(a, b)` |

## Errores SCPI que verás

| Error | Cuándo |
|---|---|
| `0,"No error"` | La cola está vacía. |
| `-113,"Undefined header;…"` | La cabecera no está en el perfil, o no como orden o consulta. |
| `-200,"Execution error;…"` | Una fórmula no se pudo evaluar, p. ej. por una variable que no está en `estado`. |
