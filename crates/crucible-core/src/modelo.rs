use crate::error::{CrucibleError, Result};
use crate::estado::Estado;
use crate::perfil::ModeloDef;
use std::collections::HashMap;

pub struct EvaluadorModelos {
    semilla: u64,
    /// Cuántas muestras de ruido se han sacado ya. Cada `gauss()` avanza el
    /// contador: sin él, el «ruido» sería el mismo número en todas las
    /// lecturas, y quien simula un banco nota enseguida que el multímetro
    /// repite la medida al bit.
    muestras: u64,
}

impl EvaluadorModelos {
    pub fn con_semilla(semilla: u64) -> Self {
        Self {
            semilla,
            muestras: 0,
        }
    }

    pub fn evaluar(&mut self, modelo: &ModeloDef, estado: &Estado) -> Result<String> {
        if let Some(cuando) = &modelo.cuando
            && !evaluar_guarda(cuando, estado)
        {
            let formato = Formato::de_modelo(modelo)?;
            return Ok(match &modelo.fallback {
                Some(fb) => evaluar_fallback(fb, formato),
                None => formato.escribir(0.0),
            });
        }

        match modelo.tipo.as_str() {
            "formula" => {
                let expr = modelo
                    .expr
                    .as_ref()
                    .ok_or_else(|| CrucibleError::Evaluacion("modelo formula sin expr".into()))?;
                let arbol = Parser::analizar(expr)?;
                let val = self.calcular(&arbol, estado)?;
                Ok(Formato::de_modelo(modelo)?.escribir(val))
            }
            _ => Err(CrucibleError::Evaluacion(format!(
                "tipo de modelo no soportado: {}",
                modelo.tipo
            ))),
        }
    }

    fn calcular(&mut self, expr: &Expr, estado: &Estado) -> Result<f64> {
        Ok(match expr {
            Expr::Num(n) => *n,
            Expr::Var(nombre) => estado.get_float(nombre).ok_or_else(|| {
                CrucibleError::Evaluacion(format!(
                    "la variable '{nombre}' no existe en el estado o no es numérica"
                ))
            })?,
            Expr::Neg(e) => -self.calcular(e, estado)?,
            Expr::Bin(op, izq, der) => {
                let a = self.calcular(izq, estado)?;
                let b = self.calcular(der, estado)?;
                match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    '/' => {
                        if b == 0.0 {
                            return Err(CrucibleError::Evaluacion("división por cero".into()));
                        }
                        a / b
                    }
                    _ => unreachable!("el parser solo produce + - * /"),
                }
            }
            Expr::Func(nombre, args) => {
                let mut v = Vec::with_capacity(args.len());
                for a in args {
                    v.push(self.calcular(a, estado)?);
                }
                match nombre.as_str() {
                    "gauss" => {
                        // La primera muestra usa la semilla tal cual, así la
                        // primera lectura sigue dando el mismo número que antes
                        // de que el ruido variara (la guía de Anvil lo cita).
                        let ruido = pseudo_gauss(self.semilla.wrapping_add(self.muestras));
                        self.muestras += 1;
                        v[0] + v[1] * ruido
                    }
                    "abs" => v[0].abs(),
                    "sqrt" => v[0].sqrt(),
                    "min" => v[0].min(v[1]),
                    "max" => v[0].max(v[1]),
                    _ => unreachable!("el parser valida los nombres de función"),
                }
            }
        })
    }
}

/// Comprueba la sintaxis de una fórmula sin evaluarla.
///
/// Se llama al cargar el perfil: quien escribe un instrumento debe enterarse
/// de un paréntesis sin cerrar al arrancar el banco, no cuando su software
/// lance la primera medida y reciba un error SCPI sin contexto.
pub fn validar_formula(expr: &str) -> Result<()> {
    Parser::analizar(expr).map(|_| ())
}

#[derive(Debug)]
enum Expr {
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
    Func(String, Vec<Expr>),
}

/// Funciones admitidas y cuántos argumentos toma cada una.
const FUNCIONES: &[(&str, usize)] = &[
    ("gauss", 2),
    ("abs", 1),
    ("sqrt", 1),
    ("min", 2),
    ("max", 2),
];

/// Descenso recursivo con la precedencia de siempre.
///
/// El evaluador anterior partía la cadena por el último operador que
/// encontrara. No sabía de paréntesis, y un literal como `1e-3` lo partía por
/// el signo del exponente: dos cosas que cualquiera escribe en la fórmula de
/// un instrumento el primer día.
struct Parser<'a> {
    fuente: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn analizar(fuente: &'a str) -> Result<Expr> {
        let mut p = Parser {
            fuente,
            bytes: fuente.as_bytes(),
            pos: 0,
        };
        let e = p.suma()?;
        p.saltar_espacios();
        if p.pos < p.bytes.len() {
            return Err(p.error("sobra texto al final"));
        }
        Ok(e)
    }

    fn error(&self, motivo: &str) -> CrucibleError {
        CrucibleError::Evaluacion(format!(
            "fórmula '{}': {motivo} (posición {})",
            self.fuente,
            self.pos + 1
        ))
    }

    fn saltar_espacios(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn ver(&mut self) -> Option<u8> {
        self.saltar_espacios();
        self.bytes.get(self.pos).copied()
    }

    fn suma(&mut self) -> Result<Expr> {
        let mut izq = self.producto()?;
        while let Some(c @ (b'+' | b'-')) = self.ver() {
            self.pos += 1;
            let der = self.producto()?;
            izq = Expr::Bin(c as char, Box::new(izq), Box::new(der));
        }
        Ok(izq)
    }

    fn producto(&mut self) -> Result<Expr> {
        let mut izq = self.unario()?;
        while let Some(c @ (b'*' | b'/')) = self.ver() {
            self.pos += 1;
            let der = self.unario()?;
            izq = Expr::Bin(c as char, Box::new(izq), Box::new(der));
        }
        Ok(izq)
    }

    fn unario(&mut self) -> Result<Expr> {
        match self.ver() {
            Some(b'-') => {
                self.pos += 1;
                Ok(Expr::Neg(Box::new(self.unario()?)))
            }
            Some(b'+') => {
                self.pos += 1;
                self.unario()
            }
            _ => self.atomo(),
        }
    }

    fn atomo(&mut self) -> Result<Expr> {
        match self.ver() {
            None => Err(self.error("la fórmula acaba antes de tiempo")),
            Some(b'(') => {
                self.pos += 1;
                let e = self.suma()?;
                if self.ver() != Some(b')') {
                    return Err(self.error("falta un ')'"));
                }
                self.pos += 1;
                Ok(e)
            }
            Some(c) if c.is_ascii_digit() || c == b'.' => self.numero(),
            // Compatibilidad con perfiles que citaban la variable: "voltaje".
            Some(b'"') => {
                self.pos += 1;
                let ini = self.pos;
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b'"' {
                    self.pos += 1;
                }
                if self.pos >= self.bytes.len() {
                    return Err(self.error("comilla sin cerrar"));
                }
                let nombre = self.fuente[ini..self.pos].to_string();
                self.pos += 1;
                Ok(Expr::Var(nombre))
            }
            Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                let ini = self.pos;
                while self.pos < self.bytes.len()
                    && (self.bytes[self.pos].is_ascii_alphanumeric()
                        || self.bytes[self.pos] == b'_')
                {
                    self.pos += 1;
                }
                let nombre = self.fuente[ini..self.pos].to_string();
                if self.ver() == Some(b'(') {
                    self.funcion(nombre)
                } else {
                    Ok(Expr::Var(nombre))
                }
            }
            Some(_) => Err(self.error("carácter inesperado")),
        }
    }

    fn funcion(&mut self, nombre: String) -> Result<Expr> {
        let Some(&(_, aridad)) = FUNCIONES.iter().find(|(n, _)| *n == nombre) else {
            let conocidas: Vec<&str> = FUNCIONES.iter().map(|(n, _)| *n).collect();
            return Err(self.error(&format!(
                "función '{nombre}' desconocida; existen: {}",
                conocidas.join(", ")
            )));
        };
        self.pos += 1; // '('
        let mut args = Vec::new();
        if self.ver() != Some(b')') {
            loop {
                args.push(self.suma()?);
                match self.ver() {
                    Some(b',') => self.pos += 1,
                    Some(b')') => break,
                    _ => return Err(self.error("se esperaba ',' o ')'")),
                }
            }
        }
        self.pos += 1; // ')'
        if args.len() != aridad {
            return Err(self.error(&format!(
                "{nombre} necesita {aridad} argumento(s) y recibe {}",
                args.len()
            )));
        }
        Ok(Expr::Func(nombre, args))
    }

    fn numero(&mut self) -> Result<Expr> {
        let ini = self.pos;
        while self.pos < self.bytes.len()
            && (self.bytes[self.pos].is_ascii_digit() || self.bytes[self.pos] == b'.')
        {
            self.pos += 1;
        }
        // Exponente: la 'e' solo cuenta si la sigue un dígito, con signo o sin él.
        if self.pos < self.bytes.len() && matches!(self.bytes[self.pos], b'e' | b'E') {
            let mut fin = self.pos + 1;
            if fin < self.bytes.len() && matches!(self.bytes[fin], b'+' | b'-') {
                fin += 1;
            }
            if fin < self.bytes.len() && self.bytes[fin].is_ascii_digit() {
                self.pos = fin;
                while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                    self.pos += 1;
                }
            }
        }
        self.fuente[ini..self.pos]
            .parse::<f64>()
            .map(Expr::Num)
            .map_err(|_| self.error("número mal escrito"))
    }
}

fn evaluar_guarda(guarda: &HashMap<String, String>, estado: &Estado) -> bool {
    for (clave, esperado) in guarda {
        let val = estado.get(clave);
        let Some(val) = val else {
            return false;
        };
        let esperado = esperado.trim_matches('"');
        // Numérico si los dos lados lo son: `cuando: { rango: 1 }` debe casar
        // con un estado que guarda 1.0, aunque su texto sea "1.0".
        //
        // Booleano si lo esperado lo es: `OUTP 1` guarda 1.0 y `OUTP ON` guarda
        // true, y los dos tienen que encender la salida.
        let igual = if let Some(b) = texto_a_bool(esperado) {
            val.as_bool() == Some(b)
        } else {
            match (val.as_float(), esperado.parse::<f64>()) {
                (Some(a), Ok(b)) => a == b,
                _ => val.as_str().eq_ignore_ascii_case(esperado),
            }
        };
        if !igual {
            return false;
        }
    }
    true
}

fn texto_a_bool(t: &str) -> Option<bool> {
    match t.to_ascii_lowercase().as_str() {
        "true" | "on" => Some(true),
        "false" | "off" => Some(false),
        _ => None,
    }
}

fn evaluar_fallback(fb: &str, formato: Formato) -> String {
    let fb = fb.trim_matches('"');
    if let Ok(f) = fb.parse::<f64>() {
        formato.escribir(f)
    } else {
        fb.to_string()
    }
}

/// Cómo se escribe un número en la respuesta.
///
/// Importa más de lo que parece: el software de medida real parsea la
/// respuesta pensando en lo que devuelve el instrumento de verdad. Un `OUTP?`
/// que contesta `1.0` en vez de `1` rompe un `int()` de Python, y hay programas
/// que esperan la notación `+4.501385E+00` de muchos multímetros.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Formato {
    /// `1.0` para enteros, todos los decimales para el resto.
    PorDefecto,
    /// `1`, `-3`: redondeado, sin decimales.
    Entero,
    /// `4.502`: N decimales.
    Fijo(usize),
    /// `+4.501385E+00`: N decimales en la mantisa, signo siempre y exponente
    /// de al menos dos cifras, como en SCPI.
    Cientifico(usize),
}

impl Formato {
    pub fn parse(texto: &str) -> Result<Self> {
        let t = texto.trim().to_lowercase();
        let (tipo, n) = match t.split_once(':') {
            Some((tipo, n)) => (tipo, Some(n)),
            None => (t.as_str(), None),
        };
        let decimales = |defecto: usize| -> Result<usize> {
            match n {
                None => Ok(defecto),
                Some(n) => n
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .filter(|d| *d <= 17)
                    .ok_or_else(|| {
                        CrucibleError::PerfilInvalido(format!(
                            "formato '{texto}': los decimales deben ser un número de 0 a 17"
                        ))
                    }),
            }
        };
        match tipo {
            "entero" if n.is_none() => Ok(Formato::Entero),
            "fijo" => Ok(Formato::Fijo(decimales(3)?)),
            "cientifico" | "científico" => Ok(Formato::Cientifico(decimales(6)?)),
            _ => Err(CrucibleError::PerfilInvalido(format!(
                "formato '{texto}' desconocido; usa 'entero', 'fijo:N' o 'cientifico:N'"
            ))),
        }
    }

    fn de_modelo(modelo: &ModeloDef) -> Result<Self> {
        modelo
            .formato
            .as_deref()
            .map_or(Ok(Formato::PorDefecto), Formato::parse)
    }

    pub fn escribir(self, f: f64) -> String {
        match self {
            Formato::PorDefecto => format_resultado(f),
            Formato::Entero => format!("{}", f.round() as i64),
            Formato::Fijo(n) => format!("{f:.n$}"),
            Formato::Cientifico(n) => {
                // Rust escribe «4.5e0»; SCPI espera «+4.500000E+00».
                let s = format!("{:.n$e}", f.abs());
                let (mantisa, exp) = s.split_once('e').expect("{:e} siempre lleva 'e'");
                let exp: i32 = exp.parse().expect("exponente entero");
                let signo = if f.is_sign_negative() { '-' } else { '+' };
                let signo_exp = if exp < 0 { '-' } else { '+' };
                format!("{signo}{mantisa}E{signo_exp}{:02}", exp.abs())
            }
        }
    }
}

fn pseudo_gauss(semilla: u64) -> f64 {
    let mut s = semilla
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    s ^= s >> 29;
    s = s.wrapping_mul(0x9E3779B97F4A7C15);
    s ^= s >> 32;
    let u = (s >> 11) as f64 / (1u64 << 53) as f64;
    let v = ((s >> 40) & 0x7FF) as f64 / 2048.0;
    let two_pi = 2.0 * std::f64::consts::PI;
    let r = (-2.0 * (1.0 - u).ln()).sqrt();
    r * (two_pi * v).cos()
}

fn format_resultado(f: f64) -> String {
    if f == f.trunc() && f.abs() < 1e16 {
        format!("{:.1}", f)
    } else {
        format!("{}", f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::estado::Valor;

    fn calcular(expr: &str, estado: &Estado) -> Result<f64> {
        let arbol = Parser::analizar(expr)?;
        EvaluadorModelos::con_semilla(42).calcular(&arbol, estado)
    }

    fn estado_con(vars: &[(&str, f64)]) -> Estado {
        let mut e = Estado::new();
        for (k, v) in vars {
            e.set(k, Valor::Float(*v));
        }
        e
    }

    #[test]
    fn evalua_literal() {
        assert_eq!(calcular("5.0", &Estado::new()).unwrap(), 5.0);
    }

    #[test]
    fn evalua_variable() {
        let e = estado_con(&[("voltaje", 3.5)]);
        assert_eq!(calcular("voltaje", &e).unwrap(), 3.5);
    }

    #[test]
    fn evalua_suma() {
        let e = estado_con(&[("v", 5.0)]);
        assert_eq!(calcular("v + 1.0", &e).unwrap(), 6.0);
    }

    #[test]
    fn evalua_division() {
        let e = estado_con(&[("v", 5.0)]);
        assert!((calcular("v / 1000.0", &e).unwrap() - 0.005).abs() < 1e-12);
    }

    #[test]
    fn respeta_la_precedencia_y_los_parentesis() {
        let e = estado_con(&[("a", 2.0), ("b", 4.0)]);
        assert_eq!(calcular("a + b * 3", &e).unwrap(), 14.0);
        assert_eq!(calcular("(a + b) * 3", &e).unwrap(), 18.0);
        assert_eq!(calcular("(a + b) / 2", &e).unwrap(), 3.0);
        assert_eq!(calcular("a - b - 1", &e).unwrap(), -3.0);
        assert_eq!(calcular("-a * -b", &e).unwrap(), 8.0);
    }

    #[test]
    fn acepta_notacion_cientifica() {
        let e = estado_con(&[("i", 2.0)]);
        assert!((calcular("i * 1e-3", &e).unwrap() - 0.002).abs() < 1e-12);
        assert_eq!(calcular("2.5E+2", &e).unwrap(), 250.0);
    }

    #[test]
    fn acepta_la_variable_entre_comillas_de_los_perfiles_antiguos() {
        let e = estado_con(&[("v", 7.0)]);
        assert_eq!(calcular("\"v\"", &e).unwrap(), 7.0);
    }

    #[test]
    fn las_funciones_calculan_lo_que_dicen() {
        let e = estado_con(&[("x", -9.0)]);
        assert_eq!(calcular("sqrt(abs(x))", &e).unwrap(), 3.0);
        assert_eq!(calcular("max(x, 1) + min(x, 1)", &e).unwrap(), -8.0);
    }

    #[test]
    fn una_formula_mal_escrita_se_rechaza_al_validar() {
        for mala in ["(a + b", "a +", "foo(1)", "gauss(1)", "a $ b", "2 3"] {
            assert!(validar_formula(mala).is_err(), "aceptó '{mala}'");
        }
        assert!(validar_formula("gauss(0, 1e-3) + (v / 2)").is_ok());
    }

    #[test]
    fn una_variable_ausente_da_error_con_su_nombre() {
        let err = calcular("inexistente * 2", &Estado::new()).unwrap_err();
        assert!(err.to_string().contains("inexistente"));
    }

    fn modelo_con_guarda(clave: &str, esperado: &str) -> ModeloDef {
        ModeloDef {
            tipo: "formula".into(),
            cuando: Some(HashMap::from([(clave.into(), esperado.into())])),
            expr: Some("1".into()),
            fallback: Some("0".into()),
            formato: None,
        }
    }

    #[test]
    fn la_guarda_booleana_acepta_1_y_on() {
        let modelo = modelo_con_guarda("salida", "true");
        let mut ev = EvaluadorModelos::con_semilla(42);
        for (valor, esperado) in [
            (Valor::Float(1.0), "1.0"),
            (Valor::Bool(true), "1.0"),
            (Valor::Float(0.0), "0.0"),
            (Valor::Bool(false), "0.0"),
        ] {
            let mut e = Estado::new();
            e.set("salida", valor.clone());
            assert_eq!(ev.evaluar(&modelo, &e).unwrap(), esperado, "con {valor:?}");
        }
    }

    #[test]
    fn la_guarda_numerica_no_depende_de_como_se_escribe_el_numero() {
        let modelo = modelo_con_guarda("rango", "1");
        let e = estado_con(&[("rango", 1.0)]);
        assert_eq!(
            EvaluadorModelos::con_semilla(42)
                .evaluar(&modelo, &e)
                .unwrap(),
            "1.0"
        );
    }

    #[test]
    fn los_formatos_escriben_como_un_instrumento() {
        assert_eq!(Formato::PorDefecto.escribir(1.0), "1.0");
        assert_eq!(Formato::Entero.escribir(1.0), "1");
        assert_eq!(Formato::Entero.escribir(-2.6), "-3");
        assert_eq!(Formato::Fijo(3).escribir(4.50138), "4.501");
        assert_eq!(
            Formato::Cientifico(6).escribir(4.501385029),
            "+4.501385E+00"
        );
        assert_eq!(Formato::Cientifico(3).escribir(-0.00012346), "-1.235E-04");
        assert_eq!(Formato::Cientifico(2).escribir(0.0), "+0.00E+00");
        assert_eq!(Formato::Cientifico(1).escribir(1.5e120), "+1.5E+120");
    }

    #[test]
    fn el_formato_se_lee_del_perfil_y_rechaza_lo_desconocido() {
        assert_eq!(Formato::parse("entero").unwrap(), Formato::Entero);
        assert_eq!(Formato::parse("fijo:4").unwrap(), Formato::Fijo(4));
        assert_eq!(
            Formato::parse("Cientifico").unwrap(),
            Formato::Cientifico(6)
        );
        for malo in ["hex", "fijo:x", "entero:2", "cientifico:99"] {
            assert!(Formato::parse(malo).is_err(), "aceptó '{malo}'");
        }
    }

    #[test]
    fn el_fallback_respeta_el_formato() {
        let mut modelo = modelo_con_guarda("salida", "true");
        modelo.formato = Some("entero".into());
        let mut e = Estado::new();
        e.set("salida", Valor::Bool(false));
        assert_eq!(
            EvaluadorModelos::con_semilla(42)
                .evaluar(&modelo, &e)
                .unwrap(),
            "0"
        );
    }

    #[test]
    fn gauss_es_reproducible_entre_ejecuciones() {
        let e = Estado::new();
        let a = calcular("gauss(0, 1)", &e).unwrap();
        let b = calcular("gauss(0, 1)", &e).unwrap();
        assert_eq!(a, b, "dos evaluadores con la misma semilla deben coincidir");
    }

    #[test]
    fn gauss_varia_entre_lecturas_sucesivas() {
        let e = Estado::new();
        let arbol = Parser::analizar("gauss(0, 1)").unwrap();
        let mut ev = EvaluadorModelos::con_semilla(42);
        let lecturas: Vec<f64> = (0..5).map(|_| ev.calcular(&arbol, &e).unwrap()).collect();
        assert!(
            lecturas.windows(2).any(|w| w[0] != w[1]),
            "el ruido no cambia entre lecturas: {lecturas:?}"
        );
    }
}
