//! Frente de protocolo: el socket raw sobre TCP.
//!
//! Es el mismo transporte que exponen los instrumentos LXI reales en el puerto
//! 5025: texto plano terminado en salto de línea, sin ninguna envoltura. Por eso
//! funciona con cualquier cliente sin que hagamos nada específico para ninguno:
//!
//! ```text
//! VISA    → TCPIP0::127.0.0.1::5025::SOCKET
//! netcat  → nc 127.0.0.1 5025
//! Python  → socket.create_connection(("127.0.0.1", 5025))
//! ```
//!
//! El diseño de concurrencia es el que fija todo el proyecto: **el rack vive en
//! un único hilo y nadie más lo toca**. Cada conexión corre en su propio hilo,
//! pero cuando le llega una línea no la procesa: se la manda al hilo de
//! simulación por un canal y espera la respuesta por otro. No hay estado
//! compartido, así que no hay cerrojos, ni condiciones de carrera, ni la
//! posibilidad de que dos instrumentos vean el mundo en instantes distintos.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Sender, channel};
use std::thread;

use instrusim_model::{InstrumentId, Rack};

/// Una línea recibida por la red, camino del hilo de simulación.
pub struct Request {
    pub instrument: InstrumentId,
    pub line: String,
    /// Por dónde vuelve la respuesta. `None` significa que el mensaje no
    /// contenía consultas y no hay nada que enviar.
    pub reply: Sender<Option<String>>,
}

/// Pone a escuchar un instrumento en una dirección.
///
/// Devuelve la dirección efectiva, que puede no ser la pedida: si se pasa el
/// puerto 0, el sistema elige uno libre. Eso es lo que permite que los tests
/// arranquen servidores de verdad sin pelearse por los puertos.
pub fn serve_raw(
    addr: &str,
    instrument: InstrumentId,
    to_sim: Sender<Request>,
) -> std::io::Result<std::net::SocketAddr> {
    let listener = TcpListener::bind(addr)?;
    let local = listener.local_addr()?;

    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    let tx = to_sim.clone();
                    // Un hilo por conexión. Un instrumento real admite una sola
                    // sesión a la vez, pero aceptar varias es más cómodo para
                    // desarrollar y no cambia el comportamiento observado.
                    thread::spawn(move || {
                        if let Err(e) = atender(s, instrument, tx) {
                            eprintln!("[instrusim] conexión terminada: {e}");
                        }
                    });
                }
                Err(e) => eprintln!("[instrusim] error al aceptar conexión: {e}"),
            }
        }
    });

    Ok(local)
}

/// Bucle de una conexión: leer línea, preguntar al rack, contestar.
fn atender(
    stream: TcpStream,
    instrument: InstrumentId,
    to_sim: Sender<Request>,
) -> std::io::Result<()> {
    // Nagle agrupa envíos pequeños para ahorrar paquetes, lo que aquí solo
    // añade latencia: nuestras respuestas son cortas y se quieren de inmediato.
    stream.set_nodelay(true)?;

    let mut salida = stream.try_clone()?;
    let entrada = BufReader::new(stream);

    for linea in entrada.lines() {
        let linea = linea?;
        // Los clientes terminan con \n o \r\n indistintamente.
        let linea = linea.trim_end_matches('\r');
        if linea.is_empty() {
            continue;
        }

        let (tx, rx) = channel();
        let peticion = Request {
            instrument,
            line: linea.to_string(),
            reply: tx,
        };

        if to_sim.send(peticion).is_err() {
            // El hilo de simulación ha terminado: no hay nada más que hacer.
            break;
        }

        match rx.recv() {
            Ok(Some(respuesta)) => {
                salida.write_all(respuesta.as_bytes())?;
                salida.write_all(b"\n")?;
                salida.flush()?;
            }
            // Sin consulta no se contesta. Es deliberado: un cliente que espere
            // respuesta a un comando que no la tiene debe agotar su tiempo de
            // espera, igual que le pasaría con el instrumento real.
            Ok(None) => {}
            Err(_) => break,
        }
    }

    Ok(())
}

/// Bucle principal del hilo de simulación.
///
/// Alterna entre atender los comandos que hayan llegado y avanzar el reloj. El
/// orden importa: primero se vacía la cola de peticiones y después se hace el
/// tic, de modo que un comando recibido nunca espera más de un periodo de reloj
/// —un milisegundo a 1 kHz— antes de ejecutarse. Es un retardo del mismo orden
/// que el de la propia red y muy por debajo de cualquier tiempo de espera de
/// VISA.
pub fn run_rack(mut rack: Rack, requests: std::sync::mpsc::Receiver<Request>) {
    loop {
        // Todo lo que haya llegado, sin bloquear.
        loop {
            match requests.try_recv() {
                Ok(peticion) => {
                    let respuesta = rack.dispatch(peticion.instrument, &peticion.line);
                    // Que el cliente se haya ido mientras tanto no es un
                    // problema: se descarta la respuesta y se sigue.
                    let _ = peticion.reply.send(respuesta);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                // Todos los servidores han cerrado: no llegarán más comandos.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
            }
        }

        rack.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::time::Duration;

    use instrusim_core::{Terminal, VirtualClock};
    use instrusim_model::{GenericDcSupply, GenericDmm};

    /// Levanta el rack de demostración con sus dos instrumentos escuchando en
    /// puertos que elige el sistema, y devuelve las direcciones.
    fn levantar() -> (std::net::SocketAddr, std::net::SocketAddr) {
        let mut rack = Rack::new(Box::new(VirtualClock::from_hz(1000.0)));

        let masa = rack.world_mut().add_node("masa");
        let salida = rack.world_mut().add_node("psu_out");

        let mut psu = GenericDcSupply::generic("PSU0001");
        psu.wire(salida);
        let mut dmm = GenericDmm::generic("DMM0001");
        dmm.wire(Terminal::wired("HI", salida), Terminal::wired("LO", masa));

        let id_psu = rack.add(Box::new(psu));
        let id_dmm = rack.add(Box::new(dmm));

        let (tx, rx) = channel();
        let dir_psu = serve_raw("127.0.0.1:0", id_psu, tx.clone()).unwrap();
        let dir_dmm = serve_raw("127.0.0.1:0", id_dmm, tx).unwrap();

        thread::spawn(move || run_rack(rack, rx));

        (dir_psu, dir_dmm)
    }

    /// Un cliente mínimo, del estilo del que escribiría cualquiera.
    struct Cliente {
        salida: TcpStream,
        entrada: BufReader<TcpStream>,
    }

    impl Cliente {
        fn conectar(addr: std::net::SocketAddr) -> Self {
            let s = TcpStream::connect(addr).expect("no se pudo conectar");
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            Self {
                entrada: BufReader::new(s.try_clone().unwrap()),
                salida: s,
            }
        }

        fn escribir(&mut self, linea: &str) {
            self.salida.write_all(linea.as_bytes()).unwrap();
            self.salida.write_all(b"\n").unwrap();
            self.salida.flush().unwrap();
        }

        fn preguntar(&mut self, linea: &str) -> String {
            self.escribir(linea);
            let mut r = String::new();
            self.entrada.read_line(&mut r).expect("sin respuesta");
            r.trim_end().to_string()
        }
    }

    #[test]
    fn un_cliente_se_conecta_y_el_instrumento_se_identifica() {
        let (dir_psu, dir_dmm) = levantar();

        let mut psu = Cliente::conectar(dir_psu);
        assert_eq!(psu.preguntar("*IDN?"), "InstruSim,GPS-3003,PSU0001,1.0");

        let mut dmm = Cliente::conectar(dir_dmm);
        assert_eq!(dmm.preguntar("*IDN?"), "InstruSim,GDM-1000,DMM0001,1.0");
    }

    /// La prueba que de verdad importa: dos clientes distintos, por dos
    /// conexiones distintas, hablando con dos instrumentos que se comunican
    /// entre sí a través del mundo simulado.
    #[test]
    fn se_programa_la_fuente_por_un_socket_y_se_mide_por_otro() {
        let (dir_psu, dir_dmm) = levantar();

        let mut psu = Cliente::conectar(dir_psu);
        let mut dmm = Cliente::conectar(dir_dmm);

        psu.escribir("*RST");
        psu.escribir("VOLT 3.3");
        psu.escribir("OUTP ON");

        // Esperar a que la fuente establezca. El reloj es virtual y corre muy
        // por encima del tiempo real, así que basta con muy poco.
        std::thread::sleep(Duration::from_millis(50));

        let lectura: f64 = dmm.preguntar("MEAS:VOLT:DC?").parse().unwrap();
        assert!((lectura - 3.3).abs() < 5e-3, "el multímetro leyó {lectura}");
    }

    #[test]
    fn varias_consultas_en_una_linea_se_contestan_juntas() {
        let (dir_psu, _) = levantar();
        let mut psu = Cliente::conectar(dir_psu);

        let r = psu.preguntar("VOLT 5;:VOLT?;:OUTP?");
        assert_eq!(r, "+5.000000E+00;0");
    }

    #[test]
    fn un_comando_desconocido_se_recupera_por_la_cola_de_errores() {
        let (dir_psu, _) = levantar();
        let mut psu = Cliente::conectar(dir_psu);

        psu.escribir("*CLS");
        psu.escribir("ESTO:NO:EXISTE");

        assert_eq!(
            psu.preguntar("SYST:ERR?"),
            "-113,\"Undefined header;ESTO:NO:EXISTE\""
        );
    }

    #[test]
    fn el_instrumento_sigue_atendiendo_despues_de_un_error() {
        let (dir_psu, _) = levantar();
        let mut psu = Cliente::conectar(dir_psu);

        psu.escribir("COMANDO:INVENTADO");
        assert_eq!(psu.preguntar("*IDN?"), "InstruSim,GPS-3003,PSU0001,1.0");
    }

    /// Dos conexiones ven el mismo instrumento y el mismo estado.
    ///
    /// La consulta encadenada al final del mensaje de A no es un adorno: es lo
    /// que garantiza que su comando ya se ha ejecutado antes de que B pregunte.
    /// Sin ella habría una carrera de verdad, porque los dos clientes escriben
    /// desde hilos distintos y nada obliga a que el envío de A llegue al hilo de
    /// simulación antes que el de B. Contra un instrumento real pasaría igual;
    /// la forma de sincronizar es esperar respuesta, no confiar en el orden.
    #[test]
    fn varias_conexiones_simultaneas_al_mismo_instrumento() {
        let (dir_psu, _) = levantar();

        let mut a = Cliente::conectar(dir_psu);
        let mut b = Cliente::conectar(dir_psu);

        assert_eq!(a.preguntar("VOLT 7;:VOLT?"), "+7.000000E+00");
        assert_eq!(b.preguntar("VOLT?"), "+7.000000E+00");
    }

    #[test]
    fn los_terminadores_de_windows_tambien_valen() {
        let (dir_psu, _) = levantar();
        let mut psu = Cliente::conectar(dir_psu);

        psu.salida.write_all(b"*IDN?\r\n").unwrap();
        psu.salida.flush().unwrap();

        let mut r = String::new();
        psu.entrada.read_line(&mut r).unwrap();
        assert_eq!(r.trim_end(), "InstruSim,GPS-3003,PSU0001,1.0");
    }
}
