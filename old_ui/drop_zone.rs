//! Área de carga con botón, layout y tokens de gpuikit.

use gpui::{App, ClickEvent, IntoElement, ParentElement, RenderOnce, Window, prelude::*, px};
use gpuikit::{
    elements::{
        badge::badge,
        button::button,
        typography::{h3, p},
    },
    layout::{h_stack, v_stack},
    theme::{ActiveTheme, ControlSize, Themeable},
    traits::control_sized::ControlSized,
};

use super::UiClickHandler;
use super::theme;

#[derive(IntoElement)]
pub struct DropZone {
    button_label: &'static str,
    on_click: Box<UiClickHandler>,
}

impl DropZone {
    pub fn new(
        button_label: &'static str,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            button_label,
            on_click: Box::new(on_click),
        }
    }
}

impl RenderOnce for DropZone {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        v_stack()
            .id("drop-zone")
            .w_full()
            .max_w(px(theme::DROP_ZONE_WIDTH + 80.0))
            .items_center()
            .justify_center()
            .p_6()
            .gap_3()
            .rounded_xl()
            .border_dashed()
            .border_2()
            .border_color(theme.border())
            .bg(theme.surface_secondary())
            .cursor_pointer()
            .hover(|s| s.bg(theme.surface_tertiary()).border_color(theme.accent()))
            .on_click(self.on_click)
            .child(h3("Carga un archivo ZIP"))
            .child(p("Suelta aquí el ZIP con tus CFDI o selecciónalo desde el equipo.").muted())
            .child(button("browse-button", self.button_label).control_size(ControlSize::Large))
            .child(
                h_stack()
                    .gap_2()
                    .child(
                        badge("CFDI 3.3 / 4.0")
                            .secondary()
                            .control_size(ControlSize::Small),
                    )
                    .child(
                        badge("Exportación Excel a 4 hojas")
                            .secondary()
                            .control_size(ControlSize::Small),
                    )
                    .child(
                        badge("Continúa ante XML dañados")
                            .secondary()
                            .control_size(ControlSize::Small),
                    ),
            )
    }
}
