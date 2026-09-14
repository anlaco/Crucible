# Instalación

Los instaladores están en la página de
[versiones de GitHub](https://github.com/anlaco/Crucible/releases).

## Linux (Debian, Ubuntu y derivadas)

```console
$ sudo apt install ./crucible_0.1.0_amd64.deb
```

Deja:

- el comando `crucible` en `/usr/bin`;
- el banco de ejemplo en `/usr/share/crucible/banco`, de solo lectura (el
  capítulo 5 explica cómo copiarlo para editarlo);

Comprueba la instalación:

```console
{{#include listings/sesiones/02-version.txt}}
```

## Windows

1. Ejecuta `crucible-0.1.0-setup.exe`. No pide permisos de administrador.
2. El instalador deja:
   - En el **menú Inicio → Crucible**: *Arrancar banco*, *Abrir carpeta del
     banco*, *Terminal de Crucible* (una consola con la ayuda del comando) y
     *Manual* (abre esta web).
   - El comando `crucible` en el `PATH`, para cualquier terminal nueva (cmd o
     PowerShell).
   - El banco de ejemplo en `%LOCALAPPDATA%\Programs\Crucible\banco`, que
     puedes editar ahí mismo.

Al actualizar a una versión nueva **no se sobrescriben** los ficheros del banco
que ya existan: tus cambios se conservan.

## Desde el código fuente

Con Rust 1.90 o posterior:

```console
$ git clone https://github.com/anlaco/Crucible
$ cd Crucible
$ cargo build --release -p crucible
```

El binario queda en `target/release/crucible` y el banco de ejemplo en
`banco/`. Sustituye `crucible` por esa ruta en el resto del manual, o añádela
al `PATH`.
