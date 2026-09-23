//! Portmapper UDP 111 (prog 100000) mínimo para anunciar VXI-11.
//! Responde a GETPORT(3) para PROG_VXI11_CORE 395183 con el puerto TCP del core.

use crate::rpc::*;
use tokio::net::UdpSocket;

/// Arranca el portmapper en 0.0.0.0:111 (o puerto alternativo para tests).
/// `vxi_port` es el puerto TCP donde escucha el core VXI-11.
pub async fn servir_portmapper(vxi_port: u16, bind_addr: &str) -> anyhow::Result<()> {
    let socket = UdpSocket::bind(bind_addr).await?;
    eprintln!(
        "portmapper escuchando en {} -> vxi11 en {}",
        socket.local_addr()?,
        vxi_port
    );
    let mut buf = vec![0u8; 2048];
    loop {
        let (n, peer) = socket.recv_from(&mut buf).await?;
        let data = &buf[..n];
        if let Some(reply) = handle_portmap_request(data, vxi_port) {
            let _ = socket.send_to(&reply, peer).await;
        }
    }
}

fn handle_portmap_request(data: &[u8], vxi_port: u16) -> Option<Vec<u8>> {
    let (xid, prog, _vers, proc, off) = parse_call(data)?;
    if prog != PROG_PORTMAP {
        return None;
    }
    match proc {
        PROC_NULL => {
            let mut reply = Vec::new();
            write_reply_header(&mut reply, xid);
            Some(reply)
        }
        PROC_GETPORT => {
            let mut pos = off;
            let req_prog = read_u32(data, &mut pos)?;
            let _req_vers = read_u32(data, &mut pos)?;
            let _req_proto = read_u32(data, &mut pos)?; // 6=TCP,17=UDP
            let _req_port = read_u32(data, &mut pos)?;
            let port = if req_prog == PROG_VXI11_CORE {
                vxi_port as u32
            } else {
                0
            };
            let mut reply = Vec::new();
            write_reply_header(&mut reply, xid);
            write_u32(&mut reply, port);
            Some(reply)
        }
        _ => {
            // Para DUMP u otros, devolver port 0
            let mut reply = Vec::new();
            write_reply_header(&mut reply, xid);
            write_u32(&mut reply, 0);
            Some(reply)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_getport(xid: u32, prog: u32) -> Vec<u8> {
        let mut call = Vec::new();
        write_u32(&mut call, xid);
        write_u32(&mut call, MSG_CALL);
        write_u32(&mut call, 2);
        write_u32(&mut call, PROG_PORTMAP);
        write_u32(&mut call, 2);
        write_u32(&mut call, PROC_GETPORT);
        write_auth_null(&mut call);
        write_auth_null(&mut call);
        write_u32(&mut call, prog);
        write_u32(&mut call, 1);
        write_u32(&mut call, 6); // TCP
        write_u32(&mut call, 0);
        call
    }

    #[test]
    fn getport_vxi11_devuelve_puerto() {
        let call = build_getport(99, PROG_VXI11_CORE);
        let reply = handle_portmap_request(&call, 9099).unwrap();
        // reply = header(24) + u32 port
        let mut pos = 0;
        let xid = read_u32(&reply, &mut pos).unwrap();
        assert_eq!(xid, 99);
        let mtype = read_u32(&reply, &mut pos).unwrap();
        assert_eq!(mtype, MSG_REPLY);
        let _ = read_u32(&reply, &mut pos).unwrap(); // accepted
        skip_auth(&reply, &mut pos).unwrap();
        let acc = read_u32(&reply, &mut pos).unwrap();
        assert_eq!(acc, ACCEPT_SUCCESS);
        let port = read_u32(&reply, &mut pos).unwrap();
        assert_eq!(port, 9099);
    }

    #[test]
    fn getport_otro_programa_devuelve_cero() {
        let call = build_getport(1, 12345);
        let reply = handle_portmap_request(&call, 9099).unwrap();
        let mut pos = 0;
        let _xid = read_u32(&reply, &mut pos).unwrap();
        let _mtype = read_u32(&reply, &mut pos).unwrap();
        let _accepted = read_u32(&reply, &mut pos).unwrap();
        skip_auth(&reply, &mut pos).unwrap();
        let _acc = read_u32(&reply, &mut pos).unwrap();
        let port = read_u32(&reply, &mut pos).unwrap();
        assert_eq!(port, 0);
    }
}
