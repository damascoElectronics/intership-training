# Catálogo de Servicios PUS-C
**ECSS-E-ST-70-41C**

---

## Servicios Obligatorios para un OBC Mínimo Viable

| Servicio | Nombre | TCs Clave | TMs Clave |
|----------|--------|-----------|-----------|
| **1** | Verificación de TC | _(ninguno — el suelo no envía estos)_ | 1,1=Aceptado; 1,2=Rechazado; 1,7=Completado; 1,8=Fallido |
| **3** | Monitoreo de Estado | 3,129=Generar informe puntual; 3,130=Habilitar periódico | 3,25=Informe de parámetros HK |
| **5** | Reporte de Eventos | _(ninguno — el OBC genera los eventos)_ | 5,1=Información; 5,2=Severidad baja; 5,3=Media; 5,4=Alta |
| **17** | Operaciones a Bordo | 17,1=Ping ¿Estás vivo? | 17,2=Pong Estoy vivo |

---

## Servicios Recomendados

| Servicio | Nombre | TCs Clave | TMs Clave |
|----------|--------|-----------|-----------|
| **6** | Gestión de Memoria | 6,2=Cargar memoria; 6,5=Volcar memoria | 6,6=Informe de volcado de memoria |
| **9** | Gestión de Tiempo | 9,128=Establecer tiempo a bordo | 9,2=Informe de OBT actual |
| **11** | Planificación Temporal de TC | 11,4=Insertar TC en la agenda | 11,10=Informe de resumen |
| **12** | Monitoreo a Bordo | 12,1=Habilitar monitoreo de parámetros | 12,12=Informe de parámetro fuera de límites |
| **20** | Gestión de Parámetros | 20,128=Establecer valor de parámetro; 20,129=Obtener parámetro | 20,130=Informe de parámetro |

---

## Servicio 1 — Detalles de Verificación de TC

Cada TC aceptado que lo solicite debe generar los siguientes TMs:

```
TC received   → TM(1,1) Acceptance Successful  OR  TM(1,2) Acceptance Failed
TC executed   → TM(1,7) Completion Successful  OR  TM(1,8) Completion Failed
```

El subservicio distingue en qué etapa ocurrió el fallo, lo que facilita enormemente el diagnóstico.

---

## Servicio 3 — Detalles del Monitoreo de Estado

**TC(3,129) — Generar Informe HK Puntual**
- Datos de aplicación: [Report ID: u8] que identifica qué conjunto de parámetros reportar
- Respuesta: TM(3,25) con los valores de los parámetros

**Formato de TM(3,25) — Informe de Parámetros HK** (depende de la definición del informe):
- Datos de aplicación: [Report ID: u8] [valores de parámetros en orden]

---

## Servicio 17 — Operaciones a Bordo (Prueba de Conectividad)

La prueba extremo a extremo más sencilla posible:

```
Ground: TC(17,1) → OBC
OBC:    TM(1,1) + TM(17,2) → Ground
```

Si se recibe TM(17,2), el enlace ascendente, el software del OBC y el enlace descendente funcionan correctamente.
Este es el primer TC que se implementa y lo último que se prueba antes del lanzamiento.

---

## IDs de Origen de TC (convención)

| ID | Origen |
|----|--------|
| 0 | Estación terrena principal |
| 1 | Estación terrena secundaria |
| 2 | Autonomía a bordo (el OBC se comanda a sí mismo) |
| 255 | Simulación/prueba |

---

## Errores Comunes

1. **Olvidar la Verificación de TC** — cada TC aceptado/ejecutado debe generar un TM(1,x)
2. **APID vs Servicio** — enrutar por APID, no por número de servicio
3. **Contador de secuencia por APID** — no un contador global único
4. **CRC** — verificar al recibir, agregar al transmitir, en todo momento
5. **OBT en TM** — debe ser monótono; un OBT incorrecto confunde las herramientas de análisis en tierra
