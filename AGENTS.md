# AGENTS.md

Guía para agentes de IA que trabajen en este repositorio. Leer antes de modificar código.

## Qué es

App de escritorio en **Rust + GPUI** que lee un ZIP de CFDI (XML), extrae datos por concepto y exporta un Excel de 4 hojas con vista previa. Ver `README.md` para el detalle funcional.

## Reglas inamovibles

1. **Solo extraer. No interpretar fiscalmente.** La clasificación contable es una sugerencia por palabras clave y debe rotularse como tal.
2. **No inventar datos.** Atributo o nodo ausente → celda vacía (`None`), nunca `0`, `""` con significado, ni valores por defecto.
3. **Namespaces por URI**, no por prefijo. `cfdi` = `http://www.sat.gob.mx/cfd/4` y `/cfd/3`; `tfd` = `http://www.sat.gob.mx/TimbreFiscalDigital`.
4. **Una fila por concepto.** CFDI con N conceptos → N filas.
5. **Un XML defectuoso no detiene el proceso.** Se registra en la hoja Errores y se continúa.
6. **Dinero con `rust_decimal`**, jamás `f32`/`f64`.
7. **Ante ambigüedad, preguntar al usuario antes de implementar.** Ver "Preguntas abiertas" en el README.

## Arquitectura

Hexagonal / clean. Dependencias solo hacia adentro:

`ui → application → domain` y `infrastructure → application (ports) → domain`

- `domain/`: tipos y reglas puras. Sin I/O, sin GPUI, sin crates de ZIP/XML/Excel.
- `application/`: casos de uso y traits (`ports.rs`).
- `infrastructure/`: implementaciones de los ports (zip, quick-xml, rust_xlsxwriter).
- `ui/`: GPUI + Ely. No parsea ni escribe archivos; invoca casos de uso.

No saltarse capas. Si se necesita un nuevo I/O, definir primero el port.

## Dependencias y UI

- Las dependencias Git de la UI deben fijar un `rev` inmutable; no usar ramas ni tags móviles.
- No mezclar Ely con implementaciones alternativas de componentes GPUI ni incorporar una segunda versión incompatible de GPUI.
- La UI no parsea XML/ZIP ni escribe Excel/configuración directamente: delega esas operaciones a casos de uso y ports.

## Convenciones de código

- Errores con `thiserror`; sin `unwrap()`/`expect()` fuera de tests.
- Parser: usar `quick_xml::NsReader` y comparar `ResolveResult::Bound(Namespace(uri))`.
- Fechas, montos y catálogos se guardan tal como vienen en el XML (texto/decimal); sin reformateo en el dominio.
- Procesamiento pesado fuera del hilo de UI; la UI solo recibe progreso y resultados.
- `cargo fmt` y `cargo clippy -- -D warnings` deben pasar.

## Tests

Todo cambio en el parser requiere fixture en `tests/fixtures/`:

- CFDI 4.0 y 3.3
- Varios conceptos (verifica N filas)
- Con IVA trasladado (002), ISR (001) e IVA (002) retenidos
- Tipo P, N y T
- Campos faltantes (celdas vacías)
- XML corrupto y archivo no-CFDI (van a Errores)
- Prefijos de namespace no estándar

Ejecutar `cargo test` antes de dar una tarea por terminada.

Regla adicional: todo valor nuevo configurable debe vivir en `AppConfig` con su `Default`, su validación y su test correspondiente. Si un ajuste solo cambia presentación/exportación/classificación, va ahí; si cambia el XML o el dato extraído, no se agrega a config.

## Hojas del Excel (contrato)

1. CFDI nivel concepto · 2. Resumen por producto (`ClaveProdServ + Descripción`, suma de importe) · 3. Clasificación contable sugerida (Ingreso, Gasto, Honorarios, Nómina, Inversión, Otros) · 4. Errores.

Cambiar columnas u orden requiere actualizar README y tests.

## Qué NO hacer

- No añadir dependencias pesadas sin justificarlo.
- No validar timbre, sellos ni cadena original (fuera de alcance).
- No calcular ni corregir impuestos; solo transcribir lo que dice el XML.
- No asumir RFC propio para marcar emitidas/recibidas hasta que se resuelva la pregunta abierta.
