//! Estado, acciones y componentes de la vista principal.
//!
//! El módulo se divide por responsabilidades:
//! - `state`: estado de la aplicación y transiciones puras.
//! - `util`: formateo, validación y utilidades de rutas.
//! - `cells`: filas ya formateadas para las tablas.
//! - `components`: controles reutilizables de la interfaz.
//! - `actions`: casos de uso iniciados desde la UI, agrupados por dependencia
//!   (archivos, procesamiento, exportación, navegación y configuración).
//! - `render`: vistas de contenido y opciones.
//! - `settings`: modal de configuración.
//! - `settings_sections`: contenido de cada sección de configuración.
//! - `view`: composición del componente y suscripciones GPUI.

mod actions;
mod cells;
mod components;
mod render;
mod settings;
mod settings_sections;
mod state;
mod util;
mod view;

pub use cells::TablasPreparadas;
pub use state::{AppState, EstadoApp, SettingsSection, Vista};
pub use util::{format_decimal, format_integer, validar_rfc};
pub use view::AppView;
