use crucible::{aceptar_conexiones, bind_tcp};
use crucible_core::{Banco, Dispositivo, Perfil, Transporte};
use std::path::{Path, PathBuf};

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

Cada instrumento escucha en 127.0.0.1 y en su puerto. Desde VISA:
  TCPIP0::127.0.0.1::<puerto>::SOCKET  (transporte tcp)
  TCPIP0::127.0.0.1::inst0::INSTR      (transporte vxi11)

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

/// Dos instrumentos en la misma dirección: el segundo no podría escuchar.
/// Mejor decirlo con sus nombres que dejar que el sistema diga «address in use».
fn comprobar_puertos_unicos(instrumentos: &[Instrumento]) -> anyhow::Result<()> {
    for (i, a) in instrumentos.iter().enumerate() {
        if let Some(b) = instrumentos[i + 1..].iter().find(|b| {
            b.transporte.puerto() == a.transporte.puerto()
                && b.transporte.host_or_default() == a.transporte.host_or_default()
                && b.transporte.is_vxi11() == a.transporte.is_vxi11()
                && (!b.transporte.is_vxi11()
                    || b.transporte.device_name() == a.transporte.device_name())
        }) {
            let host_a = a.transporte.host_or_default();
            let puerto_a = a.transporte.puerto().unwrap_or(0);
            anyhow::bail!(
                "'{}' y '{}' usan los dos {}:{}; cada instrumento necesita su puerto{}",
                a.id,
                b.id,
                host_a,
                puerto_a,
                if a.transporte.is_vxi11() {
                    format!(" (device {})", a.transporte.device_name())
                } else {
                    String::new()
                }
            );
        }
    }
    Ok(())
}

enum Listo {
    Tcp {
        inst: Instrumento,
        listener: tokio::net::TcpListener,
    },
    Vxi11 {
        inst: Instrumento,
        listener: tokio::net::TcpListener,
    },
}

async fn arrancar(path: &Path, instrumentos: Vec<Instrumento>) -> anyhow::Result<()> {
    // Se enlazan todos antes de servir ninguno: si un puerto está ocupado, el
    // banco no arranca. Un banco al que le falta un instrumento no está
    // degradado, da resultados falsos.
    let mut listos: Vec<Listo> = Vec::new();
    for inst in instrumentos {
        let host = inst.transporte.host_or_default().to_string();
        let puerto = inst.transporte.puerto().unwrap();
        let is_vxi = inst.transporte.is_vxi11();
        if is_vxi {
            let listener = crucible_vxi11::bind_vxi11(&host, puerto)
                .await
                .map_err(|e| {
                    anyhow::anyhow!(
                        "'{}' no puede escuchar en {}:{} ({e}). ¿Hay otro programa, u otro \
                         Crucible, usando ese puerto?",
                        inst.id,
                        host,
                        puerto
                    )
                })?;
            listos.push(Listo::Vxi11 { inst, listener });
        } else {
            let listener = bind_tcp(&host, puerto).await.map_err(|e| {
                anyhow::anyhow!(
                    "'{}' no puede escuchar en {}:{} ({e}). ¿Hay otro programa, u otro \
                     Crucible, usando ese puerto?",
                    inst.id,
                    host,
                    puerto
                )
            })?;
            listos.push(Listo::Tcp { inst, listener });
        }
    }

    println!(
        "Crucible {} — {}",
        env!("CARGO_PKG_VERSION"),
        path.display()
    );
    let refs: Vec<&Instrumento> = listos
        .iter()
        .map(|l| match l {
            Listo::Tcp { inst, .. } | Listo::Vxi11 { inst, .. } => inst,
        })
        .collect();
    imprimir_tabla(&refs);
    println!("\nListo. Ctrl+C para parar.");

    // Portmapper opcional si hay algún VXI-11 (requiere privilegio en 111)
    let vxi_ports: Vec<u16> = listos
        .iter()
        .filter_map(|l| match l {
            Listo::Vxi11 { inst, .. } => inst.transporte.puerto(),
            _ => None,
        })
        .collect();
    if !vxi_ports.is_empty() {
        let pm_port = vxi_ports[0];
        tokio::spawn(async move {
            if let Err(e) = crucible_vxi11::servir_portmapper(pm_port, "0.0.0.0:111").await {
                eprintln!("portmapper no disponible (requiere admin para UDP 111): {e:#}");
                eprintln!("Los instrumentos VXI-11 siguen accesibles por su puerto TCP directo.");
            }
        });
    }

    let mut tareas = Vec::new();
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
            Listo::Vxi11 { inst, listener } => {
                let id = inst.id.clone();
                tareas.push(tokio::spawn(async move {
                    if let Err(e) =
                        crucible_vxi11::aceptar_conexiones_vxi11(listener, inst.disp).await
                    {
                        eprintln!("error en '{id}': {e}");
                    }
                }));
            }
        }
    }

    tokio::select! {
        _ = tokio::signal::ctrl_c() => println!("\nParado."),
        _ = alguno_termina(tareas) => anyhow::bail!("un instrumento dejó de aceptar conexiones"),
    }
    Ok(())
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
