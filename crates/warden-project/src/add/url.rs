//! Classificação do link colado no campo único de "Adicionar" (SPEC T08 "Link"; P1-11).
//!
//! O Warden reconhece o link pelo formato, sem nenhuma requisição:
//!
//! - **Modrinth:** página de projeto (`https://modrinth.com/mod/sodium`), de versão
//!   (`…/sodium/version/mc1.21.1-0.6.0-fabric`) e o arquivo no CDN
//!   (`https://cdn.modrinth.com/data/<projeto>/versions/<versão>/<arquivo>`), que vira o link
//!   da versão. Resolvidos pela API do Modrinth ([`super::link`]).
//! - **CurseForge:** projeto ou arquivo, pelo [`super::link_curseforge::parse_link`] da P1-10
//!   (lidos pelo packwiz numa cópia do pack).
//! - **Direto:** qualquer outro `https://` que termina num arquivo `.jar` ou `.zip`.
//!
//! **`http://` é recusado** antes de qualquer outra coisa (CA-T08-07): nenhum download, nenhuma
//! consulta. Link com usuário e senha embutidos também é recusado (a senha iria para o
//! `.pw.toml` e para os registros).

use url::Url;

use super::link_curseforge::{CurseforgeLink, parse_link};
use crate::search::ProjectKind;

/// Maior link aceito (o mesmo corte das mensagens da `warden-http`).
pub const MAX_LINK_CHARS: usize = 2048;

/// Um link do Modrinth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModrinthLink {
    /// Página de um projeto.
    Project {
        /// Slug ou ID do projeto.
        project: String,
    },
    /// Página de uma versão (`/version/<número ou ID>`) ou arquivo do CDN.
    Version {
        /// Slug ou ID do projeto.
        project: String,
        /// Número da versão (`mc1.21.1-0.6.0-fabric`) ou ID (`SMxNOGZ6`).
        version: String,
    },
}

impl ModrinthLink {
    /// Slug ou ID do projeto.
    #[must_use]
    pub fn project(&self) -> &str {
        match self {
            Self::Project { project } | Self::Version { project, .. } => project,
        }
    }
}

/// Um link direto `https://` para um arquivo `.jar` ou `.zip`, já conferido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectLink {
    url: Url,
    file_name: String,
}

impl DirectLink {
    /// O link (sempre `https`, sem usuário e senha, sem o trecho `#`).
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Nome do arquivo, como o `packwiz url add` grava (último trecho do caminho, decodificado).
    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// Tipo provável pelo nome: `.jar` é mod; `.zip` pode ser resource pack ou shader (quem
    /// decide é o conteúdo, lido depois do download).
    #[must_use]
    pub fn kind_hint(&self) -> Option<ProjectKind> {
        is_jar(&self.file_name).then_some(ProjectKind::Mod)
    }
}

/// O que o texto do campo único é.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkClass {
    /// Não é link: o texto é uma busca por nome.
    NotLink,
    /// Link `http://`: recusado sem baixar nada (CA-T08-07).
    Insecure,
    /// Link `https://` que o Warden não sabe adicionar (site desconhecido que não termina em
    /// `.jar`/`.zip`, link com senha, link longo demais).
    Unsupported,
    /// Link do Modrinth.
    Modrinth(ModrinthLink),
    /// Link da CurseForge (P1-10).
    Curseforge(CurseforgeLink),
    /// Link direto para um arquivo.
    Direct(DirectLink),
}

fn has_scheme(text: &str, scheme: &str) -> bool {
    text.len() > scheme.len()
        && text.as_bytes()[..scheme.len()].eq_ignore_ascii_case(scheme.as_bytes())
        && text.as_bytes()[scheme.len()..].starts_with(b"://")
}

/// Classifica o texto do campo único. Nunca faz requisição.
#[must_use]
pub fn classify_link(text: &str) -> LinkClass {
    let text = text.trim();
    if has_scheme(text, "http") {
        return LinkClass::Insecure;
    }
    if !has_scheme(text, "https") {
        // Outros esquemas (`ftp://`, `file://`) não são busca por nome.
        return if text.contains("://") {
            LinkClass::Unsupported
        } else {
            LinkClass::NotLink
        };
    }
    if text.chars().count() > MAX_LINK_CHARS {
        return LinkClass::Unsupported;
    }
    let Ok(mut url) = Url::parse(text) else {
        return LinkClass::Unsupported;
    };
    if !url.username().is_empty() || url.password().is_some() {
        return LinkClass::Unsupported;
    }
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return LinkClass::Unsupported;
    };
    if let Some(link) = parse_link(text) {
        return LinkClass::Curseforge(link);
    }
    if let Some(link) = modrinth_link(&host, &url) {
        return LinkClass::Modrinth(link);
    }
    url.set_fragment(None);
    match direct_file_name(&url) {
        Some(file_name) => LinkClass::Direct(DirectLink { url, file_name }),
        None => LinkClass::Unsupported,
    }
}

/// Tipos de projeto que aparecem no caminho das páginas do Modrinth.
const MODRINTH_TYPES: &[&str] = &[
    "mod",
    "plugin",
    "datapack",
    "resourcepack",
    "shader",
    "modpack",
    "project",
];

fn modrinth_link(host: &str, url: &Url) -> Option<ModrinthLink> {
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    match host {
        "modrinth.com" | "www.modrinth.com" => {
            let [kind, project, rest @ ..] = segments.as_slice() else {
                return None;
            };
            if !MODRINTH_TYPES.contains(kind) || !valid_id(project) {
                return None;
            }
            let project = decode(project)?;
            match rest {
                ["version", version, ..] if valid_id(version) => Some(ModrinthLink::Version {
                    project,
                    version: decode(version)?,
                }),
                // As abas do projeto (`/versions`, `/gallery`, `/changelog`…) são o projeto.
                _ => Some(ModrinthLink::Project { project }),
            }
        }
        "cdn.modrinth.com" => match segments.as_slice() {
            ["data", project, "versions", version, _file]
                if valid_id(project) && valid_id(version) =>
            {
                Some(ModrinthLink::Version {
                    project: decode(project)?,
                    version: decode(version)?,
                })
            }
            _ => None,
        },
        _ => None,
    }
}

/// Um trecho de slug, ID ou número de versão: sem nada que mude o caminho na API.
fn valid_id(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= 128
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '%'))
        && segment != "."
        && segment != ".."
}

fn decode(segment: &str) -> Option<String> {
    let decoded = percent_decode(segment)?;
    (!decoded.is_empty() && !decoded.contains(['/', '\\']) && decoded != "..").then_some(decoded)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn is_jar(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
}

fn is_zip(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}

/// O nome do arquivo de um link direto (o mesmo do `packwiz url add`), se for `.jar`/`.zip` e
/// servir de nome de arquivo no Windows.
fn direct_file_name(url: &Url) -> Option<String> {
    let name = warden_packwiz::filename_from_url(url.as_str()).ok()?;
    (safe_file_name(&name) && (is_jar(&name) || is_zip(&name))).then_some(name)
}

/// Um nome que pode virar arquivo dentro do pack em qualquer sistema: um trecho só, sem
/// caracteres proibidos no Windows, sem controle e sem nome reservado.
#[must_use]
pub fn safe_file_name(name: &str) -> bool {
    const RESERVED: &[&str] = &[
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && !name.ends_with(['.', ' '])
        && !name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        && !RESERVED.contains(&stem.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modrinth(text: &str) -> Option<ModrinthLink> {
        match classify_link(text) {
            LinkClass::Modrinth(link) => Some(link),
            _ => None,
        }
    }

    fn direct(text: &str) -> Option<DirectLink> {
        match classify_link(text) {
            LinkClass::Direct(link) => Some(link),
            _ => None,
        }
    }

    #[test]
    fn http_e_recusado_antes_de_tudo() {
        for text in [
            "http://example.com/mod.jar",
            "HTTP://EXAMPLE.COM/MOD.JAR",
            "  http://modrinth.com/mod/sodium ",
            "http://www.curseforge.com/minecraft/mc-mods/jei",
            "http://127.0.0.1:8080/a.zip",
        ] {
            assert_eq!(classify_link(text), LinkClass::Insecure, "{text}");
        }
    }

    #[test]
    fn texto_comum_e_busca() {
        for text in [
            "sodium",
            "",
            "  jei  ",
            "https",
            "http",
            "not enough items",
            "mod.jar",
        ] {
            assert_eq!(classify_link(text), LinkClass::NotLink, "{text}");
        }
    }

    #[test]
    fn reconhece_links_do_modrinth() {
        let project = |slug: &str| {
            Some(ModrinthLink::Project {
                project: slug.into(),
            })
        };
        assert_eq!(
            modrinth("https://modrinth.com/mod/sodium"),
            project("sodium")
        );
        for text in [
            "https://modrinth.com/mod/sodium/",
            "https://www.modrinth.com/mod/sodium?x=1#y",
            "https://MODRINTH.com/mod/sodium/versions",
            "https://modrinth.com/mod/sodium/gallery",
        ] {
            assert_eq!(modrinth(text), project("sodium"), "{text}");
        }
        assert_eq!(
            modrinth("https://modrinth.com/shader/complementary-reimagined"),
            project("complementary-reimagined")
        );
        assert_eq!(
            modrinth("https://modrinth.com/mod/sodium/version/mc1.21.1-0.6.0-fabric"),
            Some(ModrinthLink::Version {
                project: "sodium".into(),
                version: "mc1.21.1-0.6.0-fabric".into()
            })
        );
        assert_eq!(
            modrinth(
                "https://cdn.modrinth.com/data/AANobbMI/versions/SMxNOGZ6/sodium-fabric-0.8.13.jar"
            ),
            Some(ModrinthLink::Version {
                project: "AANobbMI".into(),
                version: "SMxNOGZ6".into()
            })
        );
        // Página que não é de projeto.
        assert_eq!(modrinth("https://modrinth.com/mods"), None);
        assert_eq!(modrinth("https://modrinth.com/user/jellysquid3"), None);
        assert_eq!(modrinth("https://modrinth.com/mod/..%2F..%2Fx"), None);
    }

    #[test]
    fn links_da_curseforge_seguem_pela_p1_10() {
        assert!(matches!(
            classify_link("https://www.curseforge.com/minecraft/mc-mods/jei"),
            LinkClass::Curseforge(CurseforgeLink::Project { .. })
        ));
        assert!(matches!(
            classify_link("https://www.curseforge.com/minecraft/mc-mods/jei/files/4712345"),
            LinkClass::Curseforge(CurseforgeLink::File { .. })
        ));
    }

    #[test]
    fn link_direto_so_para_jar_e_zip() {
        let link = direct("https://example.com/files/Meu%20Mod%2B1.0.jar#frag").unwrap();
        assert_eq!(link.file_name(), "Meu Mod+1.0.jar");
        assert_eq!(
            link.url().as_str(),
            "https://example.com/files/Meu%20Mod%2B1.0.jar"
        );
        assert_eq!(link.kind_hint(), Some(ProjectKind::Mod));
        let zip = direct("https://example.com/Faithful.ZIP?download=1").unwrap();
        assert_eq!(zip.file_name(), "Faithful.ZIP");
        assert_eq!(zip.kind_hint(), None);
        for text in [
            "https://example.com/",
            "https://example.com/page.html",
            "https://user:senha@example.com/mod.jar",
            "https://example.com/con.jar",
            "https://example.com/a%3Ab.jar",
            "ftp://example.com/mod.jar",
            "file:///C:/mods/mod.jar",
        ] {
            assert!(
                matches!(classify_link(text), LinkClass::Unsupported),
                "{text}: {:?}",
                classify_link(text)
            );
        }
        let long = format!("https://example.com/{}.jar", "a".repeat(MAX_LINK_CHARS));
        assert_eq!(classify_link(&long), LinkClass::Unsupported);
    }

    #[test]
    fn nomes_de_arquivo_seguros() {
        for ok in ["sodium.jar", "Meu Mod (1.0) ção.jar", "a+b.zip"] {
            assert!(safe_file_name(ok), "{ok}");
        }
        for bad in [
            "",
            ".jar",
            "a/b.jar",
            "a\\b.jar",
            "..",
            "NUL.jar",
            "a:b.jar",
            "a.jar.",
            "a\u{0}.jar",
        ] {
            assert!(!safe_file_name(bad), "{bad:?}");
        }
    }
}
