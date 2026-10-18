//! `start.sh`, `start.bat` e `user_jvm_args.txt` do pacote para servidor (ARCHITECTURE §12.1;
//! SPEC T19). Escritos do zero pelo Warden (os casos tratados se inspiram no `ServerPackCreator`,
//! LGPL-2.1, sem copiar código).
//!
//! Os dois scripts fazem o mesmo, na ordem:
//! 1. confere o Java (o `JAVA_HOME`, se definido, ou o `java` do sistema) contra a faixa da
//!    política do Java do Warden;
//! 2. pergunta a quem roda o servidor se aceita a EULA do Minecraft (só enquanto o `eula.txt`
//!    não tiver `eula=true`; o Warden nunca grava o aceite no pacote);
//! 3. pelo link: roda o packwiz-installer-bootstrap fixado com `--bootstrap-no-update` e
//!    `-s server` (se falhar com os mods já instalados, avisa e abre com eles) e copia o
//!    `server-overrides/` por cima (o `server.properties` só se ainda não existir);
//! 4. instala o loader na primeira vez (ou quando a versão do pacote muda): o `server.jar`, o
//!    lançador do Fabric ou o instalador do Forge/NeoForge com `--installServer`, conferindo o
//!    SHA-1 quando a fonte publica um;
//! 5. abre o servidor com a memória de `user_jvm_args.txt` e `nogui`.
//!
//! Finais de linha: `start.sh` e `user_jvm_args.txt` com LF; `start.bat` com CRLF (o `cmd.exe`
//! lê rótulos e blocos errado com LF). Os valores gravados nos scripts passam por
//! [`checked_value`] e [`checked_url`]: nada que o shell ou o `cmd.exe` interprete.

use url::Url;

use super::model::{JavaRange, LoaderDownload};
use crate::{Error, ExportErrorCode, Result};

/// Nome do bootstrap dentro do pacote.
pub(super) const BOOTSTRAP_JAR: &str = "packwiz-installer-bootstrap.jar";
/// Nome do installer dentro do pacote.
pub(super) const INSTALLER_JAR: &str = "packwiz-installer.jar";
/// O script do Linux e do macOS.
pub(super) const START_SH: &str = "start.sh";
/// O script do Windows.
pub(super) const START_BAT: &str = "start.bat";
/// A memória do servidor.
pub(super) const JVM_ARGS: &str = "user_jvm_args.txt";
/// O que o script instalou (`forge 1.20.1 47.3.0`).
pub(super) const LOADER_MARKER: &str = ".warden-loader";

/// O loader do pack, como os scripts o tratam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LoaderKind {
    Vanilla,
    Fabric,
    Forge,
    NeoForge,
}

impl LoaderKind {
    /// Pelo nome do `pack.toml` (`None`: sem scripts de servidor na v1).
    pub(super) fn from_pack(loader: Option<&str>) -> Option<Self> {
        match loader {
            None => Some(Self::Vanilla),
            Some("fabric") => Some(Self::Fabric),
            Some("forge") => Some(Self::Forge),
            Some("neoforge") => Some(Self::NeoForge),
            Some(_) => None,
        }
    }

    const fn id(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Fabric => "fabric",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Vanilla => "Minecraft",
            Self::Fabric => "Fabric",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
        }
    }
}

/// Como o servidor abre depois de instalado.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Launch {
    /// `java -jar <arquivo> nogui`.
    Jar(&'static str),
    /// O `forge-*<versão>*.jar` que o instalador deixa (Forge até a 1.16.5).
    ForgeJar,
    /// `java @libraries/…/<versão>/unix_args.txt nogui` (Forge 1.17+ e `NeoForge`).
    ArgsFile,
}

/// Tudo o que os scripts precisam.
#[derive(Debug, Clone)]
pub(super) struct ScriptInput<'a> {
    /// Nome do pack (só nos comentários, sem caracteres especiais).
    pub(super) pack_name: &'a str,
    /// Versão do pack (só nos comentários).
    pub(super) pack_version: &'a str,
    /// Versão do Minecraft.
    pub(super) minecraft: &'a str,
    /// Loader.
    pub(super) loader: LoaderKind,
    /// Versão exata do loader (vazia no vanilla).
    pub(super) loader_version: &'a str,
    /// De onde baixar o loader.
    pub(super) download: &'a LoaderDownload,
    /// O Java aceito.
    pub(super) java: JavaRange,
    /// O link do `pack.toml` (variante pelo link).
    pub(super) link: Option<&'a Url>,
}

/// Os três arquivos gerados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Scripts {
    pub(super) start_sh: String,
    pub(super) start_bat: String,
    pub(super) jvm_args: String,
}

fn unsafe_value(what: &str, value: &str) -> Error {
    Error::new(
        ExportErrorCode::ServerPackUnsupportedPack,
        format!("{what} com caracteres que os scripts não aceitam: {value}"),
    )
}

/// Versões e nomes de arquivo: só letras, números e `.`, `_`, `+`, `-`.
pub(super) fn checked_value<'a>(what: &str, value: &'a str) -> Result<&'a str> {
    let ok = !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'))
        && !value.contains("..");
    if ok {
        Ok(value)
    } else {
        Err(unsafe_value(what, value))
    }
}

/// Endereços: `https` (ou `http` só para este computador, nos testes), sem nada que o shell ou
/// o `cmd.exe` interprete.
pub(super) fn checked_url<'a>(what: &str, url: &'a Url) -> Result<&'a str> {
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost"));
    let scheme_ok = url.scheme() == "https" || (url.scheme() == "http" && loopback);
    let text = url.as_str();
    let chars_ok = text.bytes().all(|b| {
        b.is_ascii_graphic()
            && !matches!(
                b,
                b'"' | b'\'' | b'%' | b'^' | b'&' | b'|' | b'<' | b'>' | b'!' | b'`' | b'$' | b'\\'
            )
    });
    if scheme_ok && chars_ok {
        Ok(text)
    } else {
        Err(unsafe_value(what, text))
    }
}

/// Só o que pode ir num comentário de script.
fn comment_text(text: &str) -> String {
    let clean: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-' | '+') {
                c
            } else {
                ' '
            }
        })
        .collect();
    clean.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `4G` ou `3584M`.
pub(super) fn memory_arg(memory_mb: u32) -> String {
    if memory_mb.is_multiple_of(1024) {
        format!("{}G", memory_mb / 1024)
    } else {
        format!("{memory_mb}M")
    }
}

/// O `user_jvm_args.txt` (LF), no formato do instalador do Forge: comentários com `#` e um
/// argumento por linha.
pub(super) fn jvm_args(memory_mb: u32) -> String {
    let memory = memory_arg(memory_mb);
    format!(
        "# Memória do servidor, gravada pelo Warden: -Xms é a memória inicial e -Xmx, a máxima.\n\
         # Para mudar, troque os dois valores juntos (por exemplo, -Xms8G e -Xmx8G para 8 GB).\n\
         # O start.bat e o start.sh leem este arquivo; as linhas que começam com # são\n\
         # comentários.\n\
         -Xms{memory}\n\
         -Xmx{memory}\n"
    )
}

/// Se a versão do Minecraft é a 1.17 ou mais nova (Forge com `unix_args.txt`).
fn at_least_1_17(minecraft: &str) -> bool {
    let mut parts = minecraft.split(['.', '-']).map(|p| p.parse::<u32>().ok());
    match (parts.next().flatten(), parts.next().flatten()) {
        (Some(major), Some(minor)) => major > 1 || minor >= 17,
        // Snapshots (`24w14a`) e nomes novos: só existem com Forge moderno ou sem loader.
        _ => true,
    }
}

struct Plan<'a> {
    loader: LoaderKind,
    loader_version: &'a str,
    marker: String,
    /// Arquivo baixado e o endereço.
    file: String,
    url: &'a str,
    sha1: &'a str,
    launch: Launch,
    link: Option<&'a str>,
    java: JavaRange,
    header: Vec<String>,
}

#[allow(clippy::too_many_lines)] // cada loader com o arquivo, o endereço e o jeito de abrir
fn plan<'a>(input: &'a ScriptInput<'a>) -> Result<Plan<'a>> {
    let minecraft = checked_value("versão do Minecraft", input.minecraft)?;
    let loader_version = if input.loader == LoaderKind::Vanilla {
        ""
    } else {
        checked_value("versão do loader", input.loader_version)?
    };
    let (file, url, sha1, launch) = match (input.loader, input.download) {
        (LoaderKind::Vanilla, LoaderDownload::Vanilla { url, sha1 }) => (
            "server.jar".to_owned(),
            checked_url("endereço do servidor", url)?,
            sha1.as_deref().unwrap_or_default(),
            Launch::Jar("server.jar"),
        ),
        (LoaderKind::Fabric, LoaderDownload::Fabric { url }) => (
            "fabric-server-launch.jar".to_owned(),
            checked_url("endereço do Fabric", url)?,
            "",
            Launch::Jar("fabric-server-launch.jar"),
        ),
        (LoaderKind::Forge | LoaderKind::NeoForge, LoaderDownload::Installer { url, sha1 }) => {
            let name = url
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .unwrap_or_default();
            let launch = if input.loader == LoaderKind::NeoForge || at_least_1_17(minecraft) {
                Launch::ArgsFile
            } else {
                Launch::ForgeJar
            };
            (
                checked_value("nome do instalador", name)?.to_owned(),
                checked_url("endereço do instalador", url)?,
                sha1.as_deref().unwrap_or_default(),
                launch,
            )
        }
        _ => {
            return Err(Error::internal(
                "o download do loader não combina com o loader do pack",
            ));
        }
    };
    if !sha1.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(unsafe_value("SHA-1 do loader", sha1));
    }
    let link = input
        .link
        .map(|url| checked_url("link do pack", url))
        .transpose()?;
    let marker = if loader_version.is_empty() {
        format!("{} {minecraft}", input.loader.id())
    } else {
        format!("{} {minecraft} {loader_version}", input.loader.id())
    };
    let loader_text = if loader_version.is_empty() {
        format!("Minecraft {minecraft}")
    } else {
        format!(
            "Minecraft {minecraft} com {} {loader_version}",
            input.loader.label()
        )
    };
    let mut header = vec![
        format!(
            "Servidor do pack {} {}, gerado pelo Warden.",
            comment_text(input.pack_name),
            comment_text(input.pack_version)
        ),
        format!("{loader_text}; precisa do Java {}.", java_text(input.java)),
        String::new(),
        "O que este script faz, na ordem:".to_owned(),
        "  1. confere o Java (o do JAVA_HOME, se definido, ou o java do sistema);".to_owned(),
        "  2. pergunta se você aceita a EULA do Minecraft (só na primeira vez);".to_owned(),
    ];
    let mut step = 3;
    if link.is_some() {
        header.push(format!(
            "  {step}. baixa e atualiza os mods pelo link do pack (packwiz-installer, lado servidor);"
        ));
        step += 1;
    }
    header.push(format!(
        "  {step}. instala o {} na primeira vez;",
        input.loader.label()
    ));
    header.push(format!(
        "  {}. abre o servidor com a memória de {JVM_ARGS}.",
        step + 1
    ));
    if link.is_some() {
        header.push(String::new());
        header.push(
            "Se uma versão nova do pack mudar o Minecraft ou o loader, gere o pacote de novo."
                .to_owned(),
        );
    }
    Ok(Plan {
        loader: input.loader,
        loader_version,
        marker,
        file,
        url,
        sha1,
        launch,
        link,
        java: input.java,
        header,
    })
}

fn java_text(java: JavaRange) -> String {
    match java.max {
        Some(max) if max == java.min => java.min.to_string(),
        Some(max) => format!("{} a {max}", java.min),
        None => format!("{} ou mais novo", java.min),
    }
}

/// Acrescenta texto formatado (escrever numa `String` não falha).
fn push(s: &mut String, args: std::fmt::Arguments<'_>) {
    use std::fmt::Write as _;
    let _ = s.write_fmt(args);
}

/// Gera os três arquivos.
pub(super) fn render(input: &ScriptInput<'_>, memory_mb: u32) -> Result<Scripts> {
    let plan = plan(input)?;
    Ok(Scripts {
        start_sh: start_sh(&plan),
        start_bat: start_bat(&plan).replace('\n', "\r\n"),
        jvm_args: jvm_args(memory_mb),
    })
}

#[allow(clippy::too_many_lines)] // o script inteiro, na ordem em que roda
fn start_sh(plan: &Plan<'_>) -> String {
    let mut s = String::from("#!/bin/sh\n");
    for line in &plan.header {
        if line.is_empty() {
            s.push_str("#\n");
        } else {
            s.push_str("# ");
            s.push_str(line);
            s.push('\n');
        }
    }
    let label = plan.loader.label();
    let max = plan.java.max.map(|m| m.to_string()).unwrap_or_default();
    push(
        &mut s,
        format_args!(
            r#"
set -u
cd "$(dirname "$0")" || exit 1

LOADER_MARKER='{marker}'
LOADER_FILE='{file}'
LOADER_URL='{url}'
LOADER_SHA1='{sha1}'
JAVA_MIN={java_min}
JAVA_MAX='{max}'
"#,
            marker = plan.marker,
            file = plan.file,
            url = plan.url,
            sha1 = plan.sha1,
            java_min = plan.java.min,
        ),
    );
    if !matches!(plan.launch, Launch::Jar(_)) {
        push(
            &mut s,
            format_args!("LOADER_VERSION='{}'\n", plan.loader_version),
        );
    }
    if let Some(link) = plan.link {
        push(&mut s, format_args!("PACK_LINK='{link}'\n"));
    }
    s.push_str(
        r##"
falhar() {
    echo "" >&2
    echo "ERRO: $1" >&2
    exit 1
}

sha1_de() {
    if command -v sha1sum >/dev/null 2>&1; then
        sha1sum "$1" | cut -d ' ' -f 1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 1 "$1" | cut -d ' ' -f 1
    fi
}

# baixar ENDEREÇO ARQUIVO SHA1: baixa para ARQUIVO.part, confere e só então renomeia.
baixar() {
    echo "Baixando $2..."
    rm -f "$2.part"
    if command -v curl >/dev/null 2>&1; then
        curl -fL --retry 3 --progress-bar -o "$2.part" "$1" || { rm -f "$2.part"; return 1; }
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$2.part" "$1" || { rm -f "$2.part"; return 1; }
    else
        falhar "Para baixar $2, instale o curl ou o wget."
    fi
    if [ -n "$3" ]; then
        CONFERIDO=$(sha1_de "$2.part")
        if [ -z "$CONFERIDO" ]; then
            echo "AVISO: sem sha1sum nem shasum, $2 não foi conferido." >&2
        elif [ "$CONFERIDO" != "$3" ]; then
            rm -f "$2.part"
            echo "O arquivo baixado não é o esperado (SHA-1 diferente)." >&2
            return 1
        fi
    fi
    mv -f "$2.part" "$2"
}

# 1. Java
if [ -n "${JAVA_HOME:-}" ] && [ -x "$JAVA_HOME/bin/java" ]; then
    JAVA="$JAVA_HOME/bin/java"
elif command -v java >/dev/null 2>&1; then
    JAVA=java
else
    falhar "Java não encontrado. Instale o Java $JAVA_MIN (por exemplo, o Temurin, em https://adoptium.net) ou defina JAVA_HOME."
fi
JAVA_VERSION=$("$JAVA" -version 2>&1 | sed -n 's/.*version "\([^"]*\)".*/\1/p' | head -n 1)
case "$JAVA_VERSION" in
    1.*) JAVA_MAJOR=$(echo "$JAVA_VERSION" | cut -d . -f 2) ;;
    *) JAVA_MAJOR=$(echo "$JAVA_VERSION" | cut -d . -f 1 | cut -d - -f 1 | cut -d + -f 1) ;;
esac
case "$JAVA_MAJOR" in
    '' | *[!0-9]*) falhar "Não foi possível ler a versão do Java ($JAVA)." ;;
esac
if [ "$JAVA_MAJOR" -lt "$JAVA_MIN" ] || { [ -n "$JAVA_MAX" ] && [ "$JAVA_MAJOR" -gt "$JAVA_MAX" ]; }; then
    if [ -n "$JAVA_MAX" ] && [ "$JAVA_MAX" = "$JAVA_MIN" ]; then
        falhar "Este servidor precisa do Java $JAVA_MIN, e o Java encontrado é o $JAVA_MAJOR. Instale o Java $JAVA_MIN e defina JAVA_HOME."
    elif [ -n "$JAVA_MAX" ]; then
        falhar "Este servidor precisa do Java $JAVA_MIN a $JAVA_MAX, e o Java encontrado é o $JAVA_MAJOR. Instale um deles e defina JAVA_HOME."
    else
        falhar "Este servidor precisa do Java $JAVA_MIN ou mais novo, e o Java encontrado é o $JAVA_MAJOR. Instale o Java $JAVA_MIN e defina JAVA_HOME."
    fi
fi
echo "Java $JAVA_MAJOR encontrado."

# 2. EULA do Minecraft: quem roda o servidor decide.
if ! grep -qi '^eula=true' eula.txt 2>/dev/null; then
    echo ""
    echo "Para abrir o servidor, você precisa aceitar a EULA do Minecraft:"
    echo "https://aka.ms/MinecraftEULA"
    printf 'Você leu e aceita a EULA do Minecraft? (s/N) '
    read -r RESPOSTA || RESPOSTA=''
    case "$RESPOSTA" in
        s | S | sim | Sim | SIM) ;;
        *) falhar "A EULA não foi aceita; o servidor não vai abrir." ;;
    esac
    printf '%s\n' "# EULA do Minecraft (https://aka.ms/MinecraftEULA), aceita por quem roda este servidor." "eula=true" > eula.txt
fi
"##,
    );
    if plan.link.is_some() {
        push(
            &mut s,
            format_args!(
                r#"
# 3. Mods pelo link do pack (só os do lado servidor).
echo "Atualizando os mods pelo link do pack..."
if ! "$JAVA" -jar {BOOTSTRAP_JAR} --bootstrap-no-update --bootstrap-main-jar {INSTALLER_JAR} -g -s server "$PACK_LINK"; then
    if [ -f packwiz.json ]; then
        echo "AVISO: não foi possível atualizar os mods agora; o servidor abre com os mods que já estão aqui." >&2
    else
        falhar "Não foi possível baixar os mods pelo link do pack. Confira a internet e o link: $PACK_LINK"
    fi
fi
if [ -d server-overrides ]; then
    (cd server-overrides && find . -type f) | while IFS= read -r ARQUIVO; do
        DESTINO=${{ARQUIVO#./}}
        if [ "$DESTINO" = server.properties ] && [ -f server.properties ]; then
            continue
        fi
        mkdir -p "$(dirname "$DESTINO")"
        cp -f "server-overrides/$DESTINO" "$DESTINO"
    done
fi
"#
            ),
        );
    }
    push(
        &mut s,
        format_args!(
            r#"
# {label} (na primeira vez, ou quando o pacote traz outra versão).
if [ "$(cat {LOADER_MARKER} 2>/dev/null)" != "$LOADER_MARKER" ]; then
"#
        ),
    );
    match plan.launch {
        Launch::Jar(_) => push(
            &mut s,
            format_args!(
                r#"    baixar "$LOADER_URL" "$LOADER_FILE" "$LOADER_SHA1" || falhar "Não foi possível baixar o {label}. Confira a internet e tente de novo."
"#
            ),
        ),
        Launch::ForgeJar | Launch::ArgsFile => push(
            &mut s,
            format_args!(
                r#"    echo "Instalando o {label} $LOADER_VERSION (só na primeira vez; pode levar alguns minutos)..."
    baixar "$LOADER_URL" "$LOADER_FILE" "$LOADER_SHA1" || falhar "Não foi possível baixar o instalador do {label}. Confira a internet e tente de novo."
    # O instalador pode trocar o {JVM_ARGS}: a memória do pacote volta depois.
    cp -f {JVM_ARGS} {JVM_ARGS}.warden 2>/dev/null
    "$JAVA" -jar "$LOADER_FILE" --installServer || falhar "O instalador do {label} falhou. Veja as mensagens acima."
    mv -f {JVM_ARGS}.warden {JVM_ARGS} 2>/dev/null
    rm -f "$LOADER_FILE" "$LOADER_FILE.log"
"#
            ),
        ),
    }
    push(
        &mut s,
        format_args!(
            r#"    echo "$LOADER_MARKER" > {LOADER_MARKER}
fi

# Abrir o servidor.
JVM_ARGS=$(grep -v '^[[:space:]]*#' {JVM_ARGS} 2>/dev/null | tr '\r\n' '  ')
"#
        ),
    );
    match plan.launch {
        Launch::Jar(jar) => push(
            &mut s,
            format_args!(
                r#"echo "Abrindo o servidor..."
# shellcheck disable=SC2086
exec "$JAVA" $JVM_ARGS -jar {jar} nogui "$@"
"#
            ),
        ),
        Launch::ForgeJar => push(
            &mut s,
            format_args!(
                r#"SERVER_JAR=''
for ARQUIVO in forge-*"$LOADER_VERSION"*.jar; do
    case "$ARQUIVO" in
        *installer*) continue ;;
    esac
    if [ -f "$ARQUIVO" ]; then
        SERVER_JAR=$ARQUIVO
        break
    fi
done
[ -n "$SERVER_JAR" ] || falhar "O {label} não foi encontrado na pasta. Apague o arquivo {LOADER_MARKER} e rode o script de novo."
echo "Abrindo o servidor..."
# shellcheck disable=SC2086
exec "$JAVA" $JVM_ARGS -jar "$SERVER_JAR" nogui "$@"
"#
            ),
        ),
        Launch::ArgsFile => push(
            &mut s,
            format_args!(
                r#"ARGS_FILE=$(find libraries -name unix_args.txt -path "*$LOADER_VERSION*" 2>/dev/null | head -n 1)
[ -n "$ARGS_FILE" ] || falhar "O {label} não foi encontrado na pasta. Apague o arquivo {LOADER_MARKER} e rode o script de novo."
echo "Abrindo o servidor..."
# shellcheck disable=SC2086
exec "$JAVA" $JVM_ARGS "@$ARGS_FILE" nogui "$@"
"#
            ),
        ),
    }
    s
}

#[allow(clippy::too_many_lines)] // o script inteiro, na ordem em que roda
fn start_bat(plan: &Plan<'_>) -> String {
    let mut s = String::from("@echo off\n");
    for line in &plan.header {
        if line.is_empty() {
            s.push_str("rem\n");
        } else {
            s.push_str("rem ");
            s.push_str(line);
            s.push('\n');
        }
    }
    let label = plan.loader.label();
    let max = plan.java.max.map(|m| m.to_string()).unwrap_or_default();
    push(
        &mut s,
        format_args!(
            r#"
setlocal EnableExtensions DisableDelayedExpansion
chcp 65001 >nul
cd /d "%~dp0"

set "LOADER_MARKER={marker}"
set "LOADER_FILE={file}"
set "LOADER_URL={url}"
set "LOADER_SHA1={sha1}"
set "JAVA_MIN={java_min}"
set "JAVA_MAX={max}"
"#,
            marker = plan.marker,
            file = plan.file,
            url = plan.url,
            sha1 = plan.sha1,
            java_min = plan.java.min,
        ),
    );
    if !matches!(plan.launch, Launch::Jar(_)) {
        push(
            &mut s,
            format_args!("set \"LOADER_VERSION={}\"\n", plan.loader_version),
        );
    }
    if let Some(link) = plan.link {
        push(&mut s, format_args!("set \"PACK_LINK={link}\"\n"));
    }
    s.push_str(
        r#"
rem 1. Java
set "JAVA=java"
if defined JAVA_HOME if exist "%JAVA_HOME%\bin\java.exe" set "JAVA=%JAVA_HOME%\bin\java.exe"
set "JAVA_VERSION="
for /f "tokens=3" %%v in ('call "%JAVA%" -version 2^>^&1 ^| findstr /i /c:"version"') do if not defined JAVA_VERSION set "JAVA_VERSION=%%~v"
if not defined JAVA_VERSION goto sem_java
set "J1="
set "J2="
for /f "tokens=1,2 delims=._-+" %%a in ("%JAVA_VERSION%") do (set "J1=%%a" & set "J2=%%b")
set "JAVA_MAJOR=%J1%"
if "%J1%"=="1" set "JAVA_MAJOR=%J2%"
echo %JAVA_MAJOR%| findstr /r "^[0-9][0-9]*$" >nul || goto java_ilegivel
if %JAVA_MAJOR% LSS %JAVA_MIN% goto java_errado
if defined JAVA_MAX if %JAVA_MAJOR% GTR %JAVA_MAX% goto java_errado
echo Java %JAVA_MAJOR% encontrado.

rem 2. EULA do Minecraft: quem roda o servidor decide.
findstr /b /i /c:"eula=true" eula.txt >nul 2>&1
if not errorlevel 1 goto eula_ok
echo.
echo Para abrir o servidor, você precisa aceitar a EULA do Minecraft:
echo https://aka.ms/MinecraftEULA
set "RESPOSTA="
set /p "RESPOSTA=Você leu e aceita a EULA do Minecraft? (s/N) "
if /i "%RESPOSTA%"=="s" goto eula_aceita
if /i "%RESPOSTA%"=="sim" goto eula_aceita
echo.
echo ERRO: A EULA não foi aceita; o servidor não vai abrir.
goto falhou
:eula_aceita
> eula.txt echo # EULA do Minecraft (https://aka.ms/MinecraftEULA), aceita por quem roda este servidor.
>> eula.txt echo eula=true
:eula_ok
"#,
    );
    if plan.link.is_some() {
        push(
            &mut s,
            format_args!(
                r#"
rem 3. Mods pelo link do pack (só os do lado servidor).
echo Atualizando os mods pelo link do pack...
"%JAVA%" -jar {BOOTSTRAP_JAR} --bootstrap-no-update --bootstrap-main-jar {INSTALLER_JAR} -g -s server "%PACK_LINK%"
if not errorlevel 1 goto mods_ok
if not exist packwiz.json goto sem_mods
echo AVISO: não foi possível atualizar os mods agora; o servidor abre com os mods que já estão aqui.
:mods_ok
if not exist "server-overrides\" goto overrides_ok
robocopy "server-overrides" "." /E /XF server.properties /NFL /NDL /NJH /NJS /NP >nul
if errorlevel 8 goto sem_overrides
if not exist server.properties if exist "server-overrides\server.properties" copy /y "server-overrides\server.properties" server.properties >nul
:overrides_ok
"#
            ),
        );
    }
    push(
        &mut s,
        format_args!(
            r#"
rem {label} (na primeira vez, ou quando o pacote traz outra versão).
set "INSTALADO="
if exist {LOADER_MARKER} set /p INSTALADO=<{LOADER_MARKER}
if "%INSTALADO%"=="%LOADER_MARKER%" goto loader_ok
"#
        ),
    );
    match plan.launch {
        Launch::Jar(_) => s.push_str(
            r#"call :baixar "%LOADER_URL%" "%LOADER_FILE%" "%LOADER_SHA1%" || goto sem_loader
"#,
        ),
        Launch::ForgeJar | Launch::ArgsFile => push(
            &mut s,
            format_args!(
                r#"echo Instalando o {label} %LOADER_VERSION% (só na primeira vez; pode levar alguns minutos)...
call :baixar "%LOADER_URL%" "%LOADER_FILE%" "%LOADER_SHA1%" || goto sem_loader
rem O instalador pode trocar o {JVM_ARGS}: a memória do pacote volta depois.
if exist {JVM_ARGS} copy /y {JVM_ARGS} {JVM_ARGS}.warden >nul
"%JAVA%" -jar "%LOADER_FILE%" --installServer
if errorlevel 1 goto instalador_falhou
if exist {JVM_ARGS}.warden move /y {JVM_ARGS}.warden {JVM_ARGS} >nul
del /q "%LOADER_FILE%" "%LOADER_FILE%.log" 2>nul
"#
            ),
        ),
    }
    push(
        &mut s,
        format_args!(
            r#"> {LOADER_MARKER} echo %LOADER_MARKER%
:loader_ok

rem Abrir o servidor.
set "JVM_ARGS="
if exist {JVM_ARGS} for /f "usebackq eol=# delims=" %%a in ("{JVM_ARGS}") do call set "JVM_ARGS=%%JVM_ARGS%% %%a"
"#
        ),
    );
    match plan.launch {
        Launch::Jar(jar) => push(&mut s, format_args!(
            r#"echo Abrindo o servidor...
"%JAVA%" %JVM_ARGS% -jar {jar} nogui %*
"#
        )),
        Launch::ForgeJar => s.push_str(
            r#"set "SERVER_JAR="
for /f "delims=" %%f in ('dir /b "forge-*%LOADER_VERSION%*.jar" 2^>nul ^| findstr /v /i "installer"') do if not defined SERVER_JAR set "SERVER_JAR=%%f"
if not defined SERVER_JAR goto loader_sumiu
echo Abrindo o servidor...
"%JAVA%" %JVM_ARGS% -jar "%SERVER_JAR%" nogui %*
"#,
        ),
        Launch::ArgsFile => s.push_str(
            r#"set "ARGS_FILE="
for /f "delims=" %%f in ('dir /s /b "libraries\win_args.txt" 2^>nul ^| findstr /i /c:"%LOADER_VERSION%"') do if not defined ARGS_FILE set "ARGS_FILE=%%f"
if not defined ARGS_FILE goto loader_sumiu
echo Abrindo o servidor...
"%JAVA%" %JVM_ARGS% "@%ARGS_FILE%" nogui %*
"#,
        ),
    }
    push(
        &mut s,
        format_args!(
            r#"if errorlevel 1 goto falhou
endlocal
exit /b 0

rem baixar ENDEREÇO ARQUIVO SHA1: baixa para ARQUIVO.part, confere e só então renomeia.
:baixar
echo Baixando %~2...
if exist "%~2.part" del /q "%~2.part"
where curl.exe >nul 2>&1
if errorlevel 1 goto baixar_powershell
curl.exe -fL --retry 3 --progress-bar -o "%~2.part" "%~1"
if errorlevel 1 goto baixar_falhou
goto baixar_conferir
:baixar_powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri '%~1' -OutFile '%~2.part'"
if errorlevel 1 goto baixar_falhou
:baixar_conferir
if "%~3"=="" goto baixar_pronto
set "CONFERIDO="
for /f "skip=1 delims=" %%h in ('certutil -hashfile "%~2.part" SHA1') do if not defined CONFERIDO set "CONFERIDO=%%h"
if defined CONFERIDO set "CONFERIDO=%CONFERIDO: =%"
if /i "%CONFERIDO%"=="%~3" goto baixar_pronto
echo O arquivo baixado não é o esperado (SHA-1 diferente).
goto baixar_falhou
:baixar_pronto
move /y "%~2.part" "%~2" >nul
exit /b 0
:baixar_falhou
if exist "%~2.part" del /q "%~2.part"
exit /b 1

:sem_java
echo.
echo ERRO: Java não encontrado. Instale o Java %JAVA_MIN% (por exemplo, o Temurin, em https://adoptium.net) ou defina JAVA_HOME.
goto falhou
:java_ilegivel
echo.
echo ERRO: Não foi possível ler a versão do Java (%JAVA%).
goto falhou
:java_errado
echo.
if "%JAVA_MAX%"=="%JAVA_MIN%" echo ERRO: Este servidor precisa do Java %JAVA_MIN%, e o Java encontrado é o %JAVA_MAJOR%. Instale o Java %JAVA_MIN% e defina JAVA_HOME.
if not "%JAVA_MAX%"=="%JAVA_MIN%" if defined JAVA_MAX echo ERRO: Este servidor precisa do Java %JAVA_MIN% a %JAVA_MAX%, e o Java encontrado é o %JAVA_MAJOR%. Instale um deles e defina JAVA_HOME.
if not defined JAVA_MAX echo ERRO: Este servidor precisa do Java %JAVA_MIN% ou mais novo, e o Java encontrado é o %JAVA_MAJOR%. Instale o Java %JAVA_MIN% e defina JAVA_HOME.
goto falhou
:sem_loader
echo.
echo ERRO: Não foi possível baixar o {label}. Confira a internet e tente de novo.
goto falhou
:instalador_falhou
if exist {JVM_ARGS}.warden move /y {JVM_ARGS}.warden {JVM_ARGS} >nul
echo.
echo ERRO: O instalador do {label} falhou. Veja as mensagens acima.
goto falhou
:loader_sumiu
echo.
echo ERRO: O {label} não foi encontrado na pasta. Apague o arquivo {LOADER_MARKER} e rode o script de novo.
goto falhou
"#
        ),
    );
    if plan.link.is_some() {
        s.push_str(
            r":sem_mods
echo.
echo ERRO: Não foi possível baixar os mods pelo link do pack. Confira a internet e o link: %PACK_LINK%
goto falhou
:sem_overrides
echo.
echo ERRO: Não foi possível copiar a pasta server-overrides.
goto falhou
",
        );
    }
    s.push_str(
        r":falhou
echo.
pause
endlocal
exit /b 1
",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    fn input<'a>(
        loader: LoaderKind,
        minecraft: &'a str,
        version: &'a str,
        download: &'a LoaderDownload,
        link: Option<&'a Url>,
    ) -> ScriptInput<'a> {
        ScriptInput {
            pack_name: "Vale \"Sereno\" & cia",
            pack_version: "1.4.2",
            minecraft,
            loader,
            loader_version: version,
            download,
            java: JavaRange { min: 17, max: None },
            link,
        }
    }

    #[test]
    fn finais_de_linha_lf_no_sh_e_crlf_no_bat() {
        let download = LoaderDownload::Fabric {
            url: url("https://meta.fabricmc.net/v2/versions/loader/1.20.1/0.16.5/1.0.1/server/jar"),
        };
        let link = url("https://raw.githubusercontent.com/dono/pack/main/pack.toml");
        let scripts = render(
            &input(
                LoaderKind::Fabric,
                "1.20.1",
                "0.16.5",
                &download,
                Some(&link),
            ),
            4096,
        )
        .unwrap();
        assert!(scripts.start_sh.starts_with("#!/bin/sh\n"));
        assert!(!scripts.start_sh.contains('\r'));
        assert!(!scripts.jvm_args.contains('\r'));
        let bat = scripts.start_bat.as_bytes();
        for (i, byte) in bat.iter().enumerate() {
            if *byte == b'\n' {
                assert_eq!(bat[i - 1], b'\r', "LF sem CR na posição {i}");
            }
        }
        assert!(scripts.start_bat.ends_with("\r\n"));
        // O nome do pack vai limpo para os comentários.
        assert!(
            scripts
                .start_sh
                .contains("# Servidor do pack Vale Sereno cia 1.4.2")
        );
        assert!(
            scripts
                .start_bat
                .contains("rem Servidor do pack Vale Sereno cia 1.4.2")
        );
    }

    #[test]
    fn pelo_link_roda_o_bootstrap_com_lado_servidor_e_sem_atualizar() {
        let download = LoaderDownload::Fabric {
            url: url("https://meta.fabricmc.net/x/server/jar"),
        };
        let link = url("https://raw.githubusercontent.com/dono/pack/main/pack.toml");
        let scripts = render(
            &input(
                LoaderKind::Fabric,
                "1.20.1",
                "0.16.5",
                &download,
                Some(&link),
            ),
            4096,
        )
        .unwrap();
        for script in [&scripts.start_sh, &scripts.start_bat] {
            assert!(script.contains(
                "-jar packwiz-installer-bootstrap.jar --bootstrap-no-update --bootstrap-main-jar packwiz-installer.jar -g -s server"
            ));
            assert!(script.contains(link.as_str()));
            assert!(script.contains("aka.ms/MinecraftEULA"));
            // O aceite só é gravado depois da resposta.
            assert!(!script.contains("eula=false"));
        }
        // Sem o link, nenhum bootstrap.
        let scripts = render(
            &input(LoaderKind::Fabric, "1.20.1", "0.16.5", &download, None),
            4096,
        )
        .unwrap();
        assert!(!scripts.start_sh.contains("bootstrap"));
        assert!(!scripts.start_bat.contains("bootstrap"));
    }

    #[test]
    fn forge_moderno_usa_o_arquivo_de_argumentos_e_o_antigo_o_jar() {
        let download = LoaderDownload::Installer {
            url: url(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.3.0/forge-1.20.1-47.3.0-installer.jar",
            ),
            sha1: Some("0123456789abcdef0123456789abcdef01234567".into()),
        };
        let modern = render(
            &input(LoaderKind::Forge, "1.20.1", "47.3.0", &download, None),
            6144,
        )
        .unwrap();
        assert!(modern.start_sh.contains("unix_args.txt"));
        assert!(modern.start_bat.contains("win_args.txt"));
        assert!(modern.start_sh.contains("--installServer"));
        assert!(
            modern
                .start_sh
                .contains("LOADER_FILE='forge-1.20.1-47.3.0-installer.jar'")
        );
        assert!(
            modern
                .start_sh
                .contains("LOADER_SHA1='0123456789abcdef0123456789abcdef01234567'")
        );
        assert!(modern.jvm_args.contains("\n-Xms6G\n-Xmx6G\n"));
        let old = render(
            &input(LoaderKind::Forge, "1.12.2", "14.23.5.2860", &download, None),
            3584,
        )
        .unwrap();
        assert!(
            old.start_sh
                .contains("for ARQUIVO in forge-*\"$LOADER_VERSION\"*.jar")
        );
        assert!(!old.start_sh.contains("unix_args.txt"));
        assert!(old.jvm_args.contains("-Xmx3584M"));
        let neo = render(
            &input(LoaderKind::NeoForge, "1.21.1", "21.1.77", &download, None),
            4096,
        )
        .unwrap();
        assert!(neo.start_sh.contains("unix_args.txt"));
    }

    #[test]
    fn faixa_do_java_vai_para_os_scripts() {
        let download = LoaderDownload::Installer {
            url: url("https://maven.minecraftforge.net/a/forge-1.12.2-14.23.5.2860-installer.jar"),
            sha1: None,
        };
        let mut value = input(LoaderKind::Forge, "1.12.2", "14.23.5.2860", &download, None);
        value.java = JavaRange {
            min: 8,
            max: Some(8),
        };
        let scripts = render(&value, 4096).unwrap();
        assert!(scripts.start_sh.contains("JAVA_MIN=8\nJAVA_MAX='8'"));
        assert!(scripts.start_bat.contains("set \"JAVA_MAX=8\""));
        assert!(scripts.start_sh.contains("precisa do Java 8."));
    }

    #[test]
    fn valores_perigosos_sao_recusados() {
        let download = LoaderDownload::Fabric {
            url: url("https://meta.fabricmc.net/a%20b/jar"),
        };
        assert!(
            render(
                &input(LoaderKind::Fabric, "1.20.1", "0.16.5", &download, None),
                4096
            )
            .is_err()
        );
        let download = LoaderDownload::Fabric {
            url: url("http://example.com/jar"),
        };
        assert!(
            render(
                &input(LoaderKind::Fabric, "1.20.1", "0.16.5", &download, None),
                4096
            )
            .is_err()
        );
        let download = LoaderDownload::Fabric {
            url: url("http://127.0.0.1:8080/jar"),
        };
        assert!(
            render(
                &input(LoaderKind::Fabric, "1.20.1", "0.16.5", &download, None),
                4096
            )
            .is_ok()
        );
        assert!(
            render(
                &input(LoaderKind::Fabric, "1.20.1", "0.16'5", &download, None),
                4096
            )
            .is_err()
        );
        assert!(
            render(
                &input(LoaderKind::Fabric, "1.20&1", "0.16.5", &download, None),
                4096
            )
            .is_err()
        );
        // O download precisa combinar com o loader.
        assert!(
            render(
                &input(LoaderKind::Forge, "1.20.1", "47.3.0", &download, None),
                4096
            )
            .is_err()
        );
    }

    #[test]
    fn versao_1_17_ou_mais_nova() {
        assert!(at_least_1_17("1.17"));
        assert!(at_least_1_17("1.20.1"));
        assert!(!at_least_1_17("1.16.5"));
        assert!(!at_least_1_17("1.7.10"));
    }

    /// O `start.sh` de cada loader passa no `sh -n` (e no `shellcheck`, quando instalado).
    #[cfg(unix)]
    #[test]
    fn start_sh_de_cada_loader_tem_sintaxe_valida() {
        let link = url("https://raw.githubusercontent.com/dono/pack/main/pack.toml");
        let cases = [
            (
                LoaderKind::Fabric,
                "1.20.1",
                "0.16.5",
                LoaderDownload::Fabric {
                    url: url("https://meta.fabricmc.net/a/server/jar"),
                },
            ),
            (
                LoaderKind::Forge,
                "1.20.1",
                "47.3.0",
                LoaderDownload::Installer {
                    url: url(
                        "https://maven.minecraftforge.net/a/forge-1.20.1-47.3.0-installer.jar",
                    ),
                    sha1: Some("ab".repeat(20)),
                },
            ),
            (
                LoaderKind::Forge,
                "1.12.2",
                "14.23.5.2860",
                LoaderDownload::Installer {
                    url: url(
                        "https://maven.minecraftforge.net/a/forge-1.12.2-14.23.5.2860-installer.jar",
                    ),
                    sha1: None,
                },
            ),
            (
                LoaderKind::Vanilla,
                "1.21.1",
                "",
                LoaderDownload::Vanilla {
                    url: url("https://piston-data.mojang.com/v1/objects/abc/server.jar"),
                    sha1: Some("ab".repeat(20)),
                },
            ),
        ];
        let dir = tempfile::tempdir().unwrap();
        let shellcheck = std::process::Command::new("shellcheck")
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success());
        for (i, (loader, minecraft, version, download)) in cases.iter().enumerate() {
            for link in [None, Some(&link)] {
                let scripts =
                    render(&input(*loader, minecraft, version, download, link), 4096).unwrap();
                let file = dir.path().join(format!("start-{i}-{}.sh", link.is_some()));
                std::fs::write(&file, &scripts.start_sh).unwrap();
                let syntax = std::process::Command::new("sh")
                    .arg("-n")
                    .arg(&file)
                    .output()
                    .unwrap();
                assert!(
                    syntax.status.success(),
                    "{}",
                    String::from_utf8_lossy(&syntax.stderr)
                );
                if shellcheck {
                    let report = std::process::Command::new("shellcheck")
                        .args(["-s", "sh"])
                        .arg(&file)
                        .output()
                        .unwrap();
                    assert!(
                        report.status.success(),
                        "{}",
                        String::from_utf8_lossy(&report.stdout)
                    );
                }
            }
        }
    }
}
