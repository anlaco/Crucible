//! Núcleo VXI-11 Device Core (prog 395183 v1) sobre TCP.
//! Implementa create_link / device_write / device_read / destroy_link
//! y stubs para clear/trigger/readstb.

use crate::rpc::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

const MAX_RECV_SIZE: u32 = 1024 * 1024;

/// Estado por conexión TCP.
struct ConnState {
    next_lid: i32,
    /// lid -> respuesta pendiente (bytes) tras un write con query.
    pending: HashMap<i32, Vec<u8>>,
    /// lid válido
    links: HashMap<i32, ()>,
}

impl ConnState {
    fn new() -> Self {
        Self {
            next_lid: 1,
            pending: HashMap::new(),
            links: HashMap::new(),
        }
    }
}

/// Enlaza en host:puerto (0 deja elegir al SO).
pub async fn bind_vxi11(host: &str, puerto: u16) -> anyhow::Result<TcpListener> {
    let addr = format!("{}:{}", host, puerto);
    Ok(TcpListener::bind(&addr).await?)
}

/// Acepta conexiones VXI-11 indefinidamente, todas contra el mismo dispositivo.
pub async fn aceptar_conexiones_vxi11(
    listener: TcpListener,
    disp: crucible_core::Dispositivo,
) -> anyhow::Result<()> {
    let disp = Arc::new(Mutex::new(disp));
    loop {
        let (stream, peer) = listener.accept().await?;
        let disp = Arc::clone(&disp);
        tokio::spawn(async move {
            eprintln!("vxi11 conexion desde {}", peer);
            if let Err(e) = atender(stream, disp).await {
                eprintln!("vxi11 error con {}: {e:#}", peer);
            }
            eprintln!("vxi11 conexion cerrada ({})", peer);
        });
    }
}

async fn atender(
    mut stream: TcpStream,
    disp: Arc<Mutex<crucible_core::Dispositivo>>,
) -> anyhow::Result<()> {
    let mut state = ConnState::new();
    let mut buf = Vec::new();
    loop {
        // Leer Record Marking header (4 bytes)
        let mut hdr = [0u8; 4];
        if let Err(e) = stream.read_exact(&mut hdr).await {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                break;
            }
            return Err(e.into());
        }
        let h = u32::from_be_bytes(hdr);
        let last = (h & 0x8000_0000) != 0;
        let len = (h & 0x7fff_ffff) as usize;
        if len == 0 {
            continue;
        }
        // Leer fragmento(s)
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await?;
        buf.extend_from_slice(&data);
        if !last {
            // Acumular hasta last; VXI-11 normalmente manda un solo fragmento.
            continue;
        }
        let request = std::mem::take(&mut buf);
        let reply = handle_rpc(&request, &mut state, &disp).await;
        // Enviar reply con Record Marking
        let rec = encode_record(&reply);
        stream.write_all(&rec).await?;
        stream.flush().await?;
    }
    Ok(())
}

async fn handle_rpc(
    data: &[u8],
    state: &mut ConnState,
    disp: &Arc<Mutex<crucible_core::Dispositivo>>,
) -> Vec<u8> {
    let parsed = parse_call(data);
    if parsed.is_none() {
        // Respuesta genérica de error: no podemos ni leer XID
        return vec![];
    }
    let (xid, prog, vers, proc, off) = parsed.unwrap();
    if prog != PROG_VXI11_CORE || vers != 1 {
        // Portmapper no debería llegar aquí (va por UDP), pero si llega, devolver error.
        let mut reply = Vec::new();
        write_reply_header(&mut reply, xid);
        // Accept stat 1 = PROG_UNAVAIL o similar; usamos 1
        // Para simplificar, sobrescribimos último u32 (SUCCESS) con 1
        // En realidad debería ser estructura distinta, pero cliente esperará prog unavail.
        // Truco: ya escribimos SUCCESS=0, lo cambiamos a 1
        if reply.len() >= 4 {
            let l = reply.len() - 4;
            reply[l..].copy_from_slice(&1u32.to_be_bytes());
        }
        return reply;
    }

    match proc {
        PROC_CREATE_LINK => handle_create_link(xid, data, off, state),
        PROC_DEVICE_WRITE => handle_device_write(xid, data, off, state, disp).await,
        PROC_DEVICE_READ => handle_device_read(xid, data, off, state),
        PROC_DESTROY_LINK => handle_destroy_link(xid, data, off, state),
        PROC_DEVICE_CLEAR => handle_device_clear(xid, data, off, state),
        PROC_DEVICE_READSTB => handle_readstb(xid, data, off),
        PROC_DEVICE_TRIGGER => handle_trigger(xid, data, off),
        _ => {
            // PROC no soportado → error 1 (syntax) en payload específico
            let mut reply = Vec::new();
            write_reply_header(&mut reply, xid);
            // Para VXI-11, payload de error es Device_Error { error: i32 }
            write_i32(&mut reply, 1); // syntax error
            reply
        }
    }
}

fn handle_create_link(xid: u32, data: &[u8], mut pos: usize, state: &mut ConnState) -> Vec<u8> {
    // Args: clientId (Device_Link ya, pero en create_link es struct con clientId/lockDevice/lock_timeout/device)
    // Estructura Create_LinkParms: clientId {clientId: u32}, lockDevice bool, lock_timeout u32, device string
    // Simplificamos: leer en orden.
    // clientId: leemos como u32 (aunque en spec es Device_Link con clientId)
    // El cliente suele mandar: clientId, lockDevice, lock_timeout, device
    let _client_id = read_u32(data, &mut pos).unwrap_or(0);
    let _lock_device = read_u32(data, &mut pos).unwrap_or(0);
    let _lock_timeout = read_u32(data, &mut pos).unwrap_or(0);
    let _device = read_string(data, &mut pos).unwrap_or_else(|| "inst0".to_string());

    let lid = state.next_lid;
    state.next_lid += 1;
    state.links.insert(lid, ());
    state.pending.insert(lid, Vec::new());

    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);
    write_i32(&mut reply, 0); // error = 0 OK
    write_i32(&mut reply, lid);
    write_u32(&mut reply, 0); // abortPort (no usado)
    write_u32(&mut reply, MAX_RECV_SIZE);
    reply
}

async fn handle_device_write(
    xid: u32,
    data: &[u8],
    mut pos: usize,
    state: &mut ConnState,
    disp: &Arc<Mutex<crucible_core::Dispositivo>>,
) -> Vec<u8> {
    let lid = read_i32(data, &mut pos).unwrap_or(-1);
    let _io_timeout = read_u32(data, &mut pos).unwrap_or(0);
    let _lock_timeout = read_u32(data, &mut pos).unwrap_or(0);
    let _flags = read_u32(data, &mut pos).unwrap_or(0);
    let payload = read_opaque(data, &mut pos).unwrap_or_default();

    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);

    if !state.links.contains_key(&lid) {
        write_i32(&mut reply, 3); // invalid link
        write_u32(&mut reply, 0);
        return reply;
    }

    // El payload es SCPI tal cual (puede llevar \n)
    let msg = String::from_utf8_lossy(&payload)
        .trim()
        .trim_end_matches(['\r', '\n'])
        .to_string();
    let resp = if msg.is_empty() {
        None
    } else {
        let mut d = disp.lock().await;
        d.procesar(&msg)
    };

    if let Some(r) = resp {
        // Guardar para el siguiente device_read
        let mut bytes = r.into_bytes();
        // VXI-11 espera que el read devuelva con terminación; añadimos \n si no lo tiene
        if !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        state.pending.insert(lid, bytes);
    } else {
        state.pending.insert(lid, Vec::new());
    }

    write_i32(&mut reply, 0); // error
    write_u32(&mut reply, payload.len() as u32); // size = bytes escritos
    reply
}

fn handle_device_read(xid: u32, data: &[u8], mut pos: usize, state: &mut ConnState) -> Vec<u8> {
    let lid = read_i32(data, &mut pos).unwrap_or(-1);
    let request_size = read_u32(data, &mut pos).unwrap_or(1024);
    let _io_timeout = read_u32(data, &mut pos).unwrap_or(0);
    let _lock_timeout = read_u32(data, &mut pos).unwrap_or(0);
    let _flags = read_u32(data, &mut pos).unwrap_or(0);
    let _term_char = read_u32(data, &mut pos).unwrap_or(0);

    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);

    if !state.links.contains_key(&lid) {
        write_i32(&mut reply, 3);
        write_u32(&mut reply, 0);
        write_u32(&mut reply, 0);
        write_opaque(&mut reply, b"");
        return reply;
    }

    let pending = state.pending.get(&lid).cloned().unwrap_or_default();
    // Si pending vacío, devolver vacío con reason END
    let to_send = if pending.is_empty() {
        Vec::new()
    } else {
        // Respetar request_size
        let n = std::cmp::min(pending.len(), request_size as usize);
        let chunk = pending[..n].to_vec();
        // Si lo hemos enviado todo, limpiar
        if n == pending.len() {
            state.pending.insert(lid, Vec::new());
        } else {
            state.pending.insert(lid, pending[n..].to_vec());
        }
        chunk
    };

    write_i32(&mut reply, 0); // error
    write_u32(&mut reply, 4); // reason END (bit 2)
    write_opaque(&mut reply, &to_send);
    reply
}

fn handle_destroy_link(xid: u32, data: &[u8], mut pos: usize, state: &mut ConnState) -> Vec<u8> {
    let lid = read_i32(data, &mut pos).unwrap_or(-1);
    state.links.remove(&lid);
    state.pending.remove(&lid);
    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);
    write_i32(&mut reply, 0);
    reply
}

fn handle_device_clear(_xid: u32, _data: &[u8], _pos: usize, _state: &mut ConnState) -> Vec<u8> {
    // Stub: OK
    let mut reply = Vec::new();
    // Necesitamos xid, pero lo tenemos en _xid
    write_reply_header(&mut reply, _xid);
    write_i32(&mut reply, 0);
    reply
}

fn handle_readstb(xid: u32, _data: &[u8], _pos: usize) -> Vec<u8> {
    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);
    write_i32(&mut reply, 0);
    write_u32(&mut reply, 0); // stb
    reply
}

fn handle_trigger(xid: u32, _data: &[u8], _pos: usize) -> Vec<u8> {
    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);
    write_i32(&mut reply, 0);
    reply
}
