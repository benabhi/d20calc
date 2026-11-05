pub mod formula;
pub mod storage;
pub mod themes;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use formula::Rng;
use storage::{Saved, Stored};

slint::include_modules!();
pub use slint::ComponentHandle as _;

const PRESS: &[u8] = include_bytes!("../assets/sounds/press.mp3");
const ROLLS: [&[u8]; 4] = [
    include_bytes!("../assets/sounds/roll_1.mp3"),
    include_bytes!("../assets/sounds/roll_2.mp3"),
    include_bytes!("../assets/sounds/roll_3.mp3"),
    include_bytes!("../assets/sounds/roll_4.mp3"),
];
/// Como la app original: máximo de tiradas en «Recent».
const MAX_RECENT: usize = 20;

/// Salida de audio; si no hay dispositivo, la app sigue funcionando sin sonido.
struct Audio {
    sink: Option<rodio::stream::MixerDeviceSink>,
    enabled: bool,
}

impl Audio {
    fn new(enabled: bool) -> Self {
        Audio { sink: rodio::DeviceSinkBuilder::open_default_sink().ok(), enabled }
    }

    fn play(&self, data: &'static [u8]) {
        if !self.enabled {
            return;
        }
        if let Some(sink) = &self.sink {
            if let Ok(player) = rodio::play(sink.mixer(), std::io::Cursor::new(data)) {
                player.detach();
            }
        }
    }
}

struct Calc {
    ui: slint::Weak<App>,
    formula: String,
    result: String,
    half: String,
    max: String,
    details: String,
    dimmed: bool,
    /// `true`: se ve «HALF/MAX»; `false`: se ve el detalle (estado inicial de la app original).
    show_halfmax: bool,
    /// Los números se ocultan mientras dura la animación de la tirada.
    results_visible: bool,
    roll_timer: f32,
    saved: Vec<Saved>,
    recent: Vec<String>,
    theme: usize,
    rng: Rng,
    audio: Audio,
    saved_model: Rc<VecModel<SavedItem>>,
    recent_model: Rc<VecModel<SharedString>>,
}

type Shared = Rc<RefCell<Calc>>;

impl Calc {
    fn persist(&self) {
        storage::save(&Stored { theme: self.theme, sfx: self.audio.enabled, saved: self.saved.clone() });
    }

    /// Vuelca el estado a la interfaz.
    fn refresh(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let visible = self.results_visible && !self.result.is_empty();
        ui.set_formula(formula::display(&self.formula).into());
        ui.set_result(self.result.clone().into());
        ui.set_half(self.half.clone().into());
        ui.set_max(self.max.clone().into());
        ui.set_detail(self.details.clone().into());
        ui.set_show_detail(visible && !self.show_halfmax);
        ui.set_show_halfmax(visible && self.show_halfmax);
        ui.set_dimmed(self.dimmed);
        ui.set_clear_label(if self.formula.is_empty() { "AC" } else { "C" }.into());
    }

    fn refresh_lists(&self) {
        self.saved_model.set_vec(
            self.saved
                .iter()
                .map(|s| SavedItem { name: s.name.clone().into(), formula: formula::display(&s.formula).into() })
                .collect::<Vec<_>>(),
        );
        self.recent_model
            .set_vec(self.recent.iter().map(|r| SharedString::from(formula::display(r))).collect::<Vec<_>>());
    }

    fn apply_theme(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let p = &themes::PALETTES[self.theme.min(themes::PALETTES.len() - 1)];
        let c = |v: u32| slint::Color::from_argb_encoded(v);
        ui.global::<Theme>().set_p(Pal {
            clear_bg: c(p.clear_bg), clear_fg: c(p.clear_fg),
            dice_bg: c(p.dice_bg), dice_fg: c(p.dice_fg),
            modi_bg: c(p.modi_bg), modi_fg: c(p.modi_fg),
            number_bg: c(p.number_bg), number_fg: c(p.number_fg),
            operator_bg: c(p.operator_bg), operator_fg: c(p.operator_fg),
            roll_bg: c(p.roll_bg), roll_fg: c(p.roll_fg),
            header_bg: c(p.header_bg), header_fg: c(p.header_fg),
            input_bg: c(p.input_bg), input_fg: c(p.input_fg),
            menu_bg: c(p.menu_bg), menu_fg: c(p.menu_fg),
            result_bg: c(p.result_bg), result_fg: c(p.result_fg),
        });
        ui.set_theme_index(self.theme as i32);
    }

    fn add_recent(&mut self) {
        if self.formula.is_empty() || self.recent.first() == Some(&self.formula) {
            return;
        }
        self.recent.insert(0, self.formula.clone());
        if self.recent.len() >= MAX_RECENT {
            self.recent.truncate(MAX_RECENT - 1);
        }
        self.refresh_lists();
    }
}

/// Tira la fórmula actual: con dados, anima el resultado ~350 ms con sonido; sin dados, es inmediata.
fn start_roll(c: &mut Calc, shared: &Shared) {
    if formula::has_roll(&c.formula) {
        c.roll_timer = 0.0;
        let n = (c.rng.next_u64() % ROLLS.len() as u64) as usize;
        c.audio.play(ROLLS[n]);
    } else {
        c.roll_timer = 100.0;
        c.audio.play(PRESS);
    }
    execute_roll(c, shared);
}

fn execute_roll(c: &mut Calc, shared: &Shared) {
    let Ok(roll) = formula::roll(&c.formula, &mut c.rng) else {
        return; // fórmula incompleta o inválida: no cambia nada
    };
    c.result = formula::format_number(roll.result);
    c.half.clear();
    c.max.clear();
    c.details.clear();
    c.dimmed = false;
    c.results_visible = false;
    if c.roll_timer > 0.3 {
        c.half = formula::format_number(roll.half);
        c.max = formula::format_number(roll.max);
        c.details = format!("{} = ", formula::display(&roll.details));
        c.results_visible = true;
    } else {
        let again = shared.clone();
        slint::Timer::single_shot(Duration::from_millis(50), move || {
            let mut c = again.borrow_mut();
            execute_roll(&mut c, &again);
        });
        c.roll_timer += 0.05;
    }
    c.refresh();
}

/// Atiende una pulsación de la calculadora (botón o teclado).
fn on_key(shared: &Shared, key: &str) {
    let mut c = shared.borrow_mut();
    match key {
        "roll" => {
            if !c.formula.is_empty() {
                start_roll(&mut c, shared);
                c.add_recent();
            }
        }
        "del" => {
            if !c.formula.is_empty() {
                c.formula = formula::delete_last(&c.formula);
            }
            c.audio.play(PRESS);
        }
        "ac" => {
            if !c.formula.is_empty() {
                c.formula.clear();
                c.dimmed = true;
            } else {
                c.result.clear();
                c.half.clear();
                c.max.clear();
                c.details.clear();
            }
            c.audio.play(PRESS);
        }
        _ => {
            c.formula = formula::press(&c.formula, key);
            c.dimmed = true;
            c.audio.play(PRESS);
        }
    }
    c.refresh();
}

/// Carga una fórmula (de «Saved» o «Recent») y la tira, sin añadirla a «Recent».
fn roll_formula(shared: &Shared, f: String) {
    let mut c = shared.borrow_mut();
    c.formula = f;
    start_roll(&mut c, shared);
    c.refresh();
}

/// Abre la ventana: render por software, sin exigir GPU ni controladores gráficos concretos.
pub fn run() -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let ui = build_app(true)?;
    ui.show()?;
    // La ventana toma el foco al abrir, para poder teclear sin hacer clic antes.
    ui.window().with_winit_window(|w| w.focus_window());
    ui.run()
}

/// Crea la interfaz con toda la lógica conectada (`sound = false` la deja muda, para las pruebas).
pub fn build_app(sound: bool) -> Result<App, slint::PlatformError> {
    let ui = App::new()?;
    let stored = storage::load();
    let saved_model = Rc::new(VecModel::<SavedItem>::default());
    let recent_model = Rc::new(VecModel::<SharedString>::default());
    ui.set_saved(ModelRc::from(saved_model.clone()));
    ui.set_recent(ModelRc::from(recent_model.clone()));
    ui.set_theme_names(ModelRc::new(VecModel::from(
        themes::THEME_NAMES.iter().map(|n| SharedString::from(*n)).collect::<Vec<_>>(),
    )));
    ui.set_sfx_on(stored.sfx);

    let shared: Shared = Rc::new(RefCell::new(Calc {
        ui: ui.as_weak(),
        formula: String::new(),
        result: String::new(),
        half: String::new(),
        max: String::new(),
        details: String::new(),
        dimmed: false,
        show_halfmax: false,
        results_visible: false,
        roll_timer: 0.0,
        saved: stored.saved,
        recent: Vec::new(),
        theme: stored.theme,
        rng: Rng::from_time(),
        audio: Audio::new(stored.sfx && sound),
        saved_model,
        recent_model,
    }));
    {
        let c = shared.borrow();
        c.apply_theme();
        c.refresh_lists();
        c.refresh();
    }

    let s = shared.clone();
    ui.on_key(move |k| on_key(&s, k.as_str()));

    let s = shared.clone();
    ui.on_switch_pressed(move || {
        let mut c = s.borrow_mut();
        c.show_halfmax = !c.show_halfmax;
        c.refresh();
    });

    let s = shared.clone();
    ui.on_save_roll(move |name| {
        let mut c = s.borrow_mut();
        if c.formula.is_empty() {
            return;
        }
        let entry = Saved { name: name.to_string(), formula: c.formula.clone() };
        c.saved.push(entry);
        c.persist();
        c.refresh_lists();
        if let Some(ui) = c.ui.upgrade() {
            // como en la original, al guardar se abre el menú para ver la lista
            ui.invoke_open_menu();
        }
    });

    let s = shared.clone();
    ui.on_saved_activated(move |i| {
        let f = s.borrow().saved.get(i as usize).map(|x| x.formula.clone());
        if let Some(f) = f {
            roll_formula(&s, f);
        }
    });

    let s = shared.clone();
    ui.on_recent_activated(move |i| {
        let f = s.borrow().recent.get(i as usize).cloned();
        if let Some(f) = f {
            roll_formula(&s, f);
        }
    });

    let s = shared.clone();
    ui.on_delete_saved(move |i| {
        let mut c = s.borrow_mut();
        if (i as usize) < c.saved.len() {
            c.saved.remove(i as usize);
            c.persist();
            c.refresh_lists();
        }
    });

    let s = shared.clone();
    ui.on_clear_recent(move || {
        let mut c = s.borrow_mut();
        c.recent.clear();
        c.refresh_lists();
    });

    let s = shared.clone();
    ui.on_theme_chosen(move |i| {
        let mut c = s.borrow_mut();
        c.theme = (i as usize).min(themes::PALETTES.len() - 1);
        c.apply_theme();
        c.persist();
    });

    let s = shared.clone();
    ui.on_sfx_changed(move |on| {
        let mut c = s.borrow_mut();
        c.audio.enabled = on;
        c.persist();
    });

    Ok(ui)
}
