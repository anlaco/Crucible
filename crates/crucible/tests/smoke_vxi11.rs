use crucible_core::{Dispositivo, Perfil};
use crucible_vxi11::rpc::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const PERFIL_KEITHLEY: &str = include_str!("../../../perfiles/keithley_2400.yaml");

fn levantar_dispositivo() -> Dispositivo {
    let perfil = Perfil::from_yaml(PERFIL_KEITHLEY).unwrap();
    Dispositivo::from_perfil(perfil).unwrap()
}

async fn levantar_runtime_vxi11() -> u16 {
    let disp = levantar_dispositivo();
    let listener = crucible_vxi11::bind_vxi11("127.0.0.1", 0).await.unwrap();
    let puerto = listener.local_addr().unwrap().port();
    tokio::spawn(crucible_vxi11::aceptar_conexiones_vxi11(listener, disp));
    puerto
}

struct SesionVxi11 {
    stream: TcpStream,
    xid: u32,
    lid: Option<i32>,
}

impl SesionVxi11 {
    async fn conectar(puerto: u16) -> Self {
        let stream = TcpStream::connect(("127.0.0.1", puerto)).await.unwrap();
        let mut s = Self {
            stream,
            xid: 100,
            lid: None,
        };
        let lid = s.create_link("inst0").await;
        s.lid = Some(lid);
        s
    }

    fn next_xid(&mut self) -> u32 {
        self.xid += 1;
        self.xid
    }

    async fn rpc_call(&mut self, prog: u32, vers: u32, proc: u32, payload: &[u8]) -> Vec<u8> {
        let xid = self.next_xid();
        let mut call = Vec::new();
        write_u32(&mut call, xid);
        write_u32(&mut call, MSG_CALL);
        write_u32(&mut call, 2);
        write_u32(&mut call, prog);
        write_u32(&mut call, vers);
        write_u32(&mut call, proc);
        write_auth_null(&mut call);
        write_auth_null(&mut call);
        call.extend_from_slice(payload);
        let rec = encode_record(&call);
        self.stream.write_all(&rec).await.unwrap();
        self.stream.flush().await.unwrap();

        // Leer reply con Record Marking
        let mut hdr = [0u8; 4];
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.stream.read_exact(&mut hdr),
        )
        .await
        .unwrap_or_else(|_| panic!("timeout esperando reply xid {xid}"))
        .unwrap();
        let h = u32::from_be_bytes(hdr);
        let len = (h & 0x7fff_ffff) as usize;
        let mut data = vec![0u8; len];
        self.stream.read_exact(&mut data).await.unwrap();
        // Verificar xid
        let reply_xid = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(reply_xid, xid, "xid mismatch");
        data
    }

    async fn create_link(&mut self, device: &str) -> i32 {
        let mut payload = Vec::new();
        write_u32(&mut payload, 0); // clientId
        write_u32(&mut payload, 0); // lockDevice false
        write_u32(&mut payload, 0); // lock_timeout
        write_string(&mut payload, device);
        let reply = self
            .rpc_call(PROG_VXI11_CORE, 1, PROC_CREATE_LINK, &payload)
            .await;
        let mut pos = 0;
        let _xid = read_u32(&reply, &mut pos).unwrap();
        let _mtype = read_u32(&reply, &mut pos).unwrap();
        let _accepted = read_u32(&reply, &mut pos).unwrap();
        skip_auth(&reply, &mut pos).unwrap();
        let _success = read_u32(&reply, &mut pos).unwrap();
        let err = read_i32(&reply, &mut pos).unwrap();
        assert_eq!(err, 0, "create_link error {err}");
        let lid = read_i32(&reply, &mut pos).unwrap();
        let _abort = read_u32(&reply, &mut pos).unwrap();
        let _max = read_u32(&reply, &mut pos).unwrap();
        lid
    }

    async fn device_write(&mut self, data: &str) {
        let lid = self.lid.unwrap();
        let mut payload = Vec::new();
        write_i32(&mut payload, lid);
        write_u32(&mut payload, 1000); // io_timeout
        write_u32(&mut payload, 1000); // lock_timeout
        write_u32(&mut payload, 8); // flags END
        write_opaque(&mut payload, data.as_bytes());
        let reply = self
            .rpc_call(PROG_VXI11_CORE, 1, PROC_DEVICE_WRITE, &payload)
            .await;
        let mut pos = 0;
        let _xid = read_u32(&reply, &mut pos).unwrap();
        let _m = read_u32(&reply, &mut pos).unwrap();
        let _a = read_u32(&reply, &mut pos).unwrap();
        skip_auth(&reply, &mut pos).unwrap();
        let _s = read_u32(&reply, &mut pos).unwrap();
        let err = read_i32(&reply, &mut pos).unwrap();
        assert_eq!(err, 0, "device_write error {err}");
        let _size = read_u32(&reply, &mut pos).unwrap();
    }

    async fn device_read(&mut self) -> String {
        let lid = self.lid.unwrap();
        let mut payload = Vec::new();
        write_i32(&mut payload, lid);
        write_u32(&mut payload, 8192); // requestSize
        write_u32(&mut payload, 1000);
        write_u32(&mut payload, 1000);
        write_u32(&mut payload, 0);
        write_u32(&mut payload, 10); // termChar \n
        let reply = self
            .rpc_call(PROG_VXI11_CORE, 1, PROC_DEVICE_READ, &payload)
            .await;
        let mut pos = 0;
        let _xid = read_u32(&reply, &mut pos).unwrap();
        let _m = read_u32(&reply, &mut pos).unwrap();
        let _a = read_u32(&reply, &mut pos).unwrap();
        skip_auth(&reply, &mut pos).unwrap();
        let _s = read_u32(&reply, &mut pos).unwrap();
        let err = read_i32(&reply, &mut pos).unwrap();
        assert_eq!(err, 0, "device_read error {err}");
        let _reason = read_u32(&reply, &mut pos).unwrap();
        let data = read_opaque(&reply, &mut pos).unwrap();
        String::from_utf8_lossy(&data).trim().to_string()
    }

    async fn enviar(&mut self, cmd: &str) {
        self.device_write(cmd).await;
    }

    async fn preguntar(&mut self, cmd: &str) -> String {
        self.device_write(cmd).await;
        tokio::time::timeout(std::time::Duration::from_secs(5), self.device_read())
            .await
            .unwrap_or_else(|_| panic!("el dispositivo no contestó a '{cmd}' en 5 s"))
    }
}

#[tokio::test]
async fn vxi11_keithley_completo() {
    let puerto = levantar_runtime_vxi11().await;
    let mut s = SesionVxi11::conectar(puerto).await;

    let idn = s.preguntar("*IDN?").await;
    assert_eq!(idn, "Keithley,2400,1234567,A1.2");

    s.enviar("SOUR:VOLT 5.0").await;
    s.enviar("OUTP ON").await;

    let volt = s.preguntar("MEAS:VOLT?").await;
    let volt_f: f64 = volt.parse().unwrap();
    assert!(
        (volt_f - 5.0).abs() < 0.01,
        "voltaje debe ser ~5.0, fue {}",
        volt_f
    );

    let curr = s.preguntar("MEAS:CURR?").await;
    let curr_f: f64 = curr.parse().unwrap();
    assert!(
        (curr_f - 0.005).abs() < 0.0001,
        "corriente debe ser ~0.005, fue {}",
        curr_f
    );
}

#[tokio::test]
async fn vxi11_scpi_de_verdad() {
    let puerto = levantar_runtime_vxi11().await;
    let mut s = SesionVxi11::conectar(puerto).await;

    s.enviar("source:voltage:level 5.0").await;
    assert_eq!(s.preguntar("SOUR:VOLT?").await, "5.0");

    let r = s.preguntar("*IDN?;:SOUR:VOLT?").await;
    assert_eq!(r, "Keithley,2400,1234567,A1.2;5.0");

    s.enviar("FOO:BAR 1").await;
    let err = s.preguntar("SYST:ERR?").await;
    assert!(err.starts_with("-113"), "cola de errores devolvió '{err}'");

    assert_eq!(s.preguntar("*IDN?").await, "Keithley,2400,1234567,A1.2");
}

#[tokio::test]
async fn vxi11_estado_sobrevive_a_link_nuevo() {
    // En VXI-11 el estado sobrevive aunque abras otro link, porque el Dispositivo es compartido.
    let puerto = levantar_runtime_vxi11().await;

    let mut s = SesionVxi11::conectar(puerto).await;
    s.enviar("SOUR:VOLT 4.5").await;
    s.enviar("OUTP ON").await;
    let volt = s.preguntar("MEAS:VOLT?").await;
    let volt_f: f64 = volt.parse().unwrap();
    assert!(
        (volt_f - 4.5).abs() < 0.05,
        "voltaje debe ser ~4.5, fue {}",
        volt_f
    );
    drop(s);

    let mut s2 = SesionVxi11::conectar(puerto).await;
    let volt = s2.preguntar("MEAS:VOLT?").await;
    let volt_f: f64 = volt.parse().unwrap();
    assert!(
        (volt_f - 4.5).abs() < 0.05,
        "el estado no sobrevivió a reconexión: voltaje debe ser ~4.5, fue {}",
        volt_f
    );
    assert_eq!(s2.preguntar("SOUR:VOLT?").await, "4.5");
}

#[tokio::test]
async fn vxi11_output_off_mide_cero() {
    let puerto = levantar_runtime_vxi11().await;
    let mut s = SesionVxi11::conectar(puerto).await;
    s.enviar("SOUR:VOLT 12.0").await;
    let volt = s.preguntar("MEAS:VOLT?").await;
    let volt_f: f64 = volt.parse().unwrap();
    assert!(
        (volt_f - 0.0).abs() < 0.001,
        "output off debe medir 0, fue {}",
        volt_f
    );
}
