//! Binario de InstruSim.
//!
//! Levanta un rack de demostración y lo pone a escuchar. A partir de ahí,
//! cualquier cliente que hable SCPI por TCP puede conectarse: pyvisa, LabVIEW,
//! MATLAB o un simple `netcat`.
//!
//! El escenario está escrito aquí a mano. En la fase 3 se cargará de un fichero
//! TOML y este fichero se quedará solo con el arranque.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::mpsc::channel;
use std::time::Duration;

use instrusim_core::{Terminal, VirtualClock, WallClock};
use instrusim_model::{GenericDcSupply, GenericDmm, Rack};
use instrusim_net::{run_rack, serve_raw};

/// Frecuencia del bucle de simulación.
///
/// Un milisegundo por tic es el compromiso habitual: suficiente para que los
/// tiempos de establecimiento se noten y lo bastante barato como para no
/// calentar un núcleo. Las señales rápidas no dependen de esto, porque se
/// evalúan a la resolución que pida quien las muestree.
const FRECUENCIA_HZ: f64 = 1000.0;

struct Opciones {
    bind: String,
    puerto_base: u16,
    reloj_virtual: bool,
}

impl Default for Opciones {
    fn default() -> Self {
        Self {
            // Por defecto solo se escucha en la máquina local. Abrirse a la red
            // es una decisión consciente, no algo que pase por descuido.
            bind: "127.0.0.1".to_string(),
            puerto_base: 5025,
            reloj_virtual: false,
        }
    }
}

fn main() -> ExitCode {
    let opciones = match analizar_argumentos() {
        Ok(Some(o)) => o,
        Ok(None) => return ExitCode::SUCCESS, // se pidió la ayuda
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!("Pruebe con --help.");
            return ExitCode::FAILURE;
        }
    };

    match arrancar(&opciones) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn arrancar(opciones: &Opciones) -> std::io::Result<()> {
    let reloj: Box<dyn instrusim_core::Clock> = if opciones.reloj_virtual {
        Box::new(VirtualClock::from_hz(FRECUENCIA_HZ))
    } else {
        Box::new(WallClock::from_hz(FRECUENCIA_HZ))
    };

    let mut rack = Rack::new(reloj);

    // --- El escenario -----------------------------------------------------
    //
    // Dos instrumentos y dos nodos. La salida de la fuente y la entrada del
    // multímetro cuelgan del mismo nodo, así que lo que programe uno lo mide el
    // otro. Ninguno de los dos sabe que el otro existe.

    let masa = rack.world_mut().add_node("masa");
    let salida = rack.world_mut().add_node("psu_out");

    let mut psu = GenericDcSupply::generic("PSU0001").with_settling(Duration::from_millis(10));
    psu.wire(salida);

    let mut dmm = GenericDmm::generic("DMM0001");
    dmm.wire(Terminal::wired("HI", salida), Terminal::wired("LO", masa));

    let id_dmm = rack.add(Box::new(dmm));
    let id_psu = rack.add(Box::new(psu));

    // --- Los servidores ---------------------------------------------------

    let (tx, rx) = channel();

    let dir_dmm = serve_raw(
        &formato_direccion(&opciones.bind, opciones.puerto_base),
        id_dmm,
        tx.clone(),
    )?;
    let dir_psu = serve_raw(
        &formato_direccion(&opciones.bind, opciones.puerto_base + 1),
        id_psu,
        tx,
    )?;

    banner(&[
        (dir_dmm, rack.idn(id_dmm).unwrap_or_default()),
        (dir_psu, rack.idn(id_psu).unwrap_or_default()),
    ]);

    // El hilo principal se convierte en el hilo de simulación y no vuelve.
    run_rack(rack, rx);
    Ok(())
}

fn formato_direccion(bind: &str, puerto: u16) -> String {
    // Las direcciones IPv6 literales van entre corchetes.
    if bind.contains(':') && !bind.starts_with('[') {
        format!("[{bind}]:{puerto}")
    } else {
        format!("{bind}:{puerto}")
    }
}

fn banner(instrumentos: &[(SocketAddr, String)]) {
    println!(
        "InstruSim {} — rack simulado en marcha",
        env!("CARGO_PKG_VERSION")
    );
    println!();

    for (dir, idn) in instrumentos {
        println!("  {idn}");
        println!("    VISA    TCPIP0::{}::{}::SOCKET", dir.ip(), dir.port());
        println!("    netcat  nc {} {}", dir.ip(), dir.port());
        println!();
    }

    println!("Pruébelo:");
    if let Some((dir, _)) = instrumentos.first() {
        println!("    printf '*IDN?\\n' | nc -q1 {} {}", dir.ip(), dir.port());
    }
    println!();
    println!("Ctrl-C para parar.");
}

fn analizar_argumentos() -> Result<Option<Opciones>, String> {
    let mut opciones = Opciones::default();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                ayuda();
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("instrusim {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--bind" => {
                opciones.bind = args.next().ok_or("--bind necesita una dirección")?;
            }
            "--port" => {
                let v = args.next().ok_or("--port necesita un número")?;
                opciones.puerto_base = v.parse().map_err(|_| format!("puerto inválido: {v}"))?;
            }
            "--virtual-clock" => opciones.reloj_virtual = true,
            otro => return Err(format!("opción desconocida: {otro}")),
        }
    }

    Ok(Some(opciones))
}

fn ayuda() {
    println!(
        "\
InstruSim — simulador de instrumentación de medida

USO:
    instrusim [OPCIONES]

OPCIONES:
    --bind <DIRECCIÓN>   Interfaz en la que escuchar [por defecto: 127.0.0.1]
                         Use 0.0.0.0 para aceptar conexiones de otras máquinas.
    --port <PUERTO>      Puerto del primer instrumento [por defecto: 5025]
                         El resto van a continuación.
    --virtual-clock      Reloj virtual: la simulación corre tan rápido como
                         pueda en lugar de seguir al reloj de pared. Reproducible
                         al bit, útil para tests.
    -h, --help           Esta ayuda
    -V, --version        Versión

INSTRUMENTOS DEL RACK DE DEMOSTRACIÓN:
    Puerto base      Multímetro GDM-1000   (clase IviDmm)
    Puerto base + 1  Fuente     GPS-3003   (clase IviDCPwr)

    La salida de la fuente y la entrada del multímetro comparten nodo, así que
    lo que programe en una lo mide el otro."
    );
}
