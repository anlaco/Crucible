# What Crucible is

Crucible lets you **test test software without the hardware**. Instead of
needing the power supply, the multimeter and the SMU on the desk to develop your
sequence, you describe how they behave in a few text files and Crucible puts
them on the network.

Your software cannot tell the difference, because there is none in what it
sees: it opens a VISA resource, sends `*IDN?`, configures `VOLT 5`, asks
`MEAS:CURR?` and gets answers in the real instrument's format. With the same
errors, too: a mistyped command leaves its entry in `SYST:ERR?`, as on a real
instrument.

## What it simulates and what it does not

A test instrument is, almost always, **a state machine with responses**: it
receives a command, changes something inside and answers. Crucible models that:

- **State**: the programmed voltage, whether the output is on, the range…
- **Commands**: what the instrument understands and what each one changes.
- **Models**: how it computes what it "measures", with a formula over the
  state. A supply's current can be Ohm's law with a little noise.

It does not simulate continuous physics or circuits: the supply does not really
power the multimeter. If your test needs a value to go from one instrument to
another, your script sets it (chapter 8 shows how).

## The words

| Word | What it means here |
|---|---|
| **bench** (`banco`) | The instruments simulated together. A `banco.yaml` file. |
| **profile** (`perfil`) | The description of **one model** of instrument: its `*IDN?`, its state, its commands and its models. One YAML file per model. |
| **device** (`dispositivo`) | One **unit** on the bench: a profile served on a port. Two identical multimeters are two devices with the same profile. |
| **state** (`estado`) | The variables the instrument remembers. There is one per device, shared by every connection. |
| **model** (`modelo`) | A formula that computes a query's response from the state. |
| **VISA resource** | The address your software opens the instrument with: `TCPIP0::127.0.0.1::5025::SOCKET`. |

## What you need

- **Linux x86-64 or Windows.** Everything in this manual was run on Linux. On
  Windows the commands are the same; the installation and the PowerShell test
  in chapter 4 are run by continuous integration on every release, the rest is
  **not verified** on Windows.
- To try it without your software: `nc` (netcat) on Linux, or PowerShell on
  Windows.
- For the Python example: Python 3 with `pyvisa` and `pyvisa-py`.

No instrument, no licence and no special VISA driver.
