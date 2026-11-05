//! Dimensiones específicas de la ventana y tabla; los colores y controles vienen de gpuikit.
//!
//! Temas "Clásico": papel marfil + tinta azul marino (claro) y carbón cálido +
//! azul acero suave (oscuro). Un solo acento, grises ligeramente cálidos en claro
//! y ligeramente fríos en oscuro, contraste de texto alto.

use std::sync::Arc;

use gpui::{App, FontFeatures, Hsla, WindowAppearance, hsla};
use gpuikit::theme::{ActiveTheme, GlobalTheme, Theme, ThemeVariant};

use crate::domain::config::TemaApp;

pub const WINDOW_WIDTH: f32 = 1200.0;
pub const WINDOW_HEIGHT: f32 = 760.0;
pub const WINDOW_MIN_WIDTH: f32 = 900.0;
pub const WINDOW_MIN_HEIGHT: f32 = 560.0;
pub const ROW_HEIGHT: f32 = 32.0;
pub const HEADER_HEIGHT: f32 = 32.0;
pub const DROP_ZONE_WIDTH: f32 = 560.0;
pub const DROP_ZONE_HEIGHT: f32 = 192.0;
pub const WIDTH_UUID: f32 = 240.0;
pub const WIDTH_DATE: f32 = 150.0;
pub const WIDTH_RFC: f32 = 140.0;
pub const WIDTH_NAME: f32 = 220.0;
pub const WIDTH_TYPE: f32 = 150.0;
pub const WIDTH_MONEY: f32 = 120.0;
pub const WIDTH_CODE: f32 = 120.0;
pub const WIDTH_DESCRIPTION: f32 = 260.0;
pub const WIDTH_FILE: f32 = 240.0;
pub const WIDTH_METHOD: f32 = 120.0;
pub const WIDTH_CURRENCY: f32 = 96.0;
pub const WIDTH_EXCHANGE: f32 = 120.0;
pub const WIDTH_COUNT: f32 = 144.0;
pub const WIDTH_CATEGORY: f32 = 160.0;
pub const WIDTH_CRITERION: f32 = 300.0;
pub const WIDTH_ERROR: f32 = 360.0;

pub fn tabular_numbers() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".to_owned(), 1)]))
}

pub fn follow_system_appearance(appearance: WindowAppearance, cx: &mut App) {
    apply_configured_appearance(&TemaApp::Sistema, appearance, cx);
}

pub fn apply_configured_appearance(
    configured: &TemaApp,
    appearance: WindowAppearance,
    cx: &mut App,
) {
    let is_dark = match configured {
        TemaApp::Sistema => matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
        TemaApp::Oscuro => true,
        TemaApp::Claro => false,
    };
    let theme = if is_dark {
        classic_dark_theme()
    } else {
        classic_light_theme()
    };
    if cx.theme().name != theme.name {
        cx.set_global(GlobalTheme(Arc::new(theme)));
    }
}

/// HSL opaco (h en 0..1, igual que `gpui::hsla`).
fn c(h: f32, s: f32, l: f32) -> Hsla {
    hsla(h, s, l, 1.0)
}

/// HSL con transparencia.
fn ca(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    hsla(h, s, l, a)
}

/// Clásico claro: papel marfil, tinta azul pizarra, acento azul marino (#1F3A5F).
fn classic_light_theme() -> Theme {
    const PAPER: f32 = 0.12; // matiz cálido (marfil)
    const INK: f32 = 0.597; // matiz azul pizarra
    let accent = c(0.596, 0.51, 0.25); // azul marino

    let mut theme = Theme::new(
        "Clásico Claro",
        ThemeVariant::Light,
        c(0.583, 0.25, 0.16),  // texto (tinta)
        c(PAPER, 0.40, 0.965), // fondo (marfil)
        c(PAPER, 0.30, 0.99),  // superficie (tabla, paneles)
        c(0.117, 0.20, 0.81),  // borde
        accent,                // acento
    );
    theme.fg_muted_color = Some(c(INK, 0.10, 0.40));
    theme.fg_disabled_color = Some(c(INK, 0.06, 0.62));
    theme.surface_secondary_color = Some(c(PAPER, 0.30, 0.935));
    theme.surface_tertiary_color = Some(c(PAPER, 0.28, 0.90)); // hover de filas
    theme.border_secondary_color = Some(c(0.117, 0.14, 0.68));
    theme.border_subtle_color = Some(c(0.117, 0.20, 0.88));
    theme.outline_color = Some(accent);
    theme.selection_color = Some(ca(0.596, 0.51, 0.25, 0.18));
    theme.button_bg_color = Some(c(PAPER, 0.30, 0.99));
    theme.button_bg_hover_color = Some(c(PAPER, 0.30, 0.94));
    theme.button_bg_active_color = Some(c(PAPER, 0.28, 0.90));
    theme.button_border_color = Some(c(0.117, 0.16, 0.76));
    theme.input_bg_color = Some(c(PAPER, 0.30, 0.995));
    theme.input_border_color = Some(c(0.117, 0.16, 0.78));
    theme.input_border_hover_color = Some(c(0.117, 0.12, 0.58));
    theme.input_border_focused_color = Some(accent);
    theme.input_text_color = Some(c(0.583, 0.25, 0.16));
    theme.input_placeholder_color = Some(c(INK, 0.06, 0.52));
    theme.input_cursor_color = Some(accent);
    theme
}

/// Clásico oscuro: carbón azulado, texto hueso cálido, acento azul acero (#8FB0DD).
fn classic_dark_theme() -> Theme {
    const SLATE: f32 = 0.61; // matiz azul grisáceo de fondos
    let accent = c(0.597, 0.55, 0.69); // azul acero suave
    let text = c(0.125, 0.26, 0.88); // hueso cálido

    let mut theme = Theme::new(
        "Clásico Oscuro",
        ThemeVariant::Dark,
        text,                  // texto
        c(SLATE, 0.10, 0.115), // fondo (carbón)
        c(SLATE, 0.10, 0.15),  // superficie
        c(SLATE, 0.09, 0.23),  // borde
        accent,                // acento
    );
    theme.fg_muted_color = Some(c(SLATE, 0.08, 0.63));
    theme.fg_disabled_color = Some(c(SLATE, 0.06, 0.42));
    theme.surface_secondary_color = Some(c(SLATE, 0.10, 0.18));
    theme.surface_tertiary_color = Some(c(SLATE, 0.10, 0.21)); // hover de filas
    theme.border_secondary_color = Some(c(SLATE, 0.09, 0.32));
    theme.border_subtle_color = Some(c(SLATE, 0.09, 0.18));
    theme.outline_color = Some(accent);
    theme.selection_color = Some(ca(0.597, 0.55, 0.69, 0.28));
    theme.button_bg_color = Some(c(SLATE, 0.10, 0.17));
    theme.button_bg_hover_color = Some(c(SLATE, 0.10, 0.21));
    theme.button_bg_active_color = Some(c(SLATE, 0.10, 0.14));
    theme.button_border_color = Some(c(SLATE, 0.09, 0.28));
    theme.input_bg_color = Some(c(SLATE, 0.10, 0.13));
    theme.input_border_color = Some(c(SLATE, 0.09, 0.27));
    theme.input_border_hover_color = Some(c(SLATE, 0.09, 0.40));
    theme.input_border_focused_color = Some(accent);
    theme.input_text_color = Some(text);
    theme.input_placeholder_color = Some(c(SLATE, 0.06, 0.48));
    theme.input_cursor_color = Some(accent);
    theme
}
