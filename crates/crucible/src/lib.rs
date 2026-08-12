//! Transporte TCP de Crucible.
//!
//! Vive en una librería, no solo dentro de `main`, para que los tests de
//! integración levanten el servidor de verdad en vez de reimplementar uno
//! equivalente. Antes existía solo en `main.rs`: nada fuera del binario podía
//! alcanzarlo, y un test que reimplementaba el bucle de conexión podía quedarse
//! verde aunque `main.rs` tuviera un bug real (ver el bucle de `accept()` roto
//! que arregló c87751e, invisible para ese test).

use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

/// Se pone a escuchar en `host:puerto` y devuelve el listener ya enlazado.
///
/// Pasar `puerto: 0` deja que el sistema operativo elija uno libre; el puerto
/// efectivo se lee después con `listener.local_addr()`. Es lo que permite que
/// varios tests levanten servidores reales sin pisarse un puerto fijo.
pub async fn bind_tcp(host: &str, puerto: u16) -> anyhow::Result<TcpListener> {
    let addr = format!("{}:{}", host, puerto);
    let listener = TcpListener::bind(&addr).await?;
    eprintln!("escuchando en {}", listener.local_addr()?);
    Ok(listener)
}

/// Acepta conexiones sobre `listener` indefinidamente, todas contra el mismo
/// dispositivo.
///
/// El dispositivo es uno solo y lo comparten todas las conexiones: modela un
/// aparato físico único, y un instrumento real no vuelve a sus valores de
/// fábrica porque alguien cierre la sesión. Configúralo, desconéctate, vuelve,
/// y sigue como lo dejaste.
///
/// El mutex es el de tokio, no el de std: su guard es Send, así que la
/// conexión sigue siendo spawnable aunque algún día haya un await dentro de la
/// sección crítica, y un pánico no envenena el instrumento para las demás
/// conexiones. Se bloquea por mensaje y se suelta antes de escribir en el
/// socket: un cliente lento no puede congelar al resto. Las conexiones
/// concurrentes se serializan, que es justo lo que hace un aparato de verdad.
pub async fn aceptar_conexiones(
    listener: TcpListener,
    disp: crucible_core::Dispositivo,
) -> anyhow::Result<()> {
    let disp = Arc::new(Mutex::new(disp));

    loop {
        let (stream, peer) = listener.accept().await?;
        let disp = Arc::clone(&disp);
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
                let resp = disp.lock().await.procesar(msg);
                if let Some(resp) = resp
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

/// Enlaza y sirve en un solo paso: lo que usa `main` para un dispositivo suelto
/// o uno de un banco, donde no hace falta el puerto efectivo por separado.
pub async fn servir_tcp(
    disp: crucible_core::Dispositivo,
    host: &str,
    puerto: u16,
) -> anyhow::Result<()> {
    let listener = bind_tcp(host, puerto).await?;
    aceptar_conexiones(listener, disp).await
}
