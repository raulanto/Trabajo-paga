//! Estado y lógica pura de la tabla (sin GPUI): orden multi-columna, búsqueda,
//! filtros por columna, selección, columnas (visibilidad/orden/ancho), totales
//! y copiado a TSV. Se prueba con `cargo test` sin abrir ninguna ventana.
//!
//! Regla de AGENTS.md: nada se inventa. Una suma sin valores es `None`
//! (celda vacía), nunca 0.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use std::str::FromStr;

use rust_decimal::Decimal;

pub const MIN_ANCHO: f32 = 56.0;
pub const MAX_ANCHO: f32 = 640.0;

/// Fila ya formateada: un texto por columna.
pub type Fila<S> = Box<[S]>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tipo {
    /// Texto: orden alfabético sin acentos ni mayúsculas, alineado a la izquierda.
    Texto,
    /// Número: orden numérico, alineado a la derecha, sin total en el pie.
    Numero,
    /// Número con total en el pie. Úsese SOLO en columnas a nivel concepto
    /// (p. ej. Importe); sumar columnas del comprobante repetidas por concepto
    /// duplicaría montos.
    Suma,
}

impl Tipo {
    pub fn es_numerico(self) -> bool {
        !matches!(self, Tipo::Texto)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direccion {
    Asc,
    Desc,
}

#[derive(Clone, Debug)]
pub struct ColumnaEstado {
    /// Índice de la columna original (en `Columna`/`Fila`).
    pub idx: usize,
    pub visible: bool,
    pub ancho: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Totales {
    /// Filas en la vista (después de filtrar).
    pub visibles: usize,
    pub total_filas: usize,
    pub seleccionadas: usize,
    /// true si las sumas son sobre la selección; false si son sobre la vista.
    pub sobre_seleccion: bool,
    /// Por columna original. `Some` solo en `Tipo::Suma` con al menos un valor.
    pub sumas: Vec<Option<Decimal>>,
}

#[derive(Clone, Debug)]
enum Clave {
    Vacio,
    Num(Decimal),
    Txt(String),
}

pub struct TableState {
    tipos: Vec<Tipo>,
    anchos_base: Vec<f32>,
    /// En orden de visualización.
    pub columnas: Vec<ColumnaEstado>,
    /// Criterios de orden (columna original, dirección), el primero manda.
    pub orden: Vec<(usize, Direccion)>,
    pub busqueda: String,
    pub filtros: HashMap<usize, String>,
    /// Índices de filas ORIGINALES tras filtrar y ordenar.
    pub vista: Vec<usize>,
    /// Índices ORIGINALES: la selección sobrevive a reordenar.
    pub seleccion: BTreeSet<usize>,
    /// Ancla y foco (índices originales).
    pub ancla: Option<usize>,
    pub foco: Option<usize>,
    haystack: Vec<String>,
    /// fila original -> posición en `vista` (usize::MAX si está oculta).
    pos: Vec<usize>,
    sum_cols: Vec<usize>,
    /// valores[k][fila] para la columna `sum_cols[k]`, parseados una sola vez.
    valores: Vec<Vec<Option<Decimal>>>,
    total_filas: usize,
    cache: Option<Totales>,
}

impl TableState {
    pub fn nuevo(tipos: Vec<Tipo>, anchos: Vec<f32>) -> Self {
        let columnas = anchos
            .iter()
            .enumerate()
            .map(|(idx, &ancho)| ColumnaEstado {
                idx,
                visible: true,
                ancho,
            })
            .collect();
        let sum_cols = tipos
            .iter()
            .enumerate()
            .filter(|(_, t)| **t == Tipo::Suma)
            .map(|(i, _)| i)
            .collect();
        Self {
            tipos,
            anchos_base: anchos,
            columnas,
            orden: Vec::new(),
            busqueda: String::new(),
            filtros: HashMap::new(),
            vista: Vec::new(),
            seleccion: BTreeSet::new(),
            ancla: None,
            foco: None,
            haystack: Vec::new(),
            pos: Vec::new(),
            sum_cols,
            valores: Vec::new(),
            total_filas: 0,
            cache: None,
        }
    }

    pub fn tipo(&self, col: usize) -> Tipo {
        self.tipos.get(col).copied().unwrap_or(Tipo::Texto)
    }

    /// Prepara cachés para filas nuevas y reinicia selección. Llamar al recibir resultados.
    pub fn indexar<S: AsRef<str>>(&mut self, rows: &[Fila<S>]) {
        self.haystack = rows
            .iter()
            .map(|f| {
                f.iter()
                    .map(|c| plegar(c.as_ref()))
                    .collect::<Vec<_>>()
                    .join("\u{1f}")
            })
            .collect();
        self.valores = self
            .sum_cols
            .iter()
            .map(|&c| {
                rows.iter()
                    .map(|f| f.get(c).and_then(|s| parse_decimal(s.as_ref())))
                    .collect()
            })
            .collect();
        self.total_filas = rows.len();
        self.seleccion.clear();
        self.ancla = None;
        self.foco = None;
        self.recalcular(rows);
    }

    /// Recalcula `vista` (filtro + orden). Llamar tras cambiar búsqueda, filtros u orden.
    pub fn recalcular<S: AsRef<str>>(&mut self, rows: &[Fila<S>]) {
        let tokens: Vec<String> = self.busqueda.split_whitespace().map(plegar).collect();
        let filtros: Vec<(usize, String)> = self
            .filtros
            .iter()
            .filter(|(_, v)| !v.trim().is_empty())
            .map(|(c, v)| (*c, plegar(v.trim())))
            .collect();

        let mut idx: Vec<usize> = {
            let haystack = &self.haystack;
            (0..rows.len())
                .filter(|&i| {
                    tokens.iter().all(|t| haystack[i].contains(t.as_str()))
                        && filtros.iter().all(|(c, v)| {
                            rows[i]
                                .get(*c)
                                .is_some_and(|s| plegar(s.as_ref()).contains(v.as_str()))
                        })
                })
                .collect()
        };

        if !self.orden.is_empty() {
            let claves: Vec<Vec<Clave>> = self
                .orden
                .iter()
                .map(|&(c, _)| {
                    let numerico = self.tipo(c).es_numerico();
                    idx.iter()
                        .map(|&o| clave(rows[o].get(c).map(|s| s.as_ref()), numerico))
                        .collect()
                })
                .collect();
            let mut perm: Vec<usize> = (0..idx.len()).collect();
            perm.sort_by(|&a, &b| {
                for (k, &(_, dir)) in claves.iter().zip(&self.orden) {
                    let o = cmp_clave(&k[a], &k[b], dir);
                    if o != Ordering::Equal {
                        return o;
                    }
                }
                Ordering::Equal
            });
            idx = perm.into_iter().map(|p| idx[p]).collect();
        }

        self.pos = vec![usize::MAX; rows.len()];
        for (p, &o) in idx.iter().enumerate() {
            self.pos[o] = p;
        }
        self.vista = idx;

        let pos = &self.pos;
        self.seleccion
            .retain(|&i| pos.get(i).is_some_and(|&p| p != usize::MAX));
        let oculta = |o: usize| pos.get(o).is_none_or(|&p| p == usize::MAX);
        if self.foco.is_some_and(oculta) {
            self.foco = None;
        }
        if self.ancla.is_some_and(oculta) {
            self.ancla = None;
        }
        self.cache = None;
    }

    // ---------------------------------------------------------------- orden

    /// Ciclo Asc → Desc → sin orden. Con `anadir` (Shift+clic) se agrega como criterio secundario.
    pub fn alternar_orden(&mut self, col: usize, anadir: bool) {
        let actual = self.orden.iter().position(|&(c, _)| c == col);
        let siguiente = match actual.map(|p| self.orden[p].1) {
            None => Some(Direccion::Asc),
            Some(Direccion::Asc) => Some(Direccion::Desc),
            Some(Direccion::Desc) => None,
        };
        if !anadir {
            self.orden.clear();
            if let Some(d) = siguiente {
                self.orden.push((col, d));
            }
            return;
        }
        match (actual, siguiente) {
            (Some(p), Some(d)) => self.orden[p].1 = d,
            (Some(p), None) => {
                self.orden.remove(p);
            }
            (None, _) => self.orden.push((col, Direccion::Asc)),
        }
    }

    /// Dirección y posición (1-based) del criterio de orden de una columna.
    pub fn indicador(&self, col: usize) -> Option<(Direccion, usize)> {
        self.orden
            .iter()
            .position(|&(c, _)| c == col)
            .map(|p| (self.orden[p].1, p + 1))
    }

    // ------------------------------------------------------------ selección

    pub fn pos_de(&self, original: usize) -> Option<usize> {
        self.pos.get(original).copied().filter(|&p| p != usize::MAX)
    }

    /// Clic en la posición `pos` de la vista. `ctrl` = Ctrl/Cmd, `shift` = rango.
    pub fn click(&mut self, pos: usize, ctrl: bool, shift: bool) {
        let Some(&orig) = self.vista.get(pos) else {
            return;
        };
        if shift {
            let desde = self.ancla.and_then(|a| self.pos_de(a)).unwrap_or(pos);
            let (lo, hi) = (desde.min(pos), desde.max(pos));
            if !ctrl {
                self.seleccion.clear();
            }
            for p in lo..=hi {
                self.seleccion.insert(self.vista[p]);
            }
            if self.ancla.is_none() {
                self.ancla = Some(self.vista[desde]);
            }
            self.foco = Some(orig);
        } else if ctrl {
            if !self.seleccion.remove(&orig) {
                self.seleccion.insert(orig);
            }
            self.ancla = Some(orig);
            self.foco = Some(orig);
        } else {
            self.seleccion.clear();
            self.seleccion.insert(orig);
            self.ancla = Some(orig);
            self.foco = Some(orig);
        }
        self.cache = None;
    }

    /// Mueve el foco `delta` filas. Devuelve la nueva posición (para hacer scroll).
    pub fn mover_foco(&mut self, delta: isize, extender: bool) -> Option<usize> {
        if self.vista.is_empty() {
            return None;
        }
        let ultimo = self.vista.len() - 1;
        let nueva = match self.foco.and_then(|f| self.pos_de(f)) {
            None => {
                if delta >= 0 {
                    0
                } else {
                    ultimo
                }
            }
            Some(p) => (p as isize + delta).clamp(0, ultimo as isize) as usize,
        };
        self.ir_a(nueva, extender)
    }

    /// Lleva el foco a la posición `pos` (se ajusta al rango válido).
    pub fn ir_a(&mut self, pos: usize, extender: bool) -> Option<usize> {
        if self.vista.is_empty() {
            return None;
        }
        let pos = pos.min(self.vista.len() - 1);
        if extender && self.ancla.is_none() {
            self.ancla = self.foco.or(Some(self.vista[pos]));
        }
        self.click(pos, false, extender);
        Some(pos)
    }

    pub fn seleccionar_todo(&mut self) {
        self.seleccion = self.vista.iter().copied().collect();
        self.cache = None;
    }

    pub fn limpiar_seleccion(&mut self) {
        self.seleccion.clear();
        self.cache = None;
    }

    // ------------------------------------------------------------- columnas

    pub fn visibles(&self) -> impl Iterator<Item = &ColumnaEstado> {
        self.columnas.iter().filter(|c| c.visible)
    }

    pub fn ancho_total(&self) -> f32 {
        self.visibles().map(|c| c.ancho).sum()
    }

    /// Oculta/muestra una columna. No permite ocultar la última visible.
    pub fn alternar_visible(&mut self, idx: usize) -> bool {
        let n_visibles = self.visibles().count();
        let Some(c) = self.columnas.iter_mut().find(|c| c.idx == idx) else {
            return false;
        };
        if c.visible && n_visibles <= 1 {
            return false;
        }
        c.visible = !c.visible;
        true
    }

    /// Mueve una columna entre posiciones de `columnas` (orden de visualización).
    pub fn mover_columna(&mut self, desde: usize, hasta: usize) {
        let n = self.columnas.len();
        if desde >= n || hasta >= n || desde == hasta {
            return;
        }
        let c = self.columnas.remove(desde);
        self.columnas.insert(hasta, c);
    }

    pub fn redimensionar(&mut self, idx: usize, ancho: f32) {
        if let Some(c) = self.columnas.iter_mut().find(|c| c.idx == idx) {
            c.ancho = ancho.clamp(MIN_ANCHO, MAX_ANCHO);
        }
    }

    pub fn restaurar_columnas(&mut self) {
        self.columnas = self
            .anchos_base
            .iter()
            .enumerate()
            .map(|(idx, &ancho)| ColumnaEstado {
                idx,
                visible: true,
                ancho,
            })
            .collect();
    }

    // -------------------------------------------------------------- totales

    /// Totales cacheados: sobre la selección si existe, si no sobre la vista.
    pub fn totales(&mut self) -> &Totales {
        if self.cache.is_none() {
            let sobre_seleccion = !self.seleccion.is_empty();
            let mut sumas: Vec<Option<Decimal>> = vec![None; self.tipos.len()];
            for &o in self
                .vista
                .iter()
                .filter(|o| !sobre_seleccion || self.seleccion.contains(*o))
            {
                for (k, &c) in self.sum_cols.iter().enumerate() {
                    if let Some(Some(d)) = self.valores[k].get(o) {
                        sumas[c] = Some(sumas[c].unwrap_or(Decimal::ZERO) + d);
                    }
                }
            }
            self.cache = Some(Totales {
                visibles: self.vista.len(),
                total_filas: self.total_filas,
                seleccionadas: self.seleccion.len(),
                sobre_seleccion,
                sumas,
            });
        }
        self.cache.get_or_insert_with(Totales::default)
    }

    // ---------------------------------------------------------------- copiar

    /// TSV de las columnas visibles (en orden de pantalla). Con `solo_seleccion`
    /// copia únicamente las filas seleccionadas (en orden de vista); si no, toda la vista.
    pub fn a_tsv<S: AsRef<str>>(
        &self,
        rows: &[Fila<S>],
        titulos: &[&str],
        con_encabezados: bool,
        solo_seleccion: bool,
    ) -> String {
        let cols: Vec<usize> = self.visibles().map(|c| c.idx).collect();
        let limpiar = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        let mut lineas: Vec<String> = Vec::new();
        if con_encabezados {
            lineas.push(
                cols.iter()
                    .map(|&c| limpiar(titulos.get(c).copied().unwrap_or("")))
                    .collect::<Vec<_>>()
                    .join("\t"),
            );
        }
        for &o in &self.vista {
            if solo_seleccion && !self.seleccion.contains(&o) {
                continue;
            }
            lineas.push(
                cols.iter()
                    .map(|&c| limpiar(rows[o].get(c).map_or("", |s| s.as_ref())))
                    .collect::<Vec<_>>()
                    .join("\t"),
            );
        }
        lineas.join("\n")
    }
}

// ------------------------------------------------------------------ helpers

/// Minúsculas y sin acentos (también ñ→n, útil para buscar).
pub fn plegar(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

/// Acepta "1,234.50", "$ 10", "-3". Vacío o no numérico → None.
pub fn parse_decimal(s: &str) -> Option<Decimal> {
    let limpio: String = s
        .chars()
        .filter(|c| !matches!(c, ',' | ' ' | '$' | '\u{a0}'))
        .collect();
    if limpio.is_empty() {
        return None;
    }
    Decimal::from_str(&limpio).ok()
}

/// "1,234,567.89" (presentación; redondea a 2 decimales).
pub fn formatear_monto(d: Decimal) -> String {
    let s = format!("{:.2}", d.round_dp(2));
    let (signo, resto) = match s.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", s.as_str()),
    };
    let (entero, dec) = resto.split_once('.').unwrap_or((resto, "00"));
    format!("{signo}{}.{dec}", con_miles(entero))
}

pub fn formatear_entero(n: usize) -> String {
    con_miles(&n.to_string())
}

fn con_miles(digitos: &str) -> String {
    // Recorre de derecha a izquierda e inserta una coma cada 3 dígitos.
    let mut invertido = String::with_capacity(digitos.len() + digitos.len() / 3);
    let mut grupo = 0;
    for ch in digitos.chars().rev() {
        if grupo == 3 {
            invertido.push(',');
            grupo = 0;
        }
        invertido.push(ch);
        grupo += 1;
    }
    invertido.chars().rev().collect()
}

fn clave(celda: Option<&str>, numerico: bool) -> Clave {
    let s = celda.unwrap_or("").trim();
    if s.is_empty() {
        Clave::Vacio
    } else if numerico {
        parse_decimal(s).map_or_else(|| Clave::Txt(plegar(s)), Clave::Num)
    } else {
        Clave::Txt(plegar(s))
    }
}

/// Vacíos siempre al final (sin importar la dirección); números antes que texto.
fn cmp_clave(a: &Clave, b: &Clave, dir: Direccion) -> Ordering {
    let aplicar = |o: Ordering| match dir {
        Direccion::Asc => o,
        Direccion::Desc => o.reverse(),
    };
    match (a, b) {
        (Clave::Vacio, Clave::Vacio) => Ordering::Equal,
        (Clave::Vacio, _) => Ordering::Greater,
        (_, Clave::Vacio) => Ordering::Less,
        (Clave::Num(x), Clave::Num(y)) => aplicar(x.cmp(y)),
        (Clave::Txt(x), Clave::Txt(y)) => aplicar(x.cmp(y)),
        (Clave::Num(_), Clave::Txt(_)) => Ordering::Less,
        (Clave::Txt(_), Clave::Num(_)) => Ordering::Greater,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type F = Fila<String>;

    fn fila(celdas: &[&str]) -> F {
        celdas.iter().map(|s| s.to_string()).collect()
    }

    /// Columnas: 0 Texto, 1 Numero, 2 Suma.
    fn estado(rows: &[F]) -> TableState {
        let mut e = TableState::nuevo(
            vec![Tipo::Texto, Tipo::Numero, Tipo::Suma],
            vec![100.0, 100.0, 100.0],
        );
        e.indexar(rows);
        e
    }

    fn col(e: &TableState, rows: &[F], c: usize) -> Vec<String> {
        e.vista.iter().map(|&o| rows[o][c].clone()).collect()
    }

    #[test]
    fn orden_numerico_y_vacios_al_final_en_ambas_direcciones() {
        let rows = vec![
            fila(&["a", "10", ""]),
            fila(&["b", "9", ""]),
            fila(&["c", "", ""]),
            fila(&["d", "1,000.50", ""]),
            fila(&["e", "100", ""]),
        ];
        let mut e = estado(&rows);
        e.alternar_orden(1, false);
        e.recalcular(&rows);
        assert_eq!(col(&e, &rows, 1), ["9", "10", "100", "1,000.50", ""]);
        e.alternar_orden(1, false); // Desc
        e.recalcular(&rows);
        assert_eq!(col(&e, &rows, 1), ["1,000.50", "100", "10", "9", ""]);
        e.alternar_orden(1, false); // sin orden
        e.recalcular(&rows);
        assert_eq!(col(&e, &rows, 0), ["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn orden_texto_sin_acentos_ni_mayusculas_y_estable() {
        let rows = vec![
            fila(&["árbol", "", ""]),
            fila(&["Zeta", "", ""]),
            fila(&["abeja", "", ""]),
            fila(&["Arbol", "", ""]),
        ];
        let mut e = estado(&rows);
        e.alternar_orden(0, false);
        e.recalcular(&rows);
        assert_eq!(col(&e, &rows, 0), ["abeja", "árbol", "Arbol", "Zeta"]);
    }

    #[test]
    fn orden_multiple_con_shift() {
        let rows = vec![
            fila(&["I", "5", ""]),
            fila(&["E", "7", ""]),
            fila(&["I", "9", ""]),
            fila(&["E", "1", ""]),
        ];
        let mut e = estado(&rows);
        e.alternar_orden(0, false); // tipo asc
        e.alternar_orden(1, true); // importe asc
        e.alternar_orden(1, true); // importe desc
        e.recalcular(&rows);
        assert_eq!(col(&e, &rows, 1), ["7", "1", "9", "5"]);
        assert_eq!(e.indicador(0), Some((Direccion::Asc, 1)));
        assert_eq!(e.indicador(1), Some((Direccion::Desc, 2)));
        assert_eq!(e.indicador(2), None);
    }

    #[test]
    fn busqueda_por_tokens_y_filtro_por_columna() {
        let rows = vec![
            fila(&["Ganado para engorda", "10", ""]),
            fila(&["Asesoría contable", "20", ""]),
            fila(&["Ganado lechero", "30", ""]),
        ];
        let mut e = estado(&rows);
        e.busqueda = "GANADO  engorda".into();
        e.recalcular(&rows);
        assert_eq!(e.vista, [0]);
        e.busqueda = "asesoria".into(); // sin acento encuentra "Asesoría"
        e.recalcular(&rows);
        assert_eq!(e.vista, [1]);
        e.busqueda.clear();
        e.filtros.insert(0, "lechero".into());
        e.recalcular(&rows);
        assert_eq!(e.vista, [2]);
        e.filtros.insert(0, "  ".into()); // filtro en blanco = sin filtro
        e.recalcular(&rows);
        assert_eq!(e.vista.len(), 3);
    }

    #[test]
    fn seleccion_simple_ctrl_y_rango_sobre_la_vista_ordenada() {
        let rows = vec![
            fila(&["d", "4", ""]),
            fila(&["a", "1", ""]),
            fila(&["c", "3", ""]),
            fila(&["b", "2", ""]),
        ];
        let mut e = estado(&rows);
        e.alternar_orden(0, false);
        e.recalcular(&rows); // vista: a b c d = orig 1,3,2,0
        e.click(1, false, false);
        assert_eq!(e.seleccion.iter().copied().collect::<Vec<_>>(), [3]);
        e.click(3, false, true); // rango pos 1..=3 => b c d
        assert_eq!(e.seleccion.iter().copied().collect::<Vec<_>>(), [0, 2, 3]);
        e.click(2, true, false); // ctrl quita c (orig 2)
        assert_eq!(e.seleccion.iter().copied().collect::<Vec<_>>(), [0, 3]);
        e.click(0, true, false); // ctrl agrega a (orig 1)
        assert_eq!(e.seleccion.len(), 3);
    }

    #[test]
    fn la_seleccion_sobrevive_al_reordenar_y_se_depura_al_filtrar() {
        let rows = vec![fila(&["x", "1", ""]), fila(&["y", "2", ""])];
        let mut e = estado(&rows);
        e.click(0, false, false); // orig 0
        e.alternar_orden(1, false);
        e.alternar_orden(1, false); // desc
        e.recalcular(&rows);
        assert!(e.seleccion.contains(&0));
        assert_eq!(e.pos_de(0), Some(1));
        e.busqueda = "y".into();
        e.recalcular(&rows);
        assert!(e.seleccion.is_empty());
        assert_eq!(e.foco, None);
    }

    #[test]
    fn mover_foco_limita_extiende_y_maneja_vacio() {
        let rows: Vec<F> = (0..5).map(|i| fila(&["t", &i.to_string(), ""])).collect();
        let mut e = estado(&rows);
        assert_eq!(e.mover_foco(1, false), Some(0)); // sin foco: arranca arriba
        assert_eq!(e.mover_foco(-1, false), Some(0)); // clamp
        assert_eq!(e.mover_foco(100, false), Some(4)); // clamp
        assert_eq!(e.mover_foco(-2, true), Some(2)); // extiende 4..=2
        assert_eq!(e.seleccion.iter().copied().collect::<Vec<_>>(), [2, 3, 4]);
        assert_eq!(e.ir_a(0, false), Some(0));
        let vacio: Vec<F> = vec![];
        let mut v = estado(&vacio);
        assert_eq!(v.mover_foco(1, false), None);
        assert!(v.vista.is_empty());
    }

    #[test]
    fn totales_sobre_vista_o_seleccion_sin_inventar_ceros() {
        let rows = vec![
            fila(&["a", "1", "10.50"]),
            fila(&["b", "2", "1,000"]),
            fila(&["c", "3", ""]),
            fila(&["d", "4", "x"]),
        ];
        let mut e = estado(&rows);
        let t = e.totales().clone();
        assert_eq!(t.sumas[2], Some(Decimal::from_str("1010.50").unwrap()));
        assert_eq!(t.sumas[1], None); // Numero no suma
        assert_eq!(
            (t.visibles, t.total_filas, t.sobre_seleccion),
            (4, 4, false)
        );

        e.click(0, false, false); // solo "a"
        let t = e.totales().clone();
        assert_eq!(t.sumas[2], Some(Decimal::from_str("10.50").unwrap()));
        assert!(t.sobre_seleccion);

        e.limpiar_seleccion();
        e.busqueda = "c".into();
        e.recalcular(&rows);
        assert_eq!(e.totales().sumas[2], None); // la fila "c" no tiene importe
    }

    #[test]
    fn tsv_respeta_columnas_visibles_orden_y_sanea() {
        let rows = vec![
            fila(&["uno\tdos", "1", "5"]),
            fila(&["tres\ncuatro", "2", "6"]),
        ];
        let mut e = estado(&rows);
        e.alternar_visible(1);
        e.mover_columna(2, 0); // Importe primero
        let t = ["Texto", "Num", "Importe"];
        assert_eq!(
            e.a_tsv(&rows, &t, true, false),
            "Importe\tTexto\n5\tuno dos\n6\ttres cuatro"
        );
        e.click(1, false, false);
        assert_eq!(e.a_tsv(&rows, &t, false, true), "6\ttres cuatro");
    }

    #[test]
    fn columnas_visibilidad_resize_y_restaurar() {
        let mut e = TableState::nuevo(vec![Tipo::Texto; 3], vec![100.0, 120.0, 140.0]);
        assert!(e.alternar_visible(0));
        assert!(e.alternar_visible(1));
        assert!(!e.alternar_visible(2), "no se oculta la última visible");
        assert_eq!(e.ancho_total(), 140.0);
        e.redimensionar(2, 5.0);
        assert_eq!(e.ancho_total(), MIN_ANCHO);
        e.redimensionar(2, 9999.0);
        assert_eq!(e.ancho_total(), MAX_ANCHO);
        e.mover_columna(2, 0);
        assert_eq!(e.columnas[0].idx, 2);
        e.restaurar_columnas();
        assert_eq!(e.ancho_total(), 360.0);
        assert_eq!(e.columnas[0].idx, 0);
    }

    #[test]
    fn formato_de_montos_y_enteros() {
        let d = |s: &str| Decimal::from_str(s).unwrap();
        assert_eq!(formatear_monto(d("1234567.891")), "1,234,567.89");
        assert_eq!(formatear_monto(d("-1234.5")), "-1,234.50");
        assert_eq!(formatear_monto(d("0")), "0.00");
        assert_eq!(formatear_monto(d("999.999")), "1,000.00");
        assert_eq!(formatear_entero(1248), "1,248");
        assert_eq!(formatear_entero(12), "12");
        assert_eq!(formatear_entero(1_000_000), "1,000,000");
    }

    #[test]
    fn parse_decimal_casos() {
        assert_eq!(parse_decimal("1,234.50"), Decimal::from_str("1234.50").ok());
        assert_eq!(parse_decimal("$ 10"), Decimal::from_str("10").ok());
        assert_eq!(parse_decimal(""), None);
        assert_eq!(parse_decimal("abc"), None);
    }

    #[test]
    fn rendimiento_razonable_con_20_mil_filas() {
        let rows: Vec<F> = (0..20_000)
            .map(|i| {
                fila(&[
                    &format!("Concepto {}", i % 97),
                    &format!("{}", (i * 7919) % 10_007),
                    "1.5",
                ])
            })
            .collect();
        let mut e = estado(&rows);
        let t0 = std::time::Instant::now();
        e.alternar_orden(1, false);
        e.busqueda = "concepto 5".into();
        e.recalcular(&rows);
        let _ = e.totales();
        assert!(!e.vista.is_empty());
        assert!(
            t0.elapsed().as_millis() < 2_000,
            "recalcular tardó {:?}",
            t0.elapsed()
        );
    }
}
