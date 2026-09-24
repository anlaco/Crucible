# Connecting your software

Each instrument on the bench is a **TCP socket on `127.0.0.1`**, on its own
port. For VISA, the resource is:

```text
TCPIP0::127.0.0.1::<port>::SOCKET
```

**Termination:** messages end with a newline (`\n`, 0x0A) in both directions.
With `::SOCKET` resources VISA does not add it by default: set it on your
session, or your software will wait for an answer that Crucible does not
process until it receives the newline.

## PyVISA

`pip install pyvisa pyvisa-py` is enough; NI-VISA is not needed. This script
talks to the supply and the multimeter on the example bench:

```python
{{#include ../listings/conectar.py}}
```

```console
{{#include ../listings/sesiones/04-pyvisa.txt}}
```

The supply has a simulated 10 Ω load connected, so at 5 V it gives half an
ampere. The multimeter reads 3.3 V because the example bench sets that when it
starts (chapter 5). The last digit is Gaussian noise, which repeats the same
every time you start the bench (chapter 7).

## NI MAX and LabVIEW

**Not verified** in this manual.

Add the instrument in NI MAX as *VISA TCP/IP Resource → Manual Entry of Raw
Socket*, with host `127.0.0.1` and the bench port. In LabVIEW, when opening the
VISA session, enable the `\n` (0x0A) termination character for reads
(*Termination Character Enabled*) and end every write with `\n`.

## Windows, without installing anything

From PowerShell. It is the same test continuous integration runs against the
Windows installer on every release:

```powershell
$c = New-Object Net.Sockets.TcpClient('127.0.0.1', 5025); $s = $c.GetStream()
$w = New-Object IO.StreamWriter($s); $w.AutoFlush = $true; $w.WriteLine('*IDN?')
(New-Object IO.StreamReader($s)).ReadLine(); $c.Close()
```

## If your software points at the real bench's IPs

Software that today opens `TCPIP0::192.168.1.10::5025::SOCKET`,
`TCPIP0::192.168.1.11::5025::SOCKET`… has to open
`TCPIP0::127.0.0.1::5025::SOCKET`, `TCPIP0::127.0.0.1::5026::SOCKET`… Change the
addresses in its configuration; you choose the ports in `banco.yaml`.

If it uses `::INSTR` (VXI-11) resources, you can keep them: declare the
`vxi11` transport in the bench (see below) or, if you would rather keep things
simple, change them to `::SOCKET`.

## VXI-11 (`::INSTR`)

If your software cannot change resource strings, set `tipo: vxi11` in the
transport and give each instrument its own IP, like the real bench:

```yaml
- id: generador
  perfil: perfiles/n5171b.yaml
  transporte: { tipo: vxi11, host: 127.0.0.2, puerto: 5025, device: inst0 }

- id: fuente
  perfil: perfiles/n5767a.yaml
  transporte: { tipo: vxi11, host: 127.0.0.3, puerto: 5025, device: inst0 }
```

```text
TCPIP0::127.0.0.2::inst0::INSTR
TCPIP0::127.0.0.3::inst0::INSTR
```

That is exactly the shape of the physical bench: every instrument is a box
with its own IP, its own portmapper on UDP 111 and its device `inst0`. Going
from the real bench to the simulated one, only the addresses change, neither
the port nor the device.

**On Linux** every `127.x.x.x` address is local and needs no setting up.
**On Windows** only `127.0.0.1` answers. There, put the instruments on that
IP, on the same port, and tell them apart by their `device`:

```yaml
transporte: { tipo: vxi11, host: 127.0.0.1, puerto: 5025, device: inst0 }
transporte: { tipo: vxi11, host: 127.0.0.1, puerto: 5025, device: inst1 }
```

That second shape is the one of a VXI chassis or a LAN/GPIB gateway: a single
address for several instruments. It works just as well, but it does force you
to touch your software's resource strings.

**UDP port 111 is required**, and it is privileged. On Linux, once:

```bash
sudo setcap 'cap_net_bind_service=+ep' /path/to/crucible
```

On Windows, start as administrator. Otherwise the bench does not start and
says exactly this: it would rather not start than advertise instruments
nobody can then find.

## Anvil

The [Anvil](https://anlaco.github.io/anvil/) sequencer speaks SCPI over a
socket, so it connects to Crucible with nothing special. The guide
[Anvil measures against a Crucible instrument](https://github.com/anlaco/Crucible/blob/main/docs/guia-inicio-rapido.md)
(in Spanish) walks through it end to end.
