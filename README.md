# InstruSim

Motor de simulación de instrumentación de medida.

Levanta instrumentos virtuales que hablan **SCPI** por red igual que un equipo real, para que
cualquier software se conecte a ellos y pruebe su código sin hardware. A largo plazo, un rack
completo virtualizado: instrumentos acoplados por señales reales y disparados entre sí.

## Estado

En desarrollo. Fase 1: núcleo del motor.

## Idea

Un instrumento real no soporta clientes: expone protocolos. InstruSim hace lo mismo —socket raw
y HiSLIP hablando SCPI— así que funciona con pyvisa, LabVIEW, MATLAB, C# o un simple `netcat`
sin código específico para ninguno.

```
cliente ──VISA──► TCPIP0::127.0.0.1::5025::SOCKET
                  *IDN?              → InstruSim,GDM-1000,0,1.0
                  MEAS:VOLT:DC?      → +5.000018E+00
```

Lo que lo diferencia de un simulador al uso es que los nodos del rack no guardan números sino
**señales evaluables en el tiempo**: el motor corre a 1 kHz y aun así un osciloscopio puede
muestrear a 1 GS/s.

## Requisitos

- Rust estable (1.90 o superior)
- Python con `pyvisa` para la suite de integración

## Documentación

- [Plan de arquitectura y fases](docs/PLAN.md)
- [Herencia de Crucible](docs/estandar/README.md) — el proyecto hermano que se absorbió aquí:
  propuestas de formato de perfil, topología de banco y el razonamiento de por qué esto puede
  ser un estándar y no solo una herramienta. Propuestas, no especificación.

## Licencia

Doble licencia MIT / Apache-2.0, a elección del usuario.
