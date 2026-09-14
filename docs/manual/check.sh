#!/bin/bash
# Ejecuta cada orden que muestra el manual y compara lo que sale con lo que el
# manual dice que sale.
#
# Un manual solo vale mientras es cierto, y deja de serlo la primera vez que
# el runtime cambia un mensaje. Por eso ningún capítulo lleva salidas escritas
# a mano: cada sesión de terminal es un fichero en listings/sesiones/ que el
# capítulo incluye con {{#include}}, y este script vuelve a ejecutarlas todas.
# Los ficheros que el lector escribe también se incluyen desde listings/, y los
# del banco de ejemplo desde banco/, así que tampoco pueden desfasarse.
#
# Las sesiones son las mismas para los dos idiomas (es/ y en/): Crucible imprime
# lo mismo lo lea quien lo lea.
#
# Uso (desde cualquier sitio):
#   docs/manual/check.sh [binario crucible]            # comparar
#   docs/manual/check.sh --record [binario crucible]   # reescribir las sesiones
#
# Sin binario usa target/debug/crucible. La sesión de PyVISA necesita PYTHON
# apuntando a un intérprete con pyvisa y pyvisa-py; sin él sale SKIP, nunca OK.
#
# --record es para escribir una sesión nueva, no para poner verde una roja: lee
# el diff de lo que ha reescrito antes de hacer commit.

set -u

RECORD=0
if [ "${1:-}" = "--record" ]; then RECORD=1; shift; fi

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
MANUAL=$ROOT/docs/manual
L=$MANUAL/listings
BIN=${1:-$ROOT/target/debug/crucible}
if [ ! -x "$BIN" ]; then
  echo "no encuentro el binario $BIN; compila con 'cargo build -p crucible' o pásalo" >&2
  exit 2
fi
BIN=$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")

for port in 5025 5026 5027; do
  if ss -ltn | grep -q ":$port "; then
    echo "el puerto $port está ocupado. Un Crucible olvidado contesta con su propio estado;" >&2
    echo "búscalo con: ss -ltnp | grep :$port" >&2
    exit 2
  fi
done

WORK=$(mktemp -d)
BANCO_PID=""
cleanup() {
  [ -n "$BANCO_PID" ] && kill "$BANCO_PID" 2>/dev/null
  rm -rf "$WORK"
}
trap cleanup EXIT

# El equipo del lector, como lo deja el paquete .deb: el binario en /usr/bin y
# el banco de ejemplo en /usr/share. Así los mensajes que citan rutas de
# instalación salen iguales que en una instalación de verdad, quitando $WORK.
mkdir -p "$WORK/usr/bin" "$WORK/usr/share/crucible" "$WORK/home"
cp "$BIN" "$WORK/usr/bin/crucible"
# Su carpeta de trabajo es una copia del banco de ejemplo (capítulo 5), más los
# ficheros que el manual le pide escribir.
cp -r "$ROOT/banco" "$WORK/home/mi_banco"
cp -r "$L/conectar.py" "$L/puerto_repetido.yaml" "$L/errata.yaml" "$L/perfiles" "$WORK/home/mi_banco/"
export PATH="$WORK/usr/bin:$PATH"
CASA=$WORK/home/mi_banco

ok=0; fail=0; skip=0
say() { # say <OK|FAIL|SKIP> <qué>
  case $1 in
    OK)   printf '  \033[32mOK  \033[0m %s\n' "$2"; ok=$((ok + 1)) ;;
    FAIL) printf '  \033[31mFAIL\033[0m %s\n' "$2"; fail=$((fail + 1)) ;;
    SKIP) printf '  \033[33mSKIP\033[0m %s\n' "$2"; skip=$((skip + 1)) ;;
  esac
}

# Cada sesión arranca contra un banco recién encendido: el estado es compartido
# entre conexiones y el ruido se repite desde el arranque, así que una sesión
# que heredara el banco de otra dependería del orden en que se ejecutan.
banco_arriba() {
  (cd "$CASA" && exec crucible >"$WORK/banco.log" 2>&1 </dev/null) &
  BANCO_PID=$!
  for _ in $(seq 40); do
    ss -ltn | grep -q ':5027 ' && return 0
    sleep 0.1
  done
  echo "el banco de ejemplo no arranca:" >&2
  cat "$WORK/banco.log" >&2
  exit 1
}

banco_abajo() {
  [ -z "$BANCO_PID" ] && return 0
  kill "$BANCO_PID" 2>/dev/null
  wait "$BANCO_PID" 2>/dev/null
  BANCO_PID=""
  for _ in $(seq 40); do
    ss -ltn | grep -q ':502[5-7] ' || return 0
    sleep 0.1
  done
}

# Una sesión es lo que enseña un terminal: líneas `$ orden`, cada una seguida
# de lo que imprimió. Ejecutarla es correr sus órdenes en orden en una misma
# shell, en la carpeta del lector, e imprimir la misma transcripción.
run_session() { # run_session <fichero>
  local script=$WORK/sesion.sh cmd
  : >"$script"
  while IFS= read -r line; do
    case $line in
      '$ '*)
        cmd=${line#'$ '}
        printf 'printf "%%s\\n" %q\n' "$line" >>"$script"
        case $cmd in
          # El banco ya está arrancado; lo que ve el lector es su cabecera.
          crucible) [ -n "$BANCO_PID" ] && cmd="sed '/^Listo/q' '$WORK/banco.log'" ;;
          'python3 '*) cmd="\"\$PYTHON\" ${cmd#python3 }" ;;
        esac
        printf '{ %s ; } 2>&1 </dev/null\n' "$cmd" >>"$script"
        ;;
    esac
  done <"$1"
  (cd "$CASA" && bash "$script") | sed "s#$WORK##g"
}

echo "$("$BIN" --version) desde $BIN"
echo "sesiones:"
for session in "$L"/sesiones/*.txt; do
  name=$(basename "$session" .txt)
  case $name in
    *pyvisa*)
      if [ -z "${PYTHON:-}" ] || ! "$PYTHON" -c 'import pyvisa, pyvisa_py' 2>/dev/null; then
        say SKIP "$name (falta PYTHON con pyvisa y pyvisa-py)"
        continue
      fi
      ;;
  esac
  case $name in
    # Las que enseñan qué pasa sin banco, o sin nada, lo necesitan apagado.
    *sin-banco* | 02-* | 05-* | 06-comun-* | 07-formula-* | 10-*) ;;
    *) banco_arriba ;;
  esac
  actual=$(run_session "$session")
  banco_abajo
  if [ "$RECORD" = 1 ]; then
    printf '%s\n' "$actual" >"$session"
    say OK "$name (grabada)"
  elif diff <(printf '%s\n' "$actual") "$session" >"$WORK/diff"; then
    say OK "$name"
  else
    say FAIL "$name"
    sed 's/^/      /' "$WORK/diff"
  fi
done

# Una sesión que un idioma no incluye no comprueba nada que ese lector vea, y
# suele querer decir que una traducción se ha quedado atrás.
for idioma in es en; do
  echo "sesiones incluidas en algún capítulo ($idioma):"
  for session in "$L"/sesiones/*.txt; do
    rel=../listings/sesiones/$(basename "$session")
    if grep -q "{{#include $rel}}" "$MANUAL/$idioma"/[0-9][0-9]-*.md; then
      say OK "$(basename "$session")"
    else
      say FAIL "$(basename "$session") no lo incluye ningún capítulo en $idioma"
    fi
  done
done

# El selector de idioma empareja capítulos por posición con la tabla de
# idioma.js; si un fichero cambia de nombre sin tocarla, el botón lleva a un 404.
echo "tabla de capítulos de idioma.js:"
for idioma in es en; do
  en_tabla=$(python3 - "$MANUAL/idioma.js" "$idioma" <<'PY'
import re, sys
js = open(sys.argv[1]).read()
lista = re.search(sys.argv[2] + r':\s*\[(.*?)\]', js, re.S).group(1)
print(" ".join(re.findall(r'"([^"]+)"', lista)))
PY
)
  en_disco=$(cd "$MANUAL/$idioma" && ls [0-9][0-9]-*.md | sed 's/\.md$//' | tr '\n' ' ' | sed 's/ $//')
  if [ "$en_tabla" = "$en_disco" ]; then
    say OK "$idioma"
  else
    say FAIL "$idioma: idioma.js dice '$en_tabla' y hay '$en_disco'"
  fi
done

echo "$ok OK, $fail FAIL, $skip SKIP"
[ "$fail" -eq 0 ]
