//! Pestañas gpuikit para seleccionar las tablas de vista previa.

use gpui::{AppContext, Context, Entity};
use gpuikit::elements::tabs::{Tabs, tab, tabs};

use super::app_view::{AppView, Vista, format_integer};

#[derive(Clone, Copy, Debug)]
pub struct TabCounts {
    pub cfdi: usize,
    pub resumen: usize,
    pub clasificacion: usize,
    pub errores: usize,
}

pub fn create(cx: &mut Context<AppView>, counts: Option<TabCounts>, active: Vista) -> Entity<Tabs> {
    let (cfdi_label, resumen_label, clasif_label, errores_label) = if let Some(c) = counts {
        (
            format!("CFDI ({})", format_integer(c.cfdi)),
            format!("Resumen por producto ({})", format_integer(c.resumen)),
            format!(
                "Clasificación sugerida ({})",
                format_integer(c.clasificacion)
            ),
            if c.errores > 0 {
                format!("Errores ({})", format_integer(c.errores))
            } else {
                "Errores (0)".to_owned()
            },
        )
    } else {
        (
            "CFDI".to_owned(),
            "Resumen por producto".to_owned(),
            "Clasificación sugerida".to_owned(),
            "Errores".to_owned(),
        )
    };

    cx.new(|_| {
        tabs("preview-tabs")
            .tab(tab("cfdi", cfdi_label))
            .tab(tab("resumen", resumen_label))
            .tab(tab("clasificacion", clasif_label))
            .tab(tab("errores", errores_label))
            .selected(active.id())
    })
}

pub fn from_id(id: &str) -> Option<Vista> {
    match id {
        "cfdi" => Some(Vista::Cfdi),
        "resumen" => Some(Vista::Resumen),
        "clasificacion" => Some(Vista::Clasificacion),
        "errores" => Some(Vista::Errores),
        _ => None,
    }
}
