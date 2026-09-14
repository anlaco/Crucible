# Recipe: from a real instrument to its profile

You do not need to describe the instrument's whole manual. You need to describe
**what your software uses**, and describe it well.

1. **Find out which commands your software sends.**
   - If you have the real instrument, capture the traffic: NI I/O Trace, your
     application's log, or Wireshark on the instrument's port.
   - If not, search the code for `write(`, `query(`, `VISA Write`…
2. **Copy the real device's `*IDN?`** into `dispositivo.idn`.
3. **For each configuration command** (`VOLT 5`, `RANG 10`):
   - a variable in `estado`, with its power-on value;
   - a command with `muta`;
   - and, if your software also reads it back, a query with
     `respuesta: "{variable}"`.
4. **For each measurement** (`MEAS:VOLT?`, `READ?`): a model with the simplest
   physics your test needs. A constant with some noise, or Ohm's law with the
   supply voltage. Match `formato` to the device.
5. **Add the instrument to the bench** with its port (chapter 5).
6. **Validate:** `crucible --validar banco.yaml`.
7. **Start it and run your software.** If something does not answer, ask that
   instrument `SYST:ERR?`: it tells you which header it did not recognise. Add
   it to the profile and repeat.

## Tips

- **Patterns from the manual, not from your code.** `MEASure:VOLTage[:DC]`
  accepts `MEAS:VOLT`, `measure:voltage:dc` and every other form; `MEAS:VOLT`
  only accepts itself.
- **One profile per model, one scenario per bench.** If a test needs the
  instrument to start already configured, do not touch the profile: use
  `estado_inicial` on the bench. The profile describes the device; the bench,
  the situation.
- **Simulation commands.** To provoke scenarios from the script (an
  out-of-range voltage, a failing sensor) add commands the real device does not
  have, like the example multimeter's `SIMulate:INPut`.
- **Start from the example profiles.** `fuente_dc.yaml` and `multimetro.yaml`
  are meant as templates and commented line by line.
