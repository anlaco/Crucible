# Qué no hace (todavía)

Conviene saberlo antes de describir tu banco.

| Limitación | Consecuencia |
|---|---|
| **Solo SCPI sobre socket TCP** (`::SOCKET`) **y VXI-11** (`::INSTR`). No hay HiSLIP, USB, GPIB ni serie. | Un banco por GPIB o USB no se puede simular aún. |
| **VXI-11 necesita el puerto UDP 111**, que es privilegiado. | Hay que arrancar con permisos (`sudo setcap 'cap_net_bind_service=+ep' crucible` en Linux, administrador en Windows) o el banco no arranca. |
| **Una IP por instrumento solo en Linux.** Windows únicamente atiende `127.0.0.1`. | En Windows, los instrumentos comparten IP y se distinguen por el puerto (`tcp`) o por el `device` (`vxi11`). |
| **Sin Modbus ni protocolos serie propios.** El formato los tiene previstos, pero un perfil que no sea `scpi` no arranca. | Una cámara térmica o un fixture con protocolo propio no se pueden simular aún. |
| **Los instrumentos no se ven entre sí.** La fuente no alimenta al multímetro. | Para encadenar valores, tu script los fija (capítulo 8). |
| **Sin condicionales en las fórmulas.** `cuando` solo elige entre la fórmula y un `fallback` fijo. | Un `READ?` que cambie según la función configurada (tensión o corriente) no se puede modelar; simula la que use tu prueba. |
| **Sin datos binarios**: bloques `#`, formas de onda, capturas de pantalla. | Solo respuestas de texto. |
| **Sin tiempos**: todo responde al instante. No hay `*OPC` que tarde ni timeouts simulados. | No sirve para probar cómo maneja tu software los timeouts. |
| **El ruido es reproducible**: la misma serie en cada arranque. | Bueno para pruebas automáticas; no esperes valores distintos entre ejecuciones. |
| **Los YAML se leen al arrancar.** | Tras editar un fichero, para Crucible y vuelve a arrancarlo. |

Si alguna bloquea tu banco,
[abre una incidencia](https://github.com/anlaco/Crucible/issues) con el ejemplo
concreto: qué instrumento y qué comandos.
