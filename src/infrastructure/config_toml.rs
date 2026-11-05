use std::{
    fs, io,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;

use crate::{application::config_service::ConfigRepository, domain::config::AppConfig};

#[derive(Clone, Debug)]
pub struct TomlConfigRepository {
    path: PathBuf,
}

impl TomlConfigRepository {
    pub fn new() -> Self {
        let path = Self::config_path();
        Self { path }
    }

    fn config_path() -> PathBuf {
        let project = ProjectDirs::from("dev", "trabajo-paga", "cfdi-zip-processor")
            .map(|dirs| dirs.config_local_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        project.join("config.toml")
    }

    fn ensure_parent_dir(path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    fn cargar_desde(path: &Path) -> io::Result<AppConfig> {
        let text = fs::read_to_string(path)?;
        let mut config = toml::from_str::<AppConfig>(&text).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("TOML inválido: {error}"),
            )
        })?;
        if !config.is_valid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "la configuración contiene valores inválidos",
            ));
        }
        config.reordenar_version();
        Ok(config)
    }

    fn guardar_en(path: &Path, config: &AppConfig) -> io::Result<()> {
        if !config.is_valid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "la configuración contiene valores inválidos",
            ));
        }
        Self::ensure_parent_dir(path)?;
        let tmp = path.with_extension("toml.tmp");
        let mut export_config = config.clone();
        export_config.reordenar_version();
        let text = toml::to_string_pretty(&export_config).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("config serializable: {error}"),
            )
        })?;
        fs::write(&tmp, text)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

impl Default for TomlConfigRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigRepository for TomlConfigRepository {
    fn cargar(&self) -> io::Result<AppConfig> {
        if !self.path.exists() {
            return Ok(AppConfig::default());
        }

        Self::cargar_desde(&self.path)
    }

    fn guardar(&self, config: &AppConfig) -> io::Result<()> {
        Self::guardar_en(&self.path, config)
    }

    fn importar(&self, ruta: &Path) -> io::Result<AppConfig> {
        Self::cargar_desde(ruta)
    }

    fn exportar(&self, ruta: &Path, config: &AppConfig) -> io::Result<()> {
        Self::guardar_en(ruta, config)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io};

    use crate::{application::config_service::ConfigRepository, domain::config::AppConfig};

    use super::TomlConfigRepository;

    #[test]
    fn exporta_e_importa_configuracion_valida_y_rechaza_toml_defectuoso() {
        let directory = tempfile::tempdir().expect("temporary directory is created");
        let path = directory.path().join("config.toml");
        let repository = TomlConfigRepository::new();
        let config = AppConfig::default();

        repository.exportar(&path, &config).expect("config exports");
        assert_eq!(repository.importar(&path).expect("config imports"), config);

        fs::write(&path, "excel = [").expect("invalid TOML fixture writes");
        assert_eq!(
            repository
                .importar(&path)
                .expect_err("invalid TOML is rejected")
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn no_exporta_configuracion_invalida() {
        let directory = tempfile::tempdir().expect("temporary directory is created");
        let path = directory.path().join("config.toml");
        let repository = TomlConfigRepository::new();
        let mut config = AppConfig::default();
        config.excel.hojas_incluidas.cfdi = false;
        config.excel.hojas_incluidas.resumen = false;
        config.excel.hojas_incluidas.clasificacion = false;
        config.excel.hojas_incluidas.errores = false;

        assert_eq!(
            repository
                .exportar(&path, &config)
                .expect_err("invalid config is rejected")
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
}
