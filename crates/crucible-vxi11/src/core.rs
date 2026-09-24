//! VXI-11 Device Core channel (program 395183, version 1) over TCP.
//!
//! One TCP listener serves EVERY instrument of the bank. This is not an
//! implementation shortcut, it is how VXI-11 works: a VISA resource string
//! like `TCPIP0::127.0.0.1::inst0::INSTR` has no room for a port number, so
//! the client asks the portmapper for the single core port and then says
//! which instrument it wants by name (`inst0`, `inst1`, ...) when it opens
//! the link. The name is therefore the only thing that can tell two
//! instruments apart, and we must honour it.
//!
//! Implemented: create_link / device_write / device_read / destroy_link.
//! Stubbed: device_clear / device_trigger / device_readstb.

use crate::rpc::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

/// Largest answer we tell the client it may ask for in one device_read.
const MAX_RECV_SIZE: u32 = 1024 * 1024;

// VXI-11 error codes (from the specification). We only use the few we need.
const ERROR_NONE: i32 = 0;
const ERROR_SYNTAX: i32 = 1;
/// The client named a device we do not serve.
const ERROR_DEVICE_NOT_ACCESSIBLE: i32 = 3;
/// The client used a link id we never handed out, or already destroyed.
const ERROR_INVALID_LINK: i32 = 4;

// VXI-11 "reason" flags of the device_read reply. They tell the client WHY we
// stopped sending, so it knows whether it has the whole answer or has to read
// again. These are not cosmetic: a client that is told END stops reading and
// silently loses the rest of the message.
/// We sent exactly as many bytes as the client asked for, and there is more.
const REASON_REQCNT: u32 = 1;
/// We stopped on the termination character.
const REASON_CHR: u32 = 2;
/// We sent the last byte of the answer.
const REASON_END: u32 = 4;

/// Bit of the device_read `flags` argument that means "the termChar argument
/// is meaningful". If it is off, the client does not care about term chars.
const FLAG_TERMCHRSET: u32 = 0x80;

/// The instruments served on one VXI-11 core port, indexed by the device name
/// a VISA client asks for.
///
/// Every device is shared by every connection and every link, which is the
/// same rule the plain TCP runtime follows: one instrument per name, so the
/// state survives a client reconnecting.
pub struct DeviceMap {
    devices: HashMap<String, Arc<Mutex<crucible_core::Dispositivo>>>,
}

impl DeviceMap {
    pub fn new() -> Self {
        DeviceMap {
            devices: HashMap::new(),
        }
    }

    /// Adds an instrument under the name clients will ask for ("inst0", ...).
    ///
    /// If the name was already taken the old instrument is replaced. The bank
    /// rejects duplicate names long before getting here, when it loads the
    /// YAML.
    pub fn add(&mut self, name: &str, dispositivo: crucible_core::Dispositivo) {
        self.devices
            .insert(name.to_string(), Arc::new(Mutex::new(dispositivo)));
    }

    /// True if some instrument is registered under this name.
    fn contains(&self, name: &str) -> bool {
        self.devices.contains_key(name)
    }

    /// The instrument registered under `name`, if any.
    fn get(&self, name: &str) -> Option<Arc<Mutex<crucible_core::Dispositivo>>> {
        // Cloning the Arc, not the instrument: every caller ends up talking to
        // the very same Dispositivo behind the very same Mutex.
        self.devices.get(name).map(Arc::clone)
    }
}

impl Default for DeviceMap {
    fn default() -> Self {
        DeviceMap::new()
    }
}

/// One open link, as created by create_link.
///
/// A link is per connection and per device: the same client may open one link
/// to `inst0` and another to `inst1` over a single TCP connection, and the two
/// must not get each other's answers.
struct Link {
    /// The device name this link was opened for.
    device_name: String,
    /// The answer produced by the last device_write, waiting for a
    /// device_read to come and pick it up.
    pending: Vec<u8>,
}

/// Per-connection state. Links belong to the connection that created them,
/// which is what the VXI-11 specification says, and it also means a client
/// that drops the socket cannot leak links into another client's session.
struct ConnState {
    next_lid: i32,
    links: HashMap<i32, Link>,
}

impl ConnState {
    fn new() -> Self {
        ConnState {
            next_lid: 1,
            links: HashMap::new(),
        }
    }
}

/// Binds the VXI-11 core channel on host:port (port 0 lets the OS choose).
pub async fn bind_vxi11(host: &str, port: u16) -> anyhow::Result<TcpListener> {
    let addr = format!("{}:{}", host, port);
    Ok(TcpListener::bind(&addr).await?)
}

/// Accepts VXI-11 connections forever, serving every instrument of `devices`.
pub async fn serve_connections(listener: TcpListener, devices: DeviceMap) -> anyhow::Result<()> {
    let devices = Arc::new(devices);
    loop {
        let (stream, peer) = listener.accept().await?;
        let devices = Arc::clone(&devices);
        tokio::spawn(async move {
            eprintln!("vxi11 conexion desde {}", peer);
            if let Err(e) = serve_one_connection(stream, devices).await {
                eprintln!("vxi11 error con {}: {e:#}", peer);
            }
            eprintln!("vxi11 conexion cerrada ({})", peer);
        });
    }
}

async fn serve_one_connection(
    mut stream: TcpStream,
    devices: Arc<DeviceMap>,
) -> anyhow::Result<()> {
    let mut state = ConnState::new();
    let mut buf = Vec::new();
    loop {
        // ONC RPC over TCP frames every message with a 4 byte Record Marking
        // header: bit 31 says "this is the last fragment", the rest is the
        // length of this fragment.
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
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await?;
        buf.extend_from_slice(&data);
        if !last {
            // Keep collecting. VXI-11 clients normally send a single fragment.
            continue;
        }
        let request = std::mem::take(&mut buf);
        let reply = handle_rpc(&request, &mut state, &devices).await;
        let record = encode_record(&reply);
        stream.write_all(&record).await?;
        stream.flush().await?;
    }
    Ok(())
}

async fn handle_rpc(data: &[u8], state: &mut ConnState, devices: &DeviceMap) -> Vec<u8> {
    let parsed = parse_call(data);
    if parsed.is_none() {
        // We could not even read the XID, so there is no reply we could send
        // that the client would be able to match to its call.
        return Vec::new();
    }
    let (xid, prog, vers, proc, off) = parsed.unwrap();

    if prog != PROG_VXI11_CORE {
        return write_prog_unavail(xid);
    }
    if vers != 1 {
        return write_prog_mismatch(xid, 1, 1);
    }

    match proc {
        PROC_CREATE_LINK => handle_create_link(xid, data, off, state, devices),
        PROC_DEVICE_WRITE => handle_device_write(xid, data, off, state, devices).await,
        PROC_DEVICE_READ => handle_device_read(xid, data, off, state),
        PROC_DESTROY_LINK => handle_destroy_link(xid, data, off, state),
        PROC_DEVICE_CLEAR => handle_device_clear(xid, data, off, state),
        PROC_DEVICE_READSTB => handle_readstb(xid),
        PROC_DEVICE_TRIGGER => handle_trigger(xid),
        _ => {
            // A procedure we do not implement. The VXI-11 payload for a plain
            // failure is Device_Error { error }.
            let mut reply = Vec::new();
            write_reply_header(&mut reply, xid);
            write_i32(&mut reply, ERROR_SYNTAX);
            reply
        }
    }
}

fn handle_create_link(
    xid: u32,
    data: &[u8],
    mut pos: usize,
    state: &mut ConnState,
    devices: &DeviceMap,
) -> Vec<u8> {
    // Create_LinkParms { clientId, lockDevice, lock_timeout, device }
    let _client_id = read_u32(data, &mut pos).unwrap_or(0);
    let _lock_device = read_u32(data, &mut pos).unwrap_or(0);
    let _lock_timeout = read_u32(data, &mut pos).unwrap_or(0);
    let device_name = read_string(data, &mut pos).unwrap_or_default();

    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);

    // The device name is the ONLY thing that says which instrument the client
    // wants. Accepting a name we do not serve would hand the client some other
    // instrument, which answers plausibly and is wrong: the worst kind of bug
    // for a test bench. So we refuse it.
    if !devices.contains(&device_name) {
        write_i32(&mut reply, ERROR_DEVICE_NOT_ACCESSIBLE);
        write_i32(&mut reply, 0); // no link id
        write_u32(&mut reply, 0); // abortPort
        write_u32(&mut reply, 0); // maxRecvSize
        return reply;
    }

    let lid = state.next_lid;
    state.next_lid += 1;
    state.links.insert(
        lid,
        Link {
            device_name,
            pending: Vec::new(),
        },
    );

    write_i32(&mut reply, ERROR_NONE);
    write_i32(&mut reply, lid);
    write_u32(&mut reply, 0); // abortPort: we do not offer the abort channel
    write_u32(&mut reply, MAX_RECV_SIZE);
    reply
}

async fn handle_device_write(
    xid: u32,
    data: &[u8],
    mut pos: usize,
    state: &mut ConnState,
    devices: &DeviceMap,
) -> Vec<u8> {
    // Device_WriteParms { lid, io_timeout, lock_timeout, flags, data }
    let lid = read_i32(data, &mut pos).unwrap_or(-1);
    let _io_timeout = read_u32(data, &mut pos).unwrap_or(0);
    let _lock_timeout = read_u32(data, &mut pos).unwrap_or(0);
    let _flags = read_u32(data, &mut pos).unwrap_or(0);
    let payload = read_opaque(data, &mut pos).unwrap_or_default();

    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);

    let device_name = match state.links.get(&lid) {
        Some(link) => link.device_name.clone(),
        None => {
            write_i32(&mut reply, ERROR_INVALID_LINK);
            write_u32(&mut reply, 0);
            return reply;
        }
    };
    let dispositivo = match devices.get(&device_name) {
        Some(d) => d,
        None => {
            // Cannot happen: create_link checked the name and the map never
            // changes while we are serving. Refusing is still better than
            // answering with somebody else's instrument.
            write_i32(&mut reply, ERROR_DEVICE_NOT_ACCESSIBLE);
            write_u32(&mut reply, 0);
            return reply;
        }
    };

    // The payload is the SCPI message as the client typed it, terminator and
    // all.
    let message = String::from_utf8_lossy(&payload)
        .trim()
        .trim_end_matches(['\r', '\n'])
        .to_string();
    let answer = if message.is_empty() {
        None
    } else {
        let mut d = dispositivo.lock().await;
        d.procesar(&message)
    };

    // A query leaves its answer waiting for the device_read that will follow.
    // A command leaves nothing, and clears whatever the previous one left.
    let pending = match answer {
        Some(text) => {
            let mut bytes = text.into_bytes();
            if !bytes.ends_with(b"\n") {
                bytes.push(b'\n');
            }
            bytes
        }
        None => Vec::new(),
    };
    if let Some(link) = state.links.get_mut(&lid) {
        link.pending = pending;
    }

    write_i32(&mut reply, ERROR_NONE);
    write_u32(&mut reply, payload.len() as u32);
    reply
}

fn handle_device_read(xid: u32, data: &[u8], mut pos: usize, state: &mut ConnState) -> Vec<u8> {
    // Device_ReadParms { lid, requestSize, io_timeout, lock_timeout, flags,
    //                    termChar }
    let lid = read_i32(data, &mut pos).unwrap_or(-1);
    let request_size = read_u32(data, &mut pos).unwrap_or(1024) as usize;
    let _io_timeout = read_u32(data, &mut pos).unwrap_or(0);
    let _lock_timeout = read_u32(data, &mut pos).unwrap_or(0);
    let flags = read_u32(data, &mut pos).unwrap_or(0);
    let term_char = read_u32(data, &mut pos).unwrap_or(0);

    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);

    let link = match state.links.get_mut(&lid) {
        Some(link) => link,
        None => {
            write_i32(&mut reply, ERROR_INVALID_LINK);
            write_u32(&mut reply, 0); // reason
            write_opaque(&mut reply, b"");
            return reply;
        }
    };

    // Send at most what the client asked for, and keep the rest for the next
    // device_read.
    let sent_everything = link.pending.len() <= request_size;
    let cut = std::cmp::min(link.pending.len(), request_size);
    let chunk: Vec<u8> = link.pending[..cut].to_vec();
    link.pending = link.pending[cut..].to_vec();

    let mut reason = 0;
    if sent_everything {
        // Nothing left over, so this really is the end of the message.
        reason |= REASON_END;
        // If the client set a termination character and the message happens to
        // end with it, say so. Our answers are a single line ending in "\n",
        // so the term char can only ever be the last byte.
        let wants_term_char = (flags & FLAG_TERMCHRSET) != 0;
        if wants_term_char && chunk.last() == Some(&(term_char as u8)) {
            reason |= REASON_CHR;
        }
    } else {
        // We stopped because the buffer was full, NOT because the message
        // ended. Claiming END here is what makes a client with a short buffer
        // take "Keysight" for a whole *IDN? answer and leave the rest to
        // poison its next read.
        reason |= REASON_REQCNT;
    }

    write_i32(&mut reply, ERROR_NONE);
    write_u32(&mut reply, reason);
    write_opaque(&mut reply, &chunk);
    reply
}

fn handle_destroy_link(xid: u32, data: &[u8], mut pos: usize, state: &mut ConnState) -> Vec<u8> {
    let lid = read_i32(data, &mut pos).unwrap_or(-1);
    state.links.remove(&lid);
    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);
    write_i32(&mut reply, ERROR_NONE);
    reply
}

/// device_clear: throw away the answer nobody came to read.
///
/// This is what a client calls (viClear) to get back in step after a timeout,
/// so leaving the stale answer in place would defeat its whole purpose.
fn handle_device_clear(xid: u32, data: &[u8], mut pos: usize, state: &mut ConnState) -> Vec<u8> {
    let lid = read_i32(data, &mut pos).unwrap_or(-1);
    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);
    match state.links.get_mut(&lid) {
        Some(link) => {
            link.pending.clear();
            write_i32(&mut reply, ERROR_NONE);
        }
        None => write_i32(&mut reply, ERROR_INVALID_LINK),
    }
    reply
}

/// device_readstb: we have no status byte to report, so we report zero.
fn handle_readstb(xid: u32) -> Vec<u8> {
    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);
    write_i32(&mut reply, ERROR_NONE);
    write_u32(&mut reply, 0);
    reply
}

/// device_trigger: accepted and ignored; no profile reacts to a trigger yet.
fn handle_trigger(xid: u32) -> Vec<u8> {
    let mut reply = Vec::new();
    write_reply_header(&mut reply, xid);
    write_i32(&mut reply, ERROR_NONE);
    reply
}
