//! Serviço de segredos do app: estado, gravar, remover, ler na hora do uso e a migração única
//! do `.env` antigo para o cofre (ADR-0069).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use secrecy::{ExposeSecret as _, SecretString};
use warden_core::fault;

use crate::error::SecretsError;
use crate::keyring::KeyringStore;
use crate::kind::{SecretKind, SecretsStatus};
use crate::legacy_env::{read_legacy_env, remove_legacy_env};
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
    /// Caminho do `.env` do antigo modo "guardar num arquivo" (ADR-0025), lido só pela
    /// migração para o cofre ([`Secrets::migrate_legacy_env`]).
    pub legacy_env_file: PathBuf,
    /// Substituto do cofre do sistema (só tem efeito em build de debug).
    pub keyring_override: Option<BackendOverride>,
    /// Recuo de desenvolvimento: ler `CURSEFORGE_API_KEY`, `GEMINI_API_KEY`, `OPENAI_API_KEY`,
    /// `ANTHROPIC_API_KEY` e `GITHUB_TOKEN` do ambiente quando o cofre não tem a chave (só tem
    /// efeito em build de debug).
    pub env_fallback: Option<EnvLookup>,
}

impl SecretsConfig {
    /// Configuração do app: o `.env` antigo em `legacy_env_file`, substituto do cofre se houver
    /// e recuo pelo ambiente real (ambos só em debug).
    #[must_use]
    pub fn for_app(legacy_env_file: PathBuf, keyring_override: Option<BackendOverride>) -> Self {
        Self {
            legacy_env_file,
            keyring_override,
            env_fallback: Some(|name| std::env::var(name).ok()),
        }
    }
}

/// O que a migração do `.env` antigo fez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvMigration {
    /// Chaves gravadas no cofre (e conferidas) a partir do `.env`.
    pub moved: Vec<SecretKind>,
    /// Chaves do `.env` que o cofre já tinha e ficaram como estavam no cofre.
    pub kept: Vec<SecretKind>,
    /// O `.env` foi apagado. `false`: as chaves já estão no cofre, mas o arquivo sobrou (a
    /// próxima abertura tenta apagar de novo).
    pub file_removed: bool,
}

/// Cofre que não pôde ser aberto: toda operação devolve o erro de abertura.
#[derive(Debug)]
struct UnavailableStore {
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

/// Serviço de segredos. Seguro para usar de várias threads: gravações, remoções e a migração
/// são serializadas; leituras não esperam.
pub struct Secrets {
    config: SecretsConfig,
    store: Arc<dyn SecretStore>,
    writes: Mutex<()>,
}

impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secrets")
            .field("store", &self.store)
            .field("legacy_env_file", &self.config.legacy_env_file)
            .finish_non_exhaustive()
    }
}

/// Menor chave que mostra o final (`••••` + 4 caracteres): numa chave curta, 4 caracteres
/// seriam uma parte grande demais dela. As chaves reais têm de 30 a 200 caracteres.
const MIN_LEN_FOR_HINT: usize = 16;

impl Secrets {
    /// Abre o cofre do sistema (ou o de teste, em debug). Não falha: se o cofre do sistema não
    /// abrir, cada operação devolve [`SecretsError::VaultUnavailable`].
    #[must_use]
    pub fn open(config: SecretsConfig) -> Self {
        let store = store_for(&config);
        Self {
            config,
            store,
            writes: Mutex::new(()),
        }
    }

    /// Quais segredos estão configurados no cofre (sem o recuo pelo ambiente: "configurada"
    /// quer dizer guardada pelo Warden).
    pub fn status(&self) -> Result<SecretsStatus, SecretsError> {
        let store = &self.store;
        Ok(SecretsStatus {
            curseforge: store.get(SecretKind::Curseforge)?.is_some(),
            gemini: store.get(SecretKind::Gemini)?.is_some(),
            openai: store.get(SecretKind::Openai)?.is_some(),
            anthropic: store.get(SecretKind::Anthropic)?.is_some(),
            github: store.get(SecretKind::Github)?.is_some(),
        })
    }

    /// Os últimos 4 caracteres da chave guardada, para a interface mostrar `••••xxxx` (a chave
    /// inteira nunca sai do backend). `None` sem chave ou com uma chave curta demais.
    pub fn hint(&self, kind: SecretKind) -> Result<Option<String>, SecretsError> {
        Ok(self.store.get(kind)?.and_then(|value| {
            let chars: Vec<char> = value.expose_secret().chars().collect();
            (chars.len() >= MIN_LEN_FOR_HINT).then(|| chars[chars.len() - 4..].iter().collect())
        }))
    }

    /// Grava um segredo digitado pelo usuário: limpa e valida, grava e relê para conferir.
    pub fn set(&self, kind: SecretKind, value: &SecretString) -> Result<(), SecretsError> {
        let value = normalize_secret(kind, value)?;
        let _writes = self.lock_writes();
        self.store.set(kind, &value)?;
        verify(self.store.as_ref(), kind, &value)
    }

    /// Apaga um segredo.
    pub fn remove(&self, kind: SecretKind) -> Result<(), SecretsError> {
        let _writes = self.lock_writes();
        self.store.remove(kind)
    }

    /// Lê um segredo na hora do uso (cliente da CurseForge, dos provedores de IA, do GitHub). Em
    /// build de debug, se o cofre não tiver a chave, usa a variável de ambiente.
    pub fn get(&self, kind: SecretKind) -> Result<Option<SecretString>, SecretsError> {
        if let Some(value) = self.store.get(kind)? {
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

    /// Leva as chaves do `.env` antigo para o cofre (ADR-0069), ao abrir o app:
    ///
    /// 1. sem o arquivo, não faz nada (`Ok(None)`);
    /// 2. cada chave do arquivo é gravada no cofre e relida para conferir. Com `env_wins`
    ///    (o `settings.json` ainda dizia que o `.env` era o lugar das chaves), o valor do
    ///    arquivo substitui o do cofre; sem ele, o cofre já era o lugar em uso e só recebe as
    ///    chaves que não tem;
    /// 3. só depois de **todas** conferidas, o arquivo é apagado.
    ///
    /// Se algo falhar em 2, o arquivo fica intacto e nada se perde
    /// ([`SecretsError::EnvMigrationFailed`]); a próxima abertura tenta de novo (o que já foi
    /// para o cofre é regravado igual). Se apagar falhar, as chaves já estão no cofre e
    /// [`EnvMigration::file_removed`] diz que o arquivo sobrou.
    pub fn migrate_legacy_env(&self, env_wins: bool) -> Result<Option<EnvMigration>, SecretsError> {
        let _writes = self.lock_writes();
        let path = &self.config.legacy_env_file;
        let wrap = |source| SecretsError::EnvMigrationFailed {
            source: Box::new(source),
        };
        let Some(entries) = read_legacy_env(path).map_err(wrap)? else {
            return Ok(None);
        };
        let mut moved = Vec::new();
        let mut kept = Vec::new();
        for (kind, value) in entries {
            let copied = (|| {
                let current = self.store.get(kind)?;
                if !env_wins && current.is_some() {
                    return Ok(false);
                }
                self.store.set(kind, &value)?;
                fault::check("secrets.migrate.after_copy")?;
                verify(self.store.as_ref(), kind, &value)?;
                Ok(true)
            })()
            .map_err(wrap)?;
            if copied {
                moved.push(kind);
            } else {
                kept.push(kind);
            }
        }
        let removed = fault::check("secrets.migrate.before_remove")
            .map_err(SecretsError::from)
            .and_then(|()| remove_legacy_env(path));
        let file_removed = match removed {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(%error, "migração do .env: chaves no cofre, mas o arquivo sobrou");
                false
            }
        };
        tracing::info!(
            moved = moved.len(),
            kept = kept.len(),
            file_removed,
            "chaves do .env antigo levadas para o cofre"
        );
        Ok(Some(EnvMigration {
            moved,
            kept,
            file_removed,
        }))
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

fn verify(
    store: &dyn SecretStore,
    kind: SecretKind,
    value: &SecretString,
) -> Result<(), SecretsError> {
    match store.get(kind)? {
        Some(read) if read.expose_secret() == value.expose_secret() => Ok(()),
        _ => Err(SecretsError::VerifyFailed { kind }),
    }
}

fn store_for(config: &SecretsConfig) -> Arc<dyn SecretStore> {
    #[cfg(debug_assertions)]
    if let Some(BackendOverride::TestFile(dir)) = &config.keyring_override {
        return Arc::new(crate::testfile::TestFileStore::new(dir.clone()));
    }
    #[cfg(not(debug_assertions))]
    let _ = config;
    match KeyringStore::system() {
        Ok(store) => Arc::new(store),
        Err(error) => {
            tracing::error!(%error, "cofre do sistema indisponível");
            Arc::new(UnavailableStore {
                message: error.to_string(),
            })
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
    const OPENAI: &str = "sk-proj-chave-de-teste-da-openai";
    const ANTHROPIC: &str = "sk-ant-api03-chave-de-teste-da-anthropic";

    fn secret(text: &str) -> SecretString {
        SecretString::from(text.to_owned())
    }

    fn config(root: &Path) -> SecretsConfig {
        SecretsConfig {
            legacy_env_file: root.join("config").join(".env"),
            keyring_override: Some(BackendOverride::TestFile(root.join("cofre-de-teste"))),
            env_fallback: None,
        }
    }

    fn env_file(root: &Path) -> PathBuf {
        root.join("config").join(".env")
    }

    /// Um `.env` como o do antigo modo arquivo, com as cinco chaves.
    fn write_legacy_env(root: &Path) {
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(
            env_file(root),
            format!(
                "CURSEFORGE_API_KEY='{CF}'\nGEMINI_API_KEY='{GEMINI}'\nOPENAI_API_KEY='{OPENAI}'\nANTHROPIC_API_KEY='{ANTHROPIC}'\nGITHUB_TOKEN='{GITHUB}'\n"
            ),
        )
        .unwrap();
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
        assert_eq!(value(secrets, SecretKind::Openai).as_deref(), Some(OPENAI));
        assert_eq!(
            value(secrets, SecretKind::Anthropic).as_deref(),
            Some(ANTHROPIC)
        );
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
            let secrets = Secrets::open(config(dir.path()));
            let status = secrets.status().unwrap();
            assert!(!status.curseforge && !status.gemini && !status.github);
            secrets
                .set(SecretKind::Curseforge, &secret(&format!("  {CF}\n")))
                .unwrap();
        }
        let secrets = Secrets::open(config(dir.path()));
        let status = secrets.status().unwrap();
        assert_eq!(
            status,
            SecretsStatus {
                curseforge: true,
                gemini: false,
                openai: false,
                anthropic: false,
                github: false
            }
        );
        assert_eq!(value(&secrets, SecretKind::Curseforge).as_deref(), Some(CF));
        secrets.remove(SecretKind::Curseforge).unwrap();
        assert!(!secrets.status().unwrap().curseforge);
    }

    /// Só os últimos 4 caracteres saem do cofre; chave curta não mostra nada.
    #[test]
    fn final_da_chave() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Secrets::open(config(dir.path()));
        assert_eq!(secrets.hint(SecretKind::Openai).unwrap(), None);
        secrets.set(SecretKind::Openai, &secret(OPENAI)).unwrap();
        assert_eq!(
            secrets.hint(SecretKind::Openai).unwrap().as_deref(),
            Some("enai")
        );
        secrets
            .set(SecretKind::Github, &secret("ghp_curta"))
            .unwrap();
        assert_eq!(secrets.hint(SecretKind::Github).unwrap(), None);
        // Caracteres fora do ASCII não são cortados no meio.
        secrets
            .set(SecretKind::Gemini, &secret("chave-de-teste-com-ção"))
            .unwrap();
        assert_eq!(
            secrets.hint(SecretKind::Gemini).unwrap().as_deref(),
            Some("-ção")
        );
    }

    #[test]
    fn valor_invalido_nao_grava() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Secrets::open(config(dir.path()));
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
        let secrets = Secrets::open(config(dir.path()));
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
        let secrets = Secrets::open(config);
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

    /// ADR-0069: sem `.env` antigo, a migração não faz nada.
    #[test]
    fn migracao_sem_env_nao_faz_nada() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Secrets::open(config(dir.path()));
        assert_eq!(secrets.migrate_legacy_env(true).unwrap(), None);
        assert!(!secrets.status().unwrap().curseforge);
    }

    /// ADR-0069 (CA-T21-07): quem estava no modo `.env` tem as cinco chaves levadas para o
    /// cofre, conferidas, e o arquivo apagado; reabrir mantém tudo.
    #[test]
    fn migracao_leva_as_chaves_para_o_cofre_e_apaga_o_env() {
        let dir = tempfile::tempdir().unwrap();
        write_legacy_env(dir.path());
        let secrets = Secrets::open(config(dir.path()));
        // Sobra antiga no cofre (de antes de ir para o `.env`): o `.env` vale.
        secrets
            .set(SecretKind::Github, &secret("ghp_velho"))
            .unwrap();
        let report = secrets.migrate_legacy_env(true).unwrap().unwrap();
        assert_eq!(report.moved, SecretKind::ALL.to_vec());
        assert!(report.kept.is_empty());
        assert!(report.file_removed);
        assert!(
            !env_file(dir.path()).exists(),
            "o .env deveria ter sido apagado"
        );
        assert_all(&secrets);
        assert_all(&Secrets::open(config(dir.path())));
        // Rodar de novo não faz nada.
        assert_eq!(secrets.migrate_legacy_env(true).unwrap(), None);
    }

    /// Com o cofre como lugar em uso (o `.env` era uma sobra), o cofre não perde o que tem:
    /// o arquivo só completa o que falta.
    #[test]
    fn migracao_com_o_cofre_em_uso_so_completa_o_que_falta() {
        let dir = tempfile::tempdir().unwrap();
        write_legacy_env(dir.path());
        let secrets = Secrets::open(config(dir.path()));
        secrets
            .set(SecretKind::Github, &secret("ghp_atual"))
            .unwrap();
        let report = secrets.migrate_legacy_env(false).unwrap().unwrap();
        assert_eq!(report.kept, vec![SecretKind::Github]);
        assert_eq!(report.moved.len(), 4);
        assert_eq!(
            value(&secrets, SecretKind::Github).as_deref(),
            Some("ghp_atual")
        );
        assert_eq!(value(&secrets, SecretKind::Gemini).as_deref(), Some(GEMINI));
        assert!(!env_file(dir.path()).exists());
    }

    /// Falha no meio da cópia: o `.env` fica intacto, nenhuma chave se perde, e a tentativa
    /// seguinte termina o serviço.
    #[test]
    fn migracao_que_falha_no_meio_nao_apaga_o_env() {
        for successes in 0..5 {
            let dir = tempfile::tempdir().unwrap();
            write_legacy_env(dir.path());
            let before = std::fs::read(env_file(dir.path())).unwrap();
            let secrets = Secrets::open(config(dir.path()));
            {
                let _guard = fault::arm_after("secrets.migrate.after_copy", successes);
                let error = secrets.migrate_legacy_env(true).unwrap_err();
                assert_eq!(
                    error.code(),
                    DomainCode::Domain(SecretsErrorCode::EnvMigrationFailed)
                );
                assert!(!error.to_string().contains(CF));
            }
            assert_eq!(std::fs::read(env_file(dir.path())).unwrap(), before);
            let report = secrets.migrate_legacy_env(true).unwrap().unwrap();
            assert!(report.file_removed);
            assert_all(&secrets);
        }
    }

    /// Cofre fora do ar: nada é apagado e o erro diz que dá para tentar de novo.
    #[test]
    fn migracao_com_o_cofre_fora_do_ar_mantem_o_env() {
        let dir = tempfile::tempdir().unwrap();
        write_legacy_env(dir.path());
        let secrets = Secrets {
            config: config(dir.path()),
            store: Arc::new(UnavailableStore {
                message: "serviço parado".to_owned(),
            }),
            writes: Mutex::new(()),
        };
        let error = secrets.migrate_legacy_env(true).unwrap_err();
        assert!(error.retryable());
        assert!(env_file(dir.path()).exists());
    }

    /// `.env` que não é texto: erro próprio, arquivo intacto.
    #[test]
    fn migracao_de_env_ilegivel_mantem_o_arquivo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        std::fs::write(env_file(dir.path()), [0xff, 0xfe, 0x00]).unwrap();
        let secrets = Secrets::open(config(dir.path()));
        let error = secrets.migrate_legacy_env(true).unwrap_err();
        assert_eq!(
            error.code(),
            DomainCode::Domain(SecretsErrorCode::EnvMigrationFailed)
        );
        assert!(env_file(dir.path()).exists());
    }

    /// Apagar o `.env` falha depois de as chaves estarem no cofre: a migração vale e diz que o
    /// arquivo sobrou.
    #[test]
    fn migracao_que_nao_apaga_o_env_avisa() {
        let dir = tempfile::tempdir().unwrap();
        write_legacy_env(dir.path());
        let secrets = Secrets::open(config(dir.path()));
        let _guard = fault::arm("secrets.migrate.before_remove");
        let report = secrets.migrate_legacy_env(true).unwrap().unwrap();
        assert!(!report.file_removed);
        assert_all(&secrets);
        assert!(env_file(dir.path()).exists());
    }

    #[test]
    fn cofre_indisponivel_responde_com_erro() {
        let store = UnavailableStore {
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
        // `cofre-de-teste` é um arquivo: o cofre de teste não pode criar a pasta.
        std::fs::write(dir.path().join("cofre-de-teste"), b"").unwrap();
        let secrets = Secrets::open(config(dir.path()));
        let error = secrets
            .set(SecretKind::Gemini, &secret(GEMINI))
            .unwrap_err();
        assert_eq!(error.code(), core(CoreErrorCode::Io));
        assert!(!error.to_string().contains(GEMINI));
    }

    #[test]
    fn debug_nao_mostra_valores() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Secrets::open(config(dir.path()));
        secrets.set(SecretKind::Curseforge, &secret(CF)).unwrap();
        secrets.set(SecretKind::Openai, &secret(OPENAI)).unwrap();
        secrets
            .set(SecretKind::Anthropic, &secret(ANTHROPIC))
            .unwrap();
        let text = format!(
            "{secrets:?} {:?} {:?} {:?}",
            secrets.get(SecretKind::Curseforge),
            secrets.get(SecretKind::Openai),
            secrets.get(SecretKind::Anthropic)
        );
        for value in [CF, OPENAI, ANTHROPIC] {
            assert!(!text.contains(value), "{text}");
        }
    }
}
