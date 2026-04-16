//! Tipo de error unificado para la pila de software OBC.

/// Todos los errores que pueden ocurrir dentro de la pila de software OBC.
#[derive(Debug, thiserror::Error)]
pub enum OBCError {
    /// Una operación de envío IPC falló (p. ej., el par cerró su socket).
    #[error("IPC send failed: {0}")]
    IpcSend(String),

    /// La verificación HMAC falló para un TC entrante.
    #[error("authentication failed: {0}")]
    AuthFailed(String),

    /// El número de secuencia del TC recibido cae dentro de la ventana de repetición.
    #[error("replay detected: seq={seq}")]
    ReplayDetected { seq: u16 },

    /// No se pudo analizar un paquete a partir de bytes en bruto.
    #[error("packet parse error: {0}")]
    ParseError(String),

    /// Un componente requerido reportó un estado de salud no nominal.
    #[error("component {id} is not healthy: {state}")]
    ComponentUnhealthy { id: String, state: String },

    /// Envuelve `std::io::Error` para propagación transparente.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
