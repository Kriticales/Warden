//! Pastas do app (ARCHITECTURE §13).
//!
//! [`AppPaths`] é a única fonte dos caminhos de configuração, dados locais e packs. As pastas
//! do sistema ([`SystemDirs`]) vêm de quem conhece a plataforma (a `warden-app`, pelo Tauri);
//! esta crate só monta o layout.
//!
//! Desenvolvimento (ADR-0048): **só em build de debug**, se `WARDEN_DATA_ROOT` estiver
//! definida, tudo fica dentro dela: `<raiz>/config`, `<raiz>/data` e `<raiz>/packs`. Assim o
//! app de desenvolvimento, os testes e os E2E nunca tocam nas pastas do Warden instalado. Em
//! build de release, a variável é ignorada.

use std::io;
use std::path::{Path, PathBuf};

use crate::error::CoreError;
use crate::ids::{OperationId, PackId};

/// Variável de ambiente da pasta de dados de desenvolvimento (só em build de debug).
pub const DATA_ROOT_ENV: &str = "WARDEN_DATA_ROOT";

/// Pastas do sistema para o identificador do app (`dev.kriticales.warden`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemDirs {
    /// Configuração: `%APPDATA%\dev.kriticales.warden` (Linux: `~/.config/...`).
    pub config: PathBuf,
    /// Dados locais: `%LOCALAPPDATA%\dev.kriticales.warden` (Linux: `~/.local/share/...`).
    pub local_data: PathBuf,
    /// Pasta de documentos do usuário, quando o sistema informa.
    pub documents: Option<PathBuf>,
}

/// Layout das pastas do app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    config_dir: PathBuf,
    data_dir: PathBuf,
    default_packs_dir: PathBuf,
    dev_root: Option<PathBuf>,
}

impl AppPaths {
    /// Pastas do app instalado.
    #[must_use]
    pub fn from_system(dirs: SystemDirs) -> Self {
        let default_packs_dir = dirs
            .documents
            .unwrap_or_else(|| dirs.local_data.join("packs"))
            .join("Warden");
        Self {
            config_dir: dirs.config,
            data_dir: dirs.local_data,
            default_packs_dir,
            dev_root: None,
        }
    }

    /// Pastas de desenvolvimento, todas dentro de `root` (ADR-0048). `root` relativa é
    /// resolvida a partir da pasta atual.
    pub fn from_dev_root(root: &Path) -> Result<Self, CoreError> {
        let root = std::path::absolute(root).map_err(|e| CoreError::io("ler", root, e))?;
        Ok(Self {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            default_packs_dir: root.join("packs"),
            dev_root: Some(root),
        })
    }

    /// Escolhe o layout: a pasta de desenvolvimento, se houver, ou as do sistema.
    pub fn resolve(system: SystemDirs, dev_root: Option<&Path>) -> Result<Self, CoreError> {
        match dev_root {
            Some(root) => Self::from_dev_root(root),
            None => Ok(Self::from_system(system)),
        }
    }

    /// Pasta de desenvolvimento definida em `WARDEN_DATA_ROOT`, só em build de debug.
    /// Valor vazio conta como ausente.
    #[must_use]
    pub fn dev_root_from_env() -> Option<PathBuf> {
        #[cfg(debug_assertions)]
        {
            std::env::var_os(DATA_ROOT_ENV)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        }
        #[cfg(not(debug_assertions))]
        {
            None
        }
    }

    /// A pasta de desenvolvimento em uso, se houver.
    #[must_use]
    pub fn dev_root(&self) -> Option<&Path> {
        self.dev_root.as_deref()
    }

    /// Pasta de configuração.
    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Pasta de dados locais.
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Pasta padrão dos packs (Configurações → Geral pode trocar).
    #[must_use]
    pub fn default_packs_dir(&self) -> &Path {
        &self.default_packs_dir
    }

    /// `settings.json`.
    #[must_use]
    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    /// `packs.json` (registro dos packs).
    #[must_use]
    pub fn packs_registry_file(&self) -> PathBuf {
        self.config_dir.join("packs.json")
    }

    /// `.env` das chaves, quando o usuário escolhe guardá-las em arquivo (ADR-0025).
    #[must_use]
    pub fn secrets_env_file(&self) -> PathBuf {
        self.config_dir.join(".env")
    }

    /// Registros do app (ARCHITECTURE §16).
    #[must_use]
    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    /// Perfil do WebView (WebView2 no Windows). Fica nos dados locais, como no padrão do
    /// Tauri, e por isso dentro de `WARDEN_DATA_ROOT` no desenvolvimento.
    #[must_use]
    pub fn webview_dir(&self) -> PathBuf {
        self.data_dir.join("EBWebView")
    }

    /// Instâncias de teste.
    #[must_use]
    pub fn instances_dir(&self) -> PathBuf {
        self.data_dir.join("instances")
    }

    /// Instância de teste de um pack.
    #[must_use]
    pub fn instance_dir(&self, pack: PackId) -> PathBuf {
        self.instances_dir().join(pack.to_string())
    }

    /// Armazenamento compartilhado do motor do launcher.
    #[must_use]
    pub fn shared_dir(&self) -> PathBuf {
        self.data_dir.join("shared")
    }

    /// Javas instalados pelo Warden.
    #[must_use]
    pub fn runtimes_dir(&self) -> PathBuf {
        self.shared_dir().join("runtimes")
    }

    /// Caches.
    #[must_use]
    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join("cache")
    }

    /// Arquivos baixados, por sha256.
    #[must_use]
    pub fn downloads_cache_dir(&self) -> PathBuf {
        self.cache_dir().join("downloads")
    }

    /// Cache do sidecar do packwiz (`--cache`).
    #[must_use]
    pub fn packwiz_cache_dir(&self) -> PathBuf {
        self.cache_dir().join("packwiz")
    }

    /// Cache do Modrinth e do catálogo.
    #[must_use]
    pub fn metadata_db_file(&self) -> PathBuf {
        self.cache_dir().join("metadata.sqlite")
    }

    /// Cópias temporárias do pack de uma operação.
    #[must_use]
    pub fn staging_dir(&self, operation: OperationId) -> PathBuf {
        self.cache_dir().join("staging").join(operation.to_string())
    }

    /// Temporários diversos.
    #[must_use]
    pub fn tmp_dir(&self) -> PathBuf {
        self.cache_dir().join("tmp")
    }

    /// Config vazia passada ao sidecar (`--config`).
    #[must_use]
    pub fn packwiz_config_file(&self) -> PathBuf {
        self.data_dir.join("packwiz").join("packwiz.toml")
    }

    /// Conversas com a IA de um pack.
    #[must_use]
    pub fn ai_dir(&self, pack: PackId) -> PathBuf {
        self.data_dir.join("ai").join(pack.to_string())
    }

    /// Métricas de desempenho de um pack (uma linha por teste).
    #[must_use]
    pub fn perf_file(&self, pack: PackId) -> PathBuf {
        self.data_dir.join("perf").join(format!("{pack}.jsonl"))
    }

    /// Cria as pastas que o app usa logo ao abrir (configuração, dados, registros e
    /// temporários). As demais são criadas por quem as usa.
    pub fn create_base_dirs(&self) -> Result<(), CoreError> {
        for dir in [
            self.config_dir.clone(),
            self.data_dir.clone(),
            self.logs_dir(),
            self.tmp_dir(),
        ] {
            create_dir_all(&dir)?;
        }
        Ok(())
    }
}

fn create_dir_all(dir: &Path) -> Result<(), CoreError> {
    match std::fs::create_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && dir.is_dir() => Ok(()),
        Err(error) => Err(CoreError::io("criar a pasta", dir, error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system() -> SystemDirs {
        SystemDirs {
            config: PathBuf::from("/sistema/config/dev.kriticales.warden"),
            local_data: PathBuf::from("/sistema/dados/dev.kriticales.warden"),
            documents: Some(PathBuf::from("/usuario/Documentos")),
        }
    }

    #[test]
    fn pastas_do_app_instalado() {
        let paths = AppPaths::resolve(system(), None).unwrap();
        assert_eq!(paths.dev_root(), None);
        assert_eq!(
            paths.config_dir(),
            Path::new("/sistema/config/dev.kriticales.warden")
        );
        assert_eq!(
            paths.settings_file(),
            Path::new("/sistema/config/dev.kriticales.warden/settings.json")
        );
        assert_eq!(
            paths.secrets_env_file(),
            Path::new("/sistema/config/dev.kriticales.warden/.env")
        );
        assert_eq!(
            paths.logs_dir(),
            Path::new("/sistema/dados/dev.kriticales.warden/logs")
        );
        assert_eq!(
            paths.webview_dir(),
            Path::new("/sistema/dados/dev.kriticales.warden/EBWebView")
        );
        assert_eq!(
            paths.default_packs_dir(),
            Path::new("/usuario/Documentos/Warden")
        );
    }

    #[test]
    fn sem_documentos_os_packs_ficam_nos_dados() {
        let mut dirs = system();
        dirs.documents = None;
        let paths = AppPaths::from_system(dirs);
        assert_eq!(
            paths.default_packs_dir(),
            Path::new("/sistema/dados/dev.kriticales.warden/packs/Warden")
        );
    }

    /// ADR-0048: com a pasta de desenvolvimento, nenhuma pasta fica fora dela.
    #[test]
    fn f0_05_ca6_pasta_de_desenvolvimento_contem_tudo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let paths = AppPaths::resolve(system(), Some(root)).unwrap();
        let pack = PackId::new();
        let operation = OperationId::new();
        let all = [
            paths.config_dir().to_path_buf(),
            paths.data_dir().to_path_buf(),
            paths.default_packs_dir().to_path_buf(),
            paths.settings_file(),
            paths.packs_registry_file(),
            paths.secrets_env_file(),
            paths.logs_dir(),
            paths.webview_dir(),
            paths.instances_dir(),
            paths.instance_dir(pack),
            paths.shared_dir(),
            paths.runtimes_dir(),
            paths.cache_dir(),
            paths.downloads_cache_dir(),
            paths.packwiz_cache_dir(),
            paths.metadata_db_file(),
            paths.staging_dir(operation),
            paths.tmp_dir(),
            paths.packwiz_config_file(),
            paths.ai_dir(pack),
            paths.perf_file(pack),
        ];
        for path in &all {
            assert!(
                path.starts_with(root),
                "{} fora de {}",
                path.display(),
                root.display()
            );
        }
        assert_eq!(paths.dev_root(), Some(root));
        assert_eq!(paths.webview_dir(), root.join("data").join("EBWebView"));
        assert_eq!(paths.config_dir(), root.join("config"));
    }

    #[test]
    fn pasta_de_desenvolvimento_relativa_vira_absoluta() {
        let paths = AppPaths::from_dev_root(Path::new("relativa")).unwrap();
        assert!(paths.dev_root().unwrap().is_absolute());
        assert!(paths.config_dir().is_absolute());
    }

    #[test]
    fn ids_entram_no_caminho_como_texto() {
        let paths = AppPaths::from_system(system());
        let pack: PackId = "01J9ZQ0000000000000000000A".parse().unwrap();
        assert!(
            paths
                .instance_dir(pack)
                .ends_with("instances/01J9ZQ0000000000000000000A")
        );
        assert!(
            paths
                .perf_file(pack)
                .ends_with("perf/01J9ZQ0000000000000000000A.jsonl")
        );
    }

    #[test]
    fn cria_pastas_basicas_e_repetir_nao_falha() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_dev_root(dir.path()).unwrap();
        paths.create_base_dirs().unwrap();
        paths.create_base_dirs().unwrap();
        assert!(paths.config_dir().is_dir());
        assert!(paths.logs_dir().is_dir());
        assert!(paths.tmp_dir().is_dir());
    }

    #[test]
    fn criar_pasta_sobre_arquivo_e_erro() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config"), b"").unwrap();
        let paths = AppPaths::from_dev_root(dir.path()).unwrap();
        assert!(paths.create_base_dirs().is_err());
    }
}
