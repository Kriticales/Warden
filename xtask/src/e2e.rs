//! `cargo xtask e2e-driver`: prepara o driver nativo que o `tauri-driver` usa nos testes ponta a
//! ponta (QUALITY §13.5), sem instalar nada.
//!
//! - **Windows:** lê a versão do WebView2 instalado no registro e baixa o `msedgedriver` da
//!   **mesma versão** para `<target>/e2e/msedgedriver/<versão>/` (cache fora do git, refeito
//!   quando o WebView2 se atualiza). Confere que o executável roda e informa a mesma versão.
//! - **Linux (CI):** confere que o `WebKitWebDriver` do WebKitGTK está no `PATH`.
//!
//! Nos dois casos grava o caminho do driver em `<target>/e2e/native-driver.txt`, que o
//! `apps/desktop/wdio.conf.ts` lê para iniciar o `tauri-driver --native-driver <caminho>`.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail, ensure};

use crate::util::{Cmd, find_program, target_dir};

/// Chave do registro com a versão do WebView2 Evergreen (o GUID é fixo, da Microsoft).
const WEBVIEW2_CLIENT: &str =
    r"Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

/// Onde procurar a versão, em ordem: instalação por máquina (64 e 32 bits) e por usuário.
const WEBVIEW2_KEYS: &[&str] = &[
    r"HKLM\SOFTWARE\WOW6432Node\",
    r"HKLM\SOFTWARE\",
    r"HKCU\Software\",
];

/// Endereço de download do `msedgedriver` (o antigo `msedgedriver.azureedge.net` saiu do ar).
const DRIVER_BASE_URL: &str = "https://msedgedriver.microsoft.com";

/// Limite do download (o zip tem uns 10 MB).
const DOWNLOAD_LIMIT: u64 = 64 * 1024 * 1024;

/// Nome do arquivo com o caminho do driver nativo, dentro de `<target>/e2e/`.
pub const NATIVE_DRIVER_FILE: &str = "native-driver.txt";

/// Versão no formato do Chromium: quatro números separados por ponto.
pub fn is_chromium_version(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

/// Lê o valor `pv` da saída do `reg query` (`    pv    REG_SZ    154.0.4258.53`).
pub fn parse_reg_version(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let (name, kind, value) = (parts.next()?, parts.next()?, parts.next()?);
        (name.eq_ignore_ascii_case("pv") && kind == "REG_SZ" && is_chromium_version(value))
            .then(|| value.to_owned())
    })
}

/// Versão informada por `msedgedriver --version` (`Microsoft Edge WebDriver 154.0.4258.53 (…)`).
pub fn parse_driver_version(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .find(|word| is_chromium_version(word))
        .map(str::to_owned)
}

/// Endereço do zip do `msedgedriver` de uma versão.
pub fn driver_url(version: &str) -> String {
    format!("{DRIVER_BASE_URL}/{version}/edgedriver_win64.zip")
}

/// Pasta do cache dos drivers.
fn e2e_dir() -> Result<PathBuf> {
    Ok(target_dir()?.join("e2e"))
}

/// Versão do WebView2 instalado, pelo registro.
fn webview2_version() -> Result<String> {
    let reg = find_program("reg")?;
    for prefix in WEBVIEW2_KEYS {
        let key = format!("{prefix}{WEBVIEW2_CLIENT}");
        if let Ok(output) = Cmd::new(&reg).args(["query", &key, "/v", "pv"]).read()
            && let Some(version) = parse_reg_version(&output)
        {
            return Ok(version);
        }
    }
    bail!(
        "e2e-driver: não encontrei a versão do WebView2 no registro. O WebView2 Runtime é \
         pré-requisito do app (QUALITY §13.1)."
    )
}

/// Extrai `msedgedriver.exe` do zip baixado.
fn extract_driver(zip_bytes: &[u8], destination: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .context("e2e-driver: o download não é um zip válido")?;
    let mut entry = archive
        .by_name("msedgedriver.exe")
        .context("e2e-driver: o zip não tem msedgedriver.exe")?;
    let mut bytes = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or(0));
    entry.read_to_end(&mut bytes)?;
    let partial = destination.with_extension("exe.part");
    std::fs::write(&partial, &bytes)
        .with_context(|| format!("falha ao gravar {}", partial.display()))?;
    std::fs::rename(&partial, destination)
        .with_context(|| format!("falha ao gravar {}", destination.display()))?;
    Ok(())
}

/// Versão que um `msedgedriver.exe` informa.
fn installed_driver_version(driver: &Path) -> Result<String> {
    let output = Cmd::new(driver).args(["--version"]).read()?;
    parse_driver_version(&output)
        .with_context(|| format!("e2e-driver: saída inesperada de msedgedriver: {output}"))
}

/// Garante o `msedgedriver` da versão do WebView2 e devolve o caminho.
fn ensure_msedgedriver() -> Result<PathBuf> {
    let version = webview2_version()?;
    println!("e2e-driver: WebView2 {version}");
    let folder = e2e_dir()?.join("msedgedriver").join(&version);
    let driver = folder.join("msedgedriver.exe");
    if driver.is_file() && installed_driver_version(&driver).ok().as_deref() == Some(&version) {
        println!("e2e-driver: msedgedriver {version} já está no cache.");
        return Ok(driver);
    }
    std::fs::create_dir_all(&folder)
        .with_context(|| format!("falha ao criar {}", folder.display()))?;
    let url = driver_url(&version);
    println!("e2e-driver: baixando {url}");
    let mut response = ureq::get(&url)
        .call()
        .with_context(|| format!("e2e-driver: falha ao baixar {url}"))?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(DOWNLOAD_LIMIT)
        .read_to_vec()
        .with_context(|| format!("e2e-driver: falha ao ler {url}"))?;
    extract_driver(&bytes, &driver)?;
    let got = installed_driver_version(&driver)?;
    ensure!(
        got == version,
        "e2e-driver: o msedgedriver baixado informa {got}, mas o WebView2 é {version}"
    );
    println!("e2e-driver: msedgedriver {version} pronto.");
    Ok(driver)
}

/// Confere o `WebKitWebDriver` (Linux).
fn ensure_webkit_driver() -> Result<PathBuf> {
    find_program("WebKitWebDriver").context(
        "e2e-driver: WebKitWebDriver não está no PATH. No Ubuntu, instale o pacote \
         webkit2gtk-driver (a CI da F0-02 faz isso).",
    )
}

/// `cargo xtask e2e-driver`.
pub fn run() -> Result<()> {
    find_program("tauri-driver").context("e2e-driver: rode `cargo xtask setup` antes")?;
    let driver = if cfg!(windows) {
        ensure_msedgedriver()?
    } else {
        ensure_webkit_driver()?
    };
    let dir = e2e_dir()?;
    std::fs::create_dir_all(&dir).with_context(|| format!("falha ao criar {}", dir.display()))?;
    let marker = dir.join(NATIVE_DRIVER_FILE);
    std::fs::write(&marker, driver.to_string_lossy().as_bytes())
        .with_context(|| format!("falha ao gravar {}", marker.display()))?;
    println!("e2e-driver: driver nativo em {}", driver.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_a_versao_do_reg_query() {
        let output = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}\r\n    pv    REG_SZ    154.0.4258.53\r\n\r\n";
        assert_eq!(parse_reg_version(output).as_deref(), Some("154.0.4258.53"));
    }

    #[test]
    fn reg_query_sem_versao_valida_nao_devolve_nada() {
        assert_eq!(parse_reg_version(""), None);
        assert_eq!(parse_reg_version("    pv    REG_SZ    0.0.0"), None);
        assert_eq!(
            parse_reg_version("    name    REG_SZ    154.0.4258.53"),
            None
        );
        assert_eq!(
            parse_reg_version("    pv    REG_DWORD    154.0.4258.53"),
            None
        );
    }

    #[test]
    fn le_a_versao_do_msedgedriver() {
        let output = "Microsoft Edge WebDriver 154.0.4258.53 (2a1b3c4d5e6f7a8b9c0d)\n";
        assert_eq!(
            parse_driver_version(output).as_deref(),
            Some("154.0.4258.53")
        );
        assert_eq!(parse_driver_version("Microsoft Edge WebDriver"), None);
    }

    #[test]
    fn versao_do_chromium_tem_quatro_numeros() {
        assert!(is_chromium_version("154.0.4258.53"));
        assert!(!is_chromium_version("154.0.4258"));
        assert!(!is_chromium_version("154.0.4258.a"));
        assert!(!is_chromium_version("154..4258.53"));
    }

    #[test]
    fn endereco_do_driver_e_o_da_microsoft() {
        assert_eq!(
            driver_url("154.0.4258.53"),
            "https://msedgedriver.microsoft.com/154.0.4258.53/edgedriver_win64.zip"
        );
    }

    #[test]
    fn extrai_o_executavel_do_zip_e_recusa_zip_sem_ele() {
        use std::io::Write as _;
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default();
            writer
                .start_file("Driver_Notes/credits.html", options)
                .unwrap();
            writer.write_all(b"notas").unwrap();
            writer.start_file("msedgedriver.exe", options).unwrap();
            writer.write_all(b"MZ executavel").unwrap();
            writer.finish().unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("msedgedriver.exe");
        extract_driver(buffer.get_ref(), &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"MZ executavel");
        assert!(!dir.path().join("msedgedriver.exe.part").exists());

        let mut empty = std::io::Cursor::new(Vec::new());
        zip::ZipWriter::new(&mut empty).finish().unwrap();
        let error = extract_driver(empty.get_ref(), &target).unwrap_err();
        assert!(format!("{error:#}").contains("não tem msedgedriver.exe"));
        assert!(extract_driver(b"nao e zip", &target).is_err());
    }
}
