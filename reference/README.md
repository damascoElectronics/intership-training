# Materiales de Referencia

Hojas de referencia rápida y guías de decisión.

| Archivo | Qué contiene |
|---------|--------------|
| [`ccsds_primer.md`](ccsds_primer.md) | Disposición de bits de la cabecera primaria CCSDS, asignación de APID, indicadores de secuencia, formato OBT |
| [`pus_service_catalog.md`](pus_service_catalog.md) | Tabla de servicios PUS-C: servicios obligatorios, subservicios TC/TM, errores comunes |
| [`ipc_decision_matrix.md`](ipc_decision_matrix.md) | Cuándo usar UDS vs FIFO vs POSIX MQ vs memoria compartida vs D-Bus |
| [`rust_ffi_checklist.md`](rust_ffi_checklist.md) | Lista de verificación previa a la fusión para cualquier código que cruce la frontera Rust/C |
| [`unsafe_audit_template.md`](unsafe_audit_template.md) | Plantillas de comentarios SAFETY para las 5 categorías de unsafe |

## Referencias Externas

| Estándar | Título | Acceso |
|----------|--------|--------|
| CCSDS 133.0-B-2 | Space Packet Protocol | Gratuito: public.ccsds.org |
| ECSS-E-ST-70-41C | PUS-C Packet Utilization Standard | De pago / membresía ECSS |
| ECSS-Q-ST-80C | Garantía de calidad del producto software | De pago |
| Linux man-pages | capabilities(7), prctl(2), mq_overview(7) | `man 7 capabilities` |
| Rust Reference | Código inseguro | doc.rust-lang.org/reference |
| The Rustonomicon | Las artes oscuras del Rust inseguro | doc.rust-lang.org/nomicon |
