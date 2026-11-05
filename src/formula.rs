//! Motor de fórmulas de dados: edición de la fórmula, tirada y cálculo.
//!
//! Representación interna (igual que la app original): dígitos, `d` (dado),
//! `n` (dado con caras por teclear, el botón «dx»), operadores `+ - x /` y
//! paréntesis. Al mostrarla se pasa por [`display`].

/// Error de una tirada (fórmula incompleta, desbordes, división por cero…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollError;

/// Resultado completo de una tirada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roll {
    /// Expresión con cada dado sustituido por sus valores, p. ej. `(3+5)+8`.
    pub details: String,
    pub result: i32,
    /// Mitad del resultado (división entera).
    pub half: i32,
    /// Máximo posible de la fórmula.
    pub max: i32,
}

/// Generador de números pseudoaleatorios (SplitMix64), sin dependencias.
pub struct Rng(u64);

impl Rng {
    pub fn from_time() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Rng(nanos ^ 0xD1B5_4A32_D192_ED03)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Entero uniforme en `[lo, hi]` (ambos incluidos).
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        let span = (hi as i64 - lo as i64 + 1).max(1) as u64;
        (lo as i64 + (self.next_u64() % span) as i64) as i32
    }
}

fn is_op(c: u8) -> bool {
    matches!(c, b'+' | b'-' | b'x' | b'/')
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Num,
    Die,
    DieX,
    Op,
    ParenOpen,
    ParenClose,
}

fn classify_insert(ins: &str) -> Kind {
    let b = ins.as_bytes();
    if ins.contains('d') {
        Kind::Die
    } else if ins.contains('n') {
        Kind::DieX
    } else if b.first().is_some_and(|&c| is_op(c)) {
        Kind::Op
    } else if b.first() == Some(&b'(') {
        Kind::ParenOpen
    } else if b.first() == Some(&b')') {
        Kind::ParenClose
    } else {
        Kind::Num
    }
}

/// Último componente de la fórmula: (inicio, tipo).
fn last_component(cur: &str) -> (usize, Kind) {
    let b = cur.as_bytes();
    let last = b[b.len() - 1];
    if is_op(last) {
        return (b.len() - 1, Kind::Op);
    }
    if last == b'(' {
        return (b.len() - 1, Kind::ParenOpen);
    }
    if last == b')' {
        return (b.len() - 1, Kind::ParenClose);
    }
    let mut start = b.len() - 1;
    while start > 0 && !is_op(b[start - 1]) && b[start - 1] != b'(' && b[start - 1] != b')' {
        start -= 1;
    }
    let comp = &cur[start..];
    let kind = if comp.contains('d') {
        Kind::Die
    } else if comp.contains('n') {
        Kind::DieX
    } else {
        Kind::Num
    };
    (start, kind)
}

fn parse_i32(s: &str) -> Result<i32, RollError> {
    s.parse::<i32>().map_err(|_| RollError)
}

/// Caras de un componente de dado (`1d6` → 6, `1n` o `1n12` → 0 o 12).
fn sides_of(comp: &str, marker: char) -> i32 {
    match comp.find(marker) {
        Some(i) if i + 1 < comp.len() => comp[i + 1..].parse().unwrap_or(0),
        _ => 0,
    }
}

/// Si la fórmula acaba en un dado tecleado con sus caras (`2n6`), lo da por completo (`2d6`).
fn normalize(cur: &str) -> String {
    if !cur.is_empty() {
        let (start, kind) = last_component(cur);
        if kind == Kind::DieX && sides_of(&cur[start..], 'n') > 0 {
            return format!("{}{}", &cur[..start], cur[start..].replacen('n', "d", 1));
        }
    }
    cur.to_string()
}

/// Tecla `d` del teclado: abre un dado (`2` `d` → `2d…`; sin número, `1d…`).
fn type_die(cur: &str) -> String {
    if cur.is_empty() {
        return "1n".to_string();
    }
    match last_component(cur).1 {
        Kind::Num => format!("{cur}n"),
        Kind::Op | Kind::ParenOpen => format!("{cur}1n"),
        Kind::ParenClose => format!("{cur}x1n"),
        Kind::Die | Kind::DieX => cur.to_string(),
    }
}

/// Tecla `%`: completa un dado recién abierto con 100 caras (`d%` = `d100`).
fn type_percent(cur: &str) -> String {
    if !cur.is_empty() {
        let (start, kind) = last_component(cur);
        if kind == Kind::DieX && sides_of(&cur[start..], 'n') == 0 {
            return format!("{cur}100");
        }
    }
    cur.to_string()
}

/// Pulsación de la calculadora: `d` y `%` escriben notación de dados; el resto se inserta.
pub fn press(cur: &str, key: &str) -> String {
    match key {
        "d" => type_die(cur),
        "%" => type_percent(cur),
        _ => insert(cur, key),
    }
}

/// Inserta `ins` (un dígito, `1d6`, `1n`, un operador o un paréntesis) en la fórmula.
pub fn insert(cur: &str, ins: &str) -> String {
    if cur.is_empty() {
        return ins.to_string();
    }
    let ins_kind = classify_insert(ins);
    // al pasar a otra cosa (no un dígito), el dado tecleado queda completo
    let normalized;
    let cur = if ins_kind == Kind::Num {
        cur
    } else {
        normalized = normalize(cur);
        normalized.as_str()
    };
    let (start, last_kind) = last_component(cur);
    let last = &cur[start..];
    let append = || format!("{cur}{ins}");

    // dados iguales seguidos: se suma uno al contador o se concatena con «+»
    if (ins_kind == Kind::Die && last_kind == Kind::Die) || (ins_kind == Kind::DieX && last_kind == Kind::DieX) {
        let marker = if ins_kind == Kind::DieX { 'n' } else { 'd' };
        let ins_sides = if ins_kind == Kind::DieX { 0 } else { sides_of(ins, 'd') };
        let last_sides = sides_of(last, marker);
        if ins_sides == last_sides {
            let at = last.find(marker).unwrap_or(0);
            let count = last[..at].parse::<i32>().unwrap_or(1).abs() + 1;
            return format!("{}{}{}", &cur[..start], count, &last[at..]);
        }
        return format!("{cur}+{ins}");
    }
    if ins_kind == Kind::Op && last_kind == Kind::Op {
        return format!("{}{}", &cur[..cur.len() - 1], ins);
    }
    if ins_kind == Kind::ParenOpen {
        if matches!(last_kind, Kind::Num | Kind::Die | Kind::DieX | Kind::ParenClose) {
            return format!("{cur}x(");
        }
        return append();
    }
    if ins_kind == Kind::ParenClose {
        // sin ningún «(» abierto, el cierre envuelve toda la fórmula
        let mut depth = 1;
        for &c in cur.as_bytes().iter().rev() {
            if depth == 0 {
                break;
            }
            match c {
                b'(' => depth -= 1,
                b')' => depth += 1,
                _ => {}
            }
        }
        if depth > 0 {
            return format!("({cur})");
        }
        return append();
    }
    match last_kind {
        Kind::Num if matches!(ins_kind, Kind::Die | Kind::DieX) => format!("{cur}{}", &ins[1..]),
        Kind::Die if ins_kind != Kind::Op => format!("{cur}+{ins}"),
        Kind::DieX => match ins_kind {
            // «1n» sin caras + un dado: el dado completa el contador
            Kind::Die if sides_of(last, 'n') == 0 => {
                let at = last.find('n').unwrap_or(0);
                let count = last[..at].parse::<i32>().unwrap_or(1).abs();
                format!("{}{}d{}", &cur[..start], count, sides_of(ins, 'd'))
            }
            Kind::Die => format!("{cur}+{ins}"),
            _ => append(),
        },
        Kind::ParenClose if matches!(ins_kind, Kind::Num | Kind::Die | Kind::DieX) => format!("{cur}x{ins}"),
        _ => append(),
    }
}

/// Borra lo último: un carácter, o un dado entero (`d12`) si la fórmula termina en uno.
pub fn delete_last(cur: &str) -> String {
    let b = cur.as_bytes();
    if b.is_empty() {
        return String::new();
    }
    let mut cut = b.len() - 1;
    let mut i = cut as isize;
    while i >= 0 {
        let c = b[i as usize];
        if c == b'd' || c == b'%' {
            cut = i as usize;
            break;
        }
        if is_op(c) || c == b'(' || c == b')' || c == b'n' {
            break;
        }
        i -= 1;
    }
    cur[..cut].to_string()
}

/// Texto que se muestra: `d` en lugar de `n` y operadores con espacios.
pub fn display(f: &str) -> String {
    f.replace('n', "d")
        .replace('+', " + ")
        .replace('-', " \u{2013} ")
        .replace('x', " \u{00D7} ")
        .replace('/', " \u{00F7} ")
}

pub fn has_roll(f: &str) -> bool {
    f.contains('d') || f.contains('n')
}

/// Sustituye cada dado por sus valores (o por su máximo) y devuelve la expresión resultante.
fn roll_details(formula: &str, max: bool, rng: &mut Rng) -> Result<String, RollError> {
    let f = formula.replace('n', "d");
    let b = f.as_bytes();
    let mut out = String::new();
    let mut seg_start = 0;
    let mut i = 0;
    while i <= b.len() {
        let boundary = i == b.len() || is_op(b[i]) || b[i] == b'(' || b[i] == b')';
        if boundary {
            let seg = &f[seg_start..i];
            match seg.find('d') {
                Some(d) => {
                    let count = if d == 0 { 1 } else { parse_i32(&seg[..d])? };
                    let sides_txt = &seg[d + 1..];
                    if sides_txt.is_empty() {
                        return Err(RollError);
                    }
                    let sides = parse_i32(sides_txt)?;
                    if max {
                        out.push_str(&count.wrapping_mul(sides).to_string());
                    } else if sides <= 0 || count <= 0 {
                        out.push('0');
                    } else if count > 1_000_000 {
                        return Err(RollError);
                    } else if count <= 6 {
                        let dice: Vec<String> = (0..count).map(|_| rng.range(1, sides).to_string()).collect();
                        out.push_str(&format!("({})", dice.join("+")));
                    } else {
                        let sum = (0..count).fold(0i32, |acc, _| acc.wrapping_add(rng.range(1, sides)));
                        out.push_str(&format!("({sum})"));
                    }
                }
                None => out.push_str(seg),
            }
            if i < b.len() {
                out.push(b[i] as char);
            }
            seg_start = i + 1;
        }
        i += 1;
    }
    Ok(out)
}

/// Evalúa una expresión con `+ - x /` y paréntesis, con enteros de 32 bits como Java.
pub fn solve(expr: &str) -> Result<i32, RollError> {
    struct Parser<'a> {
        b: &'a [u8],
        pos: usize,
    }
    impl Parser<'_> {
        fn peek(&self) -> Option<u8> {
            self.b.get(self.pos).copied()
        }
        fn expr(&mut self) -> Result<i32, RollError> {
            let mut acc = self.term()?;
            while let Some(c @ (b'+' | b'-')) = self.peek() {
                self.pos += 1;
                let rhs = self.term()?;
                acc = if c == b'+' { acc.wrapping_add(rhs) } else { acc.wrapping_sub(rhs) };
            }
            Ok(acc)
        }
        fn term(&mut self) -> Result<i32, RollError> {
            let mut acc = self.factor()?;
            while let Some(c @ (b'x' | b'/')) = self.peek() {
                self.pos += 1;
                let rhs = self.factor()?;
                if c == b'x' {
                    acc = acc.wrapping_mul(rhs);
                } else if rhs == 0 {
                    return Err(RollError);
                } else {
                    acc = acc.wrapping_div(rhs);
                }
            }
            Ok(acc)
        }
        fn factor(&mut self) -> Result<i32, RollError> {
            match self.peek() {
                Some(b'-') => {
                    self.pos += 1;
                    Ok(self.factor()?.wrapping_neg())
                }
                Some(b'+') => {
                    self.pos += 1;
                    self.factor()
                }
                Some(b'(') => {
                    self.pos += 1;
                    let v = self.expr()?;
                    if self.peek() == Some(b')') {
                        self.pos += 1;
                    }
                    Ok(v)
                }
                Some(c) if c.is_ascii_digit() => {
                    let start = self.pos;
                    while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                        self.pos += 1;
                    }
                    parse_i32(std::str::from_utf8(&self.b[start..self.pos]).unwrap_or(""))
                }
                _ => Err(RollError),
            }
        }
    }
    let mut p = Parser { b: expr.as_bytes(), pos: 0 };
    let v = p.expr()?;
    if p.pos == p.b.len() { Ok(v) } else { Err(RollError) }
}

/// Tira la fórmula. Los operadores sobrantes del final se ignoran.
pub fn roll(formula: &str, rng: &mut Rng) -> Result<Roll, RollError> {
    let trimmed = formula.trim_end_matches(|c: char| matches!(c, '+' | '-' | 'x' | '/'));
    if trimmed.is_empty() {
        return Err(RollError);
    }
    let details = roll_details(trimmed, false, rng)?;
    let result = solve(&details)?;
    let max = solve(&roll_details(trimmed, true, rng)?)?;
    Ok(Roll { details, result, half: result / 2, max })
}

/// Número con separador de miles, como `NumberFormat`.
pub fn format_number(n: i32) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(keys: &[&str]) -> String {
        keys.iter().fold(String::new(), |cur, k| insert(&cur, k))
    }

    #[test]
    fn dice_merge_and_modifiers() {
        assert_eq!(press(&["1d6"]), "1d6");
        assert_eq!(press(&["1d6", "1d6"]), "2d6");
        assert_eq!(press(&["1d6", "1d6", "1d6"]), "3d6");
        assert_eq!(press(&["1d6", "1d8"]), "1d6+1d8");
        assert_eq!(press(&["1d20", "+", "1", "0"]), "1d20+10");
        assert_eq!(press(&["2", "1d6"]), "2d6");
        assert_eq!(press(&["1d20", "5"]), "1d20+5");
    }

    #[test]
    fn operators_and_parentheses() {
        assert_eq!(press(&["1", "+", "-"]), "1-");
        assert_eq!(press(&["1d4", "(" ]), "1d4x(");
        assert_eq!(press(&["(", "1", "+", "2", ")", "3"]), "(1+2)x3");
        assert_eq!(press(&["(", "1", "+", "2", ")", "1d6"]), "(1+2)x1d6");
        assert_eq!(press(&["1", "+", "2", ")"]), "(1+2)");
        assert_eq!(press(&["(", "1", ")"]), "(1)");
    }

    #[test]
    fn dx_button() {
        assert_eq!(press(&["1n"]), "1n");
        assert_eq!(press(&["1n", "1", "2"]), "1n12");
        assert_eq!(press(&["1n", "1n"]), "2n");
        assert_eq!(press(&["1n", "1d6"]), "1d6");
        assert_eq!(display("1n12"), "1d12");
    }

    #[test]
    fn typed_dice_notation() {
        let typed = |keys: &[&str]| keys.iter().fold(String::new(), |cur, k| super::press(&cur, k));
        assert_eq!(display(&typed(&["2", "d", "6"])), "2d6");
        assert_eq!(display(&typed(&["d", "2", "0"])), "1d20");
        assert_eq!(display(&typed(&["2", "d", "%"])), "2d100");
        assert_eq!(display(&typed(&["d", "%"])), "1d100");
        assert_eq!(display(&typed(&["1", "d", "6", "+", "5"])), "1d6 + 5");
        assert_eq!(display(&typed(&["1", "d", "2", "0", "+", "3", "d", "6"])), "1d20 + 3d6");
        assert_eq!(display(&typed(&["(", "2", "d", "6", "+", "1", ")", "x", "2"])), "(2d6 + 1) \u{00D7} 2");
        // tras teclear un dado, el botón del mismo dado suma uno al contador
        assert_eq!(display(&super::press(&typed(&["2", "d", "6"]), "1d6")), "3d6");
        assert_eq!(display(&super::press(&typed(&["2", "d", "6"]), "1d8")), "2d6+1d8".replace('+', " + "));
        // «d» tras un dado se ignora; borrar quita primero las caras y luego la «d»
        assert_eq!(typed(&["1", "d", "6", "d"]), "1n6");
        assert_eq!(delete_last(&typed(&["2", "d", "6"])), "2n");
        assert_eq!(delete_last("2n"), "2");
        let mut rng = Rng(3);
        let r = roll(&typed(&["2", "d", "6", "+", "4"]), &mut rng).unwrap();
        assert!((6..=16).contains(&r.result) && r.max == 16);
    }

    #[test]
    fn deleting() {
        assert_eq!(delete_last("2d12+8"), "2d12+");
        assert_eq!(delete_last("2d12"), "2");
        assert_eq!(delete_last("2"), "");
        assert_eq!(delete_last("12"), "1");
        assert_eq!(delete_last("1d6+1d8"), "1d6+1");
        assert_eq!(delete_last(""), "");
    }

    #[test]
    fn display_format() {
        assert_eq!(display("2d12+8-1x3/2"), "2d12 + 8 \u{2013} 1 \u{00D7} 3 \u{00F7} 2");
    }

    #[test]
    fn solving() {
        assert_eq!(solve("1+2x3"), Ok(7));
        assert_eq!(solve("(1+2)x3"), Ok(9));
        assert_eq!(solve("7/2"), Ok(3));
        assert_eq!(solve("-7/2"), Ok(-3));
        assert_eq!(solve("-3+5"), Ok(2));
        assert_eq!(solve("1/0"), Err(RollError));
        assert_eq!(solve("99999999999"), Err(RollError));
        assert_eq!(solve("2x(3+4"), Ok(14));
    }

    #[test]
    fn rolling_is_bounded_and_consistent() {
        let mut rng = Rng(42);
        for _ in 0..500 {
            let r = roll("2d6+3", &mut rng).unwrap();
            assert!((5..=15).contains(&r.result));
            assert_eq!(r.max, 15);
            assert_eq!(r.half, r.result / 2);
            assert!(r.details.starts_with('(') && r.details.ends_with("+3"));
        }
        let r = roll("10d4", &mut rng).unwrap();
        assert!((10..=40).contains(&r.result));
        assert_eq!(r.max, 40);
        assert!(r.details.starts_with('(') && !r.details.contains('+')); // más de 6 dados: solo la suma
    }

    #[test]
    fn rolling_edge_cases() {
        let mut rng = Rng(7);
        assert_eq!(roll("5+3", &mut rng).unwrap().result, 8);
        assert_eq!(roll("1d20+", &mut rng).map(|r| r.max), Ok(20));
        assert_eq!(roll("1n", &mut rng), Err(RollError));
        assert_eq!(roll("", &mut rng), Err(RollError));
        assert_eq!(roll("1d6/0", &mut rng), Err(RollError));
        assert_eq!(roll("1d1000000000000", &mut rng), Err(RollError));
        assert_eq!(roll("2d1", &mut rng).unwrap().result, 2);
        assert_eq!(roll("1d6-10", &mut rng).map(|r| r.max), Ok(-4));
    }

    #[test]
    fn number_format() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(-1234567), "-1,234,567");
    }
}
