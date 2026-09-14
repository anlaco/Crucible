# Conectar tu software

Cada instrumento del banco es un **socket TCP en `127.0.0.1`**, en su puerto.
Para VISA, el recurso es:

```text
TCPIP0::127.0.0.1::<puerto>::SOCKET
```

**Terminación:** los mensajes acaban en salto de línea (`\n`, 0x0A) en los dos
sentidos. Con recursos `::SOCKET`, VISA no la pone por defecto: configúrala en
tu sesión, o tu software se quedará esperando una respuesta que Crucible no
procesa hasta recibir el salto de línea.

## PyVISA

Con `pip install pyvisa pyvisa-py` basta; no hace falta NI-VISA. Este script
habla con la fuente y el multímetro del banco de ejemplo:

```python
{{#include ../listings/conectar.py}}
```

```console
{{#include ../listings/sesiones/04-pyvisa.txt}}
```

La fuente tiene conectada una carga simulada de 10 Ω, así que a 5 V da medio
amperio. El multímetro mide 3,3 V porque el banco de ejemplo se lo fija al
arrancar (capítulo 5). El último decimal es ruido gaussiano, que se repite
igual cada vez que arrancas el banco (capítulo 7).

## NI MAX y LabVIEW

**No verificado** en este manual.

Añade el instrumento en NI MAX como *VISA TCP/IP Resource → Manual Entry of
Raw Socket*, con host `127.0.0.1` y el puerto del banco. En LabVIEW, al abrir
la sesión VISA, activa el carácter de terminación `\n` (0x0A) para la lectura
(*Termination Character Enabled*) y acaba cada escritura con `\n`.

## Windows, sin instalar nada

Desde PowerShell. Es la misma prueba con la que la integración continua
comprueba el instalador de Windows en cada versión:

```powershell
$c = New-Object Net.Sockets.TcpClient('127.0.0.1', 5025); $s = $c.GetStream()
$w = New-Object IO.StreamWriter($s); $w.AutoFlush = $true; $w.WriteLine('*IDN?')
(New-Object IO.StreamReader($s)).ReadLine(); $c.Close()
```

## Si tu software apunta a las IP del banco real

El software que hoy abre `TCPIP0::192.168.1.10::5025::SOCKET`,
`TCPIP0::192.168.1.11::5025::SOCKET`… tiene que abrir
`TCPIP0::127.0.0.1::5025::SOCKET`, `TCPIP0::127.0.0.1::5026::SOCKET`… Cambia
las direcciones en su configuración; los puertos los decides tú en el
`banco.yaml`.

Si usa recursos `::INSTR` (VXI-11), cámbialos por `::SOCKET`: Crucible todavía
no habla VXI-11 (ver [Qué no hace](11-limitaciones.md)).

## Anvil

El secuenciador [Anvil](https://anlaco.github.io/anvil/) habla SCPI por socket,
así que se conecta a Crucible sin nada especial. La guía
[Anvil mide contra un instrumento de Crucible](https://github.com/anlaco/Crucible/blob/main/docs/guia-inicio-rapido.md)
lo recorre de punta a punta.
