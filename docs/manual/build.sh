#!/bin/bash
# Construye el manual completo en target/manual/: un libro de mdBook por idioma
# (es/, en/) y, en la raíz, la portada que elige idioma y la página 404.
#
# mdBook no sabe de idiomas; el selector de la barra lo añade idioma.js, que
# los dos libros cargan desde la raíz.
#
#   docs/manual/build.sh                         # usa mdbook del PATH
#   MDBOOK=/ruta/a/mdbook docs/manual/build.sh
#
# Para verlo con el selector funcionando, sírvelo desde target/ para que las
# rutas empiecen por /Crucible/ como en GitHub Pages:
#   ln -sfn manual target/Crucible && python3 -m http.server -d target
#   → http://localhost:8000/Crucible/

set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
MDBOOK=${MDBOOK:-mdbook}

rm -rf "$ROOT/target/manual"
for idioma in es en; do
  "$MDBOOK" build "$ROOT/docs/manual/$idioma"
done
cp "$ROOT/docs/manual/index.html" "$ROOT/docs/manual/404.html" "$ROOT/target/manual/"
