# Manual de Crucible

Crucible simula un banco de instrumentos de medida. Describes cada instrumento
en un fichero YAML y Crucible lo sirve por red hablando **SCPI de verdad**, igual
que el aparato físico. Tu software de medida —PyVISA, LabVIEW, un secuenciador
como [Anvil](https://anlaco.github.io/anvil/)— se conecta a él como si el banco
estuviera enchufado, sin saber que no lo está.

Este manual explica cómo usarlo: instalarlo, arrancar un banco, conectar tu
software y describir tus propios instrumentos.

> **Versión 0.1.0.** Escrito y comprobado contra **Crucible 0.1.0 en Linux
> x86-64**. Crucible es joven: el capítulo [Qué no hace
> (todavía)](11-limitaciones.md) dice sin rodeos dónde se queda corto.

## Por dónde empezar

- **Quiero verlo funcionar:** capítulos 2 a 4, en orden.
- **Quiero simular mi banco:** después, 5 a 9. El [9](09-receta.md) es la receta
  paso a paso.
- **Busco un campo o una opción:** [Referencia](10-referencia.md).
- **Algo no responde:** [Problemas frecuentes](12-problemas.md).

## Convenciones

Las órdenes empiezan por `$` y se ejecutan desde **la carpeta de tu banco**, una
copia del banco de ejemplo (el capítulo 5 explica cómo hacerla), con el banco
arrancado en otra terminal, salvo que el texto diga otra cosa. Las líneas que
siguen a una orden son lo que imprimió.

Los ejemplos de terminal usan `nc` (netcat) porque viene en cualquier Linux. En
Windows, el capítulo 4 da el equivalente en PowerShell.

## Cómo se mantiene cierto este manual

Nada de lo que imprime una orden está tecleado a mano. Cada sesión de terminal
es un fichero en
[`listings/sesiones/`](https://github.com/anlaco/Crucible/tree/main/docs/manual/listings/sesiones)
que el capítulo incluye tal cual, y
[`check.sh`](https://github.com/anlaco/Crucible/blob/main/docs/manual/check.sh)
las vuelve a ejecutar contra el binario y avisa si alguna ya no da lo mismo.
Los perfiles y el banco de ejemplo que ves en las páginas son los ficheros del
repositorio, no copias.

```console
$ docs/manual/check.sh target/debug/crucible
```

Lo que no se ha podido ejecutar aquí se marca como **no verificado**, con esas
palabras.

Si buscas cómo está hecho Crucible por dentro —el formato como estándar, las
decisiones de diseño— está en la
[documentación del repositorio](https://github.com/anlaco/Crucible/tree/main/docs).
