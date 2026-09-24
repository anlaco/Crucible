use crucible::{aceptar_conexiones, bind_tcp};
use crucible_core::{Banco, Dispositivo, Perfil, Transporte};
use crucible_vxi11::DeviceMap;
use std::path::{Path, PathBuf};

/// The well known UDP port of the ONC RPC portmapper. It is not configurable
/// because VISA clients only ever look there.
const PORTMAPPER_PORT: u16 = 111;

const AYUDA: &str = "\
Crucible — simulador de bancos de instrumentos SCPI por red

USO
  crucible                         arranca el banco por defecto (ver abajo)
  crucible <banco.yaml>            arranca todos los instrumentos del banco
  crucible <perfil.yaml> [puerto]  arranca un solo instrumento (puerto 5025)
  crucible --validar <fichero>     comprueba un banco o perfil sin arrancarlo
  crucible --version

BANCO POR DEFECTO (sin argumentos), el primero que exista de:
  1. banco.yaml en el directorio actual
  2. banco/banco.yaml junto al ejecutable            (instalación Windows)
  3. ../share/crucible/banco/banco.yaml               (instalación Linux)

Cada instrumento tiene su dirección. Desde VISA:
  TCPIP0::127.0.0.1::<puerto>::SOCKET  (transporte tcp)
  TCPIP0::127.0.0.2::inst0::INSTR      (transporte vxi11)

Como en un banco real, cada instrumento puede tener su propia IP: en Linux
todo 127.x.x.x es local y no hay que configurar nada; en Windows solo
responde 127.0.0.1, y ahí se distinguen por el puerto.

Los instrumentos vxi11 necesitan además el portmapper en UDP 111 de su IP,
que es un puerto privilegiado: en Linux hace falta sudo o setcap.

Manual: https://anlaco.github.io/Crucible/";

enum Orden {
    Arrancar(PathBuf, Option<u16>),
    Validar(PathBuf),
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Sin argumentos es, casi siempre, un doble clic en Windows: si algo falla,
    // la ventana se cerraría antes de que nadie leyera el motivo.
    let doble_clic = args.is_empty();

    if let Err(e) = ejecutar(args).await {
        eprintln!("\nerror: {e:#}");
        if doble_clic {
            eprintln!("\nPulsa Enter para cerrar.");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
        std::process::exit(1);
    }
}

async fn ejecutar(args: Vec<String>) -> anyhow::Result<()> {
    let orden = match args.first().map(String::as_str) {
        None => Orden::Arrancar(buscar_banco_por_defecto()?, None),
        Some("-h" | "--help" | "--ayuda") => {
            println!("{AYUDA}");
            return Ok(());
        }
        Some("-V" | "--version") => {
            println!("crucible {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--validar") => {
            let f = args
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("falta el fichero: crucible --validar <fichero>"))?;
            Orden::Validar(PathBuf::from(f))
        }
        // Compatibilidad con la forma anterior; el tipo ya se detecta solo.
        Some("--banco") => {
            let f = args.get(1).ok_or_else(|| {
                anyhow::anyhow!("falta el fichero: crucible --banco <banco.yaml>")
            })?;
            Orden::Arrancar(PathBuf::from(f), None)
        }
        Some(f) => {
            let puerto = match args.get(1) {
                Some(p) => Some(
                    p.parse::<u16>()
                        .map_err(|_| anyhow::anyhow!("'{p}' no es un puerto válido"))?,
                ),
                None => None,
            };
            Orden::Arrancar(PathBuf::from(f), puerto)
        }
    };

    match orden {
        Orden::Validar(path) => {
            let instrumentos = cargar(&path, None)?;
            println!("{} es válido:", path.display());
            imprimir_tabla(&instrumentos);
            Ok(())
        }
        Orden::Arrancar(path, puerto) => {
            let instrumentos = cargar(&path, puerto)?;
            arrancar(&path, instrumentos).await
        }
    }
}

/// Un instrumento listo para servirse.
struct Instrumento {
    id: String,
    transporte: Transporte,
    disp: Dispositivo,
}

/// Carga un banco o un perfil suelto, según lo que contenga el fichero.
///
/// Se distingue por el contenido y no por una opción: quien tiene un YAML en
/// la mano no debería tener que recordar qué bandera le corresponde.
fn cargar(path: &Path, puerto: Option<u16>) -> anyhow::Result<Vec<Instrumento>> {
    let texto = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("no puedo leer {}: {e}", path.display()))?;
    let valor: serde_yaml::Value = serde_yaml::from_str(&texto)
        .map_err(|e| anyhow::anyhow!("{} no es YAML válido: {e}", path.display()))?;

    let instrumentos = if valor.get("banco").is_some() {
        if puerto.is_some() {
            anyhow::bail!("el puerto se indica en el banco, no en la línea de órdenes");
        }
        cargar_banco(path, &texto)?
    } else if valor.get("dispositivo").is_some() {
        let perfil =
            Perfil::from_yaml(&texto).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        let id = perfil.dispositivo.modelo.clone();
        vec![Instrumento {
            id,
            transporte: Transporte::Tcp {
                host: Some("127.0.0.1".into()),
                puerto: Some(puerto.unwrap_or(5025)),
            },
            disp: Dispositivo::from_perfil(perfil)?,
        }]
    } else {
        anyhow::bail!(
            "{} no parece ni un banco (clave 'banco:') ni un perfil (clave 'dispositivo:')",
            path.display()
        );
    };

    // Both checks also run from `--validar`, so a bad bank is caught before
    // anybody tries to start it.
    vxi11_endpoints(&instrumentos)?;
    comprobar_puertos_unicos(&instrumentos)?;
    Ok(instrumentos)
}

fn cargar_banco(path: &Path, texto: &str) -> anyhow::Result<Vec<Instrumento>> {
    let banco = Banco::from_yaml(texto).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    let base = path.parent().unwrap_or(Path::new("."));
    // Un perfil roto tumba el banco entero: arrancar a medias daría
    // resultados falsos en vez de un fallo visible.
    let dispositivos = banco.cargar_dispositivos(base)?;
    if dispositivos.is_empty() {
        anyhow::bail!("el banco no declara ningún dispositivo");
    }

    let mut out = Vec::new();
    for (inst, disp) in dispositivos {
        if inst.transporte.puerto().is_none() {
            anyhow::bail!("dispositivo '{}': falta 'transporte.puerto'", inst.id);
        }
        out.push(Instrumento {
            id: inst.id,
            transporte: inst.transporte,
            disp,
        });
    }
    Ok(out)
}

/// One VXI-11 core channel: the address it listens on, and which instruments
/// it serves.
///
/// There is one of these per IP address, exactly like a real bench, where
/// every instrument is its own box with its own IP. Instruments that share an
/// IP are told apart by their `device` name, which is what a VXI chassis or a
/// LAN/GPIB gateway does.
#[derive(Debug)]
struct Vxi11Endpoint {
    host: String,
    port: u16,
}

/// Works out one endpoint per IP address, and checks each of them could
/// actually be reached by a VISA client.
///
/// Returns an empty list when the bank has no VXI-11 instrument at all.
fn vxi11_endpoints(instrumentos: &[Instrumento]) -> anyhow::Result<Vec<Vxi11Endpoint>> {
    // Collect the VXI-11 instruments, leaving the plain TCP ones alone.
    let mut vxi11: Vec<&Instrumento> = Vec::new();
    for inst in instrumentos {
        if inst.transporte.is_vxi11() {
            vxi11.push(inst);
        }
    }

    let mut endpoints: Vec<Vxi11Endpoint> = Vec::new();
    for inst in &vxi11 {
        let host = inst.transporte.host_or_default().to_string();
        let port = inst.transporte.puerto().unwrap_or(0);

        // Have we seen this IP already?
        let ya_visto = endpoints.iter().find(|e| e.host == host);
        match ya_visto {
            None => endpoints.push(Vxi11Endpoint { host, port }),
            Some(anterior) => {
                // A resource string like TCPIP0::127.0.0.1::inst0::INSTR has
                // no room for a port: the client asks the portmapper of that
                // IP for one port and then names the instrument it wants. So
                // one IP means one port, and two ports on the same IP would
                // leave one of the instruments unreachable.
                if anterior.port != port {
                    let primero = vxi11
                        .iter()
                        .find(|o| o.transporte.host_or_default() == anterior.host)
                        .unwrap();
                    anyhow::bail!(
                        "'{}' y '{}' comparten la IP {} pero piden puertos distintos ({} y {}).\n\n\
                         Un recurso TCPIP0::...::INSTR no lleva puerto, así que en una misma IP \
                         solo cabe un puerto vxi11 y los instrumentos se distinguen por su \
                         'device'. Dales IPs distintas, o el mismo 'transporte.puerto'.",
                        primero.id,
                        inst.id,
                        anterior.host,
                        anterior.port,
                        port
                    );
                }
            }
        }
    }

    // Within one IP, two instruments answering to the same device name would
    // mean one of them is unreachable and the other answers for both.
    for (i, a) in vxi11.iter().enumerate() {
        for b in &vxi11[i + 1..] {
            let misma_ip = a.transporte.host_or_default() == b.transporte.host_or_default();
            let mismo_device = a.transporte.device_name() == b.transporte.device_name();
            if misma_ip && mismo_device {
                anyhow::bail!(
                    "'{}' y '{}' comparten la IP {} y el device '{}'. Dales IPs distintas \
                     (en un banco real cada equipo tiene la suya y todos son 'inst0'), o \
                     nombres de device distintos si quieren compartir IP.",
                    a.id,
                    b.id,
                    a.transporte.host_or_default(),
                    a.transporte.device_name()
                );
            }
        }
    }

    Ok(endpoints)
}

/// Dos instrumentos en la misma dirección: el segundo no podría escuchar.
/// Mejor decirlo con sus nombres que dejar que el sistema diga «address in use».
///
/// Two VXI-11 instruments are allowed to share an address, because there they
/// are told apart by their device name; that case is checked in
/// `vxi11_endpoints` instead.
fn comprobar_puertos_unicos(instrumentos: &[Instrumento]) -> anyhow::Result<()> {
    for (i, a) in instrumentos.iter().enumerate() {
        for b in &instrumentos[i + 1..] {
            if a.transporte.is_vxi11() && b.transporte.is_vxi11() {
                continue;
            }
            let misma_direccion = a.transporte.puerto() == b.transporte.puerto()
                && a.transporte.host_or_default() == b.transporte.host_or_default();
            if misma_direccion {
                anyhow::bail!(
                    "'{}' y '{}' usan los dos {}:{}; cada instrumento necesita su puerto",
                    a.id,
                    b.id,
                    a.transporte.host_or_default(),
                    a.transporte.puerto().unwrap_or(0)
                );
            }
        }
    }
    Ok(())
}

/// An instrument that is ready to be served.
enum Listo {
    /// A `tipo: tcp` instrument, with the listener bound for it.
    Tcp {
        inst: Instrumento,
        listener: tokio::net::TcpListener,
    },
    /// A `tipo: vxi11` instrument. It carries no listener of its own: the
    /// core channel of its IP serves it, and every instrument on that IP.
    Vxi11 { inst: Instrumento },
}

impl Listo {
    fn instrumento(&self) -> &Instrumento {
        match self {
            Listo::Tcp { inst, .. } => inst,
            Listo::Vxi11 { inst } => inst,
        }
    }
}

/// One VXI-11 IP address with everything it needs to serve: its core channel,
/// its portmapper, and the instruments that live on it.
struct Vxi11Host {
    endpoint: Vxi11Endpoint,
    listener: tokio::net::TcpListener,
    portmapper: tokio::net::UdpSocket,
    devices: DeviceMap,
}

async fn arrancar(path: &Path, instrumentos: Vec<Instrumento>) -> anyhow::Result<()> {
    let endpoints = vxi11_endpoints(&instrumentos)?;

    // Todo se enlaza antes de servir nada: si un puerto está ocupado, el banco
    // no arranca. Un banco al que le falta un instrumento no está degradado,
    // da resultados falsos.
    let mut listos: Vec<Listo> = Vec::new();
    for inst in instrumentos {
        if inst.transporte.is_vxi11() {
            listos.push(Listo::Vxi11 { inst });
            continue;
        }
        let host = inst.transporte.host_or_default().to_string();
        let puerto = inst.transporte.puerto().unwrap_or(0);
        let listener = bind_tcp(&host, puerto).await.map_err(|e| {
            anyhow::anyhow!("{}", cannot_listen_message(&inst.id, &host, puerto, &e))
        })?;
        listos.push(Listo::Tcp { inst, listener });
    }

    // One core channel and one portmapper per VXI-11 IP, bound here, in the
    // same breath as the TCP listeners.
    let mut vxi11_hosts: Vec<Vxi11Host> = Vec::new();
    for endpoint in endpoints {
        let listener = crucible_vxi11::bind_vxi11(&endpoint.host, endpoint.port)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "{}",
                    cannot_listen_message("el canal VXI-11", &endpoint.host, endpoint.port, &e)
                )
            })?;
        // The portmapper is not an extra: without it the ::INSTR resources we
        // are about to print do not resolve from any VISA client. Announcing
        // them and then not serving them would be worse than not starting.
        let portmapper = crucible_vxi11::bind_portmapper(&endpoint.host, PORTMAPPER_PORT)
            .await
            .map_err(|e| anyhow::anyhow!("{}", portmapper_failure_message(&endpoint.host, &e)))?;
        vxi11_hosts.push(Vxi11Host {
            endpoint,
            listener,
            portmapper,
            devices: DeviceMap::new(),
        });
    }

    println!(
        "Crucible {} — {}",
        env!("CARGO_PKG_VERSION"),
        path.display()
    );
    let refs: Vec<&Instrumento> = listos.iter().map(|l| l.instrumento()).collect();
    imprimir_tabla(&refs);
    for host in &vxi11_hosts {
        println!(
            "\n  vxi11 en {}:{}, anunciado por el portmapper en UDP {} de esa IP.",
            host.endpoint.host, host.endpoint.port, PORTMAPPER_PORT
        );
    }
    println!("\nListo. Ctrl+C para parar.");

    let mut tareas = Vec::new();

    // Each instrument goes where it belongs: a TCP one to its own task, a
    // VXI-11 one into the device map of its IP.
    for listo in listos {
        match listo {
            Listo::Tcp { inst, listener } => {
                let id = inst.id.clone();
                tareas.push(tokio::spawn(async move {
                    if let Err(e) = aceptar_conexiones(listener, inst.disp).await {
                        eprintln!("error en '{id}': {e}");
                    }
                }));
            }
            Listo::Vxi11 { inst } => {
                let host = inst.transporte.host_or_default().to_string();
                let destino = vxi11_hosts
                    .iter_mut()
                    .find(|h| h.endpoint.host == host)
                    // vxi11_endpoints() built one entry per IP from this very
                    // list, so the IP is always there.
                    .expect("toda IP vxi11 tiene su canal");
                destino
                    .devices
                    .add(inst.transporte.device_name(), inst.disp);
            }
        }
    }

    for host in vxi11_hosts {
        let direccion = format!("{}:{}", host.endpoint.host, host.endpoint.port);
        let puerto = host.endpoint.port;
        tareas.push(tokio::spawn(async move {
            if let Err(e) = crucible_vxi11::serve_connections(host.listener, host.devices).await {
                eprintln!("error en el canal VXI-11 de {direccion}: {e}");
            }
        }));
        tareas.push(tokio::spawn(async move {
            if let Err(e) = crucible_vxi11::serve_portmapper(host.portmapper, puerto).await {
                eprintln!("error en el portmapper: {e}");
            }
        }));
    }

    tokio::select! {
        _ = tokio::signal::ctrl_c() => println!("\nParado."),
        _ = alguno_termina(tareas) => anyhow::bail!("un instrumento dejó de aceptar conexiones"),
    }
    Ok(())
}

/// The same wording for any socket that will not open, because the cause and
/// the remedy are always the same.
fn cannot_listen_message(quien: &str, host: &str, puerto: u16, e: &anyhow::Error) -> String {
    format!(
        "'{quien}' no puede escuchar en {host}:{puerto} ({e}). ¿Hay otro programa, u otro \
         Crucible, usando ese puerto?{}",
        loopback_ip_hint(host)
    )
}

fn portmapper_failure_message(host: &str, e: &anyhow::Error) -> String {
    format!(
        "no puedo abrir el portmapper en {host}:{PORTMAPPER_PORT} ({e}).\n\n\
         VXI-11 lo necesita: un recurso TCPIP0::…::INSTR no lleva puerto, así que el cliente \
         pregunta al portmapper de esa IP por dónde escucha el instrumento. Sin él, no lo \
         encuentra nadie.\n\n\
         El puerto {PORTMAPPER_PORT} es privilegiado. En Linux, dale el permiso una vez:\n\
         \x20 sudo setcap 'cap_net_bind_service=+ep' <ruta del ejecutable>\n\
         o arranca con sudo. Comprueba también que no lo tenga ya rpcbind:\n\
         \x20 sudo ss -lunp | grep :{PORTMAPPER_PORT}{}",
        // Only when the address itself could be the problem: on a plain
        // permission error the hint is noise that hides the real remedy.
        if is_permission_denied(e) {
            String::new()
        } else {
            loopback_ip_hint(host)
        }
    )
}

/// True when the socket could not be opened because of permissions, which on
/// UDP 111 is the usual case and has nothing to do with the address.
fn is_permission_denied(e: &anyhow::Error) -> bool {
    match e.downcast_ref::<std::io::Error>() {
        Some(io) => io.kind() == std::io::ErrorKind::PermissionDenied,
        None => false,
    }
}

/// On Linux every 127.x.x.x address is local and works with no setting up at
/// all; on Windows only 127.0.0.1 answers. That is the likeliest reason a
/// perfectly reasonable bench IP will not open, so we say so.
fn loopback_ip_hint(host: &str) -> String {
    if host.starts_with("127.") && host != "127.0.0.1" {
        format!(
            "\n\nNota: {host} es una IP de loopback distinta de 127.0.0.1. En Linux funcionan \
             todas sin configurar nada, pero Windows solo atiende 127.0.0.1. Si estás en \
             Windows, usa 127.0.0.1 con un puerto por instrumento."
        )
    } else {
        String::new()
    }
}

/// Espera a que termine cualquiera de las tareas (en condiciones normales,
/// ninguna termina nunca).
async fn alguno_termina(tareas: Vec<tokio::task::JoinHandle<()>>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
    for t in tareas {
        let tx = tx.clone();
        tokio::spawn(async move {
            let _ = t.await;
            let _ = tx.send(()).await;
        });
    }
    drop(tx);
    rx.recv().await;
}

fn imprimir_tabla<I: std::borrow::Borrow<Instrumento>>(instrumentos: &[I]) {
    let ancho_id = instrumentos
        .iter()
        .map(|i| i.borrow().id.len())
        .max()
        .unwrap_or(0)
        .max(2);
    let ancho_modelo = instrumentos
        .iter()
        .map(|i| i.borrow().disp.modelo().len())
        .max()
        .unwrap_or(0)
        .max(6);
    println!();
    println!(
        "  {:ancho_id$}  {:ancho_modelo$}  RECURSO VISA",
        "ID", "MODELO"
    );
    for i in instrumentos {
        let i = i.borrow();
        println!(
            "  {:ancho_id$}  {:ancho_modelo$}  {}",
            i.id,
            i.disp.modelo(),
            i.transporte.resource_string()
        );
    }
}

/// Dónde buscar el banco cuando no se indica ninguno. El orden va de lo más
/// explícito (lo que el usuario tiene delante) a lo que dejó el instalador.
fn buscar_banco_por_defecto() -> anyhow::Result<PathBuf> {
    let mut candidatos = vec![PathBuf::from("banco.yaml")];
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        candidatos.push(dir.join("banco").join("banco.yaml"));
        if let Some(prefijo) = dir.parent() {
            // Sin «..» en la ruta: sale tal cual en la cabecera al arrancar.
            candidatos.push(prefijo.join("share/crucible/banco/banco.yaml"));
        }
    }
    candidatos
        .iter()
        .find(|c| c.is_file())
        .cloned()
        .ok_or_else(|| {
            let lista: Vec<String> = candidatos
                .iter()
                .map(|c| format!("  {}", c.display()))
                .collect();
            anyhow::anyhow!(
                "no encuentro ningún banco. He buscado en:\n{}\n\nIndica uno: crucible <banco.yaml>",
                lista.join("\n")
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERFIL: &str = include_str!("../../../perfiles/keithley_2400.yaml");

    fn vxi11_instrument(id: &str, host: &str, puerto: u16, device: &str) -> Instrumento {
        let perfil = Perfil::from_yaml(PERFIL).unwrap();
        Instrumento {
            id: id.to_string(),
            transporte: Transporte::Vxi11 {
                host: Some(host.to_string()),
                puerto: Some(puerto),
                device: Some(device.to_string()),
            },
            disp: Dispositivo::from_perfil(perfil).unwrap(),
        }
    }

    #[test]
    fn each_ip_gets_its_own_channel() {
        // The real bench: every box its own IP, all on the same port, all of
        // them called inst0.
        let banco = vec![
            vxi11_instrument("generador", "127.0.0.2", 5025, "inst0"),
            vxi11_instrument("fuente", "127.0.0.3", 5025, "inst0"),
        ];
        let endpoints = vxi11_endpoints(&banco).unwrap();
        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[0].host, "127.0.0.2");
        assert_eq!(endpoints[1].host, "127.0.0.3");
    }

    #[test]
    fn several_devices_on_one_ip_share_a_channel() {
        // The chassis case: one box fronting two instruments.
        let banco = vec![
            vxi11_instrument("generador", "127.0.0.1", 5025, "inst0"),
            vxi11_instrument("fuente", "127.0.0.1", 5025, "inst1"),
        ];
        let endpoints = vxi11_endpoints(&banco).unwrap();
        assert_eq!(endpoints.len(), 1, "una IP es un canal, no dos");
        assert_eq!(endpoints[0].port, 5025);
    }

    #[test]
    fn two_ports_on_one_ip_are_refused() {
        // An ::INSTR resource has no way of saying which of the two ports it
        // wants, so one of the instruments would be unreachable.
        let banco = vec![
            vxi11_instrument("generador", "127.0.0.1", 5025, "inst0"),
            vxi11_instrument("fuente", "127.0.0.1", 5026, "inst1"),
        ];
        let error = vxi11_endpoints(&banco).unwrap_err().to_string();
        assert!(error.contains("puertos distintos"), "dijo: {error}");
    }

    #[test]
    fn the_same_device_twice_on_one_ip_is_refused() {
        let banco = vec![
            vxi11_instrument("generador", "127.0.0.1", 5025, "inst0"),
            vxi11_instrument("fuente", "127.0.0.1", 5025, "inst0"),
        ];
        let error = vxi11_endpoints(&banco).unwrap_err().to_string();
        assert!(error.contains("device 'inst0'"), "dijo: {error}");
    }

    #[test]
    fn the_same_device_on_different_ips_is_fine() {
        // This is the normal case: on a real bench they are all inst0.
        let banco = vec![
            vxi11_instrument("generador", "127.0.0.2", 5025, "inst0"),
            vxi11_instrument("fuente", "127.0.0.3", 5025, "inst0"),
        ];
        assert!(vxi11_endpoints(&banco).is_ok());
    }

    #[test]
    fn an_unusual_loopback_ip_warns_about_windows() {
        let pista = loopback_ip_hint("127.0.0.2");
        assert!(pista.contains("Windows"), "dijo: {pista}");
        assert!(loopback_ip_hint("127.0.0.1").is_empty());
        assert!(loopback_ip_hint("192.168.1.10").is_empty());
    }
}
