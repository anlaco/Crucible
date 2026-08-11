use crucible_core::{Banco, Perfil};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

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

async fn servir_tcp(
    disp: crucible_core::Dispositivo,
    host: &str,
    puerto: u16,
) -> anyhow::Result<()> {
    let addr = format!("{}:{}", host, puerto);
    let listener = TcpListener::bind(&addr).await?;
    eprintln!("escuchando en {}", addr);

    loop {
        let (stream, peer) = listener.accept().await?;
        // Cada conexión habla con su propia copia del dispositivo. El perfil ya
        // se validó al cargarlo, así que clonar no puede fallar por otra causa.
        let mut disp_clone = disp.clonar()?;
        tokio::spawn(async move {
            eprintln!("conexion desde {}", peer);
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();
            loop {
                line.clear();
                let n = match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(_) => break,
                };
                let _ = n;
                let msg = line.trim_end_matches(['\r', '\n']);
                if msg.is_empty() {
                    continue;
                }
                // El motor SCPI ya sabe si el mensaje llevaba alguna consulta;
                // no hay que adivinarlo mirando si la línea acaba en '?', que
                // fallaba con los compuestos: "SOUR:VOLT?;:OUTP ON" lleva
                // consulta y no acaba en '?'.
                if let Some(resp) = disp_clone.procesar(msg)
                    && writer
                        .write_all(format!("{resp}\n").as_bytes())
                        .await
                        .is_err()
                {
                    break;
                }
            }
            eprintln!("conexion cerrada ({})", peer);
        });
    }
}
