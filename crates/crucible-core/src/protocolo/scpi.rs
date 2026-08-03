use crate::perfil::Comando;
use std::collections::HashMap;

pub struct CodecScpi {
    semilla: u64,
}

impl CodecScpi {
    pub fn new() -> Self {
        Self { semilla: 42 }
    }
}

impl super::Protocolo for CodecScpi {
    fn procesar(
        &mut self,
        mensaje: &str,
        perfil: &crate::perfil::Perfil,
        estado: &mut crate::estado::Estado,
    ) -> Result<String, crate::error::CrucibleError> {
        let msg = mensaje.trim();
        let (idx, args) =
            match super::encontrar_comando(&perfil.comandos, msg) {
                Some(r) => r,
                None => return Err(crate::error::CrucibleError::ComandoNoReconocido(msg.into())),
            };

        let cmd = &perfil.comandos[idx];

        if let Some(muta) = &cmd.muta {
            super::aplicar_mutacion(muta, estado, &args);
        }

        let mut evaluador =
            crate::modelo::EvaluadorModelos::con_semilla(self.semilla);
        let respuesta = super::respuesta_comando(cmd, perfil, estado, &mut evaluador)?;
        Ok(respuesta)
    }
}

pub fn match_comando(
    comandos: &[Comando],
    mensaje: &str,
) -> Option<(usize, HashMap<String, String>)> {
    let msg = mensaje.trim();
    for (i, cmd) in comandos.iter().enumerate() {
        if let Some(args) = match_patron(&cmd.patron, msg) {
            return Some((i, args));
        }
    }
    None
}

fn match_patron(patron: &str, mensaje: &str) -> Option<HashMap<String, String>> {
    let patron = patron.trim();
    let mut args = HashMap::new();

    let patron_parts: Vec<&str> = patron.split('<').collect();
    if patron_parts.len() == 1 {
        if patron.eq_ignore_ascii_case(mensaje) {
            return Some(args);
        }
        return None;
    }

    let prefix = patron_parts[0];
    if !mensaje
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
    {
        return None;
    }

    let resto_patron = &patron[prefix.len()..];
    let resto_msg = &mensaje[prefix.len()..];

    let resto_patron = resto_patron.trim_start_matches('<');
    let parts: Vec<&str> = resto_patron.split('>').collect();
    let nombre_arg = parts[0];
    let suffix = if parts.len() > 1 { parts[1] } else { "" };

    if suffix.is_empty() {
        let val = resto_msg.trim();
        if val.is_empty() {
            return None;
        }
        args.insert(nombre_arg.to_string(), val.to_string());
        return Some(args);
    }

    if let Some(pos) = resto_msg.rfind(suffix) {
        let val = resto_msg[..pos].trim();
        if val.is_empty() {
            return None;
        }
        args.insert(nombre_arg.to_string(), val.to_string());
        Some(args)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_idn() {
        let cmds = vec![Comando {
            patron: "*IDN?".into(),
            muta: None,
            respuesta: Some("Keithley,2400,1234567,A1.2".into()),
            modelo: None,
        }];
        let r = match_comando(&cmds, "*IDN?");
        assert!(r.is_some());
        let r = match_comando(&cmds, "*idn?");
        assert!(r.is_some());
    }

    #[test]
    fn match_con_arg() {
        let cmds = vec![Comando {
            patron: "SOUR:VOLT <x>".into(),
            muta: None,
            respuesta: None,
            modelo: None,
        }];
        let r = match_comando(&cmds, "SOUR:VOLT 5.0").unwrap();
        assert_eq!(r.1.get("x").unwrap(), "5.0");
    }

    #[test]
    fn no_match_vacio() {
        let cmds = vec![Comando {
            patron: "SOUR:VOLT <x>".into(),
            muta: None,
            respuesta: None,
            modelo: None,
        }];
        let r = match_comando(&cmds, "SOUR:VOLT");
        assert!(r.is_none());
    }
}