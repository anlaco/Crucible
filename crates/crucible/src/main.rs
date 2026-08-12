use crucible::servir_tcp;
use crucible_core::{Banco, Perfil};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("uso: crucible <perfil.yaml>      (un dispositivo)");
        eprintln!("     crucible --banco <banco.yaml>  (varios dispositivos)");
        std::process::exit(1);
    }

    if args[1] == "--banco" {
        if args.len() < 3 {
            eprintln!("error: falta path del banco");
            std::process::exit(1);
        }
        let path = PathBuf::from(&args[2]);
        let text = std::fs::read_to_string(&path)?;
        let banco = Banco::from_yaml(&text)?;
        let base_default = PathBuf::from(".");
        let base = path.parent().unwrap_or(&base_default);
        // Un perfil roto tumba el banco entero: arrancar a medias daría
        // resultados falsos en vez de un fallo visible.
        let dispositivos = banco.cargar_dispositivos(base)?;

        if dispositivos.is_empty() {
            eprintln!("error: el banco no declara ningún dispositivo");
            std::process::exit(1);
        }

        let mut handles = Vec::new();
        for (inst, disp) in dispositivos {
            if inst.transporte.tipo != "tcp" {
                eprintln!(
                    "aviso: transporte '{}' no soportado en MVP, saltando {}",
                    inst.transporte.tipo, inst.id
                );
                continue;
            }
            let puerto = inst.transporte.puerto.unwrap_or(5025);
            let host = inst
                .transporte
                .host
                .clone()
                .unwrap_or_else(|| "127.0.0.1".into());
            let id = inst.id.clone();
            handles.push(tokio::spawn(async move {
                if let Err(e) = servir_tcp(disp, &host, puerto).await {
                    eprintln!("error sirviendo {} en {}:{}: {}", id, host, puerto, e);
                }
            }));
        }
        eprintln!(
            "Crucible banco '{}' levantado ({} dispositivos)",
            banco.banco.nombre,
            handles.len()
        );
        for h in handles {
            let _ = h.await;
        }
    } else {
        let path = PathBuf::from(&args[1]);
        let perfil = Perfil::from_file(&path)?;
        let modelo = perfil.dispositivo.modelo.clone();
        let disp = crucible_core::Dispositivo::from_perfil(perfil)?;
        let puerto: u16 = if args.len() > 2 {
            args[2].parse().unwrap_or(5025)
        } else {
            5025
        };
        eprintln!("Crucible: {} en 127.0.0.1:{}", modelo, puerto);
        servir_tcp(disp, "127.0.0.1", puerto).await?;
    }
    Ok(())
}
