# What it does not do (yet)

Worth knowing before you describe your bench.

| Limitation | Consequence |
|---|---|
| **Only SCPI over a TCP socket** (`::SOCKET`). No VXI-11 (`::INSTR`), HiSLIP, USB, GPIB or serial. | Your software's `TCPIP0::…::INSTR` resources must become `TCPIP0::…::<port>::SOCKET`. |
| **No Modbus or custom serial protocols.** The format anticipates them, but a profile that is not `scpi` does not start. | A thermal camera or a fixture with its own protocol cannot be simulated yet. |
| **Instruments do not see each other.** The supply does not power the multimeter. | To chain values, your script sets them (chapter 8). |
| **No conditionals in formulas.** `cuando` only chooses between the formula and a fixed `fallback`. | A `READ?` that changes with the configured function (voltage or current) cannot be modelled; simulate the one your test uses. |
| **No binary data**: `#` blocks, waveforms, screenshots. | Text answers only. |
| **No timing**: everything answers instantly. No slow `*OPC`, no simulated timeouts. | It cannot test how your software handles timeouts. |
| **Noise is reproducible**: the same series on every start. | Good for automated tests; do not expect different values between runs. |
| **Messages and file keys are in Spanish.** | This manual translates them where they matter. |
| **YAML files are read at start-up.** | After editing a file, stop Crucible and start it again. |

If any of these blocks your bench,
[open an issue](https://github.com/anlaco/Crucible/issues) with the concrete
case: which instrument and which commands.
