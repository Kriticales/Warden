//! Metafiles `.pw.toml`: um arquivo baixado de fora do pack (R3 §1.7; struct `core.Mod` do
//! packwiz).

use std::collections::BTreeMap;

use crate::decode::{TableReader, parse_document};
use crate::encode::{Node, boolean, encode_document, text};
use crate::error::{Error, Result};
use crate::hash::HashFormat;
use crate::pack::Parsed;
use crate::value::Value;

/// `mode` dos metafiles da CurseForge: o link é pedido à API na hora de baixar.
pub const MODE_CURSEFORGE: &str = "metadata:curseforge";

/// Lado em que o arquivo é instalado (`side`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Side {
    /// Campo ausente ou vazio (o packwiz trata como `both`).
    Unset,
    /// `both`: cliente e servidor.
    Both,
    /// `client`: só cliente.
    Client,
    /// `server`: só servidor.
    Server,
    /// Valor que o packwiz não conhece, mantido como está.
    Other(String),
}

impl Side {
    /// Texto gravado no arquivo (vazio para [`Side::Unset`]).
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Unset => "",
            Self::Both => "both",
            Self::Client => "client",
            Self::Server => "server",
            Self::Other(other) => other,
        }
    }

    /// Lê o texto do campo `side`.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        match text {
            "" => Self::Unset,
            "both" => Self::Both,
            "client" => Self::Client,
            "server" => Self::Server,
            other => Self::Other(other.to_owned()),
        }
    }

    /// Se o arquivo vai para o cliente (vazio e `both` contam).
    #[must_use]
    pub fn on_client(&self) -> bool {
        matches!(self, Self::Unset | Self::Both | Self::Client)
    }

    /// Se o arquivo vai para o servidor (vazio e `both` contam).
    #[must_use]
    pub fn on_server(&self) -> bool {
        matches!(self, Self::Unset | Self::Both | Self::Server)
    }
}

/// Observação que acompanha o lado sugerido a partir do Modrinth (ARCHITECTURE §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideNote {
    /// `client_or_server`: "Funciona em qualquer um dos lados".
    EitherSide,
    /// `unknown`, ausente ou valor novo: "Lado desconhecido — confira".
    Unknown,
}

/// Lado gravado para o campo `environment` de uma versão do Modrinth (ARCHITECTURE §6.2).
///
/// Corrige o caso do packwiz em que `server_only_client_optional` e
/// `client_only_server_optional` viram vazio (R3 §1.7); o resultado gravado é o mesmo (`both`).
#[must_use]
pub fn side_from_modrinth(environment: Option<&str>) -> (Side, Option<SideNote>) {
    match environment {
        Some("client_only" | "singleplayer_only") => (Side::Client, None),
        Some("dedicated_server_only") => (Side::Server, None),
        Some(
            "client_and_server"
            | "server_only"
            | "client_or_server_prefers_both"
            | "client_only_server_optional"
            | "server_only_client_optional",
        ) => (Side::Both, None),
        Some("client_or_server") => (Side::Both, Some(SideNote::EitherSide)),
        _ => (Side::Both, Some(SideNote::Unknown)),
    }
}

/// Tabela `[download]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    /// Link direto (vazio no modo CurseForge).
    pub url: String,
    /// Formato do hash (`sha1`, `sha256`, `sha512`, `md5`, `murmur2`).
    pub hash_format: String,
    /// Hash do arquivo baixado.
    pub hash: String,
    /// Vazio (ou `url`) para link direto; `metadata:curseforge` para a CurseForge.
    pub mode: String,
}

/// Tabela `[option]`: arquivo opcional que o packwiz-installer pergunta ao jogador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModOption {
    /// Se é opcional.
    pub optional: bool,
    /// Texto mostrado ao jogador.
    pub description: String,
    /// Se vem marcado.
    pub default: bool,
}

/// Conteúdo de um `.pw.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct Metafile {
    /// Nome exibido (título do projeto; pode ter emoji).
    pub name: String,
    /// Nome do arquivo final, relativo à pasta do metafile.
    pub filename: String,
    /// Lado.
    pub side: Side,
    /// `pin = true`: a versão fica fixada.
    pub pin: bool,
    /// Como baixar.
    pub download: Download,
    /// `[update.<fonte>]`: dados para atualizar (`modrinth`, `curseforge`, `github`…).
    pub update: Option<BTreeMap<String, BTreeMap<String, Value>>>,
    /// `[option]`, se o arquivo for opcional para o jogador.
    pub option: Option<ModOption>,
}

/// Arquivo de uma versão do Modrinth, com os dados que o packwiz grava.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModrinthFile {
    /// Título do projeto.
    pub title: String,
    /// ID do projeto.
    pub project_id: String,
    /// ID da versão.
    pub version_id: String,
    /// Nome do arquivo.
    pub filename: String,
    /// Link do arquivo.
    pub url: String,
    /// Formato do hash escolhido (o Warden usa `sha512`).
    pub hash_format: HashFormat,
    /// Hash.
    pub hash: String,
}

/// Arquivo da CurseForge, com os dados que o packwiz grava.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurseForgeFile {
    /// Nome do projeto.
    pub name: String,
    /// ID do projeto.
    pub project_id: u32,
    /// ID do arquivo.
    pub file_id: u32,
    /// Nome do arquivo.
    pub filename: String,
    /// Formato do hash (`sha1`; `md5` ou `murmur2` se a API não der `sha1`).
    pub hash_format: HashFormat,
    /// Hash.
    pub hash: String,
}

impl Metafile {
    /// Metafile do Modrinth, como o `packwiz modrinth add` grava.
    #[must_use]
    pub fn modrinth(file: &ModrinthFile, side: Side) -> Self {
        Self {
            name: file.title.clone(),
            filename: file.filename.clone(),
            side,
            pin: false,
            download: Download {
                url: file.url.clone(),
                hash_format: file.hash_format.as_str().to_owned(),
                hash: file.hash.clone(),
                mode: String::new(),
            },
            update: Some(BTreeMap::from([(
                "modrinth".to_owned(),
                BTreeMap::from([
                    ("mod-id".to_owned(), Value::from(file.project_id.as_str())),
                    ("version".to_owned(), Value::from(file.version_id.as_str())),
                ]),
            )])),
            option: None,
        }
    }

    /// Metafile da CurseForge, como o `packwiz curseforge add` grava.
    #[must_use]
    pub fn curseforge(file: &CurseForgeFile, side: Side) -> Self {
        Self {
            name: file.name.clone(),
            filename: file.filename.clone(),
            side,
            pin: false,
            download: Download {
                url: String::new(),
                hash_format: file.hash_format.as_str().to_owned(),
                hash: file.hash.clone(),
                mode: MODE_CURSEFORGE.to_owned(),
            },
            update: Some(BTreeMap::from([(
                "curseforge".to_owned(),
                BTreeMap::from([
                    ("file-id".to_owned(), Value::from(file.file_id)),
                    ("project-id".to_owned(), Value::from(file.project_id)),
                ]),
            )])),
            option: None,
        }
    }

    /// Metafile de link direto, como o `packwiz url add <nome> <link>` grava: `filename` é o
    /// último trecho do caminho do link (decodificado), lado `both` e hash `sha256`.
    pub fn url(name: &str, url: &str, sha256: &str) -> Result<Self> {
        Ok(Self {
            name: name.to_owned(),
            filename: filename_from_url(url)?,
            side: Side::Both,
            pin: false,
            download: Download {
                url: url.to_owned(),
                hash_format: HashFormat::Sha256.as_str().to_owned(),
                hash: sha256.to_owned(),
                mode: String::new(),
            },
            update: None,
            option: None,
        })
    }

    /// Lê um `.pw.toml` como o `LoadMod` do packwiz (sem exigir fontes de atualização
    /// conhecidas: as desconhecidas ficam guardadas).
    pub fn parse(text: &str) -> Result<Parsed<Self>> {
        let doc = parse_document(text)?;
        let mut root = TableReader::new(doc.as_table(), "");
        let name = root.string("name")?.unwrap_or_default();
        let filename = root.string("filename")?.unwrap_or_default();
        let side = Side::parse(&root.string("side")?.unwrap_or_default());
        let pin = root.boolean("pin")?.unwrap_or_default();
        let mut unknown_keys = Vec::new();
        let mut download = Download {
            url: String::new(),
            hash_format: String::new(),
            hash: String::new(),
            mode: String::new(),
        };
        if let Some(mut table) = root.table("download")? {
            download.url = table.string("url")?.unwrap_or_default();
            download.hash_format = table.string("hash-format")?.unwrap_or_default();
            download.hash = table.string("hash")?.unwrap_or_default();
            download.mode = table.string("mode")?.unwrap_or_default();
            unknown_keys.extend(table.unknown_keys());
        }
        let update = root.nested_value_map("update")?;
        let option = match root.table("option")? {
            Some(mut table) => {
                let option = ModOption {
                    optional: table.boolean("optional")?.unwrap_or_default(),
                    description: table.string("description")?.unwrap_or_default(),
                    default: table.boolean("default")?.unwrap_or_default(),
                };
                unknown_keys.extend(table.unknown_keys());
                Some(option)
            }
            None => None,
        };
        unknown_keys.extend(root.unknown_keys());
        unknown_keys.sort();
        Ok(Parsed {
            value: Self {
                name,
                filename,
                side,
                pin,
                download,
                update,
                option,
            },
            unknown_keys,
            migrations: Vec::new(),
        })
    }

    /// O `.pw.toml` exatamente como o packwiz o gravaria.
    #[must_use]
    pub fn to_toml_string(&self) -> String {
        let mut fields = vec![
            ("name", text(&self.name)),
            ("filename", text(&self.filename)),
        ];
        if self.side != Side::Unset {
            fields.push(("side", text(self.side.as_str())));
        }
        if self.pin {
            fields.push(("pin", boolean(true)));
        }
        let mut download = Vec::new();
        if !self.download.url.is_empty() {
            download.push(("url", text(&self.download.url)));
        }
        download.push(("hash-format", text(&self.download.hash_format)));
        download.push(("hash", text(&self.download.hash)));
        if !self.download.mode.is_empty() {
            download.push(("mode", text(&self.download.mode)));
        }
        fields.push(("download", Node::Struct(download)));
        if let Some(update) = &self.update {
            fields.push((
                "update",
                Node::Value(Value::Table(
                    update
                        .iter()
                        .map(|(key, table)| (key.clone(), Value::Table(table.clone())))
                        .collect(),
                )),
            ));
        }
        if let Some(option) = &self.option {
            let mut table = vec![("optional", boolean(option.optional))];
            if !option.description.is_empty() {
                table.push(("description", text(&option.description)));
            }
            if option.default {
                table.push(("default", boolean(true)));
            }
            fields.push(("option", Node::Struct(table)));
        }
        encode_document(fields)
    }

    /// Formato do hash do download, se conhecido.
    pub fn hash_format(&self) -> Result<HashFormat> {
        HashFormat::parse(&self.download.hash_format)
    }

    /// Se o download é resolvido pela API da CurseForge.
    #[must_use]
    pub fn is_curseforge_mode(&self) -> bool {
        self.download.mode == MODE_CURSEFORGE
    }

    /// `[update.modrinth]`: ID do projeto e da versão.
    #[must_use]
    pub fn modrinth_ids(&self) -> Option<(&str, &str)> {
        let table = self.update.as_ref()?.get("modrinth")?;
        Some((
            table.get("mod-id")?.as_str()?,
            table.get("version")?.as_str()?,
        ))
    }

    /// `[update.curseforge]`: ID do projeto e do arquivo.
    #[must_use]
    pub fn curseforge_ids(&self) -> Option<(u32, u32)> {
        let table = self.update.as_ref()?.get("curseforge")?;
        let id = |key: &str| u32::try_from(table.get(key)?.as_integer()?).ok();
        Some((id("project-id")?, id("file-id")?))
    }
}

/// Nome do arquivo de um link, como o `url add` do packwiz: `path.Base` do caminho
/// decodificado (`%2B` vira `+`).
pub fn filename_from_url(url: &str) -> Result<String> {
    let invalid = |reason| Error::InvalidUrl {
        url: url.chars().take(2048).collect(),
        reason,
    };
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| invalid("falta o protocolo"))?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err(invalid("só links http e https são aceitos"));
    }
    let without_fragment = rest.split('#').next().unwrap_or_default();
    let without_query = without_fragment.split('?').next().unwrap_or_default();
    let path = without_query
        .find('/')
        .map_or("", |start| &without_query[start..]);
    let decoded = percent_decode(path).ok_or_else(|| invalid("escape `%` inválido no caminho"))?;
    let base = clean_base(&decoded);
    if base.is_empty() || base == "/" || base == "." || base == ".." {
        return Err(invalid("o link não termina num nome de arquivo"));
    }
    Ok(base)
}

/// `path.Base` do Go.
fn clean_base(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return if path.is_empty() {
            ".".to_owned()
        } else {
            "/".to_owned()
        };
    }
    trimmed.rsplit('/').next().unwrap_or(trimmed).to_owned()
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_digit(*bytes.get(index + 1)?)?;
            let low = hex_digit(*bytes.get(index + 2)?)?;
            out.push(high << 4 | low);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lado_do_modrinth_conforme_a_tabela_da_arquitetura() {
        let cases = [
            (Some("client_only"), Side::Client, None),
            (Some("singleplayer_only"), Side::Client, None),
            (Some("dedicated_server_only"), Side::Server, None),
            (Some("client_and_server"), Side::Both, None),
            (Some("server_only"), Side::Both, None),
            (Some("client_or_server_prefers_both"), Side::Both, None),
            (Some("client_only_server_optional"), Side::Both, None),
            (Some("server_only_client_optional"), Side::Both, None),
            (
                Some("client_or_server"),
                Side::Both,
                Some(SideNote::EitherSide),
            ),
            (Some("unknown"), Side::Both, Some(SideNote::Unknown)),
            (Some("valor_novo"), Side::Both, Some(SideNote::Unknown)),
            (None, Side::Both, Some(SideNote::Unknown)),
        ];
        for (environment, side, note) in cases {
            assert_eq!(
                side_from_modrinth(environment),
                (side, note),
                "{environment:?}"
            );
        }
    }

    #[test]
    fn lados_em_texto() {
        for (text, side) in [
            ("", Side::Unset),
            ("both", Side::Both),
            ("client", Side::Client),
            ("server", Side::Server),
            ("x", Side::Other("x".to_owned())),
        ] {
            assert_eq!(Side::parse(text), side);
            assert_eq!(side.as_str(), text);
        }
        assert!(Side::Unset.on_client() && Side::Unset.on_server());
        assert!(Side::Client.on_client() && !Side::Client.on_server());
        assert!(!Side::Server.on_client() && Side::Server.on_server());
        assert!(!Side::Other("x".to_owned()).on_client());
    }

    #[test]
    fn nome_do_arquivo_do_link() {
        let cases = [
            (
                "https://github.com/a/b/releases/download/2.4.2%2B1.21/placeholder-api-2.4.2%2B1.21.jar",
                "placeholder-api-2.4.2+1.21.jar",
            ),
            ("http://x.com/a/b.jar?x=1#f", "b.jar"),
            ("HTTPS://x.com/dir/", "dir"),
            ("https://x.com/a%20b.zip", "a b.zip"),
        ];
        for (url, expected) in cases {
            assert_eq!(filename_from_url(url).unwrap(), expected, "{url}");
        }
        for url in [
            "ftp://x.com/a.jar",
            "x.com/a.jar",
            "https://x.com",
            "https://x.com/",
            "https://x.com/a%2",
            "https://x.com/a%zz",
            "https://x.com/%ff",
        ] {
            assert!(filename_from_url(url).is_err(), "{url}");
        }
        assert_eq!(clean_base(""), ".");
        assert_eq!(clean_base("///"), "/");
    }

    #[test]
    fn opcao_e_campos_omitidos() {
        let mut metafile = Metafile::url("Coisa", "https://x.com/c.jar", "ab").unwrap();
        metafile.side = Side::Unset;
        metafile.option = Some(ModOption {
            optional: false,
            description: String::new(),
            default: false,
        });
        assert_eq!(
            metafile.to_toml_string(),
            "name = \"Coisa\"\nfilename = \"c.jar\"\n\n[download]\nurl = \"https://x.com/c.jar\"\n\
             hash-format = \"sha256\"\nhash = \"ab\"\n\n[option]\noptional = false\n"
        );
        assert!(metafile.modrinth_ids().is_none());
        assert!(metafile.curseforge_ids().is_none());
        assert!(!metafile.is_curseforge_mode());
        assert_eq!(metafile.hash_format().unwrap(), HashFormat::Sha256);
    }

    #[test]
    fn leitura_estrita_e_chaves_desconhecidas() {
        let parsed = Metafile::parse(
            "name = \"x\"\nfilename = \"x.jar\"\nalias = \"y\"\n[download]\nhash-format = \"sha1\"\n\
             hash = \"a\"\nsize = 3\n[option]\noptional = true\nz = 1\n[update.github]\nslug = \"a/b\"\n",
        )
        .unwrap();
        assert_eq!(parsed.unknown_keys, ["alias", "download.size", "option.z"]);
        assert_eq!(
            parsed.value.update.as_ref().unwrap()["github"]["slug"],
            Value::from("a/b")
        );
        for bad in [
            "pin = \"sim\"\n",
            "[download]\nhash = 1\n",
            "[option]\noptional = 1\n",
            "[update]\nmodrinth = 1\n",
            "side = 2\n",
        ] {
            assert!(Metafile::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn ids_da_curseforge_fora_da_faixa() {
        let mut metafile = Metafile::curseforge(
            &CurseForgeFile {
                name: "n".to_owned(),
                project_id: 1,
                file_id: 2,
                filename: "f.jar".to_owned(),
                hash_format: HashFormat::Sha1,
                hash: "h".to_owned(),
            },
            Side::Both,
        );
        assert_eq!(metafile.curseforge_ids(), Some((1, 2)));
        if let Some(update) = metafile.update.as_mut() {
            update
                .entry("curseforge".to_owned())
                .or_default()
                .insert("file-id".to_owned(), Value::from(-5_i64));
        }
        assert_eq!(metafile.curseforge_ids(), None);
    }
}
