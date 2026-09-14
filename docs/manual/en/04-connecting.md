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

If it uses `::INSTR` (VXI-11) resources, change them to `::SOCKET`: Crucible
does not speak VXI-11 yet (see [What it does not do](11-limitations.md)).

## Anvil

The [Anvil](https://anlaco.github.io/anvil/) sequencer speaks SCPI over a
socket, so it connects to Crucible with nothing special. The guide
[Anvil measures against a Crucible instrument](https://github.com/anlaco/Crucible/blob/main/docs/guia-inicio-rapido.md)
(in Spanish) walks through it end to end.
