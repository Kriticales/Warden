//! A pasta `shared/runtimes/`: listar, instalar de forma atômica e remover Javas.
//!
//! Instalação atômica (ARCHITECTURE §7.3; critério 2 da L-01):
//! 1. o pacote é extraído (ou os arquivos da Mojang baixados) numa pasta temporária
//!    `.tmp-<ulid>/` dentro de `shared/runtimes/` (mesmo disco, para a renomeação ser atômica);
//! 2. o Java é validado executando-o;
//! 3. [`METADATA_FILE`] é gravado na pasta temporária;
//! 4. a pasta é renomeada para `<id>/`. Só então o Java aparece em [`RuntimeStore::list`].
//!
//! Qualquer falha antes do passo 4 deixa só a pasta temporária, que quem instala apaga e que
//! [`RuntimeStore::cleanup_leftovers`] apaga na próxima abertura se o processo tiver morrido.
//! Pastas que começam com ponto nunca são listadas.
//!
//! A remoção é o caminho inverso: renomeia `<id>/` para `.trash-<ulid>/` (o Java some da
//! lista na hora) e depois apaga.
//!
//! O antivírus pode segurar arquivos recém-extraídos por um instante (QUALITY §13.9): a
//! renomeação e a remoção tentam de novo com espera curta antes de falhar, e a falha final é
//! um erro ([`Error::InstallBlocked`]), nunca ignorada.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};
use crate::probe::{LOCK_RETRY_DELAYS, is_transient_lock};
use crate::runtime::{
    InstalledRuntime, METADATA_FILE, METADATA_SCHEMA, Platform, RuntimeId, RuntimeMetadata,
};

/// Prefixo das pastas de instalação em andamento.
pub const TEMP_PREFIX: &str = ".tmp-";

/// Prefixo das pastas sendo apagadas.
pub const TRASH_PREFIX: &str = ".trash-";

/// Uma pasta em `shared/runtimes/` que não é um Java válido (sem dados ou sem o programa).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenRuntime {
    /// A pasta.
    pub path: PathBuf,
    /// O que está errado.
    pub reason: String,
}

/// O conteúdo de `shared/runtimes/`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeListing {
    /// Javas instalados, em ordem de id.
    pub runtimes: Vec<InstalledRuntime>,
    /// Pastas quebradas.
    pub broken: Vec<BrokenRuntime>,
}

/// Milissegundos desde 1970.
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// A pasta dos Javas do Warden.
#[derive(Debug, Clone)]
pub struct RuntimeStore {
    dir: PathBuf,
    platform: Platform,
}

impl RuntimeStore {
    /// Loja em `dir` (`AppPaths::runtimes_dir`). A pasta é criada quando preciso.
    #[must_use]
    pub fn new(dir: PathBuf, platform: Platform) -> Self {
        Self { dir, platform }
    }

    /// A pasta.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// A plataforma.
    #[must_use]
    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Lista os Javas instalados. Pasta inexistente = lista vazia. Uma pasta que some entre a
    /// listagem e a leitura (outra remoção, antivírus) é ignorada; uma que existe e está
    /// quebrada aparece em `broken`.
    pub fn list(&self) -> Result<RuntimeListing> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(RuntimeListing::default());
            }
            Err(error) => return Err(Error::io("listar", &self.dir, error)),
        };
        let mut listing = RuntimeListing::default();
        for entry in entries {
            let entry = entry.map_err(|e| Error::io("listar", &self.dir, e))?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            let is_dir = match entry.file_type() {
                Ok(kind) => kind.is_dir(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(Error::io("ler", entry.path(), error)),
            };
            if !is_dir {
                continue;
            }
            match self.read_runtime(name, &entry.path()) {
                Ok(Some(runtime)) => listing.runtimes.push(runtime),
                Ok(None) => {}
                Err(reason) => listing.broken.push(BrokenRuntime {
                    path: entry.path(),
                    reason,
                }),
            }
        }
        listing.runtimes.sort_by(|a, b| a.id.cmp(&b.id));
        listing.broken.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(listing)
    }

    /// Lê uma pasta. `Ok(None)` se ela sumiu no meio; `Err(motivo)` se está quebrada.
    fn read_runtime(
        &self,
        name: &str,
        home: &Path,
    ) -> std::result::Result<Option<InstalledRuntime>, String> {
        let id =
            RuntimeId::parse(name).map_err(|_| format!("nome de pasta inesperado: {name:?}"))?;
        let metadata_path = home.join(METADATA_FILE);
        let bytes = match fs::read(&metadata_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if home.exists() {
                    return Err(format!("sem {METADATA_FILE}"));
                }
                return Ok(None);
            }
            Err(error) => return Err(format!("falha ao ler {METADATA_FILE}: {error}")),
        };
        let metadata: RuntimeMetadata = serde_json::from_slice(&bytes)
            .map_err(|error| format!("{METADATA_FILE} ilegível: {error}"))?;
        if metadata.schema != METADATA_SCHEMA {
            return Err(format!(
                "{METADATA_FILE} com schema {} desconhecido",
                metadata.schema
            ));
        }
        let runtime = metadata.into_runtime(id, home.to_owned(), self.platform);
        if !runtime.java.is_file() {
            return Err(format!("sem o programa {}", runtime.java.display()));
        }
        Ok(Some(runtime))
    }

    /// Um Java pelo id.
    pub fn get(&self, id: &RuntimeId) -> Result<Option<InstalledRuntime>> {
        Ok(self
            .list()?
            .runtimes
            .into_iter()
            .find(|runtime| &runtime.id == id))
    }

    /// Cria uma pasta temporária vazia para uma instalação.
    pub(crate) fn create_temp_dir(&self) -> Result<PathBuf> {
        fs::create_dir_all(&self.dir).map_err(|e| Error::io("criar a pasta", &self.dir, e))?;
        let path = self
            .dir
            .join(format!("{TEMP_PREFIX}{}", ulid::Ulid::generate()));
        fs::create_dir(&path).map_err(|e| Error::io("criar a pasta", &path, e))?;
        Ok(path)
    }

    /// Passos 3 e 4: grava os dados em `temp` e renomeia para `<id>/`. Se o Java já estiver
    /// instalado (outra instalação terminou antes), a pasta temporária é apagada e o existente
    /// é devolvido.
    pub(crate) fn commit(
        &self,
        temp: &Path,
        runtime: &InstalledRuntime,
    ) -> Result<InstalledRuntime> {
        let final_dir = self.dir.join(runtime.id.as_str());
        if let Some(existing) = self.get(&runtime.id)? {
            remove_dir_with_retry(temp)?;
            return Ok(existing);
        }
        let metadata = RuntimeMetadata::from_runtime(runtime);
        let json = serde_json::to_vec_pretty(&metadata)
            .map_err(|error| Error::Internal(format!("dados do Java: {error}")))?;
        warden_core::atomic_write(&temp.join(METADATA_FILE), &json)?;
        warden_core::fault::check("java.install.before_rename")
            .map_err(|e| Error::io("renomear para", &final_dir, e))?;
        if final_dir.exists() {
            // Pasta quebrada com o mesmo nome (sem dados): sai do caminho antes.
            self.move_to_trash_and_delete(&final_dir)?;
        }
        rename_with_retry(temp, &final_dir)?;
        self.get(&runtime.id)?.ok_or_else(|| {
            Error::Internal(format!("Java {} sumiu depois de instalado", runtime.id))
        })
    }

    /// Regrava os dados de um Java instalado (marca de substituído).
    pub(crate) fn write_metadata(runtime: &InstalledRuntime) -> Result<()> {
        let metadata = RuntimeMetadata::from_runtime(runtime);
        let json = serde_json::to_vec_pretty(&metadata)
            .map_err(|error| Error::Internal(format!("dados do Java: {error}")))?;
        warden_core::atomic_write(&runtime.home.join(METADATA_FILE), &json)?;
        Ok(())
    }

    /// Remove um Java instalado.
    pub fn remove(&self, id: &RuntimeId) -> Result<()> {
        let path = self.dir.join(id.as_str());
        if !path.exists() {
            return Err(Error::RuntimeNotFound { id: id.to_string() });
        }
        self.move_to_trash_and_delete(&path)
    }

    /// Remove uma pasta quebrada (só dentro de `shared/runtimes/`).
    pub fn remove_broken(&self, broken: &BrokenRuntime) -> Result<()> {
        if broken.path.parent() != Some(self.dir.as_path()) {
            return Err(Error::Internal(format!(
                "pasta fora de shared/runtimes: {}",
                broken.path.display()
            )));
        }
        self.move_to_trash_and_delete(&broken.path)
    }

    fn move_to_trash_and_delete(&self, path: &Path) -> Result<()> {
        let trash = self
            .dir
            .join(format!("{TRASH_PREFIX}{}", ulid::Ulid::generate()));
        rename_with_retry(path, &trash)?;
        if let Err(error) = remove_dir_with_retry(&trash) {
            // O Java já saiu da lista; o resto da pasta é apagado na próxima abertura.
            tracing::warn!(%error, path = %trash.display(), "sobra de Java removido não apagada");
        }
        Ok(())
    }

    /// Apaga as sobras de instalações e remoções interrompidas (`.tmp-*`, `.trash-*`). Devolve
    /// quantas apagou. "Não encontrado" conta como já limpo.
    pub fn cleanup_leftovers(&self) -> Result<usize> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(Error::io("listar", &self.dir, error)),
        };
        let mut removed = 0;
        for entry in entries {
            let entry = entry.map_err(|e| Error::io("listar", &self.dir, e))?;
            let name = entry.file_name();
            let leftover = name.to_str().is_some_and(|name| {
                name.starts_with(TEMP_PREFIX) || name.starts_with(TRASH_PREFIX)
            });
            if leftover {
                remove_dir_with_retry(&entry.path())?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

/// Renomeia tentando de novo enquanto o antivírus segura algum arquivo.
pub(crate) fn rename_with_retry(from: &Path, to: &Path) -> Result<()> {
    let mut delays = LOCK_RETRY_DELAYS.iter();
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if is_transient_lock(&error) => match delays.next() {
                Some(delay) => {
                    tracing::debug!(%error, ?delay, from = %from.display(), "pasta segurada; nova tentativa");
                    std::thread::sleep(*delay);
                }
                None => {
                    return Err(Error::InstallBlocked {
                        path: to.to_owned(),
                        source: error,
                    });
                }
            },
            Err(error) => return Err(Error::io("renomear para", to, error)),
        }
    }
}

/// Apaga uma pasta inteira, tentando de novo enquanto o antivírus segura algum arquivo.
/// Pasta inexistente conta como apagada.
pub(crate) fn remove_dir_with_retry(path: &Path) -> Result<()> {
    let mut delays = LOCK_RETRY_DELAYS.iter();
    loop {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error)
                if is_transient_lock(&error)
                    || error.kind() == io::ErrorKind::DirectoryNotEmpty =>
            {
                match delays.next() {
                    Some(delay) => std::thread::sleep(*delay),
                    None => return Err(Error::io("apagar", path, error)),
                }
            }
            Err(error) => return Err(Error::io("apagar", path, error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::RuntimeSource;
    use crate::version::JavaVersion;

    fn platform() -> Platform {
        Platform::current().unwrap()
    }

    fn fake_runtime(store: &RuntimeStore, version: &str) -> (PathBuf, InstalledRuntime) {
        let temp = store.create_temp_dir().unwrap();
        let java = platform().java_in(&temp);
        fs::create_dir_all(java.parent().unwrap()).unwrap();
        fs::write(&java, b"java").unwrap();
        let version = JavaVersion::parse(version).unwrap();
        let id = RuntimeId::new(
            RuntimeSource::Temurin,
            &version,
            version.to_string().as_str(),
            platform(),
        );
        let home = store.dir().join(id.as_str());
        let runtime = InstalledRuntime {
            java: platform().java_in(&home),
            launcher: platform().javaw_in(&home),
            id,
            source: RuntimeSource::Temurin,
            version,
            vendor: "Eclipse Adoptium".into(),
            release_name: "jdk".into(),
            arch: platform().arch_key().into(),
            home,
            installed_at_ms: 1,
            update_cap: None,
            mojang_component: None,
            archive_sha256: None,
            superseded_by: None,
        };
        (temp, runtime)
    }

    #[test]
    fn pasta_inexistente_lista_vazia() {
        let dir = tempfile::tempdir().unwrap();
        let store = RuntimeStore::new(dir.path().join("runtimes"), platform());
        assert_eq!(store.list().unwrap(), RuntimeListing::default());
        assert_eq!(store.cleanup_leftovers().unwrap(), 0);
    }

    #[test]
    fn instala_lista_e_remove() {
        let dir = tempfile::tempdir().unwrap();
        let store = RuntimeStore::new(dir.path().join("runtimes"), platform());
        let (temp, runtime) = fake_runtime(&store, "21.0.12.1+1");
        let installed = store.commit(&temp, &runtime).unwrap();
        assert_eq!(installed, runtime);
        assert!(!temp.exists());
        assert_eq!(store.list().unwrap().runtimes, vec![runtime.clone()]);
        // Instalar de novo o mesmo Java devolve o existente e apaga a temporária.
        let (temp2, again) = fake_runtime(&store, "21.0.12.1+1");
        assert_eq!(store.commit(&temp2, &again).unwrap(), runtime);
        assert!(!temp2.exists());
        store.remove(&runtime.id).unwrap();
        assert!(store.list().unwrap().runtimes.is_empty());
        assert!(matches!(
            store.remove(&runtime.id),
            Err(Error::RuntimeNotFound { .. })
        ));
        // Nada de sobra.
        assert_eq!(fs::read_dir(store.dir()).unwrap().count(), 0);
    }

    #[test]
    fn falha_antes_da_renomeacao_nao_deixa_java_instalado() {
        let dir = tempfile::tempdir().unwrap();
        let store = RuntimeStore::new(dir.path().join("runtimes"), platform());
        let (temp, runtime) = fake_runtime(&store, "17.0.20.1+1");
        let _guard = warden_core::fault::arm("java.install.before_rename");
        assert!(store.commit(&temp, &runtime).is_err());
        assert!(store.list().unwrap().runtimes.is_empty());
        // Só a temporária sobrou, e a limpeza a apaga.
        assert!(temp.exists());
        assert_eq!(store.cleanup_leftovers().unwrap(), 1);
        assert_eq!(fs::read_dir(store.dir()).unwrap().count(), 0);
    }

    #[test]
    fn pastas_quebradas_e_temporarias() {
        let dir = tempfile::tempdir().unwrap();
        let store = RuntimeStore::new(dir.path().to_owned(), platform());
        fs::create_dir(dir.path().join("temurin-21-x-x64")).unwrap();
        fs::create_dir(dir.path().join(".tmp-abc")).unwrap();
        fs::create_dir(dir.path().join("lixo qualquer")).unwrap();
        fs::write(dir.path().join("arquivo-solto.txt"), b"x").unwrap();
        let bad_json = dir.path().join("temurin-17-y-x64");
        fs::create_dir(&bad_json).unwrap();
        fs::write(bad_json.join(METADATA_FILE), b"{").unwrap();
        let listing = store.list().unwrap();
        assert!(listing.runtimes.is_empty());
        assert_eq!(listing.broken.len(), 3, "{listing:?}");
        assert!(
            listing
                .broken
                .iter()
                .any(|b| b.reason.contains("sem warden-runtime.json"))
        );
        assert!(listing.broken.iter().any(|b| b.reason.contains("ilegível")));
        assert!(
            listing
                .broken
                .iter()
                .any(|b| b.reason.contains("nome de pasta"))
        );
        for broken in &listing.broken {
            store.remove_broken(broken).unwrap();
        }
        assert!(store.list().unwrap().broken.is_empty());
        let outside = BrokenRuntime {
            path: dir.path().parent().unwrap().to_owned(),
            reason: String::new(),
        };
        assert!(store.remove_broken(&outside).is_err());
    }

    #[test]
    fn java_sem_programa_e_quebrado() {
        let dir = tempfile::tempdir().unwrap();
        let store = RuntimeStore::new(dir.path().to_owned(), platform());
        let (temp, runtime) = fake_runtime(&store, "25.0.4.1+1");
        store.commit(&temp, &runtime).unwrap();
        fs::remove_file(&runtime.java).unwrap();
        let listing = store.list().unwrap();
        assert!(listing.runtimes.is_empty());
        assert!(listing.broken[0].reason.contains("sem o programa"));
    }

    #[test]
    fn regrava_os_dados() {
        let dir = tempfile::tempdir().unwrap();
        let store = RuntimeStore::new(dir.path().to_owned(), platform());
        let (temp, mut runtime) = fake_runtime(&store, "17.0.15");
        store.commit(&temp, &runtime).unwrap();
        runtime.superseded_by = Some(RuntimeId::parse("temurin-17-17.0.20-x64").unwrap());
        RuntimeStore::write_metadata(&runtime).unwrap();
        assert_eq!(
            store.get(&runtime.id).unwrap().unwrap().superseded_by,
            runtime.superseded_by
        );
    }
}
