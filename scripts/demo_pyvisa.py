#!/usr/bin/env python3
"""La misma demostración, pero a través de VISA.

Es la prueba que importa: si funciona con pyvisa sin adaptación alguna,
funcionará con LabVIEW, MATLAB o cualquier otro cliente, porque todos hablan
con el instrumento por la misma vía.

    pip install pyvisa pyvisa-py
    cargo run --release --bin instrusim
    python3 scripts/demo_pyvisa.py
"""

import sys

try:
    import pyvisa
except ImportError:
    print("Falta pyvisa:  pip install pyvisa pyvisa-py", file=sys.stderr)
    sys.exit(1)

RECURSO_DMM = "TCPIP0::127.0.0.1::5025::SOCKET"
RECURSO_PSU = "TCPIP0::127.0.0.1::5026::SOCKET"


def main():
    rm = pyvisa.ResourceManager()

    dmm = rm.open_resource(RECURSO_DMM)
    psu = rm.open_resource(RECURSO_PSU)

    # Un recurso SOCKET no lleva terminador implícito: hay que decírselo.
    for inst in (dmm, psu):
        inst.read_termination = "\n"
        inst.write_termination = "\n"
        inst.timeout = 5000

    print("DMM:", dmm.query("*IDN?"))
    print("PSU:", psu.query("*IDN?"))

    # Los dos, a un estado conocido: el simulador conserva la configuración
    # entre sesiones igual que un instrumento real.
    dmm.write("*RST")
    psu.write("*RST")
    psu.write("VOLT 5")
    psu.write("OUTP ON")

    import time
    time.sleep(0.1)

    print("Medida:", dmm.query("MEAS:VOLT:DC?"), "V")
    print("Errores DMM:", dmm.query("SYST:ERR?"))
    print("Errores PSU:", psu.query("SYST:ERR?"))

    psu.write("OUTP OFF")
    dmm.close()
    psu.close()


if __name__ == "__main__":
    main()
