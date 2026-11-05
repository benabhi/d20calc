# d20 Calculator

Calculadora de dados de rol hecha en Rust: fórmulas como `2d12 + 8`, resultado con detalle de cada dado, mitad y máximo,
tiradas guardadas y recientes, 8 temas y efectos de sonido. Interfaz con [Slint](https://slint.dev) (render por software, sin GPU).

## Portable
`d20calc.exe` es un único archivo: imágenes, fuentes y sonidos van incrustados, el runtime de C++ está enlazado de forma
estática y solo usa DLLs del propio Windows (10 u 11). Se copia a cualquier carpeta o USB y se ejecuta.
Los ajustes y las tiradas guardadas van en `d20calc.json`, junto al `.exe` (si esa carpeta no es escribible, en
`%LOCALAPPDATA%\d20calc`). La ventana tiene tamaño fijo (352×552 dp × 1,25 = 440×690 px) y no se puede redimensionar.

## Uso
- Teclado: `0-9`, `+ - * /`, `( )`, `Enter` (tirar), `Retroceso` (del), `Supr` (AC), `M` (menú), `Esc`.
- Notación de dados tecleada: `2` `d` `6` → `2d6`, `d` `%` → `1d100`, `2` `d` `%` → `2d100`, `d` `2` `0` → `1d20`.
  La ventana toma el foco al abrirse, así que se puede empezar a teclear sin hacer clic.
- Menú «Saved / Recent»: botón de las tres rayas (arriba a la izquierda) o tecla `M`. Se desliza por encima de la calculadora; se cierra tocando fuera, con `Esc`. Arrastra una guardada a un lado para borrarla.
- Ajustes (engranaje del menú): efectos de sonido y tema.

## Compilar
```
cargo build --release      # target\release\d20calc.exe
cargo test                 # motor de fórmulas
cargo run --example shot -- <carpeta>   # capturas PNG sin ventana (prueba visual)
```
`ui/app.slint` es la interfaz; `src/formula.rs`, el motor; `src/themes.rs`, los colores de los 8 temas.
Cambiar `Theme.dp` en `ui/app.slint` escala toda la ventana.

## Licencias
La fuente Roboto (Apache 2.0) va incrustada.
