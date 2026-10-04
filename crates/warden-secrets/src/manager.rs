//! Serviço de segredos do app: estado, gravar, remover, ler na hora do uso e trocar de modo.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};

use secrecy::{ExposeSecret as _, SecretString};
use warden_core::fault;

use crate::envfile::EnvFileStore;
use crate::error::SecretsError;
use crate::keyring::KeyringStore;
use crate::kind::{BackendKind, SecretKind, SecretsStatus};
use crate::store::{SecretStore, normalize_secret};

/// Variável que troca o cofre do sistema pelo cofre de teste em arquivo, só em build de
/// debug: `WARDEN_SECRET_BACKEND=file:<pasta>` (ADR-0048).
pub const BACKEND_OVERRIDE_ENV: &str = "WARDEN_SECRET_BACKEND";

/// Substituto do cofre do sistema no desenvolvimento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendOverride {
    /// Cofre de teste em arquivo nesta pasta.
    TestFile(PathBuf),
}

impl BackendOverride {
    /// Interpreta o valor de `WARDEN_SECRET_BACKEND`. Só `file:<pasta>` é aceito.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let dir = value.strip_prefix("file:")?.trim();
        (!dir.is_empty()).then(|| Self::TestFile(PathBuf::from(dir)))
    }

    /// O substituto em vigor neste processo, só em build de debug:
    /// - `WARDEN_SECRET_BACKEND=file:<pasta>`, se definida;
    /// - senão, com a pasta de desenvolvimento (`WARDEN_DATA_ROOT`) definida,
    ///   `<pasta>/cofre-de-teste`: quem usa pasta de desenvolvimento nunca toca no cofre real,
    ///   mesmo se esquecer a segunda variável.
    ///
    /// Erro (com o valor recebido) se a variável estiver definida com outro formato: melhor não
    /// abrir do que cair no cofre real por engano.
    pub fn from_env(dev_root: Option<&Path>) -> Result<Option<Self>, String> {
        #[cfg(debug_assertions)]
        {
            if let Some(value) = std::env::var_os(BACKEND_OVERRIDE_ENV).filter(|v| !v.is_empty()) {
                let text = value.to_string_lossy();
                return Self::parse(&text).map(Some).ok_or_else(|| {
                    format!("{BACKEND_OVERRIDE_ENV} inválida: {text:?} (use file:<pasta>)")
                });
            }
            Ok(dev_root.map(|root| Self::TestFile(root.join("cofre-de-teste"))))
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = dev_root;
            Ok(None)
        }
    }
}

/// Como ler uma variável de ambiente (injetável nos testes, sem mexer no ambiente real).
pub type EnvLookup = fn(&str) -> Option<String>;

/// Configuração do serviço de segredos.
#[derive(Debug, Clone)]
pub struct SecretsConfig {
    /// Caminho do `.env` do modo [`BackendKind::Envfile`].
    pub env_file: PathBuf,
    /// Substituto do cofre do sistema (só tem efeito em build de debug).
    pub keyring_override: Option<BackendOverride>,
    /// Recuo de desenvolvimento: ler `CURSEFORGE_API_KEY`, `GEMINI_API_KEY` e `GITHUB_TOKEN`
    /// do ambiente quando o armazenamento não tem a chave (só tem efeito em build de debug).
    pub env_fallback: Option<EnvLookup>,
}

impl SecretsConfig {
    /// Configuração do app: `.env` em `env_file`, substituto do cofre se houver e recuo pelo
    /// ambiente real (ambos só em debug).
    #[must_use]
    pub fn for_app(env_file: PathBuf, keyring_override: Option<BackendOverride>) -> Self {
        Self {
            env_file,
            keyring_override,
            env_fallback: Some(|name| std::env::var(name).ok()),
        }
    }
}

/// Resultado de uma troca de modo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchReport {
    /// Modo anterior.
    pub from: BackendKind,
    /// Modo novo (já em vigor e gravado).
    pub to: BackendKind,
    /// Segredos movidos.
    pub moved: Vec<SecretKind>,
    /// Segredos que não puderam ser apagados da origem (a cópia nova vale; a antiga sobrou).
    pub leftovers: Vec<SecretKind>,
}

/// Armazenamento que não pôde ser aberto: toda operação devolve o erro de abertura.
#[derive(Debug)]
struct UnavailableStore {
    backend: BackendKind,
    message: String,
}

impl UnavailableStore {
    fn error(&self, kind: SecretKind) -> SecretsError {
        SecretsError::VaultUnavailable {
            kind,
            message: self.message.clone(),
        }
    }
}

impl SecretStore for UnavailableStore {
    fn backend(&self) -> BackendKind {
        self.backend
    }

    fn get(&self, kind: SecretKind) -> Result<Option<SecretString>, SecretsError> {
        Err(self.error(kind))
    }

    fn set(&self, kind: SecretKind, _value: &SecretString) -> Result<(), SecretsError> {
        Err(self.error(kind))
    }

    fn remove(&self, kind: SecretKind) -> Result<(), SecretsError> {
        Err(self.error(kind))
    }
}

/// Serviço de segredos. Seguro para usar de várias threads: gravações, remoções e trocas de
/// modo são serializadas; leituras não esperam.
pub struct Secrets {
    config: SecretsConfig,
    active: RwLock<Arc<dyn SecretStore>>,
    writes: Mutex<()>,
}

impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secrets")
            .field("backend", &self.backend())
            .field("env_file", &self.config.env_file)
            .finish_non_exhaustive()
    }
}

impl Secrets {
    /// Abre o serviço no modo `backend` (o gravado em `settings.json`). Não falha: se o cofre
    /// do sistema não abrir, cada operação devolve [`SecretsError::VaultUnavailable`].
    #[must_use]
    pub fn open(config: SecretsConfig, backend: BackendKind) -> Self {
        let store = store_for(&config, backend);
        Self {
            config,
            active: RwLock::new(store),
            writes: Mutex::new(()),
        }
    }

    /// Modo em uso.
    #[must_use]
    pub fn backend(&self) -> BackendKind {
        self.active_store().backend()
    }

    /// Quais segredos estão configurados no armazenamento em uso (sem o recuo pelo ambiente:
    /// "Configurada" quer dizer guardada pelo Warden).
    pub fn status(&self) -> Result<SecretsStatus, SecretsError> {
        let store = self.active_store();
        Ok(SecretsStatus {
            backend: store.backend(),
            curseforge: store.get(SecretKind::Curseforge)?.is_some(),
            gemini: store.get(SecretKind::Gemini)?.is_some(),
            github: store.get(SecretKind::Github)?.is_some(),
        })
    }

    /// Grava um segredo digitado pelo usuário: limpa e valida, grava e relê para conferir.
    pub fn set(&self, kind: SecretKind, value: &SecretString) -> Result<(), SecretsError> {
        let value = normalize_secret(kind, value)?;
        let _writes = self.lock_writes();
        let store = self.active_store();
        store.set(kind, &value)?;
        verify(store.as_ref(), kind, &value)
    }

    /// Apaga um segredo.
    pub fn remove(&self, kind: SecretKind) -> Result<(), SecretsError> {
        let _writes = self.lock_writes();
        self.active_store().remove(kind)
    }

    /// Lê um segredo na hora do uso (cliente da CurseForge, do Gemini, do GitHub). Em build de
    /// debug, se o armazenamento não tiver a chave, usa a variável de ambiente.
    pub fn get(&self, kind: SecretKind) -> Result<Option<SecretString>, SecretsError> {
        if let Some(value) = self.active_store().get(kind)? {
            return Ok(Some(value));
        }
        Ok(self.env_fallback(kind))
    }

    /// Lê um segredo que precisa existir.
    pub fn require(&self, kind: SecretKind) -> Result<SecretString, SecretsError> {
        self.get(kind)?.ok_or(SecretsError::NotConfigured { kind })
    }

    /// Valores atuais de todos os segredos (para a varredura antes de publicar e a redação,
    /// ARCHITECTURE §14). Inclui o recuo pelo ambiente em debug.
    pub fn current_values(&self) -> Result<Vec<SecretString>, SecretsError> {
        let mut values = Vec::new();
        for kind in SecretKind::ALL {
            if let Some(value) = self.get(kind)? {
                values.push(value);
            }
        }
        Ok(values)
    }

    /// Troca o modo, movendo as chaves (ARCHITECTURE §14; ADR-0025):
    ///
    /// 1. para cada segredo, grava no destino e relê para conferir (o que não existe na origem
    ///    é apagado do destino, para o destino ficar igual à origem);
    /// 2. chama `persist(to)`, que grava o modo novo em `settings.json`;
    /// 3. passa a usar o destino e só então apaga da origem (voltar para o cofre apaga o
    ///    `.env`).
    ///
    /// Se 1 ou 2 falharem, o destino volta a ser como era, o modo continua o antigo e nenhuma
    /// chave se perde ([`SecretsError::BackendSwitchFailed`]). Se apagar da origem falhar, a
    /// troca vale e o que sobrou aparece em [`SwitchReport::leftovers`].
    pub fn switch_backend(
        &self,
        to: BackendKind,
        persist: impl FnOnce(BackendKind) -> Result<(), String>,
    ) -> Result<SwitchReport, SecretsError> {
        let _writes = self.lock_writes();
        let from_store = self.active_store();
        let from = from_store.backend();
        if from == to {
            return Ok(SwitchReport {
                from,
                to,
                moved: Vec::new(),
                leftovers: Vec::new(),
            });
        }
        let to_store = store_for(&self.config, to);

        let mut previous = Vec::new();
        let copied =
            copy_all(from_store.as_ref(), to_store.as_ref(), &mut previous).and_then(|moved| {
                fault::check("secrets.switch.before_persist")?;
                persist(to).map_err(|message| SecretsError::PersistFailed { message })?;
                Ok(moved)
            });
        let moved = match copied {
            Ok(moved) => moved,
            Err(error) => {
                restore(to_store.as_ref(), previous);
                return Err(SecretsError::BackendSwitchFailed {
                    from,
                    to,
                    source: Box::new(error),
                });
            }
        };

        *self.active.write().unwrap_or_else(PoisonError::into_inner) = Arc::clone(&to_store);

        let mut leftovers = Vec::new();
        for kind in SecretKind::ALL {
            let removed = fault::check("secrets.switch.before_remove_source")
                .map_err(SecretsError::from)
                .and_then(|()| from_store.remove(kind));
            if let Err(error) = removed {
                tracing::warn!(?kind, %error, "troca de modo: cópia antiga não apagada");
                leftovers.push(kind);
            }
        }
        if leftovers.is_empty()
            && let Err(error) = from_store.purge()
        {
            tracing::warn!(%error, "troca de modo: armazenamento antigo não esvaziado");
        }
        tracing::info!(
            ?from,
            ?to,
            moved = moved.len(),
            "chaves movidas de armazenamento"
        );
        Ok(SwitchReport {
            from,
            to,
            moved,
            leftovers,
        })
    }

    fn active_store(&self) -> Arc<dyn SecretStore> {
        Arc::clone(&self.active.read().unwrap_or_else(PoisonError::into_inner))
    }

    fn lock_writes(&self) -> MutexGuard<'_, ()> {
        self.writes.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn env_fallback(&self, kind: SecretKind) -> Option<SecretString> {
        #[cfg(debug_assertions)]
        {
            let lookup = self.config.env_fallback?;
            lookup(kind.env_var())
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .map(SecretString::from)
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = kind;
            None
        }
    }
}

/// Copia e confere cada segredo; guarda em `previous` o que o destino tinha antes de cada
/// mudança, para desfazer.
fn copy_all(
    from: &dyn SecretStore,
    to: &dyn SecretStore,
    previous: &mut Vec<(SecretKind, Option<SecretString>)>,
) -> Result<Vec<SecretKind>, SecretsError> {
    let mut moved = Vec::new();
    for kind in SecretKind::ALL {
        let value = from.get(kind)?;
        let before = to.get(kind)?;
        previous.push((kind, before));
        match value {
            Some(value) => {
                to.set(kind, &value)?;
                fault::check("secrets.switch.after_copy")?;
                verify(to, kind, &value)?;
                moved.push(kind);
            }
            None => to.remove(kind)?,
        }
    }
    Ok(moved)
}

/// Desfaz as mudanças no destino. Falhas aqui não perdem nada (a origem está intacta), mas
/// ficam registradas.
fn restore(to: &dyn SecretStore, previous: Vec<(SecretKind, Option<SecretString>)>) {
    for (kind, before) in previous {
        let result = match before {
            Some(value) => to.set(kind, &value),
            None => to.remove(kind),
        };
        if let Err(error) = result {
            tracing::error!(?kind, %error, "troca de modo: não foi possível desfazer no destino");
        }
    }
    if let Some(error) = purge_if_empty(to) {
        tracing::error!(%error, "troca de modo: destino não esvaziado");
    }
}

/// Se o destino ficou vazio depois de desfazer, apaga o que o representa (o `.env` vazio).
fn purge_if_empty(to: &dyn SecretStore) -> Option<SecretsError> {
    let empty = SecretKind::ALL
        .iter()
        .all(|kind| matches!(to.get(*kind), Ok(None)));
    if empty { to.purge().err() } else { None }
}

fn verify(
    store: &dyn SecretStore,
    kind: SecretKind,
    value: &SecretString,
) -> Result<(), SecretsError> {
    match store.get(kind)? {
        Some(read) if read.expose_secret() == value.expose_secret() => Ok(()),
        _ => Err(SecretsError::VerifyFailed {
            kind,
            backend: store.backend(),
        }),
    }
}

fn store_for(config: &SecretsConfig, backend: BackendKind) -> Arc<dyn SecretStore> {
    match backend {
        BackendKind::Envfile => Arc::new(EnvFileStore::new(config.env_file.clone())),
        BackendKind::Keyring => {
            #[cfg(debug_assertions)]
            if let Some(BackendOverride::TestFile(dir)) = &config.keyring_override {
                return Arc::new(crate::testfile::TestFileStore::new(dir.clone()));
            }
            match KeyringStore::system() {
                Ok(store) => Arc::new(store),
                Err(error) => {
                    tracing::error!(%error, "cofre do sistema indisponível");
                    Arc::new(UnavailableStore {
                        backend,
                        message: error.to_string(),
                    })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{SecretsErrorCode, core};
    use std::path::Path;
    use warden_core::{CoreErrorCode, DomainCode, DomainError as _};

    const CF: &str = "$2a$10$chave-de-teste-da-curseforge";
    const GEMINI: &str = "AIza-chave-de-teste";
    const GITHUB: &str = "ghp_token_de_teste";

    fn secret(text: &str) -> SecretString {
        SecretString::from(text.to_owned())
    }

    fn config(root: &Path) -> SecretsConfig {
        SecretsConfig {
            env_file: root.join("config").join(".env"),
            keyring_override: Some(BackendOverride::TestFile(root.join("cofre-de-teste"))),
            env_fallback: None,
        }
    }

    fn fill(secrets: &Secrets) {
        secrets.set(SecretKind::Curseforge, &secret(CF)).unwrap();
        secrets.set(SecretKind::Gemini, &secret(GEMINI)).unwrap();
        secrets.set(SecretKind::Github, &secret(GITHUB)).unwrap();
    }

    fn value(secrets: &Secrets, kind: SecretKind) -> Option<String> {
        secrets
            .get(kind)
            .unwrap()
            .map(|value| value.expose_secret().to_owned())
    }

    fn assert_all(secrets: &Secrets) {
        assert_eq!(value(secrets, SecretKind::Curseforge).as_deref(), Some(CF));
        assert_eq!(value(secrets, SecretKind::Gemini).as_deref(), Some(GEMINI));
        assert_eq!(value(secrets, SecretKind::Github).as_deref(), Some(GITHUB));
    }

    #[test]
    fn substituto_do_cofre() {
        assert_eq!(
            BackendOverride::parse("file:C:/dados/cofre"),
            Some(BackendOverride::TestFile(PathBuf::from("C:/dados/cofre")))
        );
        assert_eq!(BackendOverride::parse("file:"), None);
        assert_eq!(BackendOverride::parse("keyring"), None);
        assert_eq!(BackendOverride::parse("C:/x"), None);
    }

    /// Critério 3 da F0-05 (cofre de teste em arquivo): gravar e "reiniciar" mantém o estado.
    #[test]
    fn f0_05_ca3_cofre_de_teste_mantem_o_estado_ao_reabrir() {
        let dir = tempfile::tempdir().unwrap();
        {
            let secrets = Secrets::open(config(dir.path()), BackendKind::Keyring);
            let status = secrets.status().unwrap();
            assert!(!status.curseforge && !status.gemini && !status.github);
            secrets
                .set(SecretKind::Curseforge, &secret(&format!("  {CF}\n")))
                .unwrap();
        }
        let secrets = Secrets::open(config(dir.path()), BackendKind::Keyring);
        let status = secrets.status().unwrap();
        assert_eq!(
            status,
            SecretsStatus {
                backend: BackendKind::Keyring,
                curseforge: true,
                gemini: false,
                github: false
            }
        );
        assert_eq!(value(&secrets, SecretKind::Curseforge).as_deref(), Some(CF));
        secrets.remove(SecretKind::Curseforge).unwrap();
        assert!(!secrets.status().unwrap().curseforge);
    }

    /// Critério 3 da F0-05 (modo `.env`): gravar e reabrir mantém o estado.
    #[test]
    fn f0_05_ca3_modo_env_mantem_o_estado_ao_reabrir() {
        let dir = tempfile::tempdir().unwrap();
        Secrets::open(config(dir.path()), BackendKind::Envfile)
            .set(SecretKind::Gemini, &secret(GEMINI))
            .unwrap();
        let secrets = Secrets::open(config(dir.path()), BackendKind::Envfile);
        assert!(secrets.status().unwrap().gemini);
        assert_eq!(secrets.backend(), BackendKind::Envfile);
        let text = std::fs::read_to_string(dir.path().join("config/.env")).unwrap();
        assert_eq!(text, format!("GEMINI_API_KEY='{GEMINI}'\n"));
    }

    #[test]
    fn valor_invalido_nao_grava() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Secrets::open(config(dir.path()), BackendKind::Keyring);
        let error = secrets
            .set(SecretKind::Github, &secret(" \n "))
            .unwrap_err();
        assert_eq!(
            error.code(),
            DomainCode::Domain(SecretsErrorCode::InvalidValue)
        );
        assert!(!secrets.status().unwrap().github);
    }

    #[test]
    fn require_sem_chave_e_nao_configurada() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Secrets::open(config(dir.path()), BackendKind::Keyring);
        let error = secrets.require(SecretKind::Gemini).unwrap_err();
        assert_eq!(
            error.code(),
            DomainCode::Domain(SecretsErrorCode::NotConfigured)
        );
    }

    #[test]
    fn recuo_pelo_ambiente_so_completa_o_que_falta() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config(dir.path());
        config.env_fallback = Some(|name| match name {
            "GEMINI_API_KEY" => Some(" do-ambiente ".to_owned()),
            "CURSEFORGE_API_KEY" => Some("ambiente-perde".to_owned()),
            _ => None,
        });
        let secrets = Secrets::open(config, BackendKind::Keyring);
        secrets.set(SecretKind::Curseforge, &secret(CF)).unwrap();
        assert_eq!(value(&secrets, SecretKind::Curseforge).as_deref(), Some(CF));
        assert_eq!(
            value(&secrets, SecretKind::Gemini).as_deref(),
            Some("do-ambiente")
        );
        assert_eq!(value(&secrets, SecretKind::Github), None);
        // O estado mostra só o que o Warden guarda.
        assert!(!secrets.status().unwrap().gemini);
        assert_eq!(secrets.current_values().unwrap().len(), 2);
    }

    /// CA-T21-03 (backend) e critério 3 da F0-05: ida para o `.env` e volta para o cofre movem
    /// as três chaves e apagam a cópia anterior; reabrir mantém o estado.
    #[test]
    fn f0_05_ca3_ca_t21_03_troca_de_modo_move_as_chaves() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("cofre-de-teste");
        let env_file = dir.path().join("config/.env");
        let secrets = Secrets::open(config(dir.path()), BackendKind::Keyring);
        fill(&secrets);

        let mut persisted = None;
        let report = secrets
            .switch_backend(BackendKind::Envfile, |mode| {
                persisted = Some(mode);
                Ok(())
            })
            .unwrap();
        assert_eq!(persisted, Some(BackendKind::Envfile));
        assert_eq!(report.moved, SecretKind::ALL.to_vec());
        assert!(report.leftovers.is_empty());
        assert_eq!(secrets.backend(), BackendKind::Envfile);
        assert_all(&secrets);
        let text = std::fs::read_to_string(&env_file).unwrap();
        assert!(text.contains(CF) && text.contains(GEMINI) && text.contains(GITHUB));
        assert_eq!(
            std::fs::read_dir(&vault).unwrap().count(),
            0,
            "cofre não esvaziado"
        );

        let reopened = Secrets::open(config(dir.path()), BackendKind::Envfile);
        assert_all(&reopened);
        let status = reopened.status().unwrap();
        assert!(status.curseforge && status.gemini && status.github);

        reopened
            .switch_backend(BackendKind::Keyring, |_| Ok(()))
            .unwrap();
        assert!(!env_file.exists(), "voltar para o cofre apaga o .env");
        assert_all(&reopened);
        assert_all(&Secrets::open(config(dir.path()), BackendKind::Keyring));
    }

    #[test]
    fn trocar_para_o_mesmo_modo_nao_faz_nada() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Secrets::open(config(dir.path()), BackendKind::Keyring);
        fill(&secrets);
        let report = secrets
            .switch_backend(BackendKind::Keyring, |_| panic!("não deve gravar"))
            .unwrap();
        assert!(report.moved.is_empty());
        assert_all(&secrets);
    }

    #[test]
    fn destino_com_sobra_antiga_fica_igual_a_origem() {
        let dir = tempfile::tempdir().unwrap();
        let env = EnvFileStore::new(dir.path().join("config/.env"));
        env.set(SecretKind::Github, &secret("ghp_velho")).unwrap();
        let secrets = Secrets::open(config(dir.path()), BackendKind::Keyring);
        secrets.set(SecretKind::Gemini, &secret(GEMINI)).unwrap();
        secrets
            .switch_backend(BackendKind::Envfile, |_| Ok(()))
            .unwrap();
        assert_eq!(value(&secrets, SecretKind::Github), None);
        assert_eq!(value(&secrets, SecretKind::Gemini).as_deref(), Some(GEMINI));
    }

    fn assert_rolled_back(dir: &Path, secrets: &Secrets, error: &SecretsError) {
        assert_eq!(
            error.code(),
            DomainCode::Domain(SecretsErrorCode::BackendSwitchFailed)
        );
        assert_eq!(secrets.backend(), BackendKind::Keyring);
        assert_all(secrets);
        assert_all(&Secrets::open(config(dir), BackendKind::Keyring));
        assert!(
            !dir.join("config/.env").exists(),
            "o destino desfeito não deve deixar .env"
        );
    }

    /// Critério 3 da F0-05 / CA-T21-03: falha injetada no meio da cópia não perde nenhuma
    /// chave e mantém o modo antigo.
    #[test]
    fn f0_05_ca3_falha_no_meio_da_copia_nao_perde_chaves() {
        for successes in 0..3 {
            let dir = tempfile::tempdir().unwrap();
            let secrets = Secrets::open(config(dir.path()), BackendKind::Keyring);
            fill(&secrets);
            let _guard = fault::arm_after("secrets.switch.after_copy", successes);
            let error = secrets
                .switch_backend(BackendKind::Envfile, |_| panic!("não deve gravar o modo"))
                .unwrap_err();
            assert_eq!(fault::hits("secrets.switch.after_copy"), successes + 1);
            assert_rolled_back(dir.path(), &secrets, &error);
        }
    }

    /// Falha ao gravar no disco no meio da cópia (escrita atômica do `.env`).
    #[test]
    fn f0_05_ca3_falha_de_disco_no_destino_nao_perde_chaves() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Secrets::open(config(dir.path()), BackendKind::Keyring);
        fill(&secrets);
        let _guard = fault::arm_after("atomic_write.before_rename", 1);
        let error = secrets
            .switch_backend(BackendKind::Envfile, |_| Ok(()))
            .unwrap_err();
        let source = std::error::Error::source(&error).unwrap().to_string();
        assert!(source.contains("renomear"), "{source}");
        assert_rolled_back(dir.path(), &secrets, &error);
    }

    /// Falha ao gravar o modo em `settings.json`: nada muda.
    #[test]
    fn f0_05_ca3_falha_ao_gravar_o_modo_nao_perde_chaves() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Secrets::open(config(dir.path()), BackendKind::Keyring);
        fill(&secrets);
        let error = secrets
            .switch_backend(BackendKind::Envfile, |_| Err("disco cheio".to_owned()))
            .unwrap_err();
        assert!(error.detail().unwrap().contains("disco cheio"));
        assert_rolled_back(dir.path(), &secrets, &error);

        let _guard = fault::arm("secrets.switch.before_persist");
        let error = secrets
            .switch_backend(BackendKind::Envfile, |_| panic!("não deve gravar o modo"))
            .unwrap_err();
        assert_rolled_back(dir.path(), &secrets, &error);
    }

    /// Falha ao apagar da origem depois de a troca valer: as chaves existem no destino, a
    /// troca vale e o que sobrou é informado.
    #[test]
    fn f0_05_ca3_falha_ao_apagar_a_origem_mantem_a_troca() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Secrets::open(config(dir.path()), BackendKind::Keyring);
        fill(&secrets);
        let _guard = fault::arm_after("secrets.switch.before_remove_source", 1);
        let report = secrets
            .switch_backend(BackendKind::Envfile, |_| Ok(()))
            .unwrap();
        assert_eq!(report.leftovers, vec![SecretKind::Gemini]);
        assert_eq!(secrets.backend(), BackendKind::Envfile);
        assert_all(&secrets);
    }

    #[test]
    fn cofre_indisponivel_responde_com_erro() {
        let store = UnavailableStore {
            backend: BackendKind::Keyring,
            message: "serviço parado".to_owned(),
        };
        for result in [
            store.get(SecretKind::Gemini).map(|_| ()),
            store.set(SecretKind::Gemini, &secret("x")),
            store.remove(SecretKind::Gemini),
        ] {
            let error = result.unwrap_err();
            assert_eq!(
                error.code(),
                DomainCode::Domain(SecretsErrorCode::VaultUnavailable)
            );
            assert!(error.retryable());
        }
    }

    #[test]
    fn erro_de_disco_vira_codigo_comum() {
        let dir = tempfile::tempdir().unwrap();
        // `config` é um arquivo: o `.env` não pode ser criado dentro dele.
        std::fs::write(dir.path().join("config"), b"").unwrap();
        let secrets = Secrets::open(config(dir.path()), BackendKind::Envfile);
        let error = secrets
            .set(SecretKind::Gemini, &secret(GEMINI))
            .unwrap_err();
        assert_eq!(error.code(), core(CoreErrorCode::Io));
        assert!(!error.to_string().contains(GEMINI));
    }

    #[test]
    fn debug_nao_mostra_valores() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Secrets::open(config(dir.path()), BackendKind::Keyring);
        fill(&secrets);
        let text = format!("{secrets:?} {:?}", secrets.get(SecretKind::Curseforge));
        assert!(!text.contains(CF), "{text}");
    }
}
