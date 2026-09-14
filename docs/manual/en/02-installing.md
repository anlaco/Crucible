# Installing

The installers are on the GitHub
[releases page](https://github.com/anlaco/Crucible/releases).

## Linux (Debian, Ubuntu and derivatives)

```console
$ sudo apt install ./crucible_0.1.0_amd64.deb
```

It leaves:

- the `crucible` command in `/usr/bin`;
- the example bench in `/usr/share/crucible/banco`, read-only (chapter 5 shows
  how to copy it to edit it).

Check the installation:

```console
{{#include ../listings/sesiones/02-version.txt}}
```

## Windows

1. Run `crucible-0.1.0-setup.exe`. It does not ask for administrator rights.
2. The installer leaves:
   - In the **Start menu → Crucible**: *Arrancar banco* (start bench), *Abrir
     carpeta del banco* (open the bench folder), *Terminal de Crucible* (a
     console showing the command's help) and *Manual* (opens this website).
   - The `crucible` command on the `PATH`, for any new terminal (cmd or
     PowerShell).
   - The example bench in `%LOCALAPPDATA%\Programs\Crucible\banco`, which you
     can edit right there.

Upgrading to a new version **does not overwrite** bench files that already
exist: your changes are kept.

## From source

With Rust 1.90 or later:

```console
$ git clone https://github.com/anlaco/Crucible
$ cd Crucible
$ cargo build --release -p crucible
```

The binary ends up in `target/release/crucible` and the example bench in
`banco/`. Replace `crucible` with that path in the rest of the manual, or add it
to your `PATH`.
