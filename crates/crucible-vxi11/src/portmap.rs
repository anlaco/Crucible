//! Minimal ONC RPC portmapper (program 100000) on UDP 111.
//!
//! A VISA client that is given `TCPIP0::host::inst0::INSTR` has no port to
//! connect to, so the first thing it does is ask the portmapper on UDP 111
//! "which TCP port serves program 395183?". Without an answer to that question
//! the resource string simply does not resolve, which is why Crucible treats
//! the portmapper as part of the bank and not as an optional extra.
//!
//! We answer GETPORT(3) and nothing else that matters.

use crate::rpc::*;
use tokio::net::UdpSocket;

/// Binds the portmapper socket.
///
/// Kept apart from `serve_portmapper` on purpose: the bank binds every socket
/// it needs BEFORE it announces itself, so a failure here (UDP 111 is a
/// privileged port) stops the bank instead of leaving it half up.
pub async fn bind_portmapper(host: &str, port: u16) -> anyhow::Result<UdpSocket> {
    let addr = format!("{}:{}", host, port);
    Ok(UdpSocket::bind(&addr).await?)
}

/// Answers portmapper requests forever, always pointing at `vxi_port`.
///
/// There is a single `vxi_port` because there is a single VXI-11 core
/// listener for the whole bank; the instruments are told apart by their
/// device name, not by their port.
pub async fn serve_portmapper(socket: UdpSocket, vxi_port: u16) -> anyhow::Result<()> {
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

    /// The unit tests above check the bytes; this one checks the wiring, so
    /// that `bind_portmapper` + `serve_portmapper` really answer a client.
    /// It binds on port 0, not on 111, so it needs no privileges.
    #[tokio::test]
    async fn the_portmapper_answers_over_udp() {
        let socket = bind_portmapper("127.0.0.1", 0).await.unwrap();
        let portmapper_port = socket.local_addr().unwrap().port();
        tokio::spawn(serve_portmapper(socket, 7777));

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client
            .send_to(
                &build_getport(42, PROG_VXI11_CORE),
                ("127.0.0.1", portmapper_port),
            )
            .await
            .unwrap();

        let mut buf = [0u8; 512];
        // Con tope de tiempo: si nadie contesta, el test falla en vez de
        // colgar la CI.
        let (n, _) = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            client.recv_from(&mut buf),
        )
        .await
        .expect("el portmapper no contestó en 5 s")
        .unwrap();

        let mut pos = 0;
        assert_eq!(read_u32(&buf[..n], &mut pos).unwrap(), 42);
        let _message_type = read_u32(&buf[..n], &mut pos).unwrap();
        let _reply_state = read_u32(&buf[..n], &mut pos).unwrap();
        skip_auth(&buf[..n], &mut pos).unwrap();
        let _accept_state = read_u32(&buf[..n], &mut pos).unwrap();
        assert_eq!(read_u32(&buf[..n], &mut pos).unwrap(), 7777);
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
