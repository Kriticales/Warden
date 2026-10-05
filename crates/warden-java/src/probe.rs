//! Validação de um Java executando-o (ARCHITECTURE §7.3; R2 §2.5 item 4).
//!
//! [`ProcessProbe`] roda `java -XshowSettings:properties -version` uma vez e lê as
//! propriedades que o Java imprime na saída de erro. [`parse_properties`] é a parte pura,
//! testada com saídas reais gravadas (`tests/fixtures/probe/`).
//!
//! O programa roda com argumentos em vetor, sem shell, sem janela de console no Windows e sem
//! as variáveis que mudam o Java por fora (`JAVA_TOOL_OPTIONS`, `_JAVA_OPTIONS`,
//! `JDK_JAVA_OPTIONS`). O resultado fica em cache por caminho + data de modificação
//! ([`ValidationCache`]).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt as _;

use crate::error::{Error, Result};
use crate::version::JavaVersion;

/// Tempo máximo para o Java responder. A primeira execução de um Java recém-extraído pode
/// demorar enquanto o antivírus examina os arquivos.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// Maior saída lida do Java (o normal são ~5 KB; o `java.library.path` pode trazer o PATH
/// inteiro).
const MAX_OUTPUT: u64 = 256 * 1024;

/// Variáveis de ambiente que mudam o comportamento do Java por fora e são retiradas.
const JAVA_ENV_OVERRIDES: [&str; 3] = ["JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS", "JDK_JAVA_OPTIONS"];

/// O que o Java disse sobre si mesmo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInfo {
    /// Versão (de `java.runtime.version`, ou `java.version`).
    pub version: JavaVersion,
    /// `java.vendor`.
    pub vendor: String,
    /// `java.vm.name`.
    pub vm_name: String,
    /// `os.arch`.
    pub arch: String,
    /// Se é de 64 bits (`sun.arch.data.model` = 64, ou `os.arch` de 64 bits).
    pub is_64bit: bool,
}

/// Lê as propriedades impressas por `-XshowSettings:properties`.
///
/// Formato: um cabeçalho `Property settings:`, depois `    nome = valor`; propriedades com
/// vários valores continuam em linhas com mais recuo (que são ignoradas). Linhas de outros
/// avisos (`Picked up JAVA_TOOL_OPTIONS`, `openjdk version …`) não atrapalham.
#[must_use]
pub fn parse_properties(output: &str) -> BTreeMap<String, String> {
    let mut properties = BTreeMap::new();
    let mut in_block = false;
    for line in output.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim() == "Property settings:" {
            in_block = true;
            continue;
        }
        if !in_block {
            continue;
        }
        // Propriedade: exatamente 4 espaços de recuo. Continuação: 8 ou mais.
        let Some(rest) = line.strip_prefix("    ") else {
            if !line.trim().is_empty() {
                in_block = false;
            }
            continue;
        };
        if rest.starts_with(' ') {
            continue;
        }
        if let Some((name, value)) = rest.split_once(" = ") {
            properties.insert(name.trim().to_owned(), value.trim().to_owned());
        } else if let Some(name) = rest.strip_suffix(" =") {
            properties.insert(name.trim().to_owned(), String::new());
        }
    }
    properties
}

/// Monta o [`JavaInfo`] a partir das propriedades. `Err` com o motivo se faltar o essencial.
pub fn info_from_properties(
    properties: &BTreeMap<String, String>,
) -> std::result::Result<JavaInfo, String> {
    let version_text = properties
        .get("java.runtime.version")
        .or_else(|| properties.get("java.version"))
        .ok_or("a saída não tem java.version")?;
    let version = JavaVersion::parse(version_text)
        .or_else(|| {
            properties
                .get("java.version")
                .and_then(|v| JavaVersion::parse(v))
        })
        .ok_or_else(|| format!("versão do Java ilegível: {version_text:?}"))?;
    let arch = properties.get("os.arch").cloned().unwrap_or_default();
    let is_64bit = match properties.get("sun.arch.data.model").map(String::as_str) {
        Some("64") => true,
        Some("32") => false,
        _ => matches!(arch.as_str(), "amd64" | "x86_64" | "aarch64" | "arm64"),
    };
    Ok(JavaInfo {
        version,
        vendor: properties.get("java.vendor").cloned().unwrap_or_default(),
        vm_name: properties.get("java.vm.name").cloned().unwrap_or_default(),
        arch,
        is_64bit,
    })
}

/// Quem valida um Java. O app usa [`ProcessProbe`]; os testes podem usar um falso.
#[async_trait::async_trait]
pub trait JavaProbe: Send + Sync + std::fmt::Debug {
    /// Executa (ou simula) o Java e devolve o que ele diz de si.
    async fn probe(&self, java: &Path) -> Result<JavaInfo>;
}

/// Executa o Java de verdade.
#[derive(Debug, Clone)]
pub struct ProcessProbe {
    timeout: Duration,
}

impl Default for ProcessProbe {
    fn default() -> Self {
        Self {
            timeout: PROBE_TIMEOUT,
        }
    }
}

impl ProcessProbe {
    /// Com outro tempo máximo (testes).
    #[must_use]
    pub const fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

fn validation(path: &Path, message: impl Into<String>, output: Option<&str>) -> Error {
    Error::Validation {
        path: path.to_owned(),
        message: message.into(),
        output: output.map(|text| truncate(text, 4096)),
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Se o erro é o de um arquivo segurado por outro programa (antivírus examinando o Java
/// recém-extraído): acesso negado ou violação de compartilhamento (`ERROR_SHARING_VIOLATION`).
pub(crate) fn is_transient_lock(error: &std::io::Error) -> bool {
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;
    cfg!(windows)
        && (error.kind() == std::io::ErrorKind::PermissionDenied
            || matches!(
                error.raw_os_error(),
                Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
            ))
}

/// Esperas entre as tentativas quando um arquivo está segurado (antivírus; QUALITY §13.9).
pub(crate) const LOCK_RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(100),
    Duration::from_millis(250),
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
];

/// Abre o processo, tentando de novo com espera curta se o antivírus ainda segura o arquivo.
async fn spawn_with_retry(
    command: &mut tokio::process::Command,
) -> std::io::Result<tokio::process::Child> {
    let mut delays = LOCK_RETRY_DELAYS.iter();
    loop {
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(error) if is_transient_lock(&error) => match delays.next() {
                Some(delay) => {
                    tracing::debug!(%error, ?delay, "Java segurado por outro programa; nova tentativa");
                    tokio::time::sleep(*delay).await;
                }
                None => return Err(error),
            },
            Err(error) => return Err(error),
        }
    }
}

#[async_trait::async_trait]
impl JavaProbe for ProcessProbe {
    async fn probe(&self, java: &Path) -> Result<JavaInfo> {
        let mut command = tokio::process::Command::new(java);
        command
            .args(["-XshowSettings:properties", "-version"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for name in JAVA_ENV_OVERRIDES {
            command.env_remove(name);
        }
        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW: o `java.exe` é programa de console; sem isto, o app (que não
            // tem console) abriria uma janela preta por um instante.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = spawn_with_retry(&mut command)
            .await
            .map_err(|error| validation(java, format!("o Java não abriu: {error}"), None))?;
        let mut stdout = child.stdout.take();
        let mut stderr = child.stderr.take();
        let run = async {
            let mut out = Vec::new();
            let mut err = Vec::new();
            let read_out = async {
                if let Some(stream) = stdout.as_mut() {
                    stream.take(MAX_OUTPUT).read_to_end(&mut out).await?;
                }
                Ok::<_, std::io::Error>(())
            };
            let read_err = async {
                if let Some(stream) = stderr.as_mut() {
                    stream.take(MAX_OUTPUT).read_to_end(&mut err).await?;
                }
                Ok::<_, std::io::Error>(())
            };
            let (read_out, read_err) = tokio::join!(read_out, read_err);
            read_out?;
            read_err?;
            let status = child.wait().await?;
            Ok::<_, std::io::Error>((status, out, err))
        };
        let (status, out, err) = match tokio::time::timeout(self.timeout, run).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => {
                return Err(validation(
                    java,
                    format!("falha ao ler a saída: {error}"),
                    None,
                ));
            }
            Err(_) => {
                return Err(validation(
                    java,
                    format!("o Java não respondeu em {} s", self.timeout.as_secs()),
                    None,
                ));
            }
        };
        let mut output = String::from_utf8_lossy(&err).into_owned();
        output.push_str(&String::from_utf8_lossy(&out));
        if !status.success() {
            return Err(validation(
                java,
                format!("o Java terminou com {status}"),
                Some(&output),
            ));
        }
        let properties = parse_properties(&output);
        let info = info_from_properties(&properties)
            .map_err(|message| validation(java, message, Some(&output)))?;
        if !info.is_64bit {
            return Err(Error::Not64Bit {
                path: java.to_owned(),
                arch: info.arch,
            });
        }
        Ok(info)
    }
}

/// Chave do cache: caminho, data de modificação e tamanho do programa.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    path: PathBuf,
    modified: Option<SystemTime>,
    size: u64,
}

impl CacheKey {
    fn of(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        Some(Self {
            path: path.to_owned(),
            modified: metadata.modified().ok(),
            size: metadata.len(),
        })
    }
}

/// Cache das validações, em memória, por caminho + data de modificação + tamanho
/// (ARCHITECTURE §7.3). Um Java trocado no disco é validado de novo.
#[derive(Debug, Default)]
pub struct ValidationCache {
    entries: Mutex<HashMap<CacheKey, JavaInfo>>,
}

impl ValidationCache {
    /// Valida `java` com `probe`, usando o cache quando o arquivo não mudou.
    pub async fn validate(&self, probe: &dyn JavaProbe, java: &Path) -> Result<JavaInfo> {
        let key = CacheKey::of(java);
        if let Some(key) = &key {
            let cached = self
                .entries
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(key)
                .cloned();
            if let Some(info) = cached {
                return Ok(info);
            }
        }
        let info = probe.probe(java).await?;
        if let Some(key) = key {
            self.entries
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(key, info.clone());
        }
        Ok(info)
    }

    /// Esquece um Java (removido ou trocado).
    pub fn forget(&self, home: &Path) {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|key, _| !key.path.starts_with(home));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMURIN_21: &str = "Property settings:\n    file.encoding = Cp1252\n    java.home = C:\\runtimes\\temurin\n    java.library.path = C:\\a\n        C:\\b\n        .\n    java.runtime.version = 21.0.12.1+1-LTS\n    java.vendor = Eclipse Adoptium\n    java.version = 21.0.12.1\n    java.vm.name = OpenJDK 64-Bit Server VM\n    line.separator = \\r \\n \n    os.arch = amd64\n    sun.arch.data.model = 64\n    user.language =\n\nopenjdk version \"21.0.12.1\" 2026-08-18 LTS\nOpenJDK Runtime Environment Temurin-21.0.12.1+1 (build 21.0.12.1+1-LTS)\n";

    #[test]
    fn le_as_propriedades() {
        let properties = parse_properties(TEMURIN_21);
        assert_eq!(properties["java.vendor"], "Eclipse Adoptium");
        assert_eq!(properties["java.library.path"], "C:\\a");
        assert_eq!(properties["user.language"], "");
        assert!(!properties.contains_key("C:\\b"));
        assert!(!properties.keys().any(|k| k.starts_with("openjdk")));
        let info = info_from_properties(&properties).unwrap();
        assert_eq!(info.version, JavaVersion::new(21, 0, 12, 1, 1));
        assert!(info.is_64bit);
        assert_eq!(info.arch, "amd64");
        assert_eq!(info.vm_name, "OpenJDK 64-Bit Server VM");
    }

    #[test]
    fn crlf_e_avisos_antes_do_bloco() {
        let text = "Picked up JAVA_TOOL_OPTIONS: -Dx=y\r\nProperty settings:\r\n    java.version = 1.8.0_51\r\n    os.arch = x86\r\n    sun.arch.data.model = 32\r\n\r\njava version \"1.8.0_51\"\r\n";
        let properties = parse_properties(text);
        let info = info_from_properties(&properties).unwrap();
        assert_eq!(info.version, JavaVersion::new(8, 0, 51, 0, 0));
        assert!(!info.is_64bit);
    }

    #[test]
    fn saida_sem_versao() {
        assert!(info_from_properties(&parse_properties("Error: no JVM")).is_err());
        let mut properties = BTreeMap::new();
        properties.insert("java.version".to_owned(), "abc".to_owned());
        assert!(
            info_from_properties(&properties)
                .unwrap_err()
                .contains("ilegível")
        );
    }

    #[test]
    fn arquitetura_sem_data_model() {
        let mut properties = BTreeMap::new();
        properties.insert("java.version".to_owned(), "17.0.15".to_owned());
        properties.insert("os.arch".to_owned(), "aarch64".to_owned());
        assert!(info_from_properties(&properties).unwrap().is_64bit);
        properties.insert("os.arch".to_owned(), "x86".to_owned());
        assert!(!info_from_properties(&properties).unwrap().is_64bit);
    }

    #[test]
    fn truncamento_respeita_caracteres() {
        assert_eq!(truncate("ação", 2), "a…");
        assert_eq!(truncate("abc", 10), "abc");
    }

    #[derive(Debug)]
    struct Counting(std::sync::atomic::AtomicUsize);

    #[async_trait::async_trait]
    impl JavaProbe for Counting {
        async fn probe(&self, _java: &Path) -> Result<JavaInfo> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(JavaInfo {
                version: JavaVersion::new(17, 0, 15, 0, 0),
                vendor: "x".into(),
                vm_name: "y".into(),
                arch: "amd64".into(),
                is_64bit: true,
            })
        }
    }

    #[tokio::test]
    async fn cache_por_caminho_e_data() {
        let dir = tempfile::tempdir().unwrap();
        let java = dir.path().join("java");
        std::fs::write(&java, b"a").unwrap();
        let probe = Counting(std::sync::atomic::AtomicUsize::new(0));
        let cache = ValidationCache::default();
        cache.validate(&probe, &java).await.unwrap();
        cache.validate(&probe, &java).await.unwrap();
        assert_eq!(probe.0.load(std::sync::atomic::Ordering::SeqCst), 1);
        // Arquivo trocado (outro tamanho): valida de novo.
        std::fs::write(&java, b"abc").unwrap();
        cache.validate(&probe, &java).await.unwrap();
        assert_eq!(probe.0.load(std::sync::atomic::Ordering::SeqCst), 2);
        cache.forget(dir.path());
        cache.validate(&probe, &java).await.unwrap();
        assert_eq!(probe.0.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn programa_inexistente_vira_erro_de_validacao() {
        let error = ProcessProbe::default()
            .probe(Path::new("nao-existe/bin/java"))
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Validation { .. }), "{error:?}");
    }
}
