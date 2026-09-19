# Spikes

Código **desechable** para responder una pregunta concreta antes de
comprometer una fase de trabajo. No forma parte del producto y por eso vive
fuera del workspace de Cargo: cada spike tiene su propio `Cargo.toml` con un
`[workspace]` vacío.

Un spike se borra cuando ha contestado su pregunta y la respuesta está
recogida en el documento que la motivó. Si uno sobrevive mucho tiempo, o es
que la pregunta no estaba clara o es que ya no era un spike.

| Spike | Pregunta que contesta | Estado |
|---|---|---|
| `portmapper` | ¿Llega el broadcast de descubrimiento de NI-MAX a un proceso de la misma máquina? (`docs/bancos/plan-banco-rf.md` §5.1) | Pendiente de ejecutar en Windows |
