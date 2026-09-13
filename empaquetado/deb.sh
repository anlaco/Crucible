#!/usr/bin/env bash
# Construye el paquete .deb de Crucible.
#
# El binario es estático (musl): así el mismo .deb instala en Ubuntu 20.04 y en
# Debian 13 sin depender de la glibc de la máquina que compiló.
#
# Uso: empaquetado/deb.sh            → dist/crucible_<versión>_amd64.deb
set -euo pipefail

raiz="$(cd "$(dirname "$0")/.." && pwd)"
cd "$raiz"

objetivo=x86_64-unknown-linux-musl
version="$(cargo metadata --no-deps --format-version 1 \
  | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="crucible"))')"

cargo build --release --locked -p crucible --target "$objetivo"

paquete="$(mktemp -d)"
trap 'rm -rf "$paquete"' EXIT

install -Dm755 "target/$objetivo/release/crucible" "$paquete/usr/bin/crucible"
# El banco va a /usr/share, de solo lectura: es el que arranca `crucible` sin
# argumentos y la plantilla que el usuario copia para hacer el suyo.
mkdir -p "$paquete/usr/share/crucible"
cp -r banco "$paquete/usr/share/crucible/banco"
install -Dm644 banco/MANUAL.md "$paquete/usr/share/doc/crucible/MANUAL.md"
install -Dm644 LICENSE "$paquete/usr/share/doc/crucible/copyright"

# mktemp crea la raíz con 0700 y el umask puede dejar escritura de grupo:
# dpkg aplicaría esos permisos a / y a /usr/share.
chmod -R u=rwX,go=rX "$paquete"
mkdir -p "$paquete/DEBIAN"
cat > "$paquete/DEBIAN/control" <<CONTROL
Package: crucible
Version: $version
Section: electronics
Priority: optional
Architecture: amd64
Maintainer: ANLACO <anlaco@proton.me>
Homepage: https://github.com/anlaco/Crucible
Description: simulador de bancos de instrumentos SCPI por red
 Describe cada instrumento en un YAML y Crucible lo sirve por TCP con SCPI
 real, para desarrollar y probar software de medida sin el banco físico.
CONTROL

mkdir -p dist
salida="dist/crucible_${version}_amd64.deb"
dpkg-deb --build --root-owner-group "$paquete" "$salida" >/dev/null
echo "$salida"
