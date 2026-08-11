use crucible_core::{Dispositivo, Perfil};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

const PERFIL_KEITHLEY: &str = include_str!("../../../perfiles/keithley_2400.yaml");

async fn levantar_runtime() -> Arc<Mutex<Dispositivo>> {
    let perfil = Perfil::from_yaml(PERFIL_KEITHLEY).unwrap();
    let disp = Dispositivo::from_perfil(perfil).unwrap();
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
                    let msg = line.trim_end_matches(['\r', '\n']);
                    if msg.is_empty() {
                        continue;
                    }
                    let mut d = disp.lock().await;
                    // El motor dice si hubo consulta; no se adivina por el '?'
                    // final, que falla con los mensajes compuestos.
                    if let Some(resp) = d.procesar(msg) {
                        writer
                            .write_all(format!("{resp}\n").as_bytes())
                            .await
                            .unwrap();
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
        self.writer
            .write_all(format!("{}\n", cmd).as_bytes())
            .await
            .unwrap();
    }

    /// Envía una consulta y espera la respuesta, con un tope de paciencia.
    ///
    /// El tope no es decorativo: si el dispositivo deja de contestar a algo que
    /// debería contestar, sin él el test se cuelga para siempre y bloquea CI en
    /// vez de fallar. Pasó durante la unificación del SCPI y costó más tiempo
    /// diagnosticarlo que arreglar el bug.
    async fn preguntar(&mut self, cmd: &str) -> String {
        self.enviar(cmd).await;
        let mut buf = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.reader.read_line(&mut buf),
        )
        .await
        .unwrap_or_else(|_| panic!("el dispositivo no contestó a '{cmd}' en 5 s"))
        .unwrap();
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

/// Lo que el codec anterior no podía hacer, contra un socket de verdad.
///
/// Tres cosas a la vez: la cabecera escrita en forma larga y en minúsculas, un
/// mensaje compuesto con varias consultas cuyas respuestas vuelven unidas por
/// punto y coma, y un comando desconocido que se anota en la cola de errores en
/// lugar de tumbar la conversación.
#[tokio::test]
async fn smoke_scpi_de_verdad() {
    let disp = levantar_runtime().await;
    let puerto = 15527u16;
    servir_en_puerto(disp, puerto).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut s = Sesion::conectar(puerto).await;

    // Forma larga y minúsculas: el codec anterior solo aceptaba la forma exacta
    // que estuviera escrita en el perfil.
    s.enviar("source:voltage:level 5.0").await;
    assert_eq!(s.preguntar("SOUR:VOLT?").await, "5.0");

    // Mensaje compuesto: dos consultas, dos respuestas unidas por ';'.
    let r = s.preguntar("*IDN?;:SOUR:VOLT?").await;
    assert_eq!(r, "Keithley,2400,1234567,A1.2;5.0");

    // Un comando que no existe no rompe la sesión: va a la cola de errores.
    s.enviar("FOO:BAR 1").await;
    let err = s.preguntar("SYST:ERR?").await;
    assert!(err.starts_with("-113"), "cola de errores devolvió '{err}'");

    // Y la sesión sigue viva después del error.
    assert_eq!(s.preguntar("*IDN?").await, "Keithley,2400,1234567,A1.2");
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
    assert!(
        (volt_f - 0.0).abs() < 0.001,
        "output off debe medir 0, fue {}",
        volt_f
    );
}
