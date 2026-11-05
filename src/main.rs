use cfdi_zip_processor::ui::{
    AppView,
    dimensions::{WINDOW_HEIGHT, WINDOW_MIN_HEIGHT, WINDOW_MIN_WIDTH, WINDOW_WIDTH},
};
use ely_gpui_component::Assets;
use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};

fn main() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            if let Err(error) = ely_gpui_component::init(cx) {
                eprintln!("Ely failed to start: {error:#}");
                cx.quit();
                return;
            }

            let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(WINDOW_MIN_WIDTH), px(WINDOW_MIN_HEIGHT))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Procesamiento de CFDI".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            );

            match result {
                Ok(_) => cx.activate(true),
                Err(error) => {
                    eprintln!("No se pudo abrir la ventana: {error:#}");
                    cx.quit();
                }
            }
        });
}
