//! End to end tests for the VXI-11 transport: a real socket, real ONC RPC
//! records, and the real Dispositivo behind them.

use crucible_core::{Dispositivo, Perfil};
use crucible_vxi11::DeviceMap;
use crucible_vxi11::rpc::*;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const KEITHLEY_PROFILE: &str = include_str!("../../../perfiles/keithley_2400.yaml");
const N5767A_PROFILE: &str = include_str!("../../../bancos/rf/perfiles/n5767a.yaml");

/// No read in a test may wait forever: an instrument that never answers has
/// to fail the test, not hang CI.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

fn load(profile: &str) -> Dispositivo {
    let perfil = Perfil::from_yaml(profile).unwrap();
    Dispositivo::from_perfil(perfil).unwrap()
}

/// Starts a VXI-11 core channel on a port the OS picks, serving the given
/// instruments, and returns the port. No portmapper: the tests know the port
/// already, so they do not need one.
async fn start_core_channel(instruments: Vec<(&str, &str)>) -> u16 {
    let mut devices = DeviceMap::new();
    for (device_name, profile) in instruments {
        devices.add(device_name, load(profile));
    }
    let listener = crucible_vxi11::bind_vxi11("127.0.0.1", 0).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(crucible_vxi11::serve_connections(listener, devices));
    port
}

/// A bank with a single Keithley on "inst0", which is what most tests want.
async fn start_one_keithley() -> u16 {
    start_core_channel(vec![("inst0", KEITHLEY_PROFILE)]).await
}

/// The reply of one RPC call, already stripped of the reply header, so the
/// tests can read the VXI-11 payload straight away.
struct ReplyBody {
    bytes: Vec<u8>,
    pos: usize,
}

impl ReplyBody {
    fn next_i32(&mut self) -> i32 {
        read_i32(&self.bytes, &mut self.pos).unwrap()
    }
    fn next_u32(&mut self) -> u32 {
        read_u32(&self.bytes, &mut self.pos).unwrap()
    }
    fn next_opaque(&mut self) -> Vec<u8> {
        read_opaque(&self.bytes, &mut self.pos).unwrap()
    }
}

/// A VXI-11 client, just complete enough to drive the tests.
struct Session {
    stream: TcpStream,
    xid: u32,
    lid: i32,
}

impl Session {
    /// Opens a connection and a link to `device`, expecting it to succeed.
    async fn open(port: u16, device: &str) -> Session {
        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let mut session = Session {
            stream,
            xid: 100,
            lid: 0,
        };
        let (error, lid) = session.create_link(device).await;
        assert_eq!(error, 0, "create_link('{device}') devolvió error {error}");
        session.lid = lid;
        session
    }

    /// Opens a connection without opening a link, so a test can check what
    /// create_link does with a name nobody serves.
    async fn connect(port: u16) -> Session {
        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        Session {
            stream,
            xid: 100,
            lid: 0,
        }
    }

    fn next_xid(&mut self) -> u32 {
        self.xid += 1;
        self.xid
    }

    async fn call(&mut self, proc: u32, args: &[u8]) -> ReplyBody {
        let xid = self.next_xid();
        let mut message = Vec::new();
        write_u32(&mut message, xid);
        write_u32(&mut message, MSG_CALL);
        write_u32(&mut message, 2); // RPC version
        write_u32(&mut message, PROG_VXI11_CORE);
        write_u32(&mut message, 1); // program version
        write_u32(&mut message, proc);
        write_auth_null(&mut message);
        write_auth_null(&mut message);
        message.extend_from_slice(args);
        let record = encode_record(&message);
        self.stream.write_all(&record).await.unwrap();
        self.stream.flush().await.unwrap();

        // Record Marking header, then the record itself. Both reads are timed
        // out, not only the first one.
        let mut header = [0u8; 4];
        self.read_exact_before_timeout(&mut header, xid).await;
        let length = (u32::from_be_bytes(header) & 0x7fff_ffff) as usize;
        let mut bytes = vec![0u8; length];
        self.read_exact_before_timeout(&mut bytes, xid).await;

        let mut pos = 0;
        assert_eq!(
            read_u32(&bytes, &mut pos).unwrap(),
            xid,
            "la respuesta no es la de la llamada que hicimos"
        );
        let _message_type = read_u32(&bytes, &mut pos).unwrap();
        let _reply_state = read_u32(&bytes, &mut pos).unwrap();
        skip_auth(&bytes, &mut pos).unwrap();
        let accept_state = read_u32(&bytes, &mut pos).unwrap();
        assert_eq!(accept_state, ACCEPT_SUCCESS, "la llamada no fue aceptada");
        ReplyBody { bytes, pos }
    }

    async fn read_exact_before_timeout(&mut self, buf: &mut [u8], xid: u32) {
        tokio::time::timeout(READ_TIMEOUT, self.stream.read_exact(buf))
            .await
            .unwrap_or_else(|_| panic!("el banco no contestó a la llamada {xid} en 5 s"))
            .unwrap();
    }

    /// Returns (error, link id) instead of asserting, so tests can check the
    /// refusal as well as the happy path.
    async fn create_link(&mut self, device: &str) -> (i32, i32) {
        let mut args = Vec::new();
        write_u32(&mut args, 0); // clientId
        write_u32(&mut args, 0); // lockDevice: false
        write_u32(&mut args, 0); // lock_timeout
        write_string(&mut args, device);
        let mut reply = self.call(PROC_CREATE_LINK, &args).await;
        let error = reply.next_i32();
        let lid = reply.next_i32();
        let _abort_port = reply.next_u32();
        let _max_recv_size = reply.next_u32();
        (error, lid)
    }

    async fn write(&mut self, message: &str) {
        let mut args = Vec::new();
        write_i32(&mut args, self.lid);
        write_u32(&mut args, 1000); // io_timeout
        write_u32(&mut args, 1000); // lock_timeout
        write_u32(&mut args, 8); // flags: END
        write_opaque(&mut args, message.as_bytes());
        let mut reply = self.call(PROC_DEVICE_WRITE, &args).await;
        let error = reply.next_i32();
        assert_eq!(error, 0, "device_write devolvió error {error}");
        let _written = reply.next_u32();
    }

    /// One device_read, returning (reason, bytes) so a test can check why the
    /// bank stopped sending and not only what it sent.
    async fn read_chunk(&mut self, request_size: u32) -> (u32, Vec<u8>) {
        let mut args = Vec::new();
        write_i32(&mut args, self.lid);
        write_u32(&mut args, request_size);
        write_u32(&mut args, 1000); // io_timeout
        write_u32(&mut args, 1000); // lock_timeout
        write_u32(&mut args, 0x80); // flags: termChar is meaningful
        write_u32(&mut args, 10); // termChar: \n
        let mut reply = self.call(PROC_DEVICE_READ, &args).await;
        let error = reply.next_i32();
        assert_eq!(error, 0, "device_read devolvió error {error}");
        let reason = reply.next_u32();
        (reason, reply.next_opaque())
    }

    async fn read(&mut self) -> String {
        let (_reason, bytes) = self.read_chunk(8192).await;
        String::from_utf8_lossy(&bytes).trim().to_string()
    }

    async fn ask(&mut self, command: &str) -> String {
        self.write(command).await;
        self.read().await
    }

    async fn clear(&mut self) {
        let mut args = Vec::new();
        write_i32(&mut args, self.lid);
        write_u32(&mut args, 0); // flags
        write_u32(&mut args, 1000); // io_timeout
        write_u32(&mut args, 1000); // lock_timeout
        let mut reply = self.call(PROC_DEVICE_CLEAR, &args).await;
        let error = reply.next_i32();
        assert_eq!(error, 0, "device_clear devolvió error {error}");
    }
}

#[tokio::test]
async fn a_keithley_answers_over_vxi11() {
    let port = start_one_keithley().await;
    let mut s = Session::open(port, "inst0").await;

    assert_eq!(s.ask("*IDN?").await, "Keithley,2400,1234567,A1.2");

    s.write("SOUR:VOLT 5.0").await;
    s.write("OUTP ON").await;

    let volts: f64 = s.ask("MEAS:VOLT?").await.parse().unwrap();
    assert!((volts - 5.0).abs() < 0.01, "esperaba ~5.0 V, medí {volts}");

    let amps: f64 = s.ask("MEAS:CURR?").await.parse().unwrap();
    assert!((amps - 0.005).abs() < 0.0001, "esperaba ~5 mA, medí {amps}");
}

#[tokio::test]
async fn the_scpi_layer_is_the_same_one_as_over_plain_tcp() {
    let port = start_one_keithley().await;
    let mut s = Session::open(port, "inst0").await;

    s.write("source:voltage:level 5.0").await;
    assert_eq!(s.ask("SOUR:VOLT?").await, "5.0");

    assert_eq!(
        s.ask("*IDN?;:SOUR:VOLT?").await,
        "Keithley,2400,1234567,A1.2;5.0"
    );

    s.write("FOO:BAR 1").await;
    let error = s.ask("SYST:ERR?").await;
    assert!(
        error.starts_with("-113"),
        "la cola de errores dio '{error}'"
    );

    assert_eq!(s.ask("*IDN?").await, "Keithley,2400,1234567,A1.2");
}

#[tokio::test]
async fn the_state_survives_a_new_link() {
    let port = start_one_keithley().await;

    let mut first = Session::open(port, "inst0").await;
    first.write("SOUR:VOLT 4.5").await;
    first.write("OUTP ON").await;
    drop(first);

    // A different connection, a different link, the same instrument.
    let mut second = Session::open(port, "inst0").await;
    let volts: f64 = second.ask("MEAS:VOLT?").await.parse().unwrap();
    assert!(
        (volts - 4.5).abs() < 0.05,
        "el estado no sobrevivió al link nuevo: medí {volts}"
    );
    assert_eq!(second.ask("SOUR:VOLT?").await, "4.5");
}

#[tokio::test]
async fn an_output_that_is_off_measures_zero() {
    let port = start_one_keithley().await;
    let mut s = Session::open(port, "inst0").await;
    s.write("SOUR:VOLT 12.0").await;
    let volts: f64 = s.ask("MEAS:VOLT?").await.parse().unwrap();
    assert!(
        (volts - 0.0).abs() < 0.001,
        "con la salida abierta medí {volts}"
    );
}

#[tokio::test]
async fn each_link_reaches_the_device_it_named() {
    // The whole point of the transport: two instruments, one port, told apart
    // by their device name. Getting this wrong hands the client somebody
    // else's instrument, and it answers plausibly.
    let port =
        start_core_channel(vec![("inst0", KEITHLEY_PROFILE), ("inst1", N5767A_PROFILE)]).await;

    let mut keithley = Session::open(port, "inst0").await;
    assert_eq!(keithley.ask("*IDN?").await, "Keithley,2400,1234567,A1.2");

    let mut supply = Session::open(port, "inst1").await;
    let idn = supply.ask("*IDN?").await;
    assert!(
        idn.contains("N5767A"),
        "'inst1' debería ser la N5767A y contestó '{idn}'"
    );
}

#[tokio::test]
async fn two_links_on_one_connection_do_not_cross_answers() {
    let port =
        start_core_channel(vec![("inst0", KEITHLEY_PROFILE), ("inst1", N5767A_PROFILE)]).await;

    // Both links over the very same socket, interleaved on purpose.
    let mut s = Session::connect(port).await;
    let (_, keithley_link) = s.create_link("inst0").await;
    let (_, supply_link) = s.create_link("inst1").await;

    s.lid = keithley_link;
    s.write("*IDN?").await;
    s.lid = supply_link;
    s.write("*IDN?").await;

    s.lid = keithley_link;
    assert_eq!(s.read().await, "Keithley,2400,1234567,A1.2");
    s.lid = supply_link;
    assert!(s.read().await.contains("N5767A"));
}

#[tokio::test]
async fn a_device_nobody_serves_is_refused() {
    let port = start_one_keithley().await;
    let mut s = Session::connect(port).await;
    let (error, _lid) = s.create_link("inst7").await;
    // 3 is "device not accessible". Anything other than an error would mean
    // the client gets an instrument it did not ask for.
    assert_eq!(
        error, 3,
        "create_link('inst7') debería haber sido rechazado"
    );
}

#[tokio::test]
async fn a_truncated_read_does_not_claim_the_end_of_the_message() {
    let port = start_one_keithley().await;
    let mut s = Session::open(port, "inst0").await;
    s.write("*IDN?").await;

    // Ask for less than the answer needs. The bank must say REQCNT (1), not
    // END (4): a client told END keeps "Keithl" as the whole answer and lets
    // the rest poison its next read.
    let (reason, first) = s.read_chunk(6).await;
    assert_eq!(first, b"Keithl");
    assert_eq!(
        reason & 4,
        0,
        "dijo END sin haber terminado (reason {reason})"
    );
    assert_eq!(
        reason & 1,
        1,
        "debería haber dicho REQCNT (reason {reason})"
    );

    // The rest is still there, and now it really is the end.
    let (reason, rest) = s.read_chunk(8192).await;
    assert_eq!(
        String::from_utf8_lossy(&rest).trim(),
        "ey,2400,1234567,A1.2"
    );
    assert_eq!(reason & 4, 4, "el final del mensaje debería decir END");
    // It ends in "\n" and we asked with termChar set, so CHR too.
    assert_eq!(reason & 2, 2, "debería haber dicho CHR (reason {reason})");
}

#[tokio::test]
async fn device_clear_throws_away_the_answer_nobody_read() {
    let port = start_one_keithley().await;
    let mut s = Session::open(port, "inst0").await;

    // A query whose answer is never collected: exactly the state a client
    // gets into after a timeout, and what it calls viClear to get out of.
    s.write("*IDN?").await;
    s.clear().await;

    let (_reason, left_over) = s.read_chunk(8192).await;
    assert!(
        left_over.is_empty(),
        "device_clear dejó '{}' esperando",
        String::from_utf8_lossy(&left_over)
    );
}
