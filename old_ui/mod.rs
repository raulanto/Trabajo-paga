//! Interfaz GPUI; delega el procesamiento y la exportación a sus casos de uso y ports.

use gpui::{App, ClickEvent, Window};

pub mod app_view;
pub mod data_table;
pub mod drop_zone;
pub mod metrics;
pub mod status_bar;
pub mod table_model;
pub mod tabs;
pub mod theme;
pub mod toolbar;

pub(crate) type UiClickHandler = dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static;

pub use app_view::AppView;
