# Crucible — Manual de uso

Crucible simula un banco de instrumentos de medida. Cada instrumento se describe
en un fichero YAML y Crucible lo sirve por red hablando **SCPI de verdad**, igual
que el instrumento físico. Tu software de medida se conecta a él como si el
banco estuviera enchufado.

Contenido:

1. [Instalación](#1-instalación)
2. [Primer arranque: el banco de ejemplo](#2-primer-arranque-el-banco-de-ejemplo)
3. [Conectar tu software](#3-conectar-tu-software)
4. [Crear tu propio banco](#4-crear-tu-propio-banco)
5. [Describir un instrumento (perfil)](#5-describir-un-instrumento-perfil)
6. [Receta: de un instrumento real a su perfil](#6-receta-de-un-instrumento-real-a-su-perfil)
7. [Qué no hace (todavía)](#7-qué-no-hace-todavía)
8. [Problemas frecuentes](#8-problemas-frecuentes)

---

## 1. Instalación

### Windows

1. Ejecuta `crucible-<versión>-setup.exe`. No pide permisos de administrador.
2. El instalador deja:
   - En el **menú Inicio → Crucible**: *Arrancar banco*, *Abrir carpeta del
     banco*, *Terminal de Crucible* (una consola con el comando listo) y
     *Manual*.
   - El comando `crucible` disponible en cualquier terminal nueva (cmd o
     PowerShell).
   - El banco de ejemplo en `%LOCALAPPDATA%\Programs\Crucible\banco`, que puedes
     editar directamente.

Al actualizar a una versión nueva, **no se sobrescriben** los ficheros del banco
que ya existan: tus cambios se conservan.

### Linux (Debian, Ubuntu y derivadas)

```sh
sudo apt install ./crucible_<versión>_amd64.deb
```

Deja el comando `crucible` en `/usr/bin` y el banco de ejemplo en
`/usr/share/crucible/banco` (solo lectura: cópialo para modificarlo, ver §4).

Comprueba la instalación:

```sh
crucible --version
```

---

## 2. Primer arranque: el banco de ejemplo

- **Windows:** menú Inicio → Crucible → *Arrancar banco*.
- **Linux:** ejecuta `crucible` sin argumentos.

Verás:

```
Crucible 0.1.0 — …/banco/banco.yaml

  ID          MODELO         RECURSO VISA
  fuente      FUENTE-DC      TCPIP0::127.0.0.1::5025::SOCKET
  multimetro  MULTIMETRO     TCPIP0::127.0.0.1::5026::SOCKET
  smu         KEITHLEY-2400  TCPIP0::127.0.0.1::5027::SOCKET

Listo. Ctrl+C para parar.
```

Tres instrumentos simulados, cada uno en su puerto. Mientras la ventana siga
abierta, están "encendidos". Cada conexión que llega se anota en la ventana.

`crucible` sin argumentos busca el banco en este orden y usa el primero que
encuentre:

1. `banco.yaml` en la carpeta desde la que lo ejecutas.
2. `banco\banco.yaml` junto al ejecutable (instalación de Windows).
3. `/usr/share/crucible/banco/banco.yaml` (instalación de Linux).

Para arrancar un banco concreto: `crucible ruta/a/mi_banco.yaml`.

---

## 3. Conectar tu software

Cada instrumento es un **socket TCP en `127.0.0.1`** con su puerto. El recurso
VISA es:

```
TCPIP0::127.0.0.1::<puerto>::SOCKET
```

**Terminación:** los mensajes acaban en salto de línea (`\n`), en los dos
sentidos. Con recursos `::SOCKET`, VISA no la pone por defecto: configúrala.

### PyVISA

```python
import pyvisa

rm = pyvisa.ResourceManager("@py")          # o ResourceManager() con NI-VISA
fuente = rm.open_resource("TCPIP0::127.0.0.1::5025::SOCKET",
                          read_termination="\n", write_termination="\n")
dmm = rm.open_resource("TCPIP0::127.0.0.1::5026::SOCKET",
                       read_termination="\n", write_termination="\n")

print(fuente.query("*IDN?"))                # Crucible,FUENTE-DC,SIM0001,1.0
fuente.write("VOLT 5")
fuente.write("OUTP ON")
print(fuente.query("MEAS:CURR?"))           # 0.50000  (5 V sobre 10 Ω)
print(dmm.query("MEAS:VOLT?"))              # +3.300139E+00
```

### NI MAX / LabVIEW

Añade el instrumento como *VISA TCP/IP Resource → Manual Entry of Raw Socket*,
con host `127.0.0.1` y el puerto del banco. En LabVIEW, al abrir la sesión,
activa el carácter de terminación `\n` (0x0A) para la lectura
(*Termination Character Enabled*) y termina cada escritura con `\n`.

### Si tu software apunta a las IP del banco real

El software que hoy habla con `192.168.1.10:5025`, `192.168.1.11:5025`… tiene que
apuntar a `127.0.0.1:5025`, `127.0.0.1:5026`… Cambia la dirección en su
configuración: los números de puerto los decides tú en el `banco.yaml`.

### Prueba rápida sin software

```sh
# Linux
printf '*IDN?\n' | nc 127.0.0.1 5025
```

```powershell
# Windows (PowerShell)
$c = New-Object Net.Sockets.TcpClient('127.0.0.1', 5025); $s = $c.GetStream()
$w = New-Object IO.StreamWriter($s); $w.AutoFlush = $true; $w.WriteLine('*IDN?')
(New-Object IO.StreamReader($s)).ReadLine(); $c.Close()
```

---

## 4. Crear tu propio banco

Un banco es una carpeta con esta forma:

```
mi_banco/
├── banco.yaml             ← qué instrumentos hay y en qué puerto
└── perfiles/
    ├── fuente_xyz.yaml    ← un perfil por MODELO de instrumento
    └── dmm_abc.yaml
```

La forma más rápida de empezar es **copiar el banco de ejemplo**:

- **Windows:** menú Inicio → Crucible → *Abrir carpeta del banco* y edita ahí, o
  copia la carpeta a otro sitio.
- **Linux:** `cp -r /usr/share/crucible/banco ~/mi_banco`

### `banco.yaml`

```yaml
banco:
  nombre: banco-linea-3
  dispositivos:
    - id: fuente_principal              # nombre para ti; sale en la tabla
      perfil: perfiles/fuente_xyz.yaml  # relativo a este fichero
      transporte: { tipo: tcp, puerto: 5025 }

    - id: dmm_entrada
      perfil: perfiles/dmm_abc.yaml
      transporte: { tipo: tcp, puerto: 5026 }

    - id: dmm_salida                    # segunda unidad del MISMO modelo
      perfil: perfiles/dmm_abc.yaml     # mismo perfil…
      transporte: { tipo: tcp, puerto: 5027 }   # …otro puerto
      estado_inicial:                   # y, si quieres, otro estado de arranque
        tension_entrada: 12.0
```

Reglas:

- Cada instrumento necesita **un puerto distinto**. Crucible se niega a arrancar
  si dos coinciden, y te dice cuáles.
- Si **un solo perfil** tiene un error, **el banco entero no arranca**. Es a
  propósito: un banco al que le falta un instrumento daría resultados falsos.
- `estado_inicial` sustituye valores de la sección `estado` del perfil solo para
  esa unidad. Ojo: `*RST` devuelve la unidad a los valores **del perfil**, no a
  los de `estado_inicial`.

### Validar antes de arrancar

```sh
crucible --validar mi_banco/banco.yaml
```

Carga el banco y todos sus perfiles, comprueba patrones, fórmulas y puertos, y
muestra la tabla de recursos sin abrir ningún puerto. Úsalo cada vez que edites
un YAML: los errores dicen qué fichero, qué comando y qué está mal.

---

## 5. Describir un instrumento (perfil)

Un perfil tiene cinco secciones. Ejemplo completo y comentado:
`perfiles/fuente_dc.yaml` en el banco de ejemplo.

```yaml
dispositivo:   # quién es
protocolo:     # siempre: scpi
estado:        # sus variables internas y su valor al encender
comandos:      # qué entiende y qué hace con cada cosa
modelos:       # cómo calcula lo que "mide"
```

### 5.1 `dispositivo`

```yaml
dispositivo:
  modelo: DMM-ABC                          # obligatorio; nombre corto
  idn: "ACME,DMM-ABC,SN12345,FW2.01"       # lo que contesta *IDN?
```

Si tu software comprueba el `*IDN?` para reconocer el instrumento, copia aquí
exactamente la respuesta del aparato real.

### 5.2 `estado`

Las variables que el instrumento recuerda. Son los valores al arrancar y a los
que vuelve con `*RST`.

```yaml
estado:
  tension: 0.0
  salida: false
  rango: 10.0
  funcion: VOLT
```

Admite números, `true`/`false` y texto.

**El estado es uno por instrumento, compartido por todas las conexiones.** Si
un programa configura `VOLT 5` y se desconecta, el siguiente que conecte
encontrará los 5 V, igual que con un aparato real. Solo `*RST` o reiniciar
Crucible lo devuelven al estado del perfil.

### 5.3 `comandos`

Cada entrada es **un comando** o **una consulta** (`?`). Si un comando tiene
las dos formas, se declara **dos veces**.

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
| `patron` | La cabecera SCPI en notación de manual (ver abajo). **Sin `?` y sin argumentos.** |
| `query` | `true` si es la consulta (`VOLT?`). Por defecto, orden. |
| `args` | Nombres para los argumentos que llegan, en orden: `VOLT 5` con `args: [v]` → `<v>` vale `5`. |
| `muta` | Variables del estado que cambia: `{ variable: "<arg>" }` o un valor fijo `{ funcion: VOLT }`. |
| `respuesta` | (consultas) Texto que se contesta. `{variable}` se sustituye por el estado y `<arg>` por el argumento. |
| `modelo` | (consultas) Nombre de un modelo de `modelos` que calcula la respuesta. |

Toda consulta necesita `respuesta` o `modelo`. Una orden puede llevar solo
`muta`, o nada si basta con que se acepte sin error.

#### Notación del patrón

Es la de los manuales de los fabricantes:

- **MAYÚSCULAS**: la forma corta obligatoria; las minúsculas, la parte opcional
  de la forma larga. `VOLTage` acepta `VOLT` y `VOLTAGE`, en cualquier
  combinación de mayúsculas y minúsculas.
- **`[corchetes]`**: nodos que se pueden omitir.
  `[SOURce:]VOLTage[:LEVel]` acepta `VOLT`, `SOUR:VOLT`, `volt:lev`,
  `SOURCE:VOLTAGE:LEVEL`…

Un patrón cubre así todas las formas que admite el instrumento real: no hace
falta enumerarlas.

#### Argumentos

- `OUTP ON`, `OUTP OFF`, `OUTP 1` y `OUTP 0` se guardan como verdadero o falso,
  y los modelos los tratan igual.
- Si un argumento declarado no llega (`CONF:VOLT` sin rango), la variable **se
  queda como estaba**.
- Si no pones nombres en `args`, los argumentos se referencian por posición:
  `<0>`, `<1>`.

#### Instrumentos de varios canales

Declara un patrón por canal, con el número dentro:

```yaml
  - patron: "SOURce1:VOLTage"
    args: [v]
    muta: { tension_1: "<v>" }
  - patron: "SOURce2:VOLTage"
    args: [v]
    muta: { tension_2: "<v>" }
```

`SOUR1:VOLT 3` va al canal 1 y `SOUR2:VOLT 7` al 2. Ojo: `SOUR:VOLT` sin número
**no** se toma como canal 1; si tu software lo envía así, declara también el
patrón sin número. Una lista de canales como `MEAS:VOLT? (@2)` llega como
argumento con el texto `(@2)`.

#### Lo que ya viene incluido: no lo declares

Todos los instrumentos responden sin declarar nada a:

| Comando | Qué hace |
|---|---|
| `*IDN?` | Devuelve `dispositivo.idn` |
| `*RST` | Devuelve el estado a los valores del perfil |
| `*CLS` | Limpia la cola de errores y el registro de estado |
| `*OPC`, `*OPC?`, `*WAI` | Sincronización (siempre listo) |
| `*ESE`, `*ESE?`, `*ESR?`, `*SRE`, `*SRE?`, `*STB?`, `*TST?` | Registros IEEE 488.2 |
| `SYSTem:ERRor[:NEXT]?` | Siguiente error de la cola, p. ej. `-113,"Undefined header;FOO"` |
| `SYSTem:VERSion?` | Versión SCPI |

También vienen gratis los **mensajes compuestos** (`VOLT 5;:OUTP ON;:MEAS:CURR?`
ejecuta los tres y contesta `0.5`). Un comando desconocido **no corta la
conexión**: se anota en la cola de errores, como en un instrumento real.

Declarar un patrón que empiece por `*` es un error de validación.

### 5.4 `modelos`

Un modelo calcula la respuesta de una consulta a partir del estado. Hoy existe
un tipo, `formula`:

```yaml
modelos:
  corriente:
    tipo: formula
    cuando: { salida: true }       # opcional: solo si se cumple…
    expr: "min(tension / carga_ohm, limite) + gauss(0, 1e-5)"
    fallback: 0.0                  # …si no, contesta esto
```

| Campo | Qué es |
|---|---|
| `tipo` | Siempre `formula`. |
| `expr` | La fórmula (ver abajo). |
| `cuando` | Condiciones sobre el estado. Si alguna no se cumple, se contesta `fallback`. Varias condiciones deben cumplirse todas. |
| `fallback` | Respuesta cuando no se cumple `cuando`. Por defecto `0.0`. Puede ser número o texto. |
| `formato` | Cómo se escribe el número (ver abajo). Opcional. |

**En `expr` puedes usar:**

- Variables del estado por su nombre: `tension`, `carga_ohm`.
- Números: `10`, `0.5`, `1e-3`, `2.5E+2`.
- Operadores `+ - * /` con la precedencia habitual, paréntesis y signo negativo.
- Funciones:

| Función | Resultado |
|---|---|
| `gauss(media, sigma)` | Ruido gaussiano. Cambia en cada lectura, pero la secuencia se repite igual cada vez que arrancas Crucible (útil para pruebas reproducibles). |
| `abs(x)` | Valor absoluto |
| `sqrt(x)` | Raíz cuadrada |
| `min(a, b)`, `max(a, b)` | Mínimo y máximo |

Los errores de sintaxis (un paréntesis sin cerrar, una función que no existe)
salen al validar o arrancar, con la posición del fallo.

Una **variable que no existe** en el estado solo se detecta al hacer la
consulta: el instrumento contesta con un error en la cola (`SYST:ERR?` muestra
el nombre). Declara siempre en `estado` todas las variables que usen tus
fórmulas.

**Formato de la respuesta (`formato`).** Tu software espera los números tal y
como los escribe el instrumento real. Elige:

| `formato` | Ejemplo de respuesta | Uso típico |
|---|---|---|
| *(sin poner)* | `1.0`, `4.501385029307777` | — |
| `entero` | `1`, `0`, `-3` | Estados (`OUTP?`) y contadores |
| `fijo:N` | `fijo:4` → `4.5014` | Fuentes que contestan con decimales fijos |
| `cientifico:N` | `cientifico:6` → `+4.501385E+00` | Multímetros y la mayoría de medidas SCPI |

El formato también se aplica al `fallback` si es numérico.

Ejemplo, `OUTP?` contestando `1`/`0` como un instrumento de verdad:

```yaml
  salida_como_numero:
    tipo: formula
    cuando: { salida: true }
    expr: "1"
    fallback: 0
    formato: entero
```

Las consultas con `respuesta: "{variable}"` devuelven el valor del estado sin
formato (`5.0`). Si tu software necesita otro, usa un modelo con `expr` igual a
la variable y el `formato` que toque.

---

## 6. Receta: de un instrumento real a su perfil

1. **Averigua qué comandos usa tu software**, no los del manual completo.
   - Si puedes, captura el tráfico con el instrumento real: NI I/O Trace,
     el log de tu aplicación o Wireshark sobre el puerto 5025.
   - Si no, busca en el código las cadenas `write(`, `query(`, `VISA Write`…
2. **Copia el `*IDN?`** del aparato real en `dispositivo.idn`.
3. **Por cada comando de configuración** (`VOLT 5`, `RANG 10`): crea una variable
   en `estado`, una entrada con `muta` y, si tu software también lo consulta,
   otra con `query: true` y `respuesta: "{variable}"`.
4. **Por cada medida** (`MEAS:VOLT?`, `READ?`): crea un modelo con la física más
   sencilla que le sirva a tu prueba, por ejemplo una constante con un poco de
   ruido o la ley de Ohm con la tensión de la fuente.
5. **Valida**: `crucible --validar banco.yaml`.
6. **Arranca y ejecuta tu software.** Si algo no responde, consulta
   `SYST:ERR?` en ese instrumento: el error dice qué cabecera no reconoció.
   Añádela al perfil y repite.

Consejo: copia la manera de escribir de los patrones del manual del fabricante
(`MEASure:VOLTage[:DC]`) y no la forma corta que usa tu software (`MEAS:VOLT`).
Así el perfil aceptará cualquier forma que otro programa envíe.

---

## 7. Qué no hace (todavía)

Conviene saberlo antes de describir el banco:

| Limitación | Consecuencia |
|---|---|
| **Solo SCPI sobre socket TCP** (`::SOCKET`). No hay VXI-11 (`::INSTR`), HiSLIP, USB, GPIB ni serie. | Los recursos `TCPIP0::…::INSTR` del software hay que cambiarlos por `TCPIP0::127.0.0.1::<puerto>::SOCKET`. |
| **Solo `127.0.0.1`** (el mismo PC). | El software de medida debe correr en el mismo equipo que Crucible. |
| **Sin Modbus.** El campo `protocolo` admite otros valores, pero solo `scpi` funciona. | Otro valor da error al cargar. |
| **Los instrumentos no se ven entre sí.** La fuente no alimenta al multímetro. | Para encadenar valores, tu script debe fijarlos (p. ej. `SIM:INP 5` del multímetro de ejemplo). |
| **Sin lógica condicional en las fórmulas.** No hay `si funcion es CURR, entonces…`; `cuando` solo elige entre la fórmula y un `fallback` fijo. | Un `READ?` que cambie según la función configurada (tensión o corriente) no se puede modelar; simula la función que use tu prueba. |
| **Sin datos binarios** (bloques `#`, formas de onda, capturas de pantalla). | Solo respuestas de texto. |
| **Sin tiempos**: todo responde al instante; no hay `*OPC` que tarde ni timeouts simulados. | No sirve para probar el manejo de timeouts. |
| **El ruido es reproducible**: misma secuencia en cada arranque. | Bueno para pruebas automáticas; no esperes valores distintos entre ejecuciones. |

Si alguna de estas bloquea vuestro banco, contádnoslo con el ejemplo concreto.

---

## 8. Problemas frecuentes

**«no puede escuchar en 127.0.0.1:5025 … ¿Hay otro programa usando ese puerto?»**
Ya hay otro Crucible abierto, u otro programa en ese puerto. Cierra la otra
ventana o cambia el puerto en `banco.yaml`.
- Windows: `netstat -ano | findstr :5025`
- Linux: `ss -ltnp | grep :5025`

**«no encuentro ningún banco»**
Has ejecutado `crucible` sin argumentos en una carpeta sin `banco.yaml`. Indica
la ruta: `crucible C:\ruta\banco.yaml`.

**La ventana de Windows se cierra al hacer doble clic**
Si arranca sin argumentos y hay un error, la ventana espera a que pulses Enter.
Si aun así se cierra, ábrela desde una terminal (`cmd`) para leer el mensaje.

**Mi software se queda esperando y da *timeout***
- Falta la terminación `\n` al escribir: Crucible no procesa la línea hasta
  recibir el salto de línea.
- Es una **orden** (sin `?`): no contesta nada, como un instrumento real. No
  leas después de una orden.
- La consulta no está en el perfil: no llega respuesta y se anota el error.
  Pregunta `SYST:ERR?`.

**`SYST:ERR?` devuelve `-113,"Undefined header;…"`**
Esa cabecera no está en el perfil. Revisa el patrón y si es orden o consulta
(`query: true`).

**Un comando encadenado no hace nada: `VOLT 5;OUTP ON`**
Es la regla de SCPI, no un fallo: tras `;`, la cabecera es **relativa** a la
anterior, así que `OUTP` se lee como `SOUR:OUTP`. Escríbelo `VOLT 5;:OUTP ON`,
con `:` tras el `;`. Un instrumento real hace lo mismo.

**`MEAS:…?` devuelve `0.0` siempre**
No se cumple el `cuando` del modelo, por ejemplo porque la salida está apagada.
Comprueba el estado con la consulta correspondiente (`OUTP?`).

**Tras cambiar un YAML no noto el cambio**
Crucible lee los ficheros al arrancar. Para (Ctrl+C) y vuelve a arrancar.

**El valor configurado "se ha quedado" de una ejecución anterior**
El estado es compartido y persiste mientras Crucible esté abierto (§5.2). Envía
`*RST` al empezar tus pruebas o reinicia Crucible.
