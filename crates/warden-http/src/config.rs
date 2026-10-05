//! Configuração do cliente: User-Agent, tempos-limite, novas tentativas e política por host
//! (ARCHITECTURE §15 e §17).

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::time::Duration;

/// Contato que vai no User-Agent.
pub const CONTACT_URL: &str = "https://github.com/Kriticales/Warden";

/// User-Agent no formato que o Modrinth pede (`github_username/project_name/versão (contato)`,
/// conferido em <https://docs.modrinth.com/api/> em 04/10/2026; R3 §3.1):
/// `Kriticales/Warden/<versão> (+https://github.com/Kriticales/Warden)`.
#[must_use]
pub fn user_agent(version: &str) -> String {
    format!("Kriticales/Warden/{version} (+{CONTACT_URL})")
}

/// Limite de um servidor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostPolicy {
    /// Requisições por minuto, em média (`None`: sem limite de taxa).
    pub per_minute: Option<NonZeroU32>,
    /// Quantas requisições podem sair de uma vez antes de o limite de taxa espaçar as
    /// seguintes.
    pub burst: NonZeroU32,
    /// Requisições simultâneas no mesmo servidor (`None`: sem limite).
    pub max_concurrent: Option<usize>,
}

impl HostPolicy {
    /// Sem limite de taxa, até `max_concurrent` simultâneas.
    #[must_use]
    pub const fn concurrency(max_concurrent: usize) -> Self {
        Self {
            per_minute: None,
            burst: NonZeroU32::MIN,
            max_concurrent: Some(max_concurrent),
        }
    }

    /// Até `per_minute` requisições por minuto, com rajada de `burst`.
    #[must_use]
    pub const fn rate(per_minute: NonZeroU32, burst: NonZeroU32) -> Self {
        Self {
            per_minute: Some(per_minute),
            burst,
            max_concurrent: None,
        }
    }

    /// Acrescenta o limite de simultâneas.
    #[must_use]
    pub const fn with_max_concurrent(mut self, max_concurrent: usize) -> Self {
        self.max_concurrent = Some(max_concurrent);
        self
    }
}

/// Servidor da API do Modrinth.
pub const MODRINTH_API_HOST: &str = "api.modrinth.com";
/// Servidor da API da CurseForge.
pub const CURSEFORGE_API_HOST: &str = "api.curseforge.com";

/// Configuração do [`crate::HttpClient`].
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// Cabeçalho `User-Agent` de todas as requisições.
    pub user_agent: String,
    /// Tempo para abrir a conexão (10 s).
    pub connect_timeout: Duration,
    /// Tempo total de uma requisição de API, do pedido ao fim do corpo (30 s). Downloads não
    /// têm limite total.
    pub request_timeout: Duration,
    /// Tempo máximo sem receber dados, em qualquer requisição (30 s).
    pub read_timeout: Duration,
    /// Novas tentativas depois da primeira, só em requisições idempotentes (3).
    pub max_retries: u32,
    /// Primeira espera entre tentativas; dobra a cada nova tentativa (500 ms, 1 s, 2 s).
    pub retry_base_delay: Duration,
    /// Maior espera que o cliente aceita fazer sozinho por causa de um 429 ou de um
    /// `Retry-After`; acima disso devolve [`crate::Error::RateLimited`] (2 min).
    pub max_rate_limit_wait: Duration,
    /// Espera usada num 429 sem `Retry-After` nem `X-Ratelimit-Reset` (5 s).
    pub default_rate_limit_wait: Duration,
    /// Maior corpo aceito numa resposta de API (64 MiB).
    pub max_body_bytes: u64,
    /// Redirecionamentos seguidos antes de desistir (10).
    pub max_redirects: u32,
    /// Política de cada servidor, pelo nome (sem porta).
    pub hosts: BTreeMap<String, HostPolicy>,
    /// Política dos demais servidores.
    pub default_host: HostPolicy,
}

impl HttpConfig {
    /// Configuração do app, com o User-Agent da versão dada.
    ///
    /// - Modrinth: até 290 requisições por minuto com rajada de 10 (o servidor aceita 300 por
    ///   minuto por IP; a folga cobre a rajada inicial).
    /// - CurseForge: até 4 simultâneas (ARCHITECTURE §17).
    /// - Demais servidores: até 8 simultâneas.
    #[must_use]
    pub fn for_version(version: &str) -> Self {
        let mut hosts = BTreeMap::new();
        hosts.insert(
            MODRINTH_API_HOST.to_owned(),
            HostPolicy::rate(nonzero(290), nonzero(10)),
        );
        hosts.insert(CURSEFORGE_API_HOST.to_owned(), HostPolicy::concurrency(4));
        Self {
            user_agent: user_agent(version),
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            read_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_base_delay: Duration::from_millis(500),
            max_rate_limit_wait: Duration::from_secs(120),
            default_rate_limit_wait: Duration::from_secs(5),
            max_body_bytes: 64 * 1024 * 1024,
            max_redirects: 10,
            hosts,
            default_host: HostPolicy::concurrency(8),
        }
    }

    /// Troca a política de um servidor.
    #[must_use]
    pub fn with_host(mut self, host: &str, policy: HostPolicy) -> Self {
        self.hosts.insert(host.to_ascii_lowercase(), policy);
        self
    }

    /// Política de um servidor.
    #[must_use]
    pub fn policy_for(&self, host: &str) -> HostPolicy {
        self.hosts
            .get(&host.to_ascii_lowercase())
            .copied()
            .unwrap_or(self.default_host)
    }
}

impl Default for HttpConfig {
    /// A configuração do app com a versão desta crate (a mesma do workspace).
    fn default() -> Self {
        Self::for_version(env!("CARGO_PKG_VERSION"))
    }
}

/// `NonZeroU32` de uma constante; zero vira 1.
#[must_use]
pub fn nonzero(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).unwrap_or(NonZeroU32::MIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_no_formato_do_modrinth() {
        assert_eq!(
            user_agent("1.2.3"),
            "Kriticales/Warden/1.2.3 (+https://github.com/Kriticales/Warden)"
        );
        let config = HttpConfig::default();
        assert!(config.user_agent.starts_with("Kriticales/Warden/0."));
    }

    #[test]
    fn politicas_por_servidor() {
        let config = HttpConfig::default();
        let modrinth = config.policy_for("API.Modrinth.com");
        assert_eq!(modrinth.per_minute, Some(nonzero(290)));
        assert_eq!(modrinth.burst, nonzero(10));
        assert_eq!(
            config.policy_for("api.curseforge.com").max_concurrent,
            Some(4)
        );
        assert_eq!(config.policy_for("cdn.modrinth.com"), config.default_host);
        let config = config.with_host("Exemplo.com", HostPolicy::concurrency(1));
        assert_eq!(config.policy_for("exemplo.com").max_concurrent, Some(1));
        assert_eq!(nonzero(0), NonZeroU32::MIN);
        let policy = HostPolicy::rate(nonzero(60), nonzero(1)).with_max_concurrent(2);
        assert_eq!(policy.max_concurrent, Some(2));
    }

    #[test]
    fn tempos_da_arquitetura() {
        let config = HttpConfig::default();
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.request_timeout, Duration::from_secs(30));
        assert_eq!(config.read_timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
    }
}
