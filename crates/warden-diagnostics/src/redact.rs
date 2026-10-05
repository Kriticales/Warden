//! Redação de dados pessoais (ARCHITECTURE §9.4; SPEC T14, CA-T14-04).
//!
//! Usada antes de qualquer envio à IA, nas palavras de erro mandadas na busca de issues do
//! GitHub e no "Copiar relatório". O texto mostrado no consentimento é exatamente o que
//! [`Redactor::redact`] devolve.
//!
//! | Dado | Substituição |
//! |---|---|
//! | Nome de usuário do sistema em caminhos (`C:\Users\<x>\`, `/home/<x>/`, `/Users/<x>/`, `\\?\`, barras duplas de JSON, `%5C` de URL), a variável `USERNAME`/`USER` em qualquer lugar e o SID do Windows | `<usuário>` |
//! | Nome do jogador (do perfil e do próprio log: `Setting user:`, `--username`, `logged in with entity id`…) | `<jogador>` |
//! | UUID do jogador (informado, `--uuid`, `UUID of player`, e o UUID offline calculado do nome) | `<uuid>` |
//! | Nome do computador (`COMPUTERNAME`/hostname, nomes padrão `DESKTOP-…`/`LAPTOP-…`, servidor de caminho UNC) | `<computador>` |
//! | IPv4/IPv6, exceto loopback (`127.0.0.0/8`, `::1`) e o endereço vazio (`0.0.0.0`, `::`) | `<ip>` |
//! | E-mails | `<email>` |
//! | Padrões de segredo (`$2a$…`, `ghp_…`, `github_pat_…`, `AIza…`, JWT, `x-api-key`, `token=`, `--accessToken`…) e os valores reais das chaves do cofre | `<segredo>` |
//!
//! Como funciona: todas as regras procuram no texto **original** e marcam trechos; trechos que
//! se sobrepõem viram um só (com a substituição mais forte), e o texto de saída é montado uma
//! vez. Nenhuma regra vê a saída de outra, então uma substituição nunca "conserta" um segredo
//! pela metade.
//!
//! Nomes (usuário, jogador, computador) com 4 caracteres ou mais são trocados em **qualquer**
//! lugar, sem diferenciar maiúsculas, mesmo dentro de outra palavra (o spike S-R5-3 achou o
//! nome do usuário do Windows dentro de `StatusConsoleListener`): o texto fica um pouco feio, mas nada
//! vaza. Nomes de 1 a 3 caracteres só são trocados como palavra inteira, senão o texto viraria
//! sopa de marcadores. Cada nome é procurado também em NFD, como o Java no console do Windows
//! o escreve em CP850 e CP1252, como UTF-8 lido como CP1252 (`JosÃ©`) e com `?` no lugar dos
//! acentos.
//!
//! Gancho 1.1 (ADR-0039): [`RedactionProfile::with_rule`] aceita regras extras (expressões ou
//! textos fixos), usadas pela redação de logs de jogadores (nome da pasta da instância etc.).

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::LazyLock;

use md5::{Digest, Md5};
use regex::Regex;
use secrecy::{ExposeSecret, SecretString};
use unicode_normalization::UnicodeNormalization;

use crate::error::{DiagnosticsError, Result};
use crate::postcrash::text_fixed_regex as fixed_regex;

/// O que um trecho trocado era.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Placeholder {
    /// Nome de usuário do sistema.
    User,
    /// Nome do jogador.
    Player,
    /// UUID do jogador.
    Uuid,
    /// Nome do computador.
    Computer,
    /// Endereço IP.
    Ip,
    /// E-mail.
    Email,
    /// Chave, token ou senha.
    Secret,
    /// Marcador de uma regra extra (gancho 1.1), como `<instância>`.
    Custom(String),
}

impl Placeholder {
    /// O texto que entra no lugar do dado.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::User => "<usuário>",
            Self::Player => "<jogador>",
            Self::Uuid => "<uuid>",
            Self::Computer => "<computador>",
            Self::Ip => "<ip>",
            Self::Email => "<email>",
            Self::Secret => "<segredo>",
            Self::Custom(text) => text,
        }
    }

    /// Força na junção de trechos sobrepostos: o mais forte dá o nome ao trecho todo.
    fn strength(&self) -> u8 {
        match self {
            Self::Secret => 7,
            Self::Email => 6,
            Self::User => 5,
            Self::Player => 4,
            Self::Uuid => 3,
            Self::Computer => 2,
            Self::Ip => 1,
            Self::Custom(_) => 0,
        }
    }
}

/// Regra extra de redação (gancho 1.1, ADR-0039).
#[derive(Clone)]
pub struct ExtraRule {
    kind: ExtraKind,
    placeholder: Placeholder,
}

#[derive(Clone)]
enum ExtraKind {
    Pattern(Regex),
    Literal(String),
}

impl fmt::Debug for ExtraRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match &self.kind {
            ExtraKind::Pattern(_) => "expressão",
            ExtraKind::Literal(_) => "texto fixo",
        };
        f.debug_struct("ExtraRule")
            .field("kind", &kind)
            .field("placeholder", &self.placeholder)
            .finish()
    }
}

impl ExtraRule {
    /// Regra por expressão regular (crate `regex`); o trecho inteiro casado é trocado.
    pub fn pattern(expression: &str, placeholder: Placeholder) -> Result<Self> {
        let regex =
            Regex::new(expression).map_err(|error| DiagnosticsError::InvalidRedactionRule {
                pattern: expression.chars().take(200).collect(),
                reason: error.to_string(),
            })?;
        if regex.is_match("") {
            return Err(DiagnosticsError::InvalidRedactionRule {
                pattern: expression.chars().take(200).collect(),
                reason: "a expressão casa com texto vazio".to_owned(),
            });
        }
        Ok(Self {
            kind: ExtraKind::Pattern(regex),
            placeholder,
        })
    }

    /// Regra por texto fixo, tratado como um nome (sem diferenciar maiúsculas, com as mesmas
    /// variantes de acento e o mesmo limite de 4 caracteres).
    #[must_use]
    pub fn literal(text: &str, placeholder: Placeholder) -> Self {
        Self {
            kind: ExtraKind::Literal(text.to_owned()),
            placeholder,
        }
    }
}

/// O que a redação precisa saber além do próprio texto.
#[derive(Clone, Default)]
pub struct RedactionProfile {
    usernames: Vec<String>,
    players: Vec<String>,
    player_uuids: Vec<String>,
    computers: Vec<String>,
    secrets: Vec<SecretString>,
    rules: Vec<ExtraRule>,
}

impl fmt::Debug for RedactionProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Nunca mostra os valores: o perfil inteiro é dado pessoal.
        f.debug_struct("RedactionProfile")
            .field("usernames", &self.usernames.len())
            .field("players", &self.players.len())
            .field("player_uuids", &self.player_uuids.len())
            .field("computers", &self.computers.len())
            .field("secrets", &self.secrets.len())
            .field("rules", &self.rules.len())
            .finish()
    }
}

impl RedactionProfile {
    /// Perfil vazio: só as regras que dependem do texto.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Perfil com os dados do ambiente: `USERNAME`, `USER`, `LOGNAME`, a última pasta de
    /// `USERPROFILE`/`HOME`, `COMPUTERNAME`, `HOSTNAME`, `USERDOMAIN` e, no Linux,
    /// `/etc/hostname`.
    #[must_use]
    pub fn from_environment() -> Self {
        let profile = Self::from_env_vars(|name| std::env::var(name).ok());
        #[cfg(unix)]
        let profile = match std::fs::read_to_string("/etc/hostname") {
            Ok(hostname) => profile.with_computer_name(hostname.trim()),
            Err(_) => profile,
        };
        profile
    }

    /// [`Self::from_environment`] com a leitura das variáveis injetada (testes).
    #[must_use]
    pub fn from_env_vars(get: impl Fn(&str) -> Option<String>) -> Self {
        let mut profile = Self::new();
        for name in ["USERNAME", "USER", "LOGNAME"] {
            if let Some(value) = get(name) {
                profile = profile.with_username(&value);
            }
        }
        for name in ["USERPROFILE", "HOME"] {
            if let Some(value) = get(name)
                && let Some(last) = value
                    .trim_end_matches(['/', '\\'])
                    .rsplit(['/', '\\'])
                    .next()
            {
                profile = profile.with_username(last);
            }
        }
        for name in ["COMPUTERNAME", "HOSTNAME", "USERDOMAIN"] {
            if let Some(value) = get(name) {
                profile = profile.with_computer_name(&value);
            }
        }
        profile
    }

    /// Acrescenta um nome de usuário do sistema.
    #[must_use]
    pub fn with_username(mut self, name: &str) -> Self {
        push_name(&mut self.usernames, name);
        self
    }

    /// Acrescenta o nome de um jogador (o UUID offline dele também é redigido).
    #[must_use]
    pub fn with_player(mut self, name: &str) -> Self {
        push_name(&mut self.players, name);
        self
    }

    /// Acrescenta o UUID de um jogador (com ou sem hífens).
    #[must_use]
    pub fn with_player_uuid(mut self, uuid: &str) -> Self {
        if let Some(compact) = compact_uuid(uuid)
            && !self.player_uuids.contains(&compact)
        {
            self.player_uuids.push(compact);
        }
        self
    }

    /// Acrescenta um nome de computador.
    #[must_use]
    pub fn with_computer_name(mut self, name: &str) -> Self {
        push_name(&mut self.computers, name);
        self
    }

    /// Acrescenta o valor real de uma chave do cofre. Valores com menos de 4 caracteres são
    /// ignorados (casariam com texto comum).
    #[must_use]
    pub fn with_secret(mut self, secret: &SecretString) -> Self {
        if secret.expose_secret().trim().chars().count() >= 4 {
            self.secrets.push(secret.clone());
        }
        self
    }

    /// Acrescenta uma regra extra (gancho 1.1).
    #[must_use]
    pub fn with_rule(mut self, rule: ExtraRule) -> Self {
        self.rules.push(rule);
        self
    }
}

fn push_name(list: &mut Vec<String>, name: &str) {
    let name = name.trim();
    if !name.is_empty() && !list.iter().any(|known| known.eq_ignore_ascii_case(name)) {
        list.push(name.to_owned());
    }
}

/// Texto redigido e quantos trechos de cada tipo foram trocados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redacted {
    /// O texto que pode sair do computador.
    pub text: String,
    /// Trechos trocados por marcador (`<usuário>` → 3).
    pub counts: BTreeMap<String, u32>,
}

/// Redator pronto para usar, montado a partir de um [`RedactionProfile`].
#[derive(Clone)]
pub struct Redactor {
    profile: RedactionProfile,
}

impl fmt::Debug for Redactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Redactor")
            .field("profile", &self.profile)
            .finish()
    }
}

/// Atalho: redige com um perfil.
#[must_use]
pub fn redact(text: &str, profile: &RedactionProfile) -> Redacted {
    Redactor::new(profile.clone()).redact(text)
}

// ---------------------------------------------------------------------------------------------
// Regras fixas
// ---------------------------------------------------------------------------------------------

/// Pasta de usuário em caminhos Windows, Unix e macOS, com barras simples, duplas (JSON),
/// `\\?\` e `%5C`/`%2F` de URL. O grupo `name` é o nome.
static USER_PATH: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(concat!(
        r#"(?i)(?:[\\/]|%5c|%2f)(?:users|home|documents and settings)(?:[\\/]|%5c|%2f)+"#,
        r#"(?P<name>[^\\/%\r\n\t"<>|:*?,;\[\]=+]+)"#,
    ))
});

/// Nomes de pasta de usuário que não identificam ninguém.
const SHARED_PROFILES: [&str; 6] = [
    "public",
    "default",
    "default user",
    "all users",
    "shared",
    "guest",
];

static USER_PROPERTY: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r#"(?i)\buser\.name\s*[=:]\s*"?(?P<name>[^\s",;\]]+)"#));

static WINDOWS_SID: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"\bS-1-5-21-\d{5,}-\d{5,}-\d{5,}(?:-\d+)?\b"));

static PLAYER_SIGNALS: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(concat!(
        r"(?:Setting user: (?P<a>[^\s,\]]+)",
        r"|--username[,=\s]+(?P<b>[^\s,\]=]+)",
        r"|UUID of player (?P<c>\S+) is ",
        r"|(?:^|[\s:\]>])(?P<d>[A-Za-z0-9_]{2,16})\[(?:local:E:[0-9a-fA-F]+|/[^\]]+)\] logged in with entity id",
        r"|(?:^|[\s:\]>])(?P<e>[A-Za-z0-9_]{2,16}) joined the game",
        r")",
    ))
});

static UUID_SIGNALS: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(concat!(
        r"(?i)(?:--uuid[,=\s]+|UUID of player \S+ is |player[_ ]?uuid[=:\s]+)",
        r"(?P<uuid>[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12})",
    ))
});

static COMPUTER_SIGNALS: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(
        r#"(?i)\b(?:computername|hostname)\s*[=:]\s*"?(?P<name>[A-Za-z0-9][A-Za-z0-9._-]{0,62})"#,
    )
});

static DEFAULT_COMPUTER: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"(?i)\b(?:DESKTOP|LAPTOP)-[A-Z0-9]{5,15}\b"));

static UNC_SERVER: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"\\\\(?P<name>[A-Za-z0-9][A-Za-z0-9-]{0,14})\\"));

static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(
        r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9](?:[A-Za-z0-9\-]*[A-Za-z0-9])?(?:\.[A-Za-z0-9](?:[A-Za-z0-9\-]*[A-Za-z0-9])?)*\.[A-Za-z]{2,}",
    )
});

/// Segredos reconhecíveis pela forma.
static SECRET_SHAPES: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(concat!(
        r"(?:\$2[abxy]?\$\d{2}\$[./A-Za-z0-9]{20,}",
        r"|\bgh[pousr]_[A-Za-z0-9]{20,}",
        r"|\bgithub_pat_[A-Za-z0-9_]{20,}",
        r"|\bAIza[0-9A-Za-z_\-]{20,}",
        r"|\beyJ[A-Za-z0-9_\-]{8,}\.eyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}",
        r")",
    ))
});

/// Valores depois de nomes de chave (`token=…`, `"accessToken": "…"`, `x-api-key: …`).
static SECRET_VALUES: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(concat!(
        r#"(?i)(?:x-goog-api-key|x-api-key|api[_-]?key|access[_-]?token|auth[_-]?token|refresh[_-]?token"#,
        r#"|client[_-]?secret|secret|password|passwd|senha|token|authorization|session[_-]?id)"#,
        r#"["']?\s*[:=]\s*["']?(?:bearer\s+|token:)?(?P<value>[^\s"',;&\]\[{}<>]+)"#,
    ))
});

/// Argumentos de lançamento com dado de conta.
static LAUNCH_SECRETS: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(r"--(?:accessToken|session|xuid|clientId)[,=\s]+(?P<value>[^\s,\]]+)")
});

static IPV4: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b"));

static IPV6: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(r"(?i)[0-9a-f]{0,4}(?::[0-9a-f]{0,4}){2,7}(?:\.\d{1,3}){0,3}(?:%[0-9a-z]+)?")
});

// ---------------------------------------------------------------------------------------------
// Variantes de nomes
// ---------------------------------------------------------------------------------------------

/// CP850 (console do Windows em pt-BR) das letras acentuadas mais comuns.
const CP850: [(char, u8); 48] = [
    ('á', 0xA0),
    ('à', 0x85),
    ('â', 0x83),
    ('ã', 0xC6),
    ('ä', 0x84),
    ('é', 0x82),
    ('è', 0x8A),
    ('ê', 0x88),
    ('ë', 0x89),
    ('í', 0xA1),
    ('ì', 0x8D),
    ('î', 0x8C),
    ('ï', 0x8B),
    ('ó', 0xA2),
    ('ò', 0x95),
    ('ô', 0x93),
    ('õ', 0xE4),
    ('ö', 0x94),
    ('ú', 0xA3),
    ('ù', 0x97),
    ('û', 0x96),
    ('ü', 0x81),
    ('ç', 0x87),
    ('ñ', 0xA4),
    ('Á', 0xB5),
    ('À', 0xB7),
    ('Â', 0xB6),
    ('Ã', 0xC7),
    ('Ä', 0x8E),
    ('É', 0x90),
    ('È', 0xD4),
    ('Ê', 0xD2),
    ('Ë', 0xD3),
    ('Í', 0xD6),
    ('Ì', 0xDE),
    ('Î', 0xD7),
    ('Ï', 0xD8),
    ('Ó', 0xE0),
    ('Ò', 0xE3),
    ('Ô', 0xE2),
    ('Õ', 0xE5),
    ('Ö', 0x99),
    ('Ú', 0xE9),
    ('Ù', 0xEB),
    ('Û', 0xEA),
    ('Ü', 0x9A),
    ('Ç', 0x80),
    ('Ñ', 0xA5),
];

/// Como um texto aparece no log quando o Java o escreve numa página de código e o Warden lê
/// a linha como Windows-1252 (o recuo de `text::decode_line`).
fn cp850_as_cp1252(name: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(name.len());
    for character in name.chars() {
        if character.is_ascii() {
            bytes.push(u8::try_from(u32::from(character)).ok()?);
        } else {
            bytes.push(CP850.iter().find(|(c, _)| *c == character)?.1);
        }
    }
    Some(
        encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(&bytes)
            .0
            .into_owned(),
    )
}

/// As formas em que um nome pode aparecer no texto.
fn name_variants(name: &str) -> Vec<String> {
    let nfc: String = name.nfc().collect();
    let mut variants = vec![nfc.clone(), name.nfd().collect(), name.to_owned()];
    if !nfc.is_ascii() {
        // UTF-8 lido como Windows-1252 ("JosÃ©").
        variants.push(
            encoding_rs::WINDOWS_1252
                .decode_without_bom_handling(nfc.as_bytes())
                .0
                .into_owned(),
        );
        if let Some(cp850) = cp850_as_cp1252(&nfc) {
            variants.push(cp850);
        }
        // Acentos trocados por '?' (página de código sem a letra).
        let question: String = nfc
            .chars()
            .map(|character| if character.is_ascii() { character } else { '?' })
            .collect();
        if question.chars().filter(char::is_ascii_alphanumeric).count() >= 3 {
            variants.push(question);
        }
        // Sem acento ("Jose" para "José"), comum em nomes de pasta.
        let plain: String = name.nfd().filter(char::is_ascii).collect();
        if plain.chars().count() >= 3 {
            variants.push(plain);
        }
    }
    variants.retain(|variant| !variant.trim().is_empty());
    variants.sort();
    variants.dedup();
    variants
}

fn compact_uuid(uuid: &str) -> Option<String> {
    let compact: String = uuid
        .chars()
        .filter(|c| *c != '-')
        .collect::<String>()
        .to_lowercase();
    (compact.len() == 32 && compact.chars().all(|c| c.is_ascii_hexdigit())).then_some(compact)
}

fn dashed_uuid(compact: &str) -> String {
    format!(
        "{}-{}-{}-{}-{}",
        &compact[0..8],
        &compact[8..12],
        &compact[12..16],
        &compact[16..20],
        &compact[20..32]
    )
}

/// UUID offline do Minecraft: MD5 de `OfflinePlayer:<nome>` com versão 3 e variante IETF
/// (`UUID.nameUUIDFromBytes`).
#[must_use]
pub fn offline_uuid(player: &str) -> String {
    let mut digest: [u8; 16] = Md5::digest(format!("OfflinePlayer:{player}").as_bytes()).into();
    digest[6] = (digest[6] & 0x0f) | 0x30;
    digest[8] = (digest[8] & 0x3f) | 0x80;
    let compact = digest
        .iter()
        .fold(String::with_capacity(32), |mut text, byte| {
            // Escrever numa `String` não falha.
            let _ = write!(text, "{byte:02x}");
            text
        });
    dashed_uuid(&compact)
}

// ---------------------------------------------------------------------------------------------
// Trechos
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct Span {
    start: usize,
    end: usize,
    placeholder: Placeholder,
}

fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// Procura um conjunto de textos (sem diferenciar maiúsculas). Textos curtos (menos de 4
/// caracteres) só casam como palavra inteira.
fn literal_spans(text: &str, literals: &[String], placeholder: &Placeholder, out: &mut Vec<Span>) {
    let mut long: Vec<&str> = Vec::new();
    let mut short: Vec<&str> = Vec::new();
    for literal in literals {
        let literal = literal.trim();
        if literal.is_empty() {
            continue;
        }
        if literal.chars().count() >= 4 {
            long.push(literal);
        } else {
            short.push(literal);
        }
    }
    for (group, bounded) in [(long, false), (short, true)] {
        if group.is_empty() {
            continue;
        }
        let mut sorted = group;
        sorted.sort_by_key(|literal| std::cmp::Reverse(literal.len()));
        let alternation = sorted
            .iter()
            .map(|literal| regex::escape(literal))
            .collect::<Vec<_>>()
            .join("|");
        let Ok(regex) = Regex::new(&format!("(?i)(?:{alternation})")) else {
            continue;
        };
        let mut position = 0;
        while let Some(found) = regex.find_at(text, position) {
            let before = text[..found.start()].chars().next_back();
            let after = text[found.end()..].chars().next();
            let ok =
                !bounded || (!before.is_some_and(is_word_char) && !after.is_some_and(is_word_char));
            if ok {
                out.push(Span {
                    start: found.start(),
                    end: found.end(),
                    placeholder: placeholder.clone(),
                });
                position = found.end();
            } else {
                position = found.start()
                    + text[found.start()..]
                        .chars()
                        .next()
                        .map_or(1, char::len_utf8);
            }
            if position >= text.len() {
                break;
            }
        }
    }
}

fn group_spans(
    regex: &Regex,
    group: &str,
    text: &str,
    placeholder: &Placeholder,
    out: &mut Vec<Span>,
) {
    for captures in regex.captures_iter(text) {
        if let Some(found) = captures.name(group) {
            let value = found.as_str();
            let trimmed = value.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            out.push(Span {
                start: found.start(),
                end: found.start() + trimmed.len(),
                placeholder: placeholder.clone(),
            });
        }
    }
}

fn match_spans(regex: &Regex, text: &str, placeholder: &Placeholder, out: &mut Vec<Span>) {
    for found in regex.find_iter(text) {
        if !found.as_str().is_empty() {
            out.push(Span {
                start: found.start(),
                end: found.end(),
                placeholder: placeholder.clone(),
            });
        }
    }
}

/// Caractere antes e depois de um trecho.
fn neighbors(text: &str, start: usize, end: usize) -> (Option<char>, Option<char>, Option<char>) {
    let before = text[..start].chars().next_back();
    let mut rest = text[end..].chars();
    (before, rest.next(), rest.next())
}

fn ip_spans(text: &str, out: &mut Vec<Span>) {
    for found in IPV4.find_iter(text) {
        let (before, after, after2) = neighbors(text, found.start(), found.end());
        // "1.2.3.4.5" e "v1.2.3.4" são versões, não endereços.
        if before.is_some_and(|c| c == '.' || c.is_alphanumeric())
            || (after == Some('.') && after2.is_some_and(|c| c.is_ascii_digit()))
        {
            continue;
        }
        if let Ok(address) = found.as_str().parse::<Ipv4Addr>()
            && !address.is_loopback()
            && !address.is_unspecified()
        {
            out.push(Span {
                start: found.start(),
                end: found.end(),
                placeholder: Placeholder::Ip,
            });
        }
    }
    for found in IPV6.find_iter(text) {
        let candidate = found.as_str();
        if candidate.matches(':').count() < 2 {
            continue;
        }
        let (before, after, _) = neighbors(text, found.start(), found.end());
        // `Classe::método`, `JavaThread::run()`: letras coladas não são endereço.
        if before.is_some_and(|c| c.is_alphanumeric() || c == ':' || c == '_' || c == '.')
            || after.is_some_and(|c| c.is_alphanumeric() || c == ':' || c == '_')
        {
            continue;
        }
        let address = candidate.split('%').next().unwrap_or(candidate);
        if let Ok(parsed) = address.parse::<Ipv6Addr>()
            && !parsed.is_loopback()
            && !parsed.is_unspecified()
        {
            out.push(Span {
                start: found.start(),
                end: found.end(),
                placeholder: Placeholder::Ip,
            });
        }
    }
}

/// Junta trechos sobrepostos e monta o texto.
fn apply(text: &str, mut spans: Vec<Span>) -> Redacted {
    spans.sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
    // Cada trecho juntado guarda o tamanho do maior trecho original: ele dá o nome ao todo
    // ("mariazinha" é o jogador, mesmo contendo o usuário "maria"); no empate, o mais forte.
    let mut merged: Vec<(Span, usize)> = Vec::new();
    for span in spans {
        if span.start >= span.end {
            continue;
        }
        let length = span.end - span.start;
        match merged.last_mut() {
            Some((last, best)) if span.start < last.end => {
                last.end = last.end.max(span.end);
                if length > *best
                    || (length == *best
                        && span.placeholder.strength() > last.placeholder.strength())
                {
                    last.placeholder = span.placeholder;
                    *best = length;
                }
            }
            _ => merged.push((span, length)),
        }
    }
    let mut output = String::with_capacity(text.len());
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    let mut position = 0;
    for (span, _) in merged {
        output.push_str(&text[position..span.start]);
        output.push_str(span.placeholder.text());
        *counts
            .entry(span.placeholder.text().to_owned())
            .or_default() += 1;
        position = span.end;
    }
    output.push_str(&text[position..]);
    Redacted {
        text: output,
        counts,
    }
}

impl Redactor {
    /// Monta o redator.
    #[must_use]
    pub fn new(profile: RedactionProfile) -> Self {
        Self { profile }
    }

    /// Nomes e UUIDs que o próprio texto revela (caminhos, `Setting user:`, `--uuid`…).
    fn discover(&self, text: &str) -> RedactionProfile {
        let mut found = self.profile.clone();
        for captures in USER_PATH.captures_iter(text) {
            if let Some(name) = captures.name("name") {
                let name = name.as_str().trim();
                if !SHARED_PROFILES.contains(&name.to_lowercase().as_str()) {
                    found = found.with_username(name);
                }
            }
        }
        for captures in USER_PROPERTY.captures_iter(text) {
            if let Some(name) = captures.name("name") {
                found = found.with_username(name.as_str());
            }
        }
        for captures in PLAYER_SIGNALS.captures_iter(text) {
            if let Some(name) = ["a", "b", "c", "d", "e"]
                .iter()
                .find_map(|group| captures.name(group))
            {
                let name = name.as_str();
                if !name.starts_with('<') && !name.starts_with("${") {
                    found = found.with_player(name);
                }
            }
        }
        for captures in UUID_SIGNALS.captures_iter(text) {
            if let Some(uuid) = captures.name("uuid") {
                found = found.with_player_uuid(uuid.as_str());
            }
        }
        for captures in COMPUTER_SIGNALS.captures_iter(text) {
            if let Some(name) = captures.name("name") {
                found = found.with_computer_name(name.as_str());
            }
        }
        found
    }

    /// Redige o texto. Nunca falha: entrada qualquer, saída sem os dados da tabela.
    #[must_use]
    pub fn redact(&self, text: &str) -> Redacted {
        let profile = self.discover(text);
        let mut spans = Vec::new();

        // Segredos.
        let secrets: Vec<String> = profile
            .secrets
            .iter()
            .map(|secret| secret.expose_secret().trim().to_owned())
            .collect();
        exact_spans(text, &secrets, &mut spans);
        match_spans(&SECRET_SHAPES, text, &Placeholder::Secret, &mut spans);
        group_spans(
            &SECRET_VALUES,
            "value",
            text,
            &Placeholder::Secret,
            &mut spans,
        );
        group_spans(
            &LAUNCH_SECRETS,
            "value",
            text,
            &Placeholder::Secret,
            &mut spans,
        );

        // E-mails e endereços.
        match_spans(&EMAIL, text, &Placeholder::Email, &mut spans);
        ip_spans(text, &mut spans);

        // Usuário do sistema.
        for captures in USER_PATH.captures_iter(text) {
            if let Some(name) = captures.name("name") {
                let trimmed = name.as_str().trim_end();
                if !trimmed.is_empty()
                    && !SHARED_PROFILES.contains(&trimmed.to_lowercase().as_str())
                {
                    spans.push(Span {
                        start: name.start(),
                        end: name.start() + trimmed.len(),
                        placeholder: Placeholder::User,
                    });
                }
            }
        }
        match_spans(&WINDOWS_SID, text, &Placeholder::User, &mut spans);
        let usernames: Vec<String> = profile
            .usernames
            .iter()
            .filter(|name| !SHARED_PROFILES.contains(&name.to_lowercase().as_str()))
            .flat_map(|name| name_variants(name))
            .collect();
        literal_spans(text, &usernames, &Placeholder::User, &mut spans);

        // Jogador e UUID (o UUID offline sai do nome).
        let players: Vec<String> = profile
            .players
            .iter()
            .flat_map(|name| name_variants(name))
            .collect();
        literal_spans(text, &players, &Placeholder::Player, &mut spans);
        let mut uuids: Vec<String> = profile.player_uuids.clone();
        uuids.extend(
            profile
                .players
                .iter()
                .filter_map(|player| compact_uuid(&offline_uuid(player))),
        );
        let uuid_forms: Vec<String> = uuids
            .iter()
            .flat_map(|compact| [compact.clone(), dashed_uuid(compact)])
            .collect();
        literal_spans(text, &uuid_forms, &Placeholder::Uuid, &mut spans);

        // Computador.
        let computers: Vec<String> = profile
            .computers
            .iter()
            .flat_map(|name| name_variants(name))
            .collect();
        literal_spans(text, &computers, &Placeholder::Computer, &mut spans);
        match_spans(&DEFAULT_COMPUTER, text, &Placeholder::Computer, &mut spans);
        group_spans(
            &UNC_SERVER,
            "name",
            text,
            &Placeholder::Computer,
            &mut spans,
        );

        // Regras extras.
        for rule in &profile.rules {
            match &rule.kind {
                ExtraKind::Pattern(regex) => {
                    match_spans(regex, text, &rule.placeholder, &mut spans);
                }
                ExtraKind::Literal(literal) => {
                    literal_spans(text, &name_variants(literal), &rule.placeholder, &mut spans);
                }
            }
        }

        apply(text, spans)
    }
}

/// Valores exatos (chaves do cofre): diferenciam maiúsculas e casam em qualquer lugar.
fn exact_spans(text: &str, values: &[String], out: &mut Vec<Span>) {
    for value in values {
        if value.is_empty() {
            continue;
        }
        let mut position = 0;
        while let Some(offset) = text[position..].find(value.as_str()) {
            let start = position + offset;
            out.push(Span {
                start,
                end: start + value.len(),
                placeholder: Placeholder::Secret,
            });
            position = start + text[start..].chars().next().map_or(1, char::len_utf8);
            if position >= text.len() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> RedactionProfile {
        RedactionProfile::new()
    }

    /// Segredos inventados, lidos de `tests/fixtures/redacao/segredos.toml` em tempo de
    /// execução: embutidos no executável de testes, disparavam a heurística do antivírus.
    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("redacao")
            .join("segredos.toml");
        let text = std::fs::read_to_string(&path).expect("fixture de redação");
        let table: toml::Table = toml::from_str(&text).expect("fixture de redação válida");
        table[name].as_str().expect("texto na fixture").to_owned()
    }

    fn clean(text: &str, profile: &RedactionProfile) -> String {
        redact(text, profile).text
    }

    #[test]
    fn ca_t14_04_caminho_jogador_ip_e_chave_somem() {
        let profile = profile()
            .with_username("Maria")
            .with_player("Steve_BR")
            .with_secret(&SecretString::from(fixture("chave_curseforge")));
        let key = fixture("chave_curseforge");
        let log = fixture("log_ca_t14_04").replace("{chave}", &key);
        let redacted = redact(&log, &profile);
        for secret in [
            "Maria",
            "Steve_BR",
            "192.168.0.15",
            &key[..7],
            "abcdefghijklmnopqrstuvwxyz",
        ] {
            assert!(
                !redacted.text.contains(secret),
                "{secret} sobrou: {}",
                redacted.text
            );
        }
        assert!(redacted.text.contains("C:\\Users\\<usuário>\\AppData"));
        assert!(redacted.text.contains("Setting user: <jogador>"));
        assert!(redacted.text.contains("<jogador>[/<ip>:51234]"));
        assert_eq!(redacted.counts["<usuário>"], 1);
        assert_eq!(redacted.counts["<jogador>"], 2);
        assert_eq!(redacted.counts["<ip>"], 1);
        assert_eq!(redacted.counts["<segredo>"], 1);
    }

    #[test]
    fn caminhos_dificeis() {
        let cases = [
            ("C:\\Users\\Maria José\\AppData", "Maria José"),
            ("C:/Users/Ana/Documents/x", "Ana"),
            ("\\\\?\\C:\\Users\\joão.silva\\x", "joão.silva"),
            ("C:\\\\Users\\\\pedro\\\\.minecraft", "pedro"),
            ("file:///C:/Users/Lu%C3%ADs/x", "Lu%C3%ADs"),
            ("C:%5CUsers%5Cbeto%5Cx", "beto"),
            ("/home/carla/.local/share/x", "carla"),
            ("/Users/dani/Library/x", "dani"),
            ("D:\\Users\\MARIAJ~1\\x", "MARIAJ~1"),
            ("C:\\Documents and Settings\\velho\\x", "velho"),
            ("C:\\Users\\Ana\\OneDrive - Contoso\\Documentos\\x", "Ana"),
            ("\\\\wsl$\\Ubuntu\\home\\linuxuser\\x", "linuxuser"),
            ("-Duser.home=C:\\Users\\zeca", "zeca"),
        ];
        for (text, secret) in cases {
            let output = clean(text, &profile());
            let visible = secret.split('%').next().unwrap();
            assert!(!output.contains(visible), "{text} → {output}");
            assert!(output.contains("<usuário>"), "{text} → {output}");
        }
        // Pastas compartilhadas não são de ninguém.
        assert_eq!(
            clean("C:\\Users\\Public\\Desktop", &profile()),
            "C:\\Users\\Public\\Desktop"
        );
    }

    #[test]
    fn nome_descoberto_no_caminho_some_do_resto_do_texto() {
        let output = clean(
            "Loading C:\\Users\\Fernanda\\x\nBem-vinda, fernanda! FERNANDA e FernandaGamer",
            &profile(),
        );
        assert!(!output.to_lowercase().contains("fernanda"), "{output}");
    }

    #[test]
    fn variantes_de_acento_e_codificacao() {
        let profile = profile().with_username("José");
        for form in [
            "José",
            "JOSÉ",
            "Jose\u{301}",
            "JosÃ©",
            "Jos‚",
            "Jos?",
            "Jose",
        ] {
            let output = clean(&format!("usuário {form} entrou"), &profile);
            assert!(output.contains("<usuário>"), "{form} → {output}");
        }
    }

    #[test]
    fn nome_curto_so_como_palavra() {
        let profile = profile().with_username("Bo").with_player("Lu");
        assert_eq!(
            clean("Bo entrou; Bob e lua ficam; lu.", &profile),
            "<usuário> entrou; Bob e lua ficam; <jogador>."
        );
        // Nome longo some até dentro de outra palavra (achado do spike S-R5-3).
        let output = clean(
            "WARN StatusConsoleListener",
            &RedactionProfile::new().with_username("nsole"),
        );
        assert_eq!(output, "WARN StatusCo<usuário>Listener");
    }

    #[test]
    fn jogador_e_uuid_do_proprio_log() {
        let text = fixture("log_jogador");
        let output = clean(&text, &profile());
        assert!(!output.to_lowercase().contains("kiko123"), "{output}");
        assert!(!output.contains("0f1e2d3c"), "{output}");
        assert!(!output.contains("abc.def"), "{output}");
    }

    #[test]
    fn uuid_offline_calculado() {
        // UUID offline real de "WardenTest" (UUID.nameUUIDFromBytes do Java).
        let uuid = offline_uuid("WardenTest");
        assert_eq!(uuid.len(), 36);
        assert_eq!(&uuid[14..15], "3");
        let output = clean(
            &format!("uuid {uuid} e {}", uuid.replace('-', "").to_uppercase()),
            &profile().with_player("WardenTest"),
        );
        assert_eq!(output, "uuid <uuid> e <uuid>");
        // Conferido com o Java: UUID.nameUUIDFromBytes("OfflinePlayer:Notch".getBytes(UTF_8)).
        assert_eq!(
            offline_uuid("Notch"),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
    }

    #[test]
    fn computador() {
        let profile = RedactionProfile::from_env_vars(|name| match name {
            "COMPUTERNAME" => Some("PC-DA-SALA".to_owned()),
            "USERNAME" => Some("rafa".to_owned()),
            "USERPROFILE" => Some("C:\\Users\\rafael.l\\".to_owned()),
            _ => None,
        });
        let output = clean(
            "Host PC-DA-SALA, pc-da-sala; DESKTOP-AB12CD3; \\\\SERVIDOR1\\share; COMPUTERNAME=OUTRO-PC; rafa e rafael.l",
            &profile,
        );
        for secret in [
            "PC-DA-SALA",
            "pc-da-sala",
            "DESKTOP-AB12CD3",
            "SERVIDOR1",
            "OUTRO-PC",
            "rafa",
            "rafael.l",
        ] {
            assert!(!output.contains(secret), "{secret} → {output}");
        }
        assert!(format!("{profile:?}").contains("usernames: 2"));
    }

    #[test]
    fn enderecos_ip() {
        let output = clean(
            "a 10.0.0.5 b 127.0.0.1 c 0.0.0.0 d [2001:db8::1]:25565 e ::1 f fe80::1%eth0 g 1.20.1.2.3 h v1.2.3.4 i 300.1.1.1",
            &profile(),
        );
        assert_eq!(
            output,
            "a <ip> b 127.0.0.1 c 0.0.0.0 d [<ip>]:25565 e ::1 f <ip> g 1.20.1.2.3 h v1.2.3.4 i 300.1.1.1"
        );
        // Não confunde com código.
        for code in [
            "JavaThread::run()",
            "Foo::bar",
            "12:34:56.789",
            "00:1A:2B:3C:4D:5E",
        ] {
            assert_eq!(clean(code, &profile()), code);
        }
    }

    #[test]
    fn emails_e_segredos() {
        let github = fixture("github");
        let github_fine = fixture("github_fino");
        let google = fixture("google");
        let jwt = fixture("jwt");
        let bearer = fixture("bearer");
        let text = fixture("log_segredos")
            .replace("{bearer}", &bearer)
            .replace("{github_fino}", &github_fine)
            .replace("{github}", &github)
            .replace("{google}", &google)
            .replace("{jwt}", &jwt)
            .replace("{sessao}", &fixture("sessao"));
        let output = clean(&text, &profile());
        for secret in [
            "maria.silva",
            "abc123XYZ",
            &bearer[..10],
            &github[..8],
            &github_fine[..13],
            &google[..8],
            &fixture("sessao"),
            &jwt[jwt.len() - 10..],
            "1234567890abcdef",
        ] {
            assert!(!output.contains(secret), "{secret} → {output}");
        }
        assert!(output.contains("<email>"));
        assert!(output.contains("token=<segredo>&x=1"));
    }

    #[test]
    fn chave_do_cofre_curta_e_ignorada_e_longa_some_em_qualquer_lugar() {
        let profile = profile()
            .with_secret(&SecretString::from("abc"))
            .with_secret(&SecretString::from("Xy9-chave-real"));
        assert_eq!(
            clean("abc prefixoXy9-chave-realsufixo", &profile),
            "abc prefixo<segredo>sufixo"
        );
        assert!(format!("{profile:?}").contains("secrets: 1"));
        assert!(!format!("{:?}", Redactor::new(profile)).contains("chave"));
    }

    #[test]
    fn regras_extras() {
        let rule =
            ExtraRule::pattern(r"inst-\d+", Placeholder::Custom("<instância>".into())).unwrap();
        let profile = profile().with_rule(rule).with_rule(ExtraRule::literal(
            "Meu Pack Secreto",
            Placeholder::Custom("<pack>".into()),
        ));
        let output = clean("pasta inst-42 do meu pack secreto", &profile);
        assert_eq!(output, "pasta <instância> do <pack>");
        assert!(matches!(
            ExtraRule::pattern("(", Placeholder::Secret),
            Err(DiagnosticsError::InvalidRedactionRule { .. })
        ));
        assert!(matches!(
            ExtraRule::pattern("a*", Placeholder::Secret),
            Err(DiagnosticsError::InvalidRedactionRule { .. })
        ));
        assert!(format!("{profile:?}").contains("rules: 2"));
        let debug = format!("{:?}", ExtraRule::literal("x", Placeholder::User));
        assert!(debug.contains("texto fixo") && !debug.contains("\"x\""));
    }

    #[test]
    fn trechos_sobrepostos_viram_um_so() {
        let profile = profile().with_username("maria").with_player("mariazinha");
        let output = clean("mariazinha@exemplo.com e mariazinha", &profile);
        assert_eq!(output, "<email> e <jogador>");
        let redacted = redact("sem nada pessoal", &RedactionProfile::new());
        assert_eq!(redacted.text, "sem nada pessoal");
        assert!(redacted.counts.is_empty());
    }

    #[test]
    fn marcadores() {
        for (placeholder, text) in [
            (Placeholder::User, "<usuário>"),
            (Placeholder::Player, "<jogador>"),
            (Placeholder::Uuid, "<uuid>"),
            (Placeholder::Computer, "<computador>"),
            (Placeholder::Ip, "<ip>"),
            (Placeholder::Email, "<email>"),
            (Placeholder::Secret, "<segredo>"),
            (Placeholder::Custom("<x>".into()), "<x>"),
        ] {
            assert_eq!(placeholder.text(), text);
        }
        assert_eq!(compact_uuid("nao-e-uuid"), None);
        assert_eq!(cp850_as_cp1252("Zoë"), Some("Zo‰".to_owned()));
        assert_eq!(cp850_as_cp1252("Ω"), None);
        assert!(name_variants("   ").is_empty());
    }
}
