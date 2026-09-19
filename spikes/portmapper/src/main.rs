//! Spike de la Fase 0: ¿descubre NI-MAX un instrumento en la propia máquina?
//!
//! **Esto es código desechable.** No implementa VXI-11 ni pretende hacerlo:
//! solo responde al portmapper lo justo para averiguar una cosa que decide el
//! coste de toda una fase del plan —si el broadcast de descubrimiento de
//! NI-MAX llega a un proceso local—, y para verlo **antes** de escribir el
//! VXI-11 de verdad. Ver `docs/bancos/plan-banco-rf.md` §5.1.
//!
//! NI-MAX busca instrumentos de red mandando un **broadcast UDP al puerto
//! 111**, el portmapper de ONC RPC, con una llamada `GETPORT` que pregunta por
//! el programa **395183** (el canal core de VXI-11). Si alguien contesta con un
//! puerto, abre ahí una conexión TCP y empieza a hablar.
//!
//! El spike hace tres cosas y las cuenta todas por consola:
//!
//! 1. Escucha en UDP 111 y **vuelca cuanto llegue**, lo entienda o no.
//! 2. Si reconoce un `GETPORT`, contesta con el puerto de abajo.
//! 3. Escucha en ese puerto TCP para ver **si NI-MAX llega a conectarse**.
//!
//! Con eso se responde de una vez: si NI-MAX pregunta, qué pregunta, y si el
//! diálogo continúa.
//!
//! ```text
//! spike-portmapper              escucha en todas las interfaces, puerto 111
//! spike-portmapper 127.0.0.1    solo loopback, para comparar
//! spike-portmapper 0.0.0.0:1111 otro puerto, para probar sin privilegios
//! ```

use std::io::Read;
use std::net::{TcpListener, UdpSocket};
use std::thread;

/// Puerto que anunciamos como canal core de VXI-11. El número da igual: lo que
/// importa es si alguien viene a llamar.
const PUERTO_ANUNCIADO: u16 = 9099;

/// Los números del estándar que hay que reconocer.
const PROG_PORTMAP: u32 = 100_000;
const PROG_VXI11_CORE: u32 = 395_183;
const PROC_NULL: u32 = 0;
const PROC_GETPORT: u32 = 3;
const PROC_DUMP: u32 = 4;
const MSG_CALL: u32 = 0;

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_else(|| "0.0.0.0".into());
    // Admite "IP" o "IP:puerto". El puerto solo se cambia para poder probar el
    // spike sin privilegios; contra NI-MAX tiene que ser el 111.
    let (bind, puerto) = match arg.rsplit_once(':') {
        Some((ip, p)) => (ip.to_string(), p.parse().unwrap_or(111)),
        None => (arg, 111u16),
    };

    println!("== spike de descubrimiento ==");
    println!();
    println!("Escuchando UDP {bind}:{puerto} (portmapper de ONC RPC)");
    println!("Anunciaré el canal core de VXI-11 en el puerto {PUERTO_ANUNCIADO}");
    println!();
    println!("Ahora, en NI-MAX: Devices and Interfaces → Network Devices →");
    println!("  Add Network Device / Find Network Instruments.");
    println!();
    println!("Si aquí no aparece NADA, el broadcast no llega a esta máquina y");
    println!("el descubrimiento no es viable en esta topología. Eso ya es un");
    println!("resultado: mejor saberlo hoy.");
    println!();

    arrancar_escucha_tcp();

    let socket = match UdpSocket::bind((bind.as_str(), puerto)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("no puedo escuchar en {bind}:{puerto} — {e}");
            eprintln!();
            eprintln!("Causas típicas:");
            eprintln!("  · hace falta ser administrador para puertos bajos;");
            eprintln!("  · el firewall de Windows no ha dado permiso todavía");
            eprintln!("    (suele salir un aviso la primera vez: hay que aceptarlo);");
            eprintln!("  · ya hay algo escuchando ahí.");
            std::process::exit(1);
        }
    };
    // Sin esto no se reciben los datagramas de difusión en algunos sistemas.
    let _ = socket.set_broadcast(true);

    let mut buf = [0u8; 2048];
    let mut n_datagramas = 0usize;

    loop {
        let (n, origen) = match socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error al recibir: {e}");
                continue;
            }
        };
        n_datagramas += 1;

        println!("--- datagrama #{n_datagramas} · {n} bytes desde {origen} ---");
        volcar_hex(&buf[..n]);

        match interpretar(&buf[..n]) {
            Some(llamada) => {
                println!("  {}", llamada.describir());
                if let Some(respuesta) = llamada.respuesta() {
                    match socket.send_to(&respuesta.bytes, origen) {
                        Ok(_) => println!("  → contestado: {}", respuesta.resumen),
                        Err(e) => println!("  → no pude contestar: {e}"),
                    }
                }
            }
            None => println!("  (no es una llamada RPC que sepa leer)"),
        }
        println!();
    }
}

/// Deja un hilo escuchando el puerto que anunciamos, para ver si el diálogo
/// continúa después del portmapper. Si aquí entra algo, el camino completo
/// funciona y la Fase 2 del plan es viable.
fn arrancar_escucha_tcp() {
    let listener = match TcpListener::bind(("0.0.0.0", PUERTO_ANUNCIADO)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("aviso: no puedo escuchar TCP en {PUERTO_ANUNCIADO} ({e});");
            eprintln!("       el spike sigue, pero no verá si NI-MAX se conecta.");
            return;
        }
    };

    thread::spawn(move || {
        for flujo in listener.incoming().flatten() {
            let peer = flujo
                .peer_addr()
                .map(|a| a.to_string())
                .unwrap_or_else(|_| "?".into());
            println!();
            println!("*** ¡CONEXIÓN TCP desde {peer} en el puerto {PUERTO_ANUNCIADO}! ***");
            println!("    El descubrimiento funciona de punta a punta.");

            let mut flujo = flujo;
            let mut buf = [0u8; 1024];
            if let Ok(n) = flujo.read(&mut buf)
                && n > 0
            {
                println!("    Primeros {n} bytes de la conversación:");
                volcar_hex(&buf[..n]);
            }
            println!();
        }
    });
}

/// Lo que se manda de vuelta, con su explicación en claro. Las dos cosas
/// juntas a propósito: en una herramienta de diagnóstico, un mensaje que no
/// describa exactamente lo que se envió es peor que no tener mensaje.
struct Respuesta {
    bytes: Vec<u8>,
    resumen: String,
}

/// Una llamada RPC, ya troceada.
struct Llamada {
    xid: u32,
    prog: u32,
    proc_: u32,
    /// Argumentos de `GETPORT`, cuando los hay: programa y versión que se piden.
    pedido: Option<(u32, u32)>,
}

impl Llamada {
    fn describir(&self) -> String {
        let nombre_proc = match self.proc_ {
            PROC_NULL => "NULL (¿estás ahí?)",
            PROC_GETPORT => "GETPORT (¿en qué puerto está este programa?)",
            PROC_DUMP => "DUMP (lista de programas registrados)",
            _ => "desconocido",
        };

        if self.prog != PROG_PORTMAP {
            return format!(
                "llamada RPC al programa {} — no es el portmapper, ignorada",
                self.prog
            );
        }

        match self.pedido {
            Some((PROG_VXI11_CORE, vers)) => format!(
                "¡ES LO QUE BUSCÁBAMOS! portmapper · {nombre_proc}\n  \
                 Preguntan por el programa 395183 (canal core de VXI-11), versión {vers}"
            ),
            Some((prog, vers)) => format!(
                "portmapper · {nombre_proc}\n  Preguntan por el programa {prog}, versión {vers} \
                 (no es VXI-11)"
            ),
            None => format!("portmapper · {nombre_proc}"),
        }
    }

    /// La respuesta a mandar, si procede contestar.
    fn respuesta(&self) -> Option<Respuesta> {
        if self.prog != PROG_PORTMAP {
            return None;
        }
        match self.proc_ {
            // Al NULL se contesta con un aceptado vacío.
            PROC_NULL => Some(Respuesta {
                bytes: self.cabecera_aceptada(),
                resumen: "aquí estoy".into(),
            }),
            PROC_GETPORT => {
                let mut bytes = self.cabecera_aceptada();
                // Solo admitimos el programa que nos interesa; para el resto,
                // el puerto 0 significa «no lo tengo», que es lo correcto.
                let (puerto, resumen) = match self.pedido {
                    Some((PROG_VXI11_CORE, _)) => (
                        u32::from(PUERTO_ANUNCIADO),
                        format!("VXI-11 está en el puerto {PUERTO_ANUNCIADO}"),
                    ),
                    _ => (0, "puerto 0 — ese programa no lo tengo".to_string()),
                };
                bytes.extend_from_slice(&puerto.to_be_bytes());
                Some(Respuesta { bytes, resumen })
            }
            _ => None,
        }
    }

    /// Cabecera de una respuesta aceptada: mismo xid, sin autenticación y sin
    /// error. Todo en big-endian y alineado a 4 bytes, que es como manda XDR.
    fn cabecera_aceptada(&self) -> Vec<u8> {
        let mut r = Vec::with_capacity(24);
        r.extend_from_slice(&self.xid.to_be_bytes());
        r.extend_from_slice(&1u32.to_be_bytes()); // REPLY
        r.extend_from_slice(&0u32.to_be_bytes()); // MSG_ACCEPTED
        r.extend_from_slice(&0u32.to_be_bytes()); // verificador: AUTH_NULL
        r.extend_from_slice(&0u32.to_be_bytes()); // …de longitud cero
        r.extend_from_slice(&0u32.to_be_bytes()); // SUCCESS
        r
    }
}

fn interpretar(d: &[u8]) -> Option<Llamada> {
    let leer = |i: usize| -> Option<u32> {
        let b = d.get(i..i + 4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };

    let xid = leer(0)?;
    if leer(4)? != MSG_CALL || leer(8)? != 2 {
        return None; // no es una llamada RPC versión 2
    }
    let prog = leer(12)?;
    let proc_ = leer(20)?;

    // Las credenciales y el verificador son de longitud variable, así que hay
    // que saltarlos para llegar a los argumentos. Cada uno es {tipo, longitud,
    // datos}, con los datos rellenados hasta múltiplo de 4.
    let mut i = 24;
    for _ in 0..2 {
        let longitud = leer(i + 4)? as usize;
        i += 8 + longitud.div_ceil(4) * 4;
    }

    // En GETPORT los argumentos son {programa, versión, protocolo, puerto}.
    let pedido = match (leer(i), leer(i + 4)) {
        (Some(p), Some(v)) if proc_ == PROC_GETPORT => Some((p, v)),
        _ => None,
    };

    Some(Llamada {
        xid,
        prog,
        proc_,
        pedido,
    })
}

/// Vuelca los bytes en hexadecimal con su texto al lado, para poder comparar
/// con una captura de Wireshark si hiciera falta.
fn volcar_hex(d: &[u8]) {
    for (n, trozo) in d.chunks(16).enumerate() {
        let hex: Vec<String> = trozo.iter().map(|b| format!("{b:02x}")).collect();
        let texto: String = trozo
            .iter()
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("  {:04x}  {:-47}  {}", n * 16, hex.join(" "), texto);
    }
}
