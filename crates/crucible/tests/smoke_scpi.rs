use crucible_core::{Dispositivo, Perfil};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

const PERFIL_KEITHLEY: &str = include_str!("../../../perfiles/keithley_2400.yaml");

async fn levantar_runtime() -> Arc<Mutex<Dispositivo>> {
    let perfil = Perfil::from_yaml(PERFIL_KEITHLEY).unwrap();
    let disp = Dispositivo::from_perfil(perfil);
    Arc::new(Mutex::new(disp))
}

async fn servir_en_puerto(disp: Arc<Mutex<Dispositivo>>, puerto: u16) {
    let listener = TcpListener::bind(("127.0.0.1", puerto)).await.unwrap();
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let disp = disp.clone();
            tokio::spawn(async move {
                let (reader, mut writer) = stream.into_split();
                let mut reader = BufReader::new(reader);
                let mut line = String::new();
                loop {
                    line.clear();
                    if reader.read_line(&mut line).await.unwrap() == 0 {
                        break;
                    }
                    let msg = line.trim_end_matches(|c| c == '\r' || c == '\n');
                    if msg.is_empty() { continue; }
                    let mut d = disp.lock().await;
                    let resp = match d.procesar(msg) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("SERVER ERROR procesando '{}': {}", msg, e);
                            continue;
                        }
                    };
                    if !resp.is_empty() && msg.ends_with('?') {
                        writer.write_all(format!("{}\n", resp).as_bytes()).await.unwrap();
                    }
                }
            });
        }
    });
}

struct Sesion {
    writer: tokio::net::tcp::OwnedWriteHalf,
    reader: tokio::io::BufReader<tokio::net::tcp::OwnedReadHalf>,
}

impl Sesion {
    async fn conectar(puerto: u16) -> Self {
        let stream = TcpStream::connect(("127.0.0.1", puerto)).await.unwrap();
        let (reader, writer) = stream.into_split();
        Self {
            writer,
            reader: BufReader::new(reader),
        }
    }

    async fn enviar(&mut self, cmd: &str) {
        self.writer.write_all(format!("{}\n", cmd).as_bytes()).await.unwrap();
    }

    async fn preguntar(&mut self, cmd: &str) -> String {
        self.enviar(cmd).await;
        let mut buf = String::new();
        self.reader.read_line(&mut buf).await.unwrap();
        buf.trim().to_string()
    }
}

#[tokio::test]
async fn smoke_keithley_completo() {
    let disp = levantar_runtime().await;
    let puerto = 15525u16;
    servir_en_puerto(disp, puerto).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut s = Sesion::conectar(puerto).await;

    let idn = s.preguntar("*IDN?").await;
    assert_eq!(idn, "Keithley,2400,1234567,A1.2");

    s.enviar("SOUR:VOLT 5.0").await;
    s.enviar("OUTP ON").await;

    let volt = s.preguntar("MEAS:VOLT?").await;
    let volt_f: f64 = volt.parse().unwrap();
    assert!((volt_f - 5.0).abs() < 0.01, "voltaje debe ser ~5.0, fue {}", volt_f);

    let curr = s.preguntar("MEAS:CURR?").await;
    let curr_f: f64 = curr.parse().unwrap();
    assert!((curr_f - 0.005).abs() < 0.0001, "corriente debe ser ~0.005, fue {}", curr_f);
}

#[tokio::test]
async fn smoke_output_off_mide_cero() {
    let disp = levantar_runtime().await;
    let puerto = 15526u16;
    servir_en_puerto(disp, puerto).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut s = Sesion::conectar(puerto).await;

    s.enviar("SOUR:VOLT 12.0").await;

    let volt = s.preguntar("MEAS:VOLT?").await;
    let volt_f: f64 = volt.parse().unwrap();
    assert!((volt_f - 0.0).abs() < 0.001, "output off debe medir 0, fue {}", volt_f);
}