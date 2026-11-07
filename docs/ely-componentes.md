# Inventario de Ely GPUI Components

Este documento registra las capacidades, decisiones y límites de Ely usados por la aplicación.

## Mapeo de necesidades

Los ejemplos son fragmentos dentro de una vista GPUI. `ely_gpui_component` es el crate reexportador de Ely.

| Necesidad de la app | Componente de Ely | Ejemplo mínimo y alcance |
|---|---|---|
| Botón primario | `buttons::Button` | `Button::new("procesar", "Procesar").primary().on_click(|_, _, _| {});` |
| Botón secundario | `buttons::Button`, `buttons::ButtonVariant` | `Button::new("cancelar", "Cancelar").variant(ButtonVariant::Secondary);` El estilo secundario es también el predeterminado. |
| Botón deshabilitado | `buttons::Button` | `Button::new("procesar", "Procesar").disabled(true);` Se atenúa y no recibe foco de Tab. |
| Input de texto con placeholder | `forms::TextInput`, `forms::Input` | `let q = cx.new(|cx| TextInput::new(window, cx).placeholder("Buscar CFDI"));` y, al dibujar, `Input::new(&q)`. `TextInput` es el estado; `Input` lo presenta. |
| Pestañas con contador | `navigation::Tabs`, `forms::Choice` | `Tabs::new("vistas", [Choice::new("cfdi", "CFDI").note("12")], "cfdi").panel(div());` `note` muestra un contador como texto al final, no un badge independiente. |
| Barra de progreso | `motion::ProgressBar` | `ProgressBar::new("progreso", 0.4);` El valor es `f32` entre 0 y 1; también existen `indeterminate`, `buffer` y `segments`. |
| Tabla de datos | `tables::DataTable`, `tables::Column`, `tables::Row`, `tables::Cell`, `tables::SortKey` | `DataTable::new("cfdi", [Column::new("uuid", "UUID"), Column::new("total", "Total")]).rows(rows).sorts([SortKey { column: "uuid".into(), rising: true }]).selected(seleccion).on_select(|_, _, _| {}).virtualized();` `rows` es una colección de `Row`; cada fila aporta una celda por columna. |
| Tabla: virtualización | `tables::DataTable::virtualized` | Sí: dibuja únicamente las filas visibles. El contenedor debe tener altura. En este modo el encabezado queda fijo sobre la lista vertical. |
| Tabla: orden y selección | `tables::DataTable::sorts`, `selected`, `on_select` | Sí: los encabezados alternan ascendente/descendente/sin orden; Shift agrega un criterio. La selección admite rangos con Shift. |
| Tabla: redimensionar columnas | `tables::Column`, `tables::DataTable` | Sí: arrastrar el borde derecho del encabezado. También se puede arrastrar un encabezado para reordenar columnas. |
| Tabla: columnas fijas a la derecha | — | **No existe** en `DataTable`: `Column::pinned()` fija columnas al inicio/izquierda. Cada columna fija requiere ancho y no se puede combinar con virtualización ni con filas expandibles. |
| Toast | `feedback::Toast`, `feedback::Toaster`, `feedback::ToastViewport` | `let t = cx.new(|_| Toaster::default());` Luego `t.update(cx, |t, cx| { t.push(Toast::new("Exportación lista"), cx); });` y se dibuja `ToastViewport::new("toasts", &t)` en la ventana. |
| Diálogo modal con navegación lateral | `overlays::Dialog` + composición con GPUI y `navigation::NavItem` | `Dialog::new("ajustes", "Ajustes", |_, _| {}).child(div().flex().child(NavItem::new("general", IconName::Settings, "General").active(true)).child(div().flex_1().child("Contenido")));` El diálogo aporta scrim, trampa de foco, Escape y devolución del foco; el menú lateral se compone dentro del cuerpo, no es una variante incorporada. |
| Switch | `forms::Switch` | `Switch::new("recordar-rfc", activo).label("Recordar RFC").on_change(|valor, _, _| {});` |
| Select | `forms::Select`, `forms::Choice` | `Select::new("tipo", [Choice::new("i", "Ingreso"), Choice::new("e", "Egreso")]).placeholder("Tipo").on_change(|valor, _, _| {});` |
| Checkbox | `forms::Checkbox`, `forms::CheckState` | `Checkbox::new("incluir", CheckState::Off).label("Incluir").on_change(|valor, _, _| {});` |
| Tooltip | `primitives::Tooltip` o `buttons::IconButton` | `IconButton::new("copiar", IconName::Copy).tooltip("Copiar");` También se puede aplicar `Tooltip::text("Guardar")` a un elemento interactivo GPUI. |

### Nota sobre datos monetarios

`tables::Cell::Number` recibe `f64`. No pasarle importes `rust_decimal` convertidos a `f64`: se pierde la garantía de precisión exigida por esta app. `Cell::Text(decimal.to_string())` conserva la representación decimal para mostrarla, pero no equivale a una celda numérica con orden y agregación decimal exactos. La ordenación numérica monetaria requeriría resolver ese límite antes de migrar la tabla.

Las tablas de resultados usan `uniform_list` para virtualizar las filas y dos `layout::Scrollbar` enlazadas a los manejadores reales de desplazamiento horizontal y vertical. Se mantienen la búsqueda, los filtros, el orden por encabezado y la selección con clic, Ctrl/Cmd y Shift. Los anchos y el orden de las columnas son fijos en esta vista. Los valores de búsqueda se normalizan al preparar los resultados y las filas filtradas se cachean para no recorrer todas las celdas en cada render. Los importes se muestran alineados a la derecha y no se ordenan numéricamente para evitar convertir `Decimal` a `f64`; el total se calcula con `rust_decimal`. La copia TSV sigue usando las filas seleccionadas o la pestaña completa.

## Integraciones de plataforma

### Arrastrar y soltar archivos

`forms::DropZone` usa los eventos nativos de GPUI y entrega rutas `PathBuf`:

```rust
DropZone::new("zip")
    .kinds(&["zip"])
    .hint("Suelta un ZIP de CFDI")
    .on_drop(|paths, _, _| { /* validar y procesar las rutas */ });
```

`.kinds` filtra por extensión, sin distinguir mayúsculas. `FileInput` también acepta archivos soltados y permite abrir el selector desde un botón.

### Diálogos nativos para abrir y guardar

- **Abrir:** `forms::FileInput`, `forms::PathInput` y `DropZone` llaman a `cx.prompt_for_paths(PathPromptOptions { ... })` de GPUI. El resultado llega de forma asíncrona. `PathPromptOptions` no expone filtros de extensión; validar `.zip` después de la selección, como ya indica el README de esta app.
- **Guardar:** GPUI 0.3.10 expone `cx.prompt_for_new_path(&directorio, Some("reporte.xlsx"))`, que muestra el diálogo nativo y devuelve un canal asíncrono con la ruta o cancelación. No encontré un wrapper de guardado de Ely: la app debe manejar cancelación/errores, escribir el archivo y decidir la extensión.

### Portapapeles

`TextInput` trae selección, copiar, cortar, pegar y deshacer. Para acciones propias, GPUI ofrece `cx.write_to_clipboard(ClipboardItem::new_string(...))` y `cx.read_from_clipboard_async()`. La lectura es asíncrona; en navegador el usuario puede tener que autorizar el acceso.

### Atajos globales

Declarar acciones con `actions!` y registrarlas con `cx.bind_keys([...])`. `KeyBinding::new(..., None)` no limita el atajo a un contexto; un contexto `Some(...)` sí lo restringe. Los textos de atajos en botones y menús solo muestran la combinación, no la registran.

Precaución para GPUI `=0.3.10`: `KeyBinding::new` recibe una combinación estática y no interpreta el modificador abstracto `secondary-`; el helper de Ely que lo expande es interno al crate. Para un atajo multiplataforma, resolver explícitamente `cmd`/`ctrl` por plataforma o usar la API de carga/parsing de GPUI que admite el modificador secundario. Evitar registrar dos veces atajos que Ely ya vincula en `ely_gpui_component::init`.

### Seguir la apariencia del sistema

`theme::Mode` solo tiene `Light` y `Dark`; no hay modo `System` automático en la API de tema. El patrón de la galería es observar cambios de apariencia de la ventana y actualizar Ely:

```rust
let _appearance = cx.observe_window_appearance(window, |_, window, cx| {
    Theme::set_mode(window.appearance().into(), cx);
});
```

Al iniciar, también establecer el modo a partir de `window.appearance()`. Si la app conserva una preferencia manual, actualizar por observación solo cuando el usuario haya elegido “Sistema”, como hace la galería.

## Revisión de la galería

Se revisaron las páginas e historias de Button/IconButton, Forms · Text y Selection, Navigation · Tabs, Tables · Core/Columns & rows/Grids, Overlays · Popovers & dialogs, Feedback · Messages, Loading · Progress, Menus y Window & Shell. Las rutas de módulos relevantes son `buttons`, `forms`, `navigation`, `tables`, `overlays`, `feedback`, `motion`, `menus` y `shell`.

La galería ejecutó la página Tables, que demuestra la tabla simple, toolbar con búsqueda/filtro/columnas, selección y orden, además de pinning izquierdo, resize/reorder y otras variantes. `DataGrid` y `Spreadsheet` son componentes separados; no sustituyen directamente a `DataTable`.

## Versión recomendada y riesgos

- **Rev fijado por la aplicación:** `89bff04957fde8bec63529bea0efe90f2a183159`. Es un pin reproducible, no una recomendación de estabilidad de producción:

  ```toml
  ely-gpui-component = { git = "https://github.com/ELY-Open-Source/Ely-GPUI-Components", rev = "89bff04957fde8bec63529bea0efe90f2a183159" }
  ```

- **Rust mínimo:** 1.98, edición 2024. Ely fija `gpui`, `gpui_platform` y `gpui_web` a `gpui-pre =0.3.10`; los ejemplos de integración asumen esa versión.
- **Compatibilidad de GPUI:** Ely está enlazado a `gpui-pre` 0.3.10. No combinarlo con componentes que dependan de otro snapshot o crate incompatible de GPUI, ya que pueden duplicar tipos GPUI incompatibles.
- **Estabilidad:** `tasks/stability.md` marca inestables todos los crates, sin sign-offs humanos; los componentes y APIs pueden cambiar.
- **Pendientes del upstream:** T49 (aceptación: URL pública y checklist MVP) y T56 (optimización) siguen sin marcarse completos en `TASKS.md`. T56 conserva O-07 abierto por duplicados transitivos que vienen de RaTeX y O-27 abierto a la firma del propietario. `tasks/acceptance.md` registra esas limitaciones.
- **Accesibilidad de diálogos:** `tasks/voiceover.md` anota límites de GPUI 0.3.10: VoiceOver puede no seguir el foco al abrir un diálogo hasta el siguiente movimiento y puede recorrer contenido detrás del diálogo porque GPUI no marca el nodo como modal.
- **Tabla:** no hay pinning a la derecha; virtualización no combina con columnas fijas ni detalle expandible. El uso de `f64` para `Cell::Number` es especialmente relevante para importes.
- **Portabilidad:** Ely necesita las features de plataforma de GPUI (`font-kit`; en Linux `wayland` y `x11`) para dibujar texto y abrir ventanas. `init` falla si falta el sistema de texto, en lugar de degradar silenciosamente.

## Ejecución verificada

El primer `cargo run -p ely-gallery` no pudo resolver el MSRV con el Rust preinstalado 1.96.1. Tras instalar Rust 1.98, ejecuté `cargo run -p ely-gallery` con esa toolchain y abrió la galería; también abrí la página `tables`. No modifiqué el código ni el `Cargo.toml` del proyecto.
