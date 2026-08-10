#!/usr/bin/env python3
"""Demostración de InstruSim con sockets planos, sin dependencias.

Programa la fuente, observa el establecimiento con el multímetro y comprueba
que el protocolo SCPI se comporta como manda el estándar.

    cargo run --release --bin instrusim
    python3 scripts/demo.py
"""

import socket
import sys
import time

PUERTO_DMM = 5025
PUERTO_PSU = 5026


class Instrumento:
    """Cliente SCPI mínimo sobre socket raw."""

    def __init__(self, host, puerto):
        self.s = socket.create_connection((host, puerto), timeout=5)
        # Sin esto, Nagle retiene las líneas cortas hasta recibir el ACK de la
        # anterior y aparecen retardos de ~40 ms que parecen del simulador y no
        # lo son. NI-VISA lo desactiva por su cuenta; un cliente a mano, no.
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.f = self.s.makefile("rwb")

    def write(self, comando):
        self.f.write((comando + "\n").encode())
        self.f.flush()

    def query(self, comando):
        self.write(comando)
        respuesta = self.f.readline()
        if not respuesta:
            raise ConnectionError("el instrumento cerró la conexión")
        return respuesta.decode().strip()

    def close(self):
        self.f.close()
        self.s.close()


def main(host="127.0.0.1"):
    try:
        dmm = Instrumento(host, PUERTO_DMM)
        psu = Instrumento(host, PUERTO_PSU)
    except OSError as e:
        print(f"No se pudo conectar: {e}", file=sys.stderr)
        print("¿Está arrancado?  cargo run --release --bin instrusim", file=sys.stderr)
        return 1

    print("== Identificación ==")
    print("  DMM:", dmm.query("*IDN?"))
    print("  PSU:", psu.query("*IDN?"))

    # Al arrancar hay que dejar los dos instrumentos en un estado conocido. El
    # simulador conserva la configuración entre conexiones, igual que un equipo
    # real: si en una sesión anterior alguien fijó un rango, ahí sigue.
    dmm.write("*RST")
    psu.write("*RST")

    print("\n== Establecimiento de la fuente ==")
    psu.write("VOLT 3.3")
    # Apertura corta en el multímetro: con la de por defecto (1 NPLC = 20 ms)
    # la propia ventana de integración promedia el transitorio y no se ve.
    dmm.write("VOLT:DC:NPLC 0.02")
    psu.write("OUTP ON")

    t0 = time.perf_counter()
    for _ in range(8):
        v = dmm.query("MEAS:VOLT:DC?")
        print(f"  t = {(time.perf_counter() - t0) * 1e3:6.1f} ms   {v} V")
        time.sleep(0.002)

    print("\n== Ya establecida ==")
    dmm.write("VOLT:DC:NPLC 1")
    time.sleep(0.05)
    print("  Consigna de la fuente :", psu.query("VOLT?"))
    print("  Salida real           :", psu.query("MEAS:VOLT?"))
    print("  Medida del multímetro :", dmm.query("MEAS:VOLT:DC?"))

    print("\n== La fuente cambia y el multímetro lo nota ==")
    for consigna in ("12", "0.5", "24"):
        psu.write(f"VOLT {consigna}")
        time.sleep(0.05)
        print(f"  VOLT {consigna:>5}  ->  {dmm.query('MEAS:VOLT:DC?')} V")

    print("\n== Conformidad del protocolo ==")
    print("  Forma corta y larga   :", dmm.query("MEAS:VOLT:DC?"), dmm.query("measure:voltage:dc?"))
    print("  Consultas encadenadas :", psu.query("VOLT?;:OUTP?;:CURR?"))

    dmm.write("*CLS")
    dmm.write("MEAS:PATATA?")
    print("  Comando inventado     :", dmm.query("SYST:ERR?"))
    print("  Cola ya vacía         :", dmm.query("SYST:ERR?"))
    print("  Registro de sucesos   :", dmm.query("*ESR?"), "(bit 5 = error de comando)")

    dmm.write("VOLT:DC:RANG 1")
    print("  Fuera de rango        :", dmm.query("READ?"), "(desbordamiento)")

    psu.write("OUTP OFF")
    dmm.close()
    psu.close()
    return 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:]))
