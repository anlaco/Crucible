# Receta: de un instrumento real a su perfil

No hace falta describir el manual entero del instrumento. Hace falta describir
**lo que usa tu software**, y hacerlo bien.

1. **Averigua qué comandos manda tu software.**
   - Si tienes el instrumento real, captura el tráfico: NI I/O Trace, el log de
     tu aplicación o Wireshark sobre el puerto del instrumento.
   - Si no, busca en el código `write(`, `query(`, `VISA Write`…
2. **Copia el `*IDN?`** del aparato real en `dispositivo.idn`.
3. **Por cada comando de configuración** (`VOLT 5`, `RANG 10`):
   - una variable en `estado`, con su valor al encender;
   - una orden con `muta`;
   - y, si tu software también lo consulta, una consulta con
     `respuesta: "{variable}"`.
4. **Por cada medida** (`MEAS:VOLT?`, `READ?`): un modelo con la física más
   sencilla que le sirva a tu prueba. Una constante con algo de ruido, o la ley
   de Ohm con la tensión de la fuente. Ajusta el `formato` al del aparato.
5. **Añade el instrumento al banco** con su puerto (capítulo 5).
6. **Valida:** `crucible --validar banco.yaml`.
7. **Arranca y ejecuta tu software.** Si algo no responde, pregunta `SYST:ERR?`
   a ese instrumento: dice qué cabecera no reconoció. Añádela al perfil y
   repite.

## Consejos

- **Patrones del manual, no de tu código.** `MEASure:VOLTage[:DC]` acepta
  `MEAS:VOLT`, `measure:voltage:dc` y el resto de formas; `MEAS:VOLT` solo
  acepta esa.
- **Un perfil por modelo, un escenario por banco.** Si una prueba necesita que
  el instrumento arranque ya configurado, no toques el perfil: usa
  `estado_inicial` en el banco. El perfil describe el aparato; el banco, la
  situación.
- **Comandos de simulación.** Para provocar escenarios desde el script (una
  tensión fuera de rango, un sensor que falla) añade comandos que el aparato
  real no tiene, como el `SIMulate:INPut` del multímetro de ejemplo.
- **Empieza por los perfiles de ejemplo.** `fuente_dc.yaml` y `multimetro.yaml`
  están pensados como plantilla y comentados línea a línea.
