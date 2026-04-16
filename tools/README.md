# Herramientas

Scripts de utilidad para trabajar con el workspace de entrenamiento.

---

## `run_obc_stack.sh`

Inicia la pila OBC completa de la Semana 2 con un solo comando.

```bash
bash tools/run_obc_stack.sh
```

**Qué hace:**
1. Ejecuta `cargo build --workspace -q` para asegurarse de que todos los binarios estén actualizados
2. Lanza `tc-receiver`, `obc-router`, `hk-service` y `sensor-daemon` en segundo plano
3. Espera 1 segundo para que los demonios vinculen sus sockets
4. Ejecuta `ground-sim` como arnés de prueba
5. Al salir (Ctrl+C o al completarse `ground-sim`), termina todos los procesos en segundo plano y elimina los archivos de socket

**Requisitos Previos:**
- Debe ejecutarse desde la raíz del repositorio o desde el directorio `tools/` (el script hace `cd` a la raíz del repositorio)
- El workspace debe compilar sin errores (`cargo build --workspace`)

---

## `check_workspace.sh`

Ejecuta todas las verificaciones de CI localmente antes de hacer push.

```bash
bash tools/check_workspace.sh
```

**Qué verifica (en orden):**
1. `cargo fmt --all -- --check` — formato (coincide con CI)
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — advertencias de linting
3. `cargo test --workspace --all-features` — todas las pruebas unitarias y de integración

Si algún paso falla, el script termina inmediatamente (`set -euo pipefail`).

**Consejo:** ejecutar este script antes de cada `git push` para detectar problemas localmente. El flujo de trabajo de CI
(`.github/workflows/ci.yml`) ejecuta exactamente las mismas verificaciones.

---

## Hacer los scripts ejecutables

Si aparece el error `Permission denied`:
```bash
chmod +x tools/run_obc_stack.sh tools/check_workspace.sh
```
