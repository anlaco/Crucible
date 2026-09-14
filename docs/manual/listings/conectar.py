import pyvisa

rm = pyvisa.ResourceManager("@py")  # o ResourceManager() si usas NI-VISA
opciones = dict(read_termination="\n", write_termination="\n")

fuente = rm.open_resource("TCPIP0::127.0.0.1::5025::SOCKET", **opciones)
dmm = rm.open_resource("TCPIP0::127.0.0.1::5026::SOCKET", **opciones)

print(fuente.query("*IDN?"))
fuente.write("VOLT 5")
fuente.write("OUTP ON")
print(fuente.query("MEAS:CURR?"))  # 5 V sobre la carga de 10 ohmios
print(dmm.query("MEAS:VOLT?"))
