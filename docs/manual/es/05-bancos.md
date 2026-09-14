# Crear tu banco

Un banco es una carpeta con esta forma:

```text
mi_banco/
├── banco.yaml             ← qué instrumentos hay y en qué puerto
└── perfiles/
    ├── fuente_xyz.yaml    ← un perfil por MODELO de instrumento
    └── dmm_abc.yaml
```

## Empezar copiando el de ejemplo

Lo más rápido es partir del banco de ejemplo y cambiarlo:

- **Linux:**

  ```console
  $ cp -r /usr/share/crucible/banco ~/mi_banco
  $ cd ~/mi_banco
  ```

- **Windows:** menú Inicio → Crucible → *Abrir carpeta del banco*, y edita ahí
  o copia la carpeta a otro sitio.

Desde esa carpeta, `crucible` sin argumentos arranca tu banco: es el primer
sitio donde busca.

## `banco.yaml`

Este es el del ejemplo:

```yaml
{{#include ../../../banco/banco.yaml}}
```

Cada entrada de `dispositivos` es **una unidad**:

| Campo | Qué es |
|---|---|
| `id` | Un nombre para ti. Sale en la tabla del arranque y en los errores. |
| `perfil` | El perfil del instrumento, con la ruta **relativa a `banco.yaml`**. |
| `transporte` | Por dónde escucha: `{ tipo: tcp, puerto: 5025 }`. Hoy `tipo` solo puede ser `tcp`. |
| `estado_inicial` | Opcional. Valores que sustituyen a los de `estado` del perfil solo para esta unidad. |

Dos unidades del mismo modelo **comparten perfil** y cambian de puerto, y si
hace falta, de `estado_inicial`:

```yaml
    - id: dmm_entrada
      perfil: perfiles/dmm_abc.yaml
      transporte: { tipo: tcp, puerto: 5026 }

    - id: dmm_salida
      perfil: perfiles/dmm_abc.yaml
      transporte: { tipo: tcp, puerto: 5027 }
      estado_inicial:
        tension_entrada: 12.0
```

## Validar antes de arrancar

```console
{{#include ../listings/sesiones/05-validar.txt}}
```

`--validar` carga el banco y todos sus perfiles, comprueba patrones, fórmulas y
puertos, y enseña la tabla de recursos **sin abrir ningún puerto**, así que
puedes usarlo con el banco arrancado. Hazlo cada vez que edites un YAML: los
errores dicen qué fichero, qué comando y qué está mal.

Cada instrumento necesita su puerto. Con este banco, que repite el 5025:

```yaml
{{#include ../listings/puerto_repetido.yaml}}
```

```console
{{#include ../listings/sesiones/05-puerto-repetido.txt}}
```

## Tres reglas que conviene conocer

**Si un perfil tiene un error, el banco entero no arranca.** Es a propósito: un
banco al que le falta un instrumento daría resultados falsos sin que nadie se
enterase.

**`*RST` devuelve la unidad a los valores del perfil, no a los de
`estado_inicial`.** El multímetro del ejemplo arranca con 3,3 V en los bornes,
pero tras un `*RST` mide 0 V (lo verás en el capítulo 8). Si tu software manda
`*RST` al empezar, fija el valor en el perfil o vuelve a fijarlo por SCPI
después.

**Una variable mal escrita en `estado_inicial` no da error.** Aquí
`tension_entarda` no existe en el perfil del multímetro, y aun así:

```yaml
{{#include ../listings/errata.yaml}}
```

```console
{{#include ../listings/sesiones/05-errata.txt}}
```

El multímetro arrancará midiendo 0 V. Si una unidad no arranca con el valor que
esperabas, revisa primero cómo has escrito el nombre.
