# CFDI ZIP Processor

Aplicación de escritorio en Rust con GPUI y Ely que procesa un archivo ZIP con CFDI en XML (emitidos y recibidos), extrae los datos a nivel concepto y genera un Excel con 4 hojas y una vista previa en pantalla.

> Regla base: **solo extrae, no interpreta fiscalmente y no inventa datos**. Si un dato falta, la celda queda vacía.

## Stack

| Capa               | Tecnología                                                                               |
| ------------------ | ---------------------------------------------------------------------------------------- |
| UI                 | Ely GPUI Components, rev `89bff04957fde8bec63529bea0efe90f2a183159`                      |
| Base de ventanas   | `gpui-pre` / `gpui-pre-platform` 0.3.10                                                  |
| Rust mínimo        | 1.98, edición 2024                                                                       |
| ZIP                | `zip`                                                                                     |
| XML con namespaces | `quick-xml` (`NsReader`)                                                                  |
| Excel              | `rust_xlsxwriter`                                                                         |
| Decimales          | `rust_decimal` (nunca `f64` para dinero)                                                  |
| Errores            | `thiserror`                                                                               |
| Tests              | `cargo test` + fixtures XML en `tests/fixtures/` + lectura XLSX con `calamine`           |

## Flujo

1. El usuario selecciona un `.zip`.
2. Se leen todos los `.xml` del ZIP sin importar su nombre ni subcarpeta.
3. Cada XML se parsea; si falla, se registra en **Errores** y se continúa.
4. Se genera una fila por concepto (CFDI con 3 conceptos = 3 filas).
5. Se resumen importes por `ClaveProdServ + Descripción`; conceptos sin importe cuentan, pero no suman.
6. Se genera una clasificación contable sugerida con el criterio de cada fila.
7. El usuario exporta el `.xlsx`.

## Namespaces

| Prefijo | URI                                         | Uso                                                 |
| ------- | ------------------------------------------- | --------------------------------------------------- |
| `cfdi`  | `http://www.sat.gob.mx/cfd/4` (y `/cfd/3`)  | Comprobante, Emisor, Receptor, Conceptos, Impuestos |
| `tfd`   | `http://www.sat.gob.mx/TimbreFiscalDigital` | `TimbreFiscalDigital` → `UUID`                      |

Se resuelve por **URI del namespace**, nunca por el texto del prefijo (un XML puede usar otro prefijo). Se soportan CFDI 3.3 y 4.0.

## Campos extraídos

**Comprobante (se repite en cada fila de concepto):**
UUID, Fecha, RFC Emisor, Nombre Emisor, RFC Receptor, Tipo de Comprobante (I/E/P/N/T), Subtotal, Descuento, IVA trasladado (002), ISR retenido (001), IVA retenido (002), Total, Uso CFDI, Método de Pago, Forma de Pago, Moneda, Tipo de Cambio.

**Concepto (una fila cada uno):**
ClaveProdServ, Descripción, Importe.

Los impuestos se toman del nodo `cfdi:Impuestos` hijo directo de `Comprobante`, por código de impuesto, y se repiten en cada fila de concepto. Si falta `TimbreFiscalDigital`, UUID queda vacío. Los tipos P y N se extraen tal como aparecen, sin tratamiento especial.

## Clasificación sugerida

La clasificación no es interpretación fiscal. Se evalúa en este orden y se conserva el criterio aplicado:

1. Tipo N → Nómina.
2. Descripción contiene una palabra clave de honorarios → Honorarios.
3. Descripción contiene una palabra clave de inversión → Inversión.
4. Tipo I con RFC propio: emisor coincide → Ingreso; receptor coincide → Gasto.
5. Otros; el criterio explica por qué no aplicó una regla.

La comparación de palabras clave ignora mayúsculas y acentos. RFC propio es opcional; sin RFC no se distingue Ingreso de Gasto. Las palabras clave y la categoría predeterminada se pueden ajustar en Configuración.

## Hojas del Excel

1. **CFDI (nivel concepto)**: todos los registros, una fila por concepto.
2. **Resumen por producto**: agrupa `ClaveProdServ + Descripción` y suma el importe.
3. **Clasificación sugerida**: campos clave del concepto, categoría y criterio. Es una **sugerencia**, no una interpretación fiscal.
4. **Errores**: archivo, ruta dentro del ZIP y motivo (XML inválido, sin `Comprobante`, sin UUID, etc.).

Los importes se calculan con `rust_decimal`; el XLSX los guarda como celdas numéricas con formato de dos decimales (seis para tipo de cambio). Los datos ausentes quedan en celdas vacías.

## Arquitectura (hexagonal)

```
src/
├── main.rs
├── lib.rs
├── domain/            # Entidades y reglas puras (sin I/O)
│   ├── cfdi.rs        # CfdiRow (una fila por concepto)
│   ├── clasificacion.rs
│   └── error.rs
├── application/       # Casos de uso
│   ├── procesar_zip.rs
│   ├── resumen.rs
│   └── ports.rs       # traits: ZipReader, CfdiParser, ExcelExporter
├── infrastructure/    # Adaptadores
│   ├── zip_reader.rs
│   ├── xml_parser.rs  # quick-xml + namespaces
│   └── xlsx_exporter.rs
└── ui/                # UI Ely y presentación de resultados
tests/
├── parser.rs
├── use_cases.rs
└── fixtures/          # XML 3.3/4.0, IVA tasa 0, tipos, corruptos y namespaces
```

La UI nunca parsea ni escribe archivos directamente: llama a los casos de uso. El procesamiento corre fuera del hilo de UI y reporta progreso.

## Interfaz de escritorio

La ventana Ely abre a 1200 × 760 px y admite un mínimo de 900 × 560 px. La pantalla permite seleccionar o soltar un ZIP, indicar opcionalmente el RFC propio, procesar en segundo plano, consultar métricas y exportar el Excel. El tema claro/oscuro sigue la apariencia del sistema o el valor configurado. Los resultados se muestran en cuatro pestañas con contadores y tablas virtualizadas: CFDI (21 columnas), Resumen por producto (4), Clasificación sugerida (7) y Errores (2). Cada tabla permite buscar y filtrar filas, ordenar y seleccionar, redimensionar y reordenar columnas, desplazarse en ambos ejes y copiar datos como TSV. Los montos se conservan como texto decimal; el único total de vista previa es Importe a nivel concepto.

La pantalla de Configuración ofrece navegación por ocho secciones, guardado automático con debounce, validación, restauración por sección o completa, e importación/exportación TOML. Incluye ajustes de RFC, extracción, clasificación sugerida, hojas y formatos de Excel, tabla, apariencia y atajos. Los cambios de apariencia se aplican en vivo; los que afectan al procesamiento requieren reprocesar el ZIP. Los campos avanzados de hilos, registro de depuración y errores detallados no se muestran porque todavía no tienen efecto en el backend. Ely tampoco permite ajustar el overscan ni desactivar el resaltado al pasar el puntero.

Para generar un ZIP de carga con 5,000 CFDI sintéticos basados en el fixture:

```bash
python3 tools/generate_load_test_zip.py /tmp/cfdi_5000.zip --count 5000
cargo run
```

Abre `/tmp/cfdi_5000.zip`, pulsa **Procesar** y verifica que la ventana siga respondiendo mientras avanza la barra de progreso y que las tablas virtualizadas se desplacen de manera vertical y horizontal.

La prueba automatizada `cargo test procesar_zip_procesa_5000_xml_sin_perder_filas` verifica que el caso de uso procese 5,000 XML y conserve todas sus filas.

## Configuración

La aplicación guarda la configuración persistente en el directorio de configuración del sistema del usuario. En Linux suele quedar en `~/.config/cfdi-zip-processor/config.toml`; en macOS y Windows se usa la ruta estándar de `directories`.

Ejemplo de `config.toml`:

```toml
version = 1

[general]
rfc_propio = "ABC970101XYZ"
recordar_rfc = true

[extraccion]
repetir_impuestos_en_cada_concepto = true
columna_origen_emitida_recibida = false

[extraccion.incluir_tipos]
i = true
e = true
p = true
n = true
t = true

[clasificacion]
categoria_por_defecto = "Otros"
regla_nomina_tipo_n = true
regla_ingreso_gasto_por_rfc = true

[excel]
nombre_hoja_cfdi = "CFDI"
nombre_hoja_resumen = "Resumen por producto"
nombre_hoja_clasificacion = "Clasificación sugerida"
nombre_hoja_errores = "Errores"
formato_nombre_archivo = "cfdi_{YYYYMMDD}_{HHmm}.xlsx"
columnas_visibles_cfdi = ["UUID", "Fecha", "Total", "ClaveProdServ", "Descripción", "Importe", "Archivo"]
decimales_monto = 2
decimales_tipo_cambio = 6
abrir_carpeta_al_exportar = false

[tabla]
altura_fila = 32
mostrar_hover = true
overscan_px = 240.0

[apariencia]
tema = "Sistema"
color_acento = "Indigo"
densidad = "Normal"
tamaño_fuente = 13.0

[atajos]
abrir_zip = "Ctrl+O"
procesar = "Ctrl+P"
exportar = "Ctrl+E"
pestaña_1 = "Ctrl+1"
pestaña_2 = "Ctrl+2"
pestaña_3 = "Ctrl+3"
pestaña_4 = "Ctrl+4"
abrir_configuracion = "Ctrl+,"

[avanzado]
hilos_de_procesamiento = "Auto"
mostrar_errores_detallados = false
registro_de_depuracion = false
```

Los cambios de configuración solo afectan la presentación, clasificación sugerida y exportación; nunca alteran los datos del XML original ni inventan valores faltantes.

## Preguntas abiertas

1. **Emitidas vs. Recibidas:** no hay columna por ahora. RFC propio opcional solo guía sugerencias de Ingreso/Gasto.

## Ejecución

### Requisitos por plataforma

- **Todas:** Rust 1.98 o posterior con Cargo (edición 2024).
- **Linux:** herramientas de compilación y `pkg-config`, junto con las bibliotecas de desarrollo de X11/XCB, Wayland, Fontconfig y FreeType. En Debian/Ubuntu: `sudo apt install build-essential pkg-config libx11-dev libxcb1-dev libxcb-xkb-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libfontconfig1-dev libfreetype6-dev`.
- **macOS:** Xcode Command Line Tools (`xcode-select --install`).
- **Windows:** Visual Studio Build Tools con la carga **Desarrollo para el escritorio con C++** y el Windows SDK; usa el target MSVC de Rust.

Instala el toolchain de Rust desde [rustup](https://rustup.rs/) y verifica que `rustc --version` sea al menos 1.98.

### Compilar y ejecutar

Desde la raíz del repositorio, en Linux, macOS o Windows, los comandos Cargo son los mismos:

```bash
cargo build --release
cargo run --release
cargo test
```

En Linux, la aplicación usa Wayland o X11 según el escritorio disponible. Para compilar de forma cruzada entre sistemas operativos, usa un entorno nativo de cada plataforma: Cargo no instala los toolchains ni SDKs de escritorio.

## Licencia

Por definir.
