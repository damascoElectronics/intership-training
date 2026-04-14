# Reference Materials

Quick-access cheat sheets and decision guides.

| File | What it contains |
|------|-----------------|
| [`ccsds_primer.md`](ccsds_primer.md) | CCSDS primary header bit layout, APID allocation, sequence flags, OBT format |
| [`pus_service_catalog.md`](pus_service_catalog.md) | PUS-C service table: mandatory services, TC/TM subservices, common mistakes |
| [`ipc_decision_matrix.md`](ipc_decision_matrix.md) | When to use UDS vs FIFO vs POSIX MQ vs shared memory vs D-Bus |
| [`rust_ffi_checklist.md`](rust_ffi_checklist.md) | Pre-merge checklist for any code crossing the Rust/C boundary |
| [`unsafe_audit_template.md`](unsafe_audit_template.md) | SAFETY comment templates for all 5 categories of unsafe |

## External References

| Standard | Title | Access |
|----------|-------|--------|
| CCSDS 133.0-B-2 | Space Packet Protocol | Free: public.ccsds.org |
| ECSS-E-ST-70-41C | PUS-C Packet Utilization Standard | Paid / ECSS membership |
| ECSS-Q-ST-80C | Software product assurance | Paid |
| Linux man-pages | capabilities(7), prctl(2), mq_overview(7) | `man 7 capabilities` |
| Rust Reference | Unsafe code | doc.rust-lang.org/reference |
| The Rustonomicon | Dark arts of unsafe Rust | doc.rust-lang.org/nomicon |
