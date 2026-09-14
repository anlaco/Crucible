# Modelos: lo que el instrumento mide

Una consulta con `respuesta: "{tension}"` devuelve lo que se configuró. Una
medida es otra cosa: la corriente que da una fuente depende de la tensión, de la
carga y de si la salida está encendida. Eso es un **modelo**.

```yaml
modelos:
  corriente_medida:
    tipo: formula
    cuando: { salida: true }
    expr: "min(tension / carga_ohm, limite_corriente) + gauss(0, 1e-5)"
    fallback: 0.0
    formato: fijo:5
```

Y la consulta que lo usa:

```yaml
  - patron: "MEASure[:SCALar]:CURRent[:DC]"
    query: true
    modelo: corriente_medida
```

## Campos

| Campo | Qué es |
|---|---|
| `tipo` | Siempre `formula`. |
| `expr` | La fórmula. |
| `cuando` | Condiciones sobre el estado. Si alguna no se cumple, se contesta `fallback`. Si hay varias, deben cumplirse todas. |
| `fallback` | La respuesta cuando no se cumple `cuando`. Por defecto `0.0`. Número o texto. |
| `formato` | Cómo se escribe el número. Opcional. |

## Qué cabe en `expr`

- Variables del estado, por su nombre: `tension`, `carga_ohm`.
- Números: `10`, `0.5`, `1e-3`, `2.5E+2`.
- `+ - * /` con la precedencia de siempre, paréntesis y signo negativo.
- Funciones:

| Función | Resultado |
|---|---|
| `gauss(media, sigma)` | Ruido gaussiano. Cambia en cada lectura, pero la serie se repite igual cada vez que arrancas Crucible, así que las pruebas automáticas son reproducibles. |
| `abs(x)` | Valor absoluto. |
| `sqrt(x)` | Raíz cuadrada. |
| `min(a, b)`, `max(a, b)` | Mínimo y máximo. |

No hay condicionales dentro de la fórmula: `cuando` solo elige entre la fórmula
y un `fallback` fijo.

## El modelo de la fuente, en marcha

Contra el banco recién arrancado:

```console
{{#include listings/sesiones/07-modelos.txt}}
```

Paso a paso:

1. Con la salida apagada no se cumple `cuando`, y la medida es el `fallback`,
   escrito con el formato del modelo: `0.00000`.
2. `OUTP?` contesta `0`, no `false`: lo contesta el modelo
   `salida_como_numero`, con `formato: entero`, como un instrumento real.
3. A 5 V sobre 10 Ω, medio amperio más ruido.
4. A 20 V serían 2 A, pero `min` recorta al límite de 1 A: la fuente entra en
   corriente constante.
5. El multímetro escribe en notación científica, como la mayoría.

## `formato`

Tu software espera los números como los escribe el instrumento real:

| `formato` | Respuesta de ejemplo | Uso típico |
|---|---|---|
| *(sin poner)* | `1.0`, `4.501385029307777` | — |
| `entero` | `1`, `0`, `-3` | Estados (`OUTP?`) y contadores. |
| `fijo:N` | `fijo:4` → `4.5014` | Fuentes que contestan con decimales fijos. |
| `cientifico:N` | `cientifico:6` → `+4.501385E+00` | Multímetros y la mayoría de medidas SCPI. |

El formato también se aplica al `fallback` si es numérico.

Las consultas con `respuesta: "{variable}"` devuelven el valor tal cual
(`5.0`). Si tu software espera otra forma, usa un modelo cuya `expr` sea la
variable, con el `formato` que toque.

## Errores en las fórmulas

Los de sintaxis —un paréntesis sin cerrar, una función que no existe— salen al
validar o al arrancar, con la posición:

```yaml
{{#include listings/perfiles/formula_rota.yaml}}
```

```console
{{#include listings/sesiones/07-formula-rota.txt}}
```

Una **variable que no existe** en el estado, en cambio, solo se detecta al
hacer la consulta: no llega respuesta y `SYST:ERR?` devuelve
`-200,"Execution error;…la variable '…' no existe en el estado…"`. Declara en
`estado` todas las variables que usen tus fórmulas.
