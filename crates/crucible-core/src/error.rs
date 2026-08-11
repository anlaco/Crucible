use thiserror::Error;

pub type Result<T> = std::result::Result<T, CrucibleError>;

#[derive(Error, Debug)]
pub enum CrucibleError {
    #[error("error de I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("error parseando YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("perfil inválido: {0}")]
    PerfilInvalido(String),

    #[error("comando no reconocido: {0}")]
    ComandoNoReconocido(String),

    #[error("modelo no encontrado: {0}")]
    ModeloNoEncontrado(String),

    #[error("error de evaluación: {0}")]
    Evaluacion(String),

    #[error("error de transporte: {0}")]
    Transporte(String),

    #[error("error de protocolo: {0}")]
    Protocolo(String),
}
