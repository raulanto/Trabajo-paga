//! Barra superior compuesta con controles y tipografía de gpuikit.

use gpui::{App, ClickEvent, IntoElement, RenderOnce, Window, div, prelude::*, rems};
use gpuikit::{
    elements::{
        button::button,
        typography::{h4, small, text},
    },
    layout::h_stack,
    theme::{ActiveTheme, ControlSize, Themeable},
    traits::control_sized::ControlSized,
};

use super::UiClickHandler;

#[derive(IntoElement)]
pub struct Toolbar {
    zip_name: Option<String>,
    export_enabled: bool,
    on_open_zip: Box<UiClickHandler>,
    on_export: Box<UiClickHandler>,
}

impl Toolbar {
    pub fn new(
        zip_name: Option<String>,
        export_enabled: bool,
        on_open_zip: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        on_export: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            zip_name,
            export_enabled,
            on_open_zip: Box::new(on_open_zip),
            on_export: Box::new(on_export),
        }
    }
}

impl RenderOnce for Toolbar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        h_stack()
            .w_full()
            .h(gpui::rems(3.2))
            .items_center()
            .justify_between()
            .px_6()
            .gap_4()
            .border_b_1()
            .border_color(theme.border())
            .bg(theme.bg())
            .child(
                h_stack()
                    .gap_4()
                    .min_w_0()
                    .items_center()
                    .child(
                        gpuikit::layout::v_stack()
                            .gap_1()
                            .child(h4("Procesamiento de CFDI"))
                            .child(small("Extrae datos XML y exporta a Excel").muted()),
                    )
                    .when_some(self.zip_name, |stack, name| {
                        stack.child(
                            div().max_w(rems(18.)).min_w_0().child(
                                text(format!("ZIP · {name}")).small().muted().truncate(true),
                            ),
                        )
                    }),
            )
            .child(
                h_stack()
                    .gap_2()
                    .items_center()
                    .child(
                        button("open-zip-toolbar", "Abrir ZIP…")
                            .control_size(ControlSize::Small)
                            .on_click(self.on_open_zip),
                    )
                    .child(
                        button("export-button", "Exportar Excel…")
                            .control_size(ControlSize::Small)
                            .disabled(!self.export_enabled)
                            .on_click(self.on_export),
                    ),
            )
    }
}
