//! XDR y ONC RPC mínimo para VXI-11 y portmapper.
//! Big-endian, alineado a 4 bytes. Sin dependencias.

pub const PROG_PORTMAP: u32 = 100_000;
pub const PROG_VXI11_CORE: u32 = 395183;

pub const PROC_NULL: u32 = 0;
pub const PROC_GETPORT: u32 = 3;

pub const PROC_CREATE_LINK: u32 = 10;
pub const PROC_DEVICE_WRITE: u32 = 11;
pub const PROC_DEVICE_READ: u32 = 12;
pub const PROC_DEVICE_READSTB: u32 = 13;
pub const PROC_DEVICE_CLEAR: u32 = 15;
pub const PROC_DEVICE_TRIGGER: u32 = 21;
pub const PROC_DESTROY_LINK: u32 = 23;

pub const MSG_CALL: u32 = 0;
pub const MSG_REPLY: u32 = 1;
pub const REPLY_ACCEPTED: u32 = 0;
pub const ACCEPT_SUCCESS: u32 = 0;
/// The server does not serve this program at all.
pub const ACCEPT_PROG_UNAVAIL: u32 = 1;
/// The server serves this program, but not in the version that was asked for.
pub const ACCEPT_PROG_MISMATCH: u32 = 2;

pub fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_be_bytes());
}
pub fn write_i32(buf: &mut Vec<u8>, v: i32) {
    write_u32(buf, v as u32);
}
pub fn read_u32(data: &[u8], pos: &mut usize) -> Option<u32> {
    if *pos + 4 > data.len() {
        return None;
    }
    let v = u32::from_be_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
    *pos += 4;
    Some(v)
}
pub fn read_i32(data: &[u8], pos: &mut usize) -> Option<i32> {
    read_u32(data, pos).map(|v| v as i32)
}

/// XDR opaque_auth: flavor + opaque bytes padded.
pub fn skip_auth(data: &[u8], pos: &mut usize) -> Option<()> {
    let _flavor = read_u32(data, pos)?;
    let len = read_u32(data, pos)? as usize;
    let pad = (4 - (len % 4)) % 4;
    if *pos + len + pad > data.len() {
        return None;
    }
    *pos += len + pad;
    Some(())
}
pub fn write_auth_null(buf: &mut Vec<u8>) {
    write_u32(buf, 0);
    write_u32(buf, 0);
}

/// XDR string: len + bytes + pad.
pub fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_u32(buf, s.len() as u32);
    buf.extend_from_slice(s.as_bytes());
    let pad = (4 - (s.len() % 4)) % 4;
    buf.extend(std::iter::repeat_n(0, pad));
}
pub fn read_string(data: &[u8], pos: &mut usize) -> Option<String> {
    let len = read_u32(data, pos)? as usize;
    if *pos + len > data.len() {
        return None;
    }
    let s = String::from_utf8_lossy(&data[*pos..*pos + len]).to_string();
    *pos += len;
    let pad = (4 - (len % 4)) % 4;
    *pos += pad;
    Some(s)
}

/// XDR opaque: len + bytes + pad.
pub fn write_opaque(buf: &mut Vec<u8>, data: &[u8]) {
    write_u32(buf, data.len() as u32);
    buf.extend_from_slice(data);
    let pad = (4 - (data.len() % 4)) % 4;
    buf.extend(std::iter::repeat_n(0, pad));
}
pub fn read_opaque(data: &[u8], pos: &mut usize) -> Option<Vec<u8>> {
    let len = read_u32(data, pos)? as usize;
    if *pos + len > data.len() {
        return None;
    }
    let v = data[*pos..*pos + len].to_vec();
    *pos += len;
    let pad = (4 - (len % 4)) % 4;
    *pos += pad;
    Some(v)
}

/// Cabecera de reply aceptada: XID, REPLY, MSG_ACCEPTED, verf null, SUCCESS.
pub fn write_reply_header(buf: &mut Vec<u8>, xid: u32) {
    write_u32(buf, xid);
    write_u32(buf, MSG_REPLY);
    write_u32(buf, REPLY_ACCEPTED);
    write_auth_null(buf);
    write_u32(buf, ACCEPT_SUCCESS);
}

/// Reply for a program we do not serve: accepted message, PROG_UNAVAIL.
///
/// Note there is no `write_reply_header` here: that helper always writes
/// SUCCESS, and an accepted reply that is not SUCCESS carries a different
/// status, not a patched one.
pub fn write_prog_unavail(xid: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    write_u32(&mut buf, xid);
    write_u32(&mut buf, MSG_REPLY);
    write_u32(&mut buf, REPLY_ACCEPTED);
    write_auth_null(&mut buf);
    write_u32(&mut buf, ACCEPT_PROG_UNAVAIL);
    buf
}

/// Reply for a program we serve but in a version we do not: accepted message,
/// PROG_MISMATCH, followed by the range of versions we do speak.
pub fn write_prog_mismatch(xid: u32, low: u32, high: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    write_u32(&mut buf, xid);
    write_u32(&mut buf, MSG_REPLY);
    write_u32(&mut buf, REPLY_ACCEPTED);
    write_auth_null(&mut buf);
    write_u32(&mut buf, ACCEPT_PROG_MISMATCH);
    write_u32(&mut buf, low);
    write_u32(&mut buf, high);
    buf
}

/// Parsea cabecera CALL y devuelve (xid, prog, vers, proc, offset_args).
/// Hace validación mínima (msg CALL, rpcvers 2).
pub fn parse_call(data: &[u8]) -> Option<(u32, u32, u32, u32, usize)> {
    let mut pos = 0;
    let xid = read_u32(data, &mut pos)?;
    let msg = read_u32(data, &mut pos)?;
    if msg != MSG_CALL {
        return None;
    }
    let rpcvers = read_u32(data, &mut pos)?;
    if rpcvers != 2 {
        return None;
    }
    let prog = read_u32(data, &mut pos)?;
    let vers = read_u32(data, &mut pos)?;
    let proc = read_u32(data, &mut pos)?;
    skip_auth(data, &mut pos)?;
    skip_auth(data, &mut pos)?;
    Some((xid, prog, vers, proc, pos))
}

/// Record Marking sobre TCP: 4 bytes, bit31 last, bits30-0 len.
pub fn encode_record(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + data.len());
    let hdr = (data.len() as u32) | 0x8000_0000;
    out.extend_from_slice(&hdr.to_be_bytes());
    out.extend_from_slice(data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdr_u32_roundtrip() {
        let mut buf = Vec::new();
        write_u32(&mut buf, 0x01020304);
        let mut pos = 0;
        assert_eq!(read_u32(&buf, &mut pos).unwrap(), 0x01020304);
        assert_eq!(pos, 4);
    }

    #[test]
    fn xdr_string_padded() {
        let mut buf = Vec::new();
        write_string(&mut buf, "hi");
        // len 2 + 2 bytes + 2 pad = 8
        assert_eq!(buf.len(), 8);
        let mut pos = 0;
        assert_eq!(read_string(&buf, &mut pos).unwrap(), "hi");
        assert_eq!(pos, 8);
    }

    #[test]
    fn xdr_opaque_padded() {
        let mut buf = Vec::new();
        write_opaque(&mut buf, b"ABC");
        // len 3 + 3 bytes +1 pad =8
        assert_eq!(buf.len(), 8);
        let mut pos = 0;
        assert_eq!(read_opaque(&buf, &mut pos).unwrap(), b"ABC");
    }

    #[test]
    fn parse_call_roundtrip() {
        let mut call = Vec::new();
        write_u32(&mut call, 123);
        write_u32(&mut call, MSG_CALL);
        write_u32(&mut call, 2);
        write_u32(&mut call, PROG_VXI11_CORE);
        write_u32(&mut call, 1);
        write_u32(&mut call, PROC_CREATE_LINK);
        write_auth_null(&mut call);
        write_auth_null(&mut call);
        // args dummy
        write_string(&mut call, "inst0");
        let (xid, prog, vers, proc, _off) = parse_call(&call).unwrap();
        assert_eq!(xid, 123);
        assert_eq!(prog, PROG_VXI11_CORE);
        assert_eq!(vers, 1);
        assert_eq!(proc, PROC_CREATE_LINK);
    }

    #[test]
    fn record_marking_roundtrip() {
        let data = b"hola";
        let rec = encode_record(data);
        let hdr = u32::from_be_bytes([rec[0], rec[1], rec[2], rec[3]]);
        assert_eq!(hdr & 0x8000_0000, 0x8000_0000);
        assert_eq!((hdr & 0x7fff_ffff) as usize, data.len());
        assert_eq!(&rec[4..], data);
    }
}
