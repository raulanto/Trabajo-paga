use std::{io, path::Path};

use crate::domain::config::AppConfig;

pub trait ConfigRepository {
    fn cargar(&self) -> io::Result<AppConfig>;
    fn guardar(&self, config: &AppConfig) -> io::Result<()>;
    fn importar(&self, ruta: &Path) -> io::Result<AppConfig>;
    fn exportar(&self, ruta: &Path, config: &AppConfig) -> io::Result<()>;
}

#[derive(Clone)]
pub struct ConfigService<R> {
    repo: R,
}

impl<R> ConfigService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: ConfigRepository> ConfigService<R> {
    pub fn cargar_o_default(&self) -> AppConfig {
        self.repo.cargar().unwrap_or_else(|_| AppConfig::default())
    }

    pub fn guardar(&self, config: &AppConfig) -> io::Result<()> {
        self.repo.guardar(config)
    }

    pub fn importar(&self, ruta: &Path) -> io::Result<AppConfig> {
        self.repo.importar(ruta)
    }

    pub fn exportar(&self, ruta: &Path, config: &AppConfig) -> io::Result<()> {
        self.repo.exportar(ruta, config)
    }
}
