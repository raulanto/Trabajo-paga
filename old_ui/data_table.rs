//! Tabla de datos (vista previa) para las cuatro pestañas.
//!
//! La lógica (orden, búsqueda, filtros, selección, columnas, totales, copiar)
//! vive en `table_model` y está probada sin ventana. Aquí solo hay UI.
//!
//! Utilidades:
//! - Clic en encabezado: orden Asc → Desc → ninguno. Shift+clic: orden multi-columna.
//! - Arrastrar el borde derecho del encabezado: redimensionar columna.
//! - "Columnas": mostrar/ocultar y reordenar; "Restaurar columnas".
//! - Selección: clic, Ctrl/Cmd+clic, Shift+clic (rango).
//! - Teclado: ↑ ↓ PgUp PgDn Home End (con Shift extiende), Ctrl/Cmd+A, Ctrl/Cmd+C, Esc.
//! - Copiar (TSV, pega directo en Excel): selección, o toda la vista si no hay selección.
//! - Pie con totales de columnas `Suma` (sobre la selección si existe, si no sobre la vista).
//! - Búsqueda global y filtros por columna: `set_busqueda` / `set_filtro`.
//! - Columna "#", cebra y hover opcionales, estados vacíos distintos.
//! - Scroll virtualizado vertical (uniform_list) y horizontal (solo columnas visibles).
//!
//! Uso desde la vista principal:
//! ```ignore
//! let tabla = cx.new(|cx| TablaVista::new(&CFDI_COLUMNS, filas.cfdi.clone(), "cfdi-rows", cx));
//! // render:   self.tabla_cfdi.clone()          (Entity<TablaVista> es un elemento)
//! // buscador: tabla.update(cx, |t, cx| t.set_busqueda(&texto, cx));
//! // datos:    tabla.update(cx, |t, cx| t.set_rows(nuevas_filas, cx));
//! ```

use std::ops::Range;
use std::sync::Arc;

use gpui::{
    ClickEvent, ClipboardItem, Context, Entity, FocusHandle, Hsla, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Render, ScrollHandle,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, Subscription,
    UniformListScrollHandle, Window, div, point, prelude::*, px, uniform_list,
};
use gpuikit::{
    elements::{button::button, text_field::text_field},
    input::{InputState, InputStateEvent},
    theme::{ActiveTheme, ControlSize, Themeable},
    traits::control_sized::ControlSized,
};

use super::table_model::{Direccion, TableState, Tipo, formatear_entero, formatear_monto};
use super::theme;

const GUTTER: f32 = 48.0;
const TOOLBAR_H: f32 = 36.0;
const FOOTER_H: f32 = 32.0;
const PAGINA: isize = 15;

/// Fila ya formateada: un texto por columna.
pub type Fila = Box<[SharedString]>;

#[derive(Clone, Copy)]
pub struct Columna {
    pub titulo: &'static str,
    pub ancho: f32,
    flex: bool,
    pub tipo: Tipo,
}

/// Formatea todas las filas una sola vez. Llamar al recibir resultados
/// (idealmente en una tarea en segundo plano), NUNCA dentro de `render`.
pub fn construir_filas<T>(
    items: &[T],
    ncols: usize,
    cell: impl Fn(&T, usize) -> String,
) -> Arc<Vec<Fila>> {
    Arc::new(
        items
            .iter()
            .map(|item| {
                (0..ncols)
                    .map(|c| SharedString::from(cell(item, c)))
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            })
            .collect(),
    )
}

// Subtotal, Descuento, IVA, ISR, Total y Tipo de Cambio son del COMPROBANTE y se
// repiten en cada concepto: se ordenan como número pero NO llevan total en el pie
// (sumarlos duplicaría montos). Solo las columnas a nivel concepto usan `col_s`.
pub const CFDI_COLUMNS: [Columna; 21] = [
    col("UUID", theme::WIDTH_UUID),
    col("Fecha", theme::WIDTH_DATE),
    col("RFC Emisor", theme::WIDTH_RFC),
    col("Nombre Emisor", theme::WIDTH_NAME),
    col("RFC Receptor", theme::WIDTH_RFC),
    col("Tipo de Comprobante", theme::WIDTH_TYPE),
    col_r("Subtotal", theme::WIDTH_MONEY),
    col_r("Descuento", theme::WIDTH_MONEY),
    col_r("IVA trasladado (002)", theme::WIDTH_MONEY),
    col_r("ISR retenido (001)", theme::WIDTH_MONEY),
    col_r("IVA retenido (002)", theme::WIDTH_MONEY),
    col_r("Total", theme::WIDTH_MONEY),
    col("Uso CFDI", theme::WIDTH_CODE),
    col("Método de Pago", theme::WIDTH_METHOD),
    col("Forma de Pago", theme::WIDTH_METHOD),
    col("Moneda", theme::WIDTH_CURRENCY),
    col_r("Tipo de Cambio", theme::WIDTH_EXCHANGE),
    col("ClaveProdServ", theme::WIDTH_CODE),
    col("Descripción", theme::WIDTH_DESCRIPTION),
    col_s("Importe", theme::WIDTH_MONEY),
    col("Archivo", theme::WIDTH_FILE),
];

pub const RESUMEN_COLUMNS: [Columna; 4] = [
    col("ClaveProdServ", theme::WIDTH_CODE),
    col_flex("Descripción", theme::WIDTH_DESCRIPTION),
    col_s("Importe total", theme::WIDTH_MONEY),
    col_s("Número de conceptos", theme::WIDTH_COUNT),
];

pub const CLASIFICACION_COLUMNS: [Columna; 7] = [
    col("Archivo", theme::WIDTH_FILE),
    col("UUID", theme::WIDTH_UUID),
    col("Tipo de Comprobante", theme::WIDTH_TYPE),
    col("ClaveProdServ", theme::WIDTH_CODE),
    col("Descripción", theme::WIDTH_DESCRIPTION),
    col("Categoría sugerida", theme::WIDTH_CATEGORY),
    col_flex("Criterio", theme::WIDTH_CRITERION),
];

pub const ERROR_COLUMNS: [Columna; 2] = [
    col("Archivo", theme::WIDTH_FILE),
    col_flex("Motivo", theme::WIDTH_ERROR),
];

const fn col(titulo: &'static str, ancho: f32) -> Columna {
    Columna {
        titulo,
        ancho,
        flex: false,
        tipo: Tipo::Texto,
    }
}

const fn col_flex(titulo: &'static str, ancho: f32) -> Columna {
    Columna {
        titulo,
        ancho,
        flex: true,
        tipo: Tipo::Texto,
    }
}

const fn col_r(titulo: &'static str, ancho: f32) -> Columna {
    Columna {
        titulo,
        ancho,
        flex: false,
        tipo: Tipo::Numero,
    }
}

const fn col_s(titulo: &'static str, ancho: f32) -> Columna {
    Columna {
        titulo,
        ancho,
        flex: false,
        tipo: Tipo::Suma,
    }
}

/// Rango [first, end) de columnas visibles según el scroll horizontal.
/// `lefts` tiene n+1 elementos (el último es el ancho total).
/// Si aún no hay medidas (primer frame), devuelve todas.
fn rango_visible(lefts: &[f32], scroll_x: f32, view_w: f32, overscan: f32) -> (usize, usize) {
    let n = lefts.len() - 1;
    if view_w <= 0. {
        return (0, n);
    }
    let starts = &lefts[..n];
    let from = (scroll_x - overscan).max(0.);
    let to = scroll_x + view_w + overscan;
    let first = starts.partition_point(|&l| l <= from).saturating_sub(1);
    let end = starts.partition_point(|&l| l < to).max(first + 1).min(n);
    (first, end)
}

fn celda(ancho: f32, numerico: bool, texto: SharedString) -> impl IntoElement {
    let mut el = div()
        .flex_shrink_0()
        .w(px(ancho))
        .h_full()
        .flex()
        .items_center()
        .px(px(8.))
        .truncate(); // incluye overflow_hidden + ellipsis + nowrap
    if numerico {
        el = el.justify_end().text_right();
    }
    el.child(texto)
}

#[derive(Clone, Copy)]
struct Colores {
    border: Hsla,
    surface: Hsla,
    fg: Hsla,
    muted: Hsla,
    hover: Hsla,
}

struct Plan {
    /// (índice de columna original, ancho) de las columnas visibles, en orden.
    cols: Vec<(usize, f32)>,
    /// Posición izquierda de cada columna; el último elemento es el ancho total.
    lefts: Vec<f32>,
}

impl Plan {
    fn total(&self) -> f32 {
        self.lefts.last().copied().unwrap_or(0.)
    }
}

struct Resize {
    idx: usize,
    x0: f32,
    ancho0: f32,
}

pub struct TablaVista {
    columnas: &'static [Columna],
    rows: Arc<Vec<Fila>>,
    estado: TableState,
    list_id: &'static str,
    scroll: UniformListScrollHandle,
    hscroll: ScrollHandle,
    focus: FocusHandle,
    busqueda_input: Entity<InputState>,
    _busqueda_subscription: Subscription,
    panel_columnas: bool,
    resize: Option<Resize>,
    alto_fila: f32,
    hover: bool,
    cebra: bool,
    overscan_px: f32,
}

impl TablaVista {
    pub fn new(
        columnas: &'static [Columna],
        rows: Arc<Vec<Fila>>,
        list_id: &'static str,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut estado = TableState::nuevo(
            columnas.iter().map(|c| c.tipo).collect(),
            columnas.iter().map(|c| c.ancho).collect(),
        );
        estado.indexar(&rows[..]);
        let busqueda_input = cx.new(InputState::new_singleline);
        let busqueda_subscription = cx.subscribe(
            &busqueda_input,
            |this, input, event: &InputStateEvent, cx| {
                if matches!(event, InputStateEvent::TextChanged) {
                    let query = input.read(cx).content().to_owned();
                    this.set_busqueda(&query, cx);
                }
            },
        );
        Self {
            columnas,
            rows,
            estado,
            list_id,
            scroll: UniformListScrollHandle::new(),
            hscroll: ScrollHandle::new(),
            focus: cx.focus_handle().tab_stop(true),
            busqueda_input,
            _busqueda_subscription: busqueda_subscription,
            panel_columnas: false,
            resize: None,
            alto_fila: theme::ROW_HEIGHT,
            hover: true,
            cebra: false,
            overscan_px: 240.,
        }
    }

    pub fn set_presentacion(
        &mut self,
        alto_fila: f32,
        hover: bool,
        overscan_px: f32,
        cx: &mut Context<Self>,
    ) {
        let alto_fila = alto_fila.clamp(28.0, 44.0);
        let overscan_px = overscan_px.max(0.0);
        if self.alto_fila != alto_fila || self.hover != hover || self.overscan_px != overscan_px {
            self.alto_fila = alto_fila;
            self.hover = hover;
            self.overscan_px = overscan_px;
            cx.notify();
        }
    }

    /// Reemplaza los datos (reinicia selección; conserva orden, filtros y columnas).
    pub fn set_rows(&mut self, rows: Arc<Vec<Fila>>, cx: &mut Context<Self>) {
        self.rows = rows;
        self.estado.indexar(&self.rows[..]);
        cx.notify();
    }

    pub fn set_busqueda(&mut self, texto: &str, cx: &mut Context<Self>) {
        if self.estado.busqueda != texto {
            self.estado.busqueda = texto.to_owned();
            self.recalcular(cx);
        }
    }

    pub fn set_filtro(&mut self, col: usize, texto: &str, cx: &mut Context<Self>) {
        if texto.trim().is_empty() {
            self.estado.filtros.remove(&col);
        } else {
            self.estado.filtros.insert(col, texto.to_owned());
        }
        self.recalcular(cx);
    }

    pub fn limpiar_filtros(&mut self, cx: &mut Context<Self>) {
        self.estado.busqueda.clear();
        self.estado.filtros.clear();
        self.busqueda_input
            .update(cx, |input, cx| input.set_content_silent("", cx));
        self.recalcular(cx);
    }

    /// Copia al portapapeles (TSV con encabezados). Con `solo_seleccion` y sin
    /// selección no hace nada; sin `solo_seleccion` copia toda la vista.
    pub fn copiar(&self, solo_seleccion: bool, cx: &mut Context<Self>) {
        if solo_seleccion && self.estado.seleccion.is_empty() {
            return;
        }
        let titulos: Vec<&str> = self.columnas.iter().map(|c| c.titulo).collect();
        let tsv = self
            .estado
            .a_tsv(&self.rows[..], &titulos, true, solo_seleccion);
        cx.write_to_clipboard(ClipboardItem::new_string(tsv));
    }

    fn recalcular(&mut self, cx: &mut Context<Self>) {
        self.estado.recalcular(&self.rows[..]);
        cx.notify();
    }

    fn plan(&self) -> Plan {
        let mut cols: Vec<(usize, f32)> =
            self.estado.visibles().map(|c| (c.idx, c.ancho)).collect();
        let ancho_visible = f32::from(self.hscroll.bounds().size.width);
        let ancho_total: f32 = cols.iter().map(|(_, ancho)| ancho).sum();
        let columnas_flexibles = cols
            .iter()
            .filter(|(idx, _)| self.columnas[*idx].flex)
            .count();
        if columnas_flexibles > 0 && ancho_visible > ancho_total {
            let extra = (ancho_visible - ancho_total) / columnas_flexibles as f32;
            for (idx, ancho) in &mut cols {
                if self.columnas[*idx].flex {
                    *ancho += extra;
                }
            }
        }
        let mut lefts = Vec::with_capacity(cols.len() + 1);
        let mut acc = 0.0_f32;
        for &(_, ancho) in &cols {
            lefts.push(acc);
            acc += ancho;
        }
        lefts.push(acc);
        Plan { cols, lefts }
    }

    fn tecla(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let m = ev.keystroke.modifiers;
        let (sec, shift) = (m.secondary(), m.shift);
        let destino = match ev.keystroke.key.as_str() {
            "down" => self.estado.mover_foco(1, shift),
            "up" => self.estado.mover_foco(-1, shift),
            "pagedown" => self.estado.mover_foco(PAGINA, shift),
            "pageup" => self.estado.mover_foco(-PAGINA, shift),
            "home" => self.estado.ir_a(0, shift),
            "end" => self.estado.ir_a(usize::MAX, shift),
            "a" if sec => {
                self.estado.seleccionar_todo();
                cx.notify();
                return;
            }
            "c" if sec => {
                self.copiar(true, cx);
                return;
            }
            "escape" => {
                self.estado.limpiar_seleccion();
                self.panel_columnas = false;
                cx.notify();
                return;
            }
            _ => return,
        };
        if let Some(pos) = destino {
            self.scroll.scroll_to_item(pos, ScrollStrategy::Nearest);
        }
        cx.notify();
        cx.stop_propagation();
    }

    fn arrastrar(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(r) = &self.resize else {
            return;
        };
        if ev.pressed_button != Some(MouseButton::Left) {
            self.resize = None;
            cx.notify();
            return;
        }
        let (idx, nuevo) = (r.idx, r.ancho0 + (f32::from(ev.position.x) - r.x0));
        self.estado.redimensionar(idx, nuevo);
        cx.notify();
    }

    /// Filas visibles del `uniform_list` (solo las columnas visibles en pantalla).
    fn filas(
        &self,
        range: Range<usize>,
        c: Colores,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let plan = self.plan();
        let total_w = plan.total() + GUTTER;
        let scroll_x = -f32::from(self.hscroll.offset().x) - GUTTER;
        let view_w = f32::from(self.hscroll.bounds().size.width);
        let (first, end) = rango_visible(&plan.lefts, scroll_x, view_w, self.overscan_px);
        let pad_izq = plan.lefts[first];
        let pad_der = plan.total() - plan.lefts[end];
        let (sel_bg, foco_bg, cebra_bg) =
            (c.fg.opacity(0.10), c.fg.opacity(0.16), c.fg.opacity(0.03));

        range
            .map(|pos| {
                let orig = self.estado.vista[pos];
                let fila = &self.rows[orig];
                let seleccionada = self.estado.seleccion.contains(&orig);
                let enfocada = self.estado.foco == Some(orig);
                let mut r = div()
                    .id(("fila", orig))
                    .flex()
                    .items_center()
                    .h(px(self.alto_fila))
                    .w(px(total_w))
                    .border_b_1()
                    .border_color(c.border)
                    .text_size(px(13.))
                    .text_color(c.fg);
                if enfocada {
                    r = r.bg(foco_bg);
                } else if seleccionada {
                    r = r.bg(sel_bg);
                } else if self.cebra && pos % 2 == 1 {
                    r = r.bg(cebra_bg);
                }
                if self.hover && !seleccionada {
                    let h = c.hover;
                    r = r.hover(move |s| s.bg(h));
                }
                r.on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                    let m = ev.modifiers();
                    this.estado.click(pos, m.secondary(), m.shift);
                    window.focus(&this.focus, cx);
                    cx.notify();
                }))
                .child(
                    div()
                        .w(px(GUTTER))
                        .flex_shrink_0()
                        .flex()
                        .justify_end()
                        .px(px(8.))
                        .text_size(px(12.))
                        .text_color(c.muted)
                        .child(formatear_entero(pos + 1)),
                )
                .child(div().flex_shrink_0().w(px(pad_izq)).h_full())
                .children(plan.cols[first..end].iter().map(|&(idx, ancho)| {
                    celda(
                        ancho,
                        self.columnas[idx].tipo.es_numerico(),
                        fila.get(idx)
                            .cloned()
                            .unwrap_or_else(|| SharedString::from("")),
                    )
                }))
                .child(div().flex_shrink_0().w(px(pad_der)).h_full())
                .into_any_element()
            })
            .collect()
    }
}

impl Render for TablaVista {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = {
            let t = cx.theme();
            Colores {
                border: t.border(),
                surface: t.surface(),
                fg: t.fg(),
                muted: t.fg_muted(),
                hover: t.surface_tertiary(),
            }
        };
        let plan = self.plan();
        let total_w = plan.total() + GUTTER;
        let totales = self.estado.totales().clone();
        let tiene_sumas = self.columnas.iter().any(|col| col.tipo == Tipo::Suma);
        let filtrando = !self.estado.busqueda.trim().is_empty()
            || self.estado.filtros.values().any(|v| !v.trim().is_empty());

        let boton = |id: &'static str, texto: &'static str| {
            button(id, texto).control_size(ControlSize::Small)
        };

        // ---- Barra superior de la tabla
        let mut resumen = format!(
            "{} de {} filas",
            formatear_entero(totales.visibles),
            formatear_entero(totales.total_filas)
        );
        if totales.seleccionadas > 0 {
            resumen.push_str(&format!(
                " · {} seleccionadas",
                formatear_entero(totales.seleccionadas)
            ));
            if tiene_sumas {
                resumen.push_str(" · totales de la selección");
            }
        }
        let toolbar = div()
            .h(px(TOOLBAR_H))
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .px(px(8.))
            .border_b_1()
            .border_color(c.border)
            .child(div().text_size(px(12.)).text_color(c.muted).child(resumen))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().text_size(px(12.)).text_color(c.muted).child("Buscar"))
                    .child(
                        div()
                            .id("tabla-busqueda")
                            .w(px(220.))
                            .on_key_down(|_: &KeyDownEvent, _, cx| cx.stop_propagation())
                            .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| {
                                cx.stop_propagation()
                            })
                            .on_click(|_: &ClickEvent, _, cx| cx.stop_propagation())
                            .child(
                                text_field(&self.busqueda_input, cx)
                                    .placeholder("Buscar en la tabla…"),
                            ),
                    )
                    .when(filtrando, |d| {
                        d.child(boton("tabla-limpiar", "Limpiar filtros").on_click(
                            cx.listener(|this, _: &ClickEvent, _w, cx| this.limpiar_filtros(cx)),
                        ))
                    })
                    .child(boton("tabla-copiar", "Copiar").on_click(cx.listener(
                        |this, _: &ClickEvent, _w, cx| {
                            let solo = !this.estado.seleccion.is_empty();
                            this.copiar(solo, cx);
                        },
                    )))
                    .child(boton("tabla-columnas", "Columnas").on_click(cx.listener(
                        |this, _: &ClickEvent, _w, cx| {
                            this.panel_columnas = !this.panel_columnas;
                            cx.notify();
                        },
                    ))),
            );

        // ---- Encabezado (orden + resize)
        let header = div()
            .flex()
            .items_center()
            .h(px(theme::HEADER_HEIGHT))
            .w(px(total_w))
            .border_b_1()
            .border_color(c.border)
            .bg(c.surface)
            .text_size(px(12.))
            .text_color(c.muted)
            .child(
                div()
                    .w(px(GUTTER))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .px(px(8.))
                    .child("#"),
            )
            .children(plan.cols.iter().map(|&(idx, ancho)| {
                let col = &self.columnas[idx];
                let etiqueta = match self.estado.indicador(idx) {
                    Some((dir, rango)) => {
                        let flecha = if dir == Direccion::Asc { "▲" } else { "▼" };
                        if self.estado.orden.len() > 1 {
                            format!("{} {flecha}{rango}", col.titulo)
                        } else {
                            format!("{} {flecha}", col.titulo)
                        }
                    }
                    None => col.titulo.to_string(),
                };
                let asa = div()
                    .id(("resize", idx))
                    .absolute()
                    .top_0()
                    .right_0()
                    .h_full()
                    .w(px(6.))
                    .cursor_col_resize()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _w, cx| {
                            this.resize = Some(Resize {
                                idx,
                                x0: f32::from(ev.position.x),
                                ancho0: ancho,
                            });
                            cx.stop_propagation();
                        }),
                    );
                let mut th = div()
                    .id(("th", idx))
                    .relative()
                    .flex_shrink_0()
                    .w(px(ancho))
                    .h_full()
                    .flex()
                    .items_center()
                    .px(px(8.))
                    .truncate()
                    .tab_stop(true)
                    .cursor_pointer()
                    .hover(move |s| s.bg(c.hover))
                    .focus_visible(move |s| s.bg(c.fg.opacity(0.14)))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, _w, cx| {
                        this.estado.alternar_orden(idx, ev.modifiers().shift);
                        this.recalcular(cx);
                    }))
                    .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, _w, cx| {
                        if matches!(ev.keystroke.key.as_str(), "enter" | "space") {
                            this.estado
                                .alternar_orden(idx, ev.keystroke.modifiers.shift);
                            this.recalcular(cx);
                            cx.stop_propagation();
                        }
                    }));
                if col.tipo.es_numerico() {
                    th = th.justify_end().text_right();
                }
                th.child(etiqueta).child(asa)
            }));

        // ---- Cuerpo (virtualizado) o estado vacío
        let mensaje = move |titulo: &'static str, detalle: &'static str| {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .text_color(c.muted)
                .child(titulo)
                .child(div().text_size(px(12.)).child(detalle))
        };
        let body = if self.rows.is_empty() {
            let detalle = if self.list_id == "error-rows" {
                "No se registraron errores en este ZIP."
            } else {
                "El procesamiento no generó filas para esta vista."
            };
            mensaje("Sin filas para mostrar", detalle).into_any_element()
        } else if self.estado.vista.is_empty() {
            mensaje(
                "Sin resultados",
                "Ninguna fila coincide con la búsqueda o los filtros.",
            )
            .into_any_element()
        } else {
            uniform_list(
                self.list_id,
                self.estado.vista.len(),
                cx.processor(move |this, range: Range<usize>, _w, cx| this.filas(range, c, cx)),
            )
            .size_full()
            .track_scroll(&self.scroll)
            .scrollbar_width(px(8.))
            .into_any_element()
        };

        // ---- Pie con totales (solo columnas `Suma`)
        let footer = tiene_sumas.then(|| {
            div()
                .flex()
                .items_center()
                .h(px(FOOTER_H))
                .w(px(total_w))
                .border_t_1()
                .border_color(c.border)
                .bg(c.surface)
                .text_size(px(12.))
                .text_color(c.fg)
                .child(
                    div()
                        .w(px(GUTTER))
                        .flex_shrink_0()
                        .flex()
                        .justify_end()
                        .px(px(8.))
                        .text_color(c.muted)
                        .child("Σ"),
                )
                .children(plan.cols.iter().map(|&(idx, ancho)| {
                    let texto = totales
                        .sumas
                        .get(idx)
                        .copied()
                        .flatten()
                        .map(formatear_monto)
                        .unwrap_or_default();
                    celda(
                        ancho,
                        self.columnas[idx].tipo.es_numerico(),
                        SharedString::from(texto),
                    )
                }))
        });

        let ancho_visible = f32::from(self.hscroll.bounds().size.width);
        let max_scroll = (total_w - ancho_visible).max(0.0);
        let at_start = self.hscroll.offset().x >= px(0.);
        let at_end = -f32::from(self.hscroll.offset().x) >= max_scroll;
        let navigation_bar = (max_scroll > 0.0).then(|| {
            let start = self.hscroll.clone();
            let previous = self.hscroll.clone();
            let next = self.hscroll.clone();
            let end = self.hscroll.clone();
            div()
                .w_full()
                .h(px(36.))
                .flex()
                .items_center()
                .justify_end()
                .gap(px(6.))
                .px(px(8.))
                .border_t_1()
                .border_color(c.border)
                .children([
                    button("tabla-scroll-inicio", "Inicio")
                        .control_size(ControlSize::Small)
                        .disabled(at_start)
                        .on_click(move |_, window, _| {
                            let offset = start.offset();
                            start.set_offset(point(px(0.), offset.y));
                            window.refresh();
                        })
                        .into_any_element(),
                    button("tabla-scroll-anterior", "Anterior")
                        .control_size(ControlSize::Small)
                        .disabled(at_start)
                        .on_click(move |_, window, _| {
                            let offset = previous.offset();
                            let x = (offset.x + px(360.)).min(px(0.));
                            previous.set_offset(point(x, offset.y));
                            window.refresh();
                        })
                        .into_any_element(),
                    button("tabla-scroll-siguiente", "Siguiente")
                        .control_size(ControlSize::Small)
                        .disabled(at_end)
                        .on_click(move |_, window, _| {
                            let offset = next.offset();
                            let limit = if next.max_offset().x > px(0.) {
                                -next.max_offset().x
                            } else {
                                px(-max_scroll)
                            };
                            let x = (offset.x - px(360.)).max(limit);
                            next.set_offset(point(x, offset.y));
                            window.refresh();
                        })
                        .into_any_element(),
                    button("tabla-scroll-final", "Final")
                        .control_size(ControlSize::Small)
                        .disabled(at_end)
                        .on_click(move |_, window, _| {
                            let offset = end.offset();
                            let limit = if end.max_offset().x > px(0.) {
                                -end.max_offset().x
                            } else {
                                px(-max_scroll)
                            };
                            end.set_offset(point(limit, offset.y));
                            window.refresh();
                        })
                        .into_any_element(),
                ])
        });

        // ---- Panel de columnas (mostrar/ocultar/reordenar)
        let panel = self.panel_columnas.then(|| {
            let column_count = self.estado.columnas.len();
            div()
                .id("panel-columnas")
                .occlude()
                .absolute()
                .top(px(TOOLBAR_H))
                .right(px(8.))
                .w(px(280.))
                .max_h(px(360.))
                .overflow_y_scroll()
                .p(px(8.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .bg(c.surface)
                .border_1()
                .border_color(c.border)
                .rounded(px(6.))
                .text_size(px(12.))
                .text_color(c.fg)
                .children(self.estado.columnas.iter().enumerate().map(|(p, ce)| {
                    let idx = ce.idx;
                    div()
                        .id(("col-fila", idx))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .h(px(24.))
                        .px(px(4.))
                        .border_b_1()
                        .border_color(c.border)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(self.columnas[idx].titulo),
                        )
                        .child(
                            button(
                                ("col-visible", idx),
                                if ce.visible { "Ocultar" } else { "Mostrar" },
                            )
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.estado.alternar_visible(idx);
                                    cx.notify();
                                },
                            )),
                        )
                        .child(
                            button(("col-subir", idx), "Subir")
                                .control_size(ControlSize::Small)
                                .disabled(p == 0)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.estado.mover_columna(p, p - 1);
                                    cx.notify();
                                })),
                        )
                        .child(
                            button(("col-bajar", idx), "Bajar")
                                .control_size(ControlSize::Small)
                                .disabled(p + 1 == column_count)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.estado.mover_columna(p, p + 1);
                                    cx.notify();
                                })),
                        )
                }))
                .child(div().mt(px(6.)).child(
                    boton("tabla-restaurar", "Restaurar columnas").on_click(cx.listener(
                        |this, _: &ClickEvent, _w, cx| {
                            this.estado.restaurar_columnas();
                            cx.notify();
                        },
                    )),
                ))
        });

        // ---- Raíz
        div()
            .id("tabla")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(c.surface)
            .key_context("TablaVista")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _w, cx| this.tecla(ev, cx)))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _w, cx| this.arrastrar(ev, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _w, cx| {
                    if this.resize.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _w, cx| {
                    if this.resize.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .child(toolbar)
            .child(
                div()
                    .id("tabla-hscroll")
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .scrollbar_width(px(8.))
                    .track_scroll(&self.hscroll)
                    .when(max_scroll > 0.0, |element| {
                        let horizontal_scroll = self.hscroll.clone();
                        element.on_scroll_wheel(move |event, window, cx| {
                            let delta = event.delta.pixel_delta(window.line_height());
                            if event.modifiers.shift || delta.x != px(0.) {
                                let horizontal_delta =
                                    if delta.x != px(0.) { delta.x } else { delta.y };
                                let offset = horizontal_scroll.offset();
                                let limit = if horizontal_scroll.max_offset().x > px(0.) {
                                    -horizontal_scroll.max_offset().x
                                } else {
                                    px(-max_scroll)
                                };
                                horizontal_scroll.set_offset(point(
                                    (offset.x + horizontal_delta).clamp(limit, px(0.)),
                                    offset.y,
                                ));
                                cx.stop_propagation();
                                window.refresh();
                            }
                        })
                    })
                    .child(
                        div()
                            .w(px(total_w))
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(header)
                            .child(div().flex_1().min_h_0().overflow_hidden().child(body))
                            .children(footer),
                    ),
            )
            .children(navigation_bar)
            .children(panel)
    }
}
