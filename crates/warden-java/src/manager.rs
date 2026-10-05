//! O serviço de Java do app: escolher, instalar, atualizar, listar e remover
//! (ARCHITECTURE §7.3; ROADMAP L-01).
//!
//! - [`JavaRuntimes::choose`]: a política com os Javas instalados ([`crate::policy`]).
//! - [`JavaRuntimes::ensure`]: devolve o Java que atende ao requisito, instalando se faltar:
//!   Temurin pelo Adoptium; se o Adoptium estiver fora do ar ou não publicar aquele Java, o
//!   runtime da Mojang.
//! - [`JavaRuntimes::check_updates`]: para cada Java instalado, procura a atualização mais
//!   nova do mesmo major (respeitando o teto, como o "8 ≤ u312"), instala e marca o antigo
//!   como substituído; o antigo é removido quando nenhum jogo o usa
//!   ([`JavaRuntimes::remove_superseded_unused`]).
//! - [`JavaRuntimes::lease`]: o jogo segura o Java enquanto roda; um Java segurado não é
//!   removido.
//! - [`JavaRuntimes::overview`]: a tabela de Configurações → Teste → Java (cada Java, os packs
//!   que o usam e por quê; CA-T21-04).
//!
//! Uma instalação por vez (trava interna): duas preparações de teste pedindo o mesmo Java não
//! baixam duas vezes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use serde::Serialize;
use warden_core::{CancellationToken, NoProgress, PackId, ProgressSink};
use warden_http::{DownloadRequest, HttpClient};
use warden_packwiz::HashFormat;

use crate::adoptium::{AdoptiumBuild, AdoptiumClient};
use crate::archive;
use crate::compat::CompatibilityTable;
use crate::error::{Error, JavaSource, Result};
use crate::mojang::{MojangComponent, MojangRuntimeClient};
use crate::policy::{self, JavaChoice, JavaChoiceRequest, JavaRequirement};
use crate::probe::{JavaProbe, ValidationCache};
use crate::runtime::{InstalledRuntime, Platform, RuntimeId, RuntimeSource};
use crate::store::{RuntimeStore, now_ms, remove_dir_with_retry};

/// Etapas do progresso de uma instalação (chaves do catálogo da interface).
pub mod stages {
    /// Consultando a fonte.
    pub const RESOLVE: &str = "java.resolve";
    /// Baixando.
    pub const DOWNLOAD: &str = "java.download";
    /// Extraindo o pacote.
    pub const EXTRACT: &str = "java.extract";
    /// Conferindo o Java (executando-o).
    pub const VALIDATE: &str = "java.validate";
}

/// Configuração do serviço.
#[derive(Debug, Clone)]
pub struct JavaRuntimesConfig {
    /// `shared/runtimes/`.
    pub runtimes_dir: PathBuf,
    /// Pasta dos pacotes baixados (`cache/tmp/java/`); cada pacote é apagado depois de
    /// instalado. Um `.part` de download interrompido fica para ser retomado.
    pub downloads_dir: PathBuf,
    /// Plataforma.
    pub platform: Platform,
}

impl JavaRuntimesConfig {
    /// Configuração com as pastas do app.
    pub fn for_app(paths: &warden_core::AppPaths) -> Result<Self> {
        Ok(Self {
            runtimes_dir: paths.runtimes_dir(),
            downloads_dir: paths.tmp_dir().join("java"),
            platform: Platform::current()?,
        })
    }
}

/// Um pack, para a tabela de uso.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PackJavaInput {
    /// Identificador do pack.
    pub pack_id: PackId,
    /// Nome do pack.
    pub name: String,
    /// O pedido de escolha do Java do pack (versão, loader, escolha do usuário).
    pub request: JavaChoiceRequest,
}

/// Um pack na tabela de Java, com o Java que ele usa e por quê.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PackJavaUse {
    /// Identificador do pack.
    pub pack_id: PackId,
    /// Nome do pack.
    pub name: String,
    /// A escolha (major, motivo e o Java instalado, se houver).
    pub choice: JavaChoice,
}

/// Um pack cujo Java não pôde ser decidido (versão sem regra e sem o JSON da versão).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PackJavaUnresolved {
    /// Identificador do pack.
    pub pack_id: PackId,
    /// Nome do pack.
    pub name: String,
    /// Versão do Minecraft.
    pub minecraft: String,
}

/// Uma linha da tabela de Java de Configurações.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRow {
    /// O Java.
    pub runtime: InstalledRuntime,
    /// Os packs que o usam, com o motivo.
    pub packs: Vec<PackJavaUse>,
    /// Se um jogo aberto está usando este Java.
    pub in_game: bool,
}

/// A tabela de Java de Configurações (CA-T21-04).
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct JavaOverview {
    /// Cada Java instalado.
    pub runtimes: Vec<RuntimeRow>,
    /// Packs cujo Java ainda não está instalado (será baixado no próximo teste).
    pub packs_to_download: Vec<PackJavaUse>,
    /// Packs sem decisão possível.
    pub unresolved: Vec<PackJavaUnresolved>,
    /// Pastas quebradas em `shared/runtimes/` (apagadas por "Remover Javas sem uso").
    pub broken_count: u32,
}

/// Uma atualização instalada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeUpdate {
    /// O Java antigo.
    pub from: RuntimeId,
    /// O Java novo.
    pub to: InstalledRuntime,
    /// Se o antigo já foi removido (não estava em uso).
    pub old_removed: bool,
}

/// Resultado de "Procurar atualizações do Java".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReport {
    /// Atualizações instaladas.
    pub updated: Vec<RuntimeUpdate>,
    /// Javas que já estavam na versão mais nova.
    pub up_to_date: Vec<RuntimeId>,
    /// Javas que não puderam ser conferidos agora (fonte fora do ar), com o código do erro.
    pub failed: Vec<RuntimeId>,
    /// Javas substituídos antes e removidos agora.
    pub removed: Vec<RuntimeId>,
}

/// Contagem de jogos usando cada Java.
#[derive(Debug, Default)]
struct Leases {
    counts: Mutex<HashMap<RuntimeId, usize>>,
}

impl Leases {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<RuntimeId, usize>> {
        self.counts.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn is_leased(&self, id: &RuntimeId) -> bool {
        self.lock().get(id).copied().unwrap_or(0) > 0
    }
}

/// Um jogo usando um Java. Enquanto existir, o Java não é removido.
#[derive(Debug)]
pub struct RuntimeLease {
    id: RuntimeId,
    leases: Arc<Leases>,
}

impl RuntimeLease {
    /// O Java segurado.
    #[must_use]
    pub fn id(&self) -> &RuntimeId {
        &self.id
    }
}

impl Drop for RuntimeLease {
    fn drop(&mut self) {
        let mut counts = self.leases.lock();
        if let Some(count) = counts.get_mut(&self.id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                counts.remove(&self.id);
            }
        }
    }
}

/// O serviço de Java. Barato de clonar.
#[derive(Debug, Clone)]
pub struct JavaRuntimes {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    store: RuntimeStore,
    downloads_dir: PathBuf,
    http: HttpClient,
    adoptium: AdoptiumClient,
    mojang: MojangRuntimeClient,
    probe: Arc<dyn JavaProbe>,
    validations: ValidationCache,
    table: &'static CompatibilityTable,
    leases: Arc<Leases>,
    install_lock: tokio::sync::Mutex<()>,
}

impl JavaRuntimes {
    /// Serviço com as fontes oficiais (ou as de `WARDEN_API_BASE_*`, em build de debug).
    pub fn new(
        config: JavaRuntimesConfig,
        http: HttpClient,
        probe: Arc<dyn JavaProbe>,
    ) -> Result<Self> {
        let adoptium = AdoptiumClient::new(http.clone())?;
        let mojang = MojangRuntimeClient::new(http.clone())?;
        Self::with_clients(config, http, adoptium, mojang, probe)
    }

    /// Serviço com clientes dados (testes).
    pub fn with_clients(
        config: JavaRuntimesConfig,
        http: HttpClient,
        adoptium: AdoptiumClient,
        mojang: MojangRuntimeClient,
        probe: Arc<dyn JavaProbe>,
    ) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(Inner {
                store: RuntimeStore::new(config.runtimes_dir, config.platform),
                downloads_dir: config.downloads_dir,
                http,
                adoptium,
                mojang,
                probe,
                validations: ValidationCache::default(),
                table: CompatibilityTable::builtin()?,
                leases: Arc::default(),
                install_lock: tokio::sync::Mutex::new(()),
            }),
        })
    }

    /// A pasta dos Javas.
    #[must_use]
    pub fn store(&self) -> &RuntimeStore {
        &self.inner.store
    }

    /// A tabela de compatibilidade em uso.
    #[must_use]
    pub fn table(&self) -> &'static CompatibilityTable {
        self.inner.table
    }

    /// Javas instalados.
    pub fn installed(&self) -> Result<Vec<InstalledRuntime>> {
        Ok(self.inner.store.list()?.runtimes)
    }

    /// Apaga sobras de instalações e remoções interrompidas e os Javas substituídos sem uso.
    /// Chamado ao abrir o app.
    pub fn startup_cleanup(&self) -> Result<usize> {
        let leftovers = self.inner.store.cleanup_leftovers()?;
        let removed = self.remove_superseded_unused()?;
        Ok(leftovers + removed.len())
    }

    /// `java_choice`: a política com os Javas instalados.
    pub fn choose(&self, request: &JavaChoiceRequest) -> Result<JavaChoice> {
        policy::choose(request, self.inner.table, &self.installed()?)
    }

    /// Segura um Java enquanto um jogo o usa.
    #[must_use]
    pub fn lease(&self, id: &RuntimeId) -> RuntimeLease {
        *self.inner.leases.lock().entry(id.clone()).or_insert(0) += 1;
        RuntimeLease {
            id: id.clone(),
            leases: Arc::clone(&self.inner.leases),
        }
    }

    /// Se um jogo está usando o Java.
    #[must_use]
    pub fn is_in_game(&self, id: &RuntimeId) -> bool {
        self.inner.leases.is_leased(id)
    }

    /// Confere um Java executando-o (com cache por caminho e data).
    pub async fn validate(&self, runtime: &InstalledRuntime) -> Result<crate::probe::JavaInfo> {
        self.inner
            .validations
            .validate(self.inner.probe.as_ref(), &runtime.java)
            .await
    }

    /// O Java para a escolha: o já instalado, ou instala o que atende.
    pub async fn ensure_choice(
        &self,
        choice: &JavaChoice,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<InstalledRuntime> {
        if let Some(runtime) = &choice.runtime {
            return Ok(runtime.clone());
        }
        let requirement = JavaRequirement {
            major: choice.major,
            max_update: choice.max_update,
        };
        self.ensure(requirement, progress, cancel).await
    }

    /// O Java instalado mais novo que atende, ou instala um.
    pub async fn ensure(
        &self,
        requirement: JavaRequirement,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<InstalledRuntime> {
        let _guard = self.inner.install_lock.lock().await;
        if let Some(runtime) = policy::pick_runtime(&requirement, &self.installed()?) {
            return Ok(runtime.clone());
        }
        self.install_newest(requirement, progress, cancel).await
    }

    /// Instala a atualização mais nova que atende ao requisito: Temurin; se a fonte falhar,
    /// Mojang. Precisa da trava de instalação.
    async fn install_newest(
        &self,
        requirement: JavaRequirement,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<InstalledRuntime> {
        progress.stage(stages::RESOLVE, "java.stage.resolve");
        match self.find_temurin(requirement, cancel).await {
            Ok(build) => {
                self.install_temurin(&build, requirement, progress, cancel)
                    .await
            }
            Err(error) if error.is_source_failure() => {
                tracing::warn!(%error, major = requirement.major, "Adoptium falhou; tentando o runtime da Mojang");
                match self.find_mojang(requirement, cancel).await {
                    Ok(component) => {
                        self.install_mojang(&component, requirement, progress, cancel)
                            .await
                    }
                    // Sem alternativa: o erro que vale é o do Adoptium (a fonte padrão).
                    Err(mojang) if mojang.is_source_failure() => {
                        tracing::warn!(error = %mojang, "runtime da Mojang também falhou");
                        Err(error)
                    }
                    Err(mojang) => Err(mojang),
                }
            }
            Err(error) => Err(error),
        }
    }

    async fn find_temurin(
        &self,
        requirement: JavaRequirement,
        cancel: &CancellationToken,
    ) -> Result<AdoptiumBuild> {
        let platform = self.inner.store.platform();
        let found = match requirement.max_update {
            Some(cap) => {
                self.inner
                    .adoptium
                    .latest_up_to(requirement.major, cap, platform, Some(cancel))
                    .await?
            }
            None => {
                self.inner
                    .adoptium
                    .latest(requirement.major, platform, Some(cancel))
                    .await?
            }
        };
        found.ok_or_else(|| Error::NoBuild {
            source_name: JavaSource::Adoptium,
            major: requirement.major,
            max_update: requirement.max_update,
            platform: platform.label(),
        })
    }

    async fn find_mojang(
        &self,
        requirement: JavaRequirement,
        cancel: &CancellationToken,
    ) -> Result<MojangComponent> {
        let platform = self.inner.store.platform();
        let components = self.inner.mojang.components(platform, Some(cancel)).await?;
        MojangRuntimeClient::pick(&components, requirement.major, requirement.max_update)
            .cloned()
            .ok_or_else(|| Error::NoBuild {
                source_name: JavaSource::Mojang,
                major: requirement.major,
                max_update: requirement.max_update,
                platform: platform.label(),
            })
    }

    /// Baixa, confere o SHA-256, extrai em pasta temporária, valida e renomeia.
    async fn install_temurin(
        &self,
        build: &AdoptiumBuild,
        requirement: JavaRequirement,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<InstalledRuntime> {
        let platform = self.inner.store.platform();
        let id = RuntimeId::new(
            RuntimeSource::Temurin,
            &build.version,
            &build.release_name,
            platform,
        );
        if let Some(existing) = self.inner.store.get(&id)? {
            return Ok(existing);
        }
        progress.stage(stages::DOWNLOAD, "java.stage.download");
        let archive = self.inner.downloads_dir.join(&build.package_name);
        let request = DownloadRequest::new(build.link.clone(), &archive)
            .expect_hash(HashFormat::Sha256, build.sha256.clone())
            .expect_size(build.size)
            .max_size(build.size);
        self.inner
            .http
            .download(&request, progress, Some(cancel))
            .await
            .map_err(Error::http(JavaSource::Adoptium))?;
        let result = self
            .install_from_archive(&archive, build, &id, requirement, progress, cancel)
            .await;
        // O pacote conferido não serve mais (instalado) ou está estragado (falhou ao extrair
        // ou validar); um download cancelado deixa só o `.part`, para retomar.
        if let Err(error) = std::fs::remove_file(&archive)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(%error, path = %archive.display(), "pacote do Java não apagado");
        }
        result
    }

    async fn install_from_archive(
        &self,
        archive_path: &Path,
        build: &AdoptiumBuild,
        id: &RuntimeId,
        requirement: JavaRequirement,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<InstalledRuntime> {
        warden_core::fault::check("java.install.after_download")
            .map_err(|e| Error::io("extrair", archive_path, e))?;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        progress.stage(stages::EXTRACT, "java.stage.extract");
        let temp = self.inner.store.create_temp_dir()?;
        let result = async {
            let source = archive_path.to_owned();
            let target = temp.clone();
            let kind = build.kind;
            tokio::task::spawn_blocking(move || archive::extract(&source, kind, &target))
                .await
                .map_err(|error| Error::Internal(format!("extração interrompida: {error}")))??;
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let runtime = InstalledRuntime {
                source: RuntimeSource::Temurin,
                release_name: build.release_name.clone(),
                update_cap: requirement.max_update,
                mojang_component: None,
                archive_sha256: Some(build.sha256.clone()),
                ..self.blank_runtime(id, build.version)
            };
            self.validate_and_commit(&temp, runtime, requirement, progress)
                .await
        }
        .await;
        if result.is_err() {
            discard_temp(&temp);
        }
        result
    }

    /// Baixa os arquivos da Mojang numa pasta temporária, valida e renomeia.
    async fn install_mojang(
        &self,
        component: &MojangComponent,
        requirement: JavaRequirement,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<InstalledRuntime> {
        let platform = self.inner.store.platform();
        let id = RuntimeId::new(
            RuntimeSource::Mojang,
            &component.version,
            &component.version_name,
            platform,
        );
        if let Some(existing) = self.inner.store.get(&id)? {
            return Ok(existing);
        }
        let entries = self.inner.mojang.manifest(component, Some(cancel)).await?;
        progress.stage(stages::DOWNLOAD, "java.stage.download");
        let temp = self.inner.store.create_temp_dir()?;
        let result = async {
            self.inner
                .mojang
                .download_all(&entries, &temp, progress, cancel)
                .await?;
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let runtime = InstalledRuntime {
                source: RuntimeSource::Mojang,
                release_name: component.version_name.clone(),
                update_cap: requirement.max_update,
                mojang_component: Some(component.name.clone()),
                archive_sha256: None,
                ..self.blank_runtime(&id, component.version)
            };
            self.validate_and_commit(&temp, runtime, requirement, progress)
                .await
        }
        .await;
        if result.is_err() {
            discard_temp(&temp);
        }
        result
    }

    fn blank_runtime(
        &self,
        id: &RuntimeId,
        version: crate::version::JavaVersion,
    ) -> InstalledRuntime {
        let platform = self.inner.store.platform();
        let home = self.inner.store.dir().join(id.as_str());
        InstalledRuntime {
            id: id.clone(),
            source: RuntimeSource::Temurin,
            version,
            vendor: String::new(),
            release_name: String::new(),
            arch: platform.arch_key().to_owned(),
            java: platform.java_in(&home),
            launcher: platform.javaw_in(&home),
            home,
            installed_at_ms: now_ms(),
            update_cap: None,
            mojang_component: None,
            archive_sha256: None,
            superseded_by: None,
        }
    }

    /// Executa o Java da pasta temporária, confere major, teto e 64 bits, e instala.
    async fn validate_and_commit(
        &self,
        temp: &Path,
        mut runtime: InstalledRuntime,
        requirement: JavaRequirement,
        progress: &dyn ProgressSink,
    ) -> Result<InstalledRuntime> {
        progress.stage(stages::VALIDATE, "java.stage.validate");
        let platform = self.inner.store.platform();
        let java = platform.java_in(temp);
        if !java.is_file() {
            return Err(Error::Archive {
                path: temp.to_owned(),
                message: format!("sem o programa {}", relative_java(platform)),
            });
        }
        let info = self.inner.probe.probe(&java).await?;
        if info.version.major != requirement.major
            || requirement
                .max_update
                .is_some_and(|cap| info.version.security > cap)
        {
            return Err(Error::UnexpectedVersion {
                expected_major: requirement.major,
                max_update: requirement.max_update,
                found: info.version,
            });
        }
        runtime.vendor = info.vendor;
        // A versão que vale é a que o próprio Java informa.
        runtime.version = info.version;
        let store = self.inner.store.clone();
        let temp = temp.to_owned();
        tokio::task::spawn_blocking(move || store.commit(&temp, &runtime))
            .await
            .map_err(|error| Error::Internal(format!("instalação interrompida: {error}")))?
    }

    /// "Procurar atualizações do Java": para cada Java instalado e não substituído, instala a
    /// atualização mais nova do mesmo major e da mesma fonte (respeitando o teto) e marca o
    /// antigo como substituído; depois remove os substituídos que nenhum jogo está usando.
    pub async fn check_updates(
        &self,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<UpdateReport> {
        let _guard = self.inner.install_lock.lock().await;
        let mut report = UpdateReport::default();
        let installed = self.installed()?;
        let current: Vec<&InstalledRuntime> = installed
            .iter()
            .filter(|runtime| runtime.superseded_by.is_none())
            .collect();
        // Um canal = fonte + major + teto; só o Java mais novo de cada canal é conferido.
        let mut channels: Vec<&InstalledRuntime> = Vec::new();
        for runtime in current {
            let same = channels.iter_mut().find(|other| {
                other.source == runtime.source
                    && other.version.major == runtime.version.major
                    && other.update_cap == runtime.update_cap
            });
            match same {
                Some(other) if other.version < runtime.version => *other = runtime,
                Some(_) => {}
                None => channels.push(runtime),
            }
        }
        for runtime in channels {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let requirement = JavaRequirement {
                major: runtime.version.major,
                max_update: runtime.update_cap,
            };
            match self
                .update_one(runtime, requirement, progress, cancel)
                .await
            {
                Ok(Some(new)) => {
                    let mut old_ids: Vec<RuntimeId> = installed
                        .iter()
                        .filter(|other| {
                            other.source == runtime.source
                                && other.version.major == runtime.version.major
                                && other.update_cap == runtime.update_cap
                                && other.id != new.id
                                && other.superseded_by.is_none()
                        })
                        .map(|other| other.id.clone())
                        .collect();
                    old_ids.sort();
                    for old in old_ids {
                        self.mark_superseded(&old, &new.id)?;
                        report.updated.push(RuntimeUpdate {
                            from: old,
                            to: new.clone(),
                            old_removed: false,
                        });
                    }
                }
                Ok(None) => report.up_to_date.push(runtime.id.clone()),
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(error) if error.is_source_failure() => {
                    tracing::warn!(%error, id = %runtime.id, "atualização do Java não conferida");
                    report.failed.push(runtime.id.clone());
                }
                Err(error) => return Err(error),
            }
        }
        let removed = self.remove_superseded_unused()?;
        for update in &mut report.updated {
            update.old_removed = removed.contains(&update.from);
        }
        report.removed = removed;
        Ok(report)
    }

    /// Instala a versão mais nova do canal, se for mais nova que `runtime`.
    async fn update_one(
        &self,
        runtime: &InstalledRuntime,
        requirement: JavaRequirement,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<Option<InstalledRuntime>> {
        match runtime.source {
            RuntimeSource::Temurin => {
                let build = self.find_temurin(requirement, cancel).await?;
                if build.version <= runtime.version {
                    return Ok(None);
                }
                self.install_temurin(&build, requirement, progress, cancel)
                    .await
                    .map(Some)
            }
            RuntimeSource::Mojang => {
                let component = self.find_mojang(requirement, cancel).await?;
                if component.version <= runtime.version {
                    return Ok(None);
                }
                self.install_mojang(&component, requirement, progress, cancel)
                    .await
                    .map(Some)
            }
        }
    }

    fn mark_superseded(&self, old: &RuntimeId, new: &RuntimeId) -> Result<()> {
        if let Some(mut runtime) = self.inner.store.get(old)? {
            runtime.superseded_by = Some(new.clone());
            RuntimeStore::write_metadata(&runtime)?;
        }
        Ok(())
    }

    /// Remove os Javas substituídos por uma atualização que nenhum jogo está usando. Devolve
    /// os removidos.
    pub fn remove_superseded_unused(&self) -> Result<Vec<RuntimeId>> {
        let mut removed = Vec::new();
        for runtime in self.installed()? {
            if runtime.superseded_by.is_some() && !self.is_in_game(&runtime.id) {
                self.inner.store.remove(&runtime.id)?;
                self.inner.validations.forget(&runtime.home);
                removed.push(runtime.id);
            }
        }
        Ok(removed)
    }

    /// `java_runtime_remove`: remove um Java, se nenhum jogo o estiver usando. Os packs que o
    /// usavam baixam de novo no próximo teste (ou usam outro do mesmo major).
    pub fn remove(&self, id: &RuntimeId) -> Result<()> {
        if self.is_in_game(id) {
            return Err(Error::RuntimeInUse { id: id.to_string() });
        }
        let runtime = self
            .inner
            .store
            .get(id)?
            .ok_or_else(|| Error::RuntimeNotFound { id: id.to_string() })?;
        self.inner.store.remove(id)?;
        self.inner.validations.forget(&runtime.home);
        Ok(())
    }

    /// "Remover Javas sem uso": remove os Javas que nenhum pack usa e nenhum jogo segura, e
    /// as pastas quebradas. Devolve os ids removidos.
    pub fn remove_unused(&self, packs: &[PackJavaInput]) -> Result<Vec<RuntimeId>> {
        let overview = self.overview(packs)?;
        let mut removed = Vec::new();
        for row in overview.runtimes {
            if row.packs.is_empty() && !row.in_game {
                self.inner.store.remove(&row.runtime.id)?;
                self.inner.validations.forget(&row.runtime.home);
                removed.push(row.runtime.id);
            }
        }
        for broken in self.inner.store.list()?.broken {
            self.inner.store.remove_broken(&broken)?;
        }
        Ok(removed)
    }

    /// A tabela de Java de Configurações: cada Java, os packs que o usam (com o motivo) e os
    /// packs cujo Java ainda será baixado.
    pub fn overview(&self, packs: &[PackJavaInput]) -> Result<JavaOverview> {
        let listing = self.inner.store.list()?;
        let mut rows: Vec<RuntimeRow> = listing
            .runtimes
            .iter()
            .map(|runtime| RuntimeRow {
                runtime: runtime.clone(),
                packs: Vec::new(),
                in_game: self.is_in_game(&runtime.id),
            })
            .collect();
        let mut packs_to_download = Vec::new();
        let mut unresolved = Vec::new();
        for pack in packs {
            match policy::choose(&pack.request, self.inner.table, &listing.runtimes) {
                Ok(choice) => {
                    let use_ = PackJavaUse {
                        pack_id: pack.pack_id,
                        name: pack.name.clone(),
                        choice,
                    };
                    let row = use_.choice.runtime.as_ref().and_then(|runtime| {
                        rows.iter_mut().find(|row| row.runtime.id == runtime.id)
                    });
                    match row {
                        Some(row) => row.packs.push(use_),
                        None => packs_to_download.push(use_),
                    }
                }
                Err(Error::VersionRequirementUnknown { minecraft }) => {
                    unresolved.push(PackJavaUnresolved {
                        pack_id: pack.pack_id,
                        name: pack.name.clone(),
                        minecraft,
                    });
                }
                Err(error) => return Err(error),
            }
        }
        for row in &mut rows {
            row.packs.sort_by_key(|pack| pack.name.to_lowercase());
        }
        // Mais novo primeiro.
        rows.sort_by(|a, b| {
            b.runtime
                .version
                .cmp(&a.runtime.version)
                .then_with(|| a.runtime.id.cmp(&b.runtime.id))
        });
        Ok(JavaOverview {
            runtimes: rows,
            packs_to_download,
            unresolved,
            broken_count: u32::try_from(listing.broken.len()).unwrap_or(u32::MAX),
        })
    }

    /// Instala a atualização mais nova de um major (botão de teste e testes de rede).
    pub async fn install_latest(
        &self,
        requirement: JavaRequirement,
        cancel: &CancellationToken,
    ) -> Result<InstalledRuntime> {
        let _guard = self.inner.install_lock.lock().await;
        self.install_newest(requirement, &NoProgress, cancel).await
    }
}

fn relative_java(platform: Platform) -> String {
    platform
        .java_in(Path::new(""))
        .to_string_lossy()
        .replace('\\', "/")
}

/// Apaga a pasta temporária de uma instalação que falhou. Se não der (antivírus), a limpeza da
/// próxima abertura apaga; o Java não aparece na lista porque a pasta começa com ponto.
fn discard_temp(temp: &Path) {
    if let Err(error) = remove_dir_with_retry(temp) {
        tracing::warn!(%error, path = %temp.display(), "pasta temporária do Java não apagada");
    }
}
