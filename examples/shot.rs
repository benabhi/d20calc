//! Arnés de pruebas visuales: renderiza la interfaz por software (sin ventana), simula
//! clics y guarda capturas PNG. Uso: `cargo run --example shot -- <carpeta_de_salida>`.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model, PhysicalSize, Rgb8Pixel};

thread_local! { static NOW: Cell<u64> = const { Cell::new(0) }; }

struct Headless(Rc<MinimalSoftwareWindow>);

impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
    fn duration_since_start(&self) -> Duration {
        Duration::from_millis(NOW.with(|n| n.get()))
    }
}

const W: u32 = 440; // 352 dp x 1,25
const H: u32 = 690; // 552 dp x 1,25
const DP: f32 = 1.25;

fn advance(ms: u64) {
    for _ in 0..(ms / 10).max(1) {
        NOW.with(|n| n.set(n.get() + 10));
        slint::platform::update_timers_and_animations();
    }
}

fn click(win: &MinimalSoftwareWindow, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    win.dispatch_event(WindowEvent::PointerMoved { position });
    win.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    advance(30);
    win.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    advance(30);
}

/// Pulsa una tecla del teclado por su columna y fila (0..3, 0..6).
fn key(win: &MinimalSoftwareWindow, col: f32, row: f32) {
    let cw = W as f32 / 4.0;
    let rh = 56.0 * DP;
    click(win, col * cw + cw / 2.0, 160.0 * DP + row * rh + rh / 2.0);
}

fn shot(win: &MinimalSoftwareWindow, path: &std::path::Path) {
    let mut buf = vec![Rgb8Pixel::default(); (W * H) as usize];
    win.request_redraw();
    win.draw_if_needed(|r| {
        r.render(&mut buf, W as usize);
    });
    let mut raw = Vec::with_capacity(buf.len() * 3);
    for p in &buf {
        raw.extend_from_slice(&[p.r, p.g, p.b]);
    }
    let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut enc = png::Encoder::new(file, W, H);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&raw).unwrap();
}

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("falta la carpeta de salida"));
    std::fs::create_dir_all(&out).unwrap();
    // Datos de prueba aparte, para no tocar los reales.
    unsafe { std::env::set_var("D20CALC_DATA", out.join("datos_prueba.json")) };
    let _ = std::fs::remove_file(out.join("datos_prueba.json"));

    let win = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(win.clone()))).unwrap();
    win.set_size(PhysicalSize::new(W, H));

    let ui = d20calc::build_app(false).unwrap();
    ui.show().unwrap();
    advance(50);
    shot(&win, &out.join("01_inicio.png"));

    // 1d20 + 5  ->  d20 (col 2, fila 2), «+» (col 3, fila 4), «5» (col 1, fila 4)
    key(&win, 2.0, 2.0);
    key(&win, 3.0, 4.0);
    key(&win, 1.0, 4.0);
    println!("formula = {:?}  clear = {:?}", ui.get_formula(), ui.get_clear_label());
    shot(&win, &out.join("02_formula.png"));

    key(&win, 3.0, 5.0); // ROLL
    advance(120);
    println!("animando: result = {:?} detail = {:?}", ui.get_result(), ui.get_detail());
    shot(&win, &out.join("03_animando.png"));
    advance(600);
    println!("final: result = {:?} detail = {:?} half = {:?} max = {:?}", ui.get_result(), ui.get_detail(), ui.get_half(), ui.get_max());
    shot(&win, &out.join("04_resultado.png"));

    // botón de intercambio: HALF / MAX
    click(&win, 36.0 * DP, 64.0 * DP + 48.0 * DP);
    println!("halfmax: show_detail={} show_halfmax={}", ui.get_show_detail(), ui.get_show_halfmax());
    shot(&win, &out.join("05_halfmax.png"));

    // otra tirada para llenar «Recent», y guardar la fórmula
    key(&win, 0.0, 0.0); // d%
    key(&win, 3.0, 5.0);
    advance(700);
    shot(&win, &out.join("06_d100.png"));

    // menú: toque en el borde izquierdo
    click(&win, 28.0 * DP, 32.0 * DP); // botón de menú
    advance(400);
    shot(&win, &out.join("07_menu.png"));
    println!("recientes = {}", ui.get_recent().row_count());

    // cerrar el menú y abrir el diálogo de guardar
    click(&win, 400.0, 300.0);
    advance(400);
    click(&win, 72.0 * DP, 32.0 * DP);
    advance(100);
    shot(&win, &out.join("08_guardar.png"));

    // temas
    for (i, name) in ["default", "light", "dark", "dungeon", "forest", "blood", "ice", "fire"].iter().enumerate() {
        ui.invoke_theme_chosen(i as i32);
        advance(50);
        shot(&win, &out.join(format!("tema_{i}_{name}.png")));
    }

    // ---- comprobaciones funcionales (teclado, guardar, borrar, ajustes)
    click(&win, 10.0, 10.0); // cierra el diálogo de guardar que dejó abierto el tramo anterior
    advance(100);
    ui.invoke_theme_chosen(0);
    advance(50);
    let text = |t: &str| {
        win.dispatch_event(WindowEvent::KeyPressed { text: t.into() });
        win.dispatch_event(WindowEvent::KeyReleased { text: t.into() });
        advance(20);
    };
    use slint::platform::Key;
    let k = |k: Key| -> slint::SharedString { k.into() };
    text(&k(Key::Delete)); // AC: borra la fórmula
    text(&k(Key::Delete)); // AC otra vez: borra los resultados
    for c in ["5", "+", "3"] {
        text(c);
    }
    println!("teclado: formula={:?}", ui.get_formula());
    text(&k(Key::Return));
    advance(600);
    println!("teclado: result={:?} detail={:?}", ui.get_result(), ui.get_detail());
    text(&k(Key::Backspace));
    println!("teclado Retroceso: formula={:?}", ui.get_formula());
    // notación de dados tecleada: 2 d 6 + d % -> 2d6 + 1d100
    text(&k(Key::Delete));
    for c in ["2", "d", "6", "+", "d", "%"] {
        text(c);
    }
    println!("notacion tecleada (2 d 6 + d %): formula={:?}", ui.get_formula());
    text(&k(Key::Delete));
    for c in ["5", "+", "3"] {
        text(c);
    }

    // guardar: icono de guardar, escribir el nombre y Enter
    click(&win, 72.0 * DP, 32.0 * DP);
    advance(100);
    for c in ["A", "t", "q"] {
        text(c);
    }
    text(&k(Key::Return));
    advance(400);
    println!("guardadas = {}  menu abierto (ver captura 09)", ui.get_saved().row_count());
    if let Some(row) = ui.get_saved().row_data(0) {
        println!("  guardada: nombre={:?} formula={:?}", row.name, row.formula);
    }
    shot(&win, &out.join("09_guardada.png"));

    // tocar la guardada la tira y cierra el menú
    click(&win, 100.0 * DP, 72.0 * DP);
    advance(700);
    println!("tocar guardada: formula={:?} result={:?}", ui.get_formula(), ui.get_result());

    // volver a abrir el menú (tecla M) y borrar deslizando
    text("m");
    advance(400);
    let (x0, y0) = (150.0 * DP, 72.0 * DP);
    win.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x0, y0) });
    win.dispatch_event(WindowEvent::PointerPressed { position: LogicalPosition::new(x0, y0), button: PointerEventButton::Left });
    for dx in [-40.0, -90.0, -150.0] {
        win.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x0 + dx, y0) });
        advance(20);
    }
    win.dispatch_event(WindowEvent::PointerReleased { position: LogicalPosition::new(x0 - 150.0, y0), button: PointerEventButton::Left });
    advance(100);
    shot(&win, &out.join("10_borrar.png"));
    // confirmar «DELETE»: botón de la derecha del diálogo
    click(&win, 333.0, 376.0);
    advance(100);
    println!("tras confirmar borrado: guardadas = {}", ui.get_saved().row_count());
    shot(&win, &out.join("11_tras_borrar.png"));

    // ajustes: engranaje del menú
    click(&win, 246.0 * DP, 528.0 * DP);
    advance(100);
    shot(&win, &out.join("12_ajustes.png"));
    click(&win, 200.0 * DP, 132.0 * DP); // sonido
    println!("sfx tras tocar = {}", ui.get_sfx_on());
    click(&win, 200.0 * DP, 244.0 * DP); // tema
    advance(100);
    shot(&win, &out.join("13_selector_tema.png"));
    click(&win, 150.0 * DP, 438.0 * DP); // última opción: Fire
    advance(100);
    println!("tema elegido = {}", ui.get_theme_index());
    shot(&win, &out.join("14_tema_elegido.png"));
}
