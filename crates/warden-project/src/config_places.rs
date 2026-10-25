//! Onde está cada config (SPEC T12, seção Configs com três colunas; ADR-0067): o pack, a
//! instância de teste e as configs pessoais, juntos numa lista só, com a diferença entre o teste
//! e o pack por chave.
//!
//! Por que juntar: os mods criam as configs na instância de teste quando o jogo abre, e elas
//! só vão para o pack se alguém trouxer. A seção que lia só a pasta do pack mostrava quase nada
//! (só o que o Warden escreveu ao criar o pack). Agora cada arquivo cai num de três grupos
//! ([`ConfigPlace`]):
//!
//! - **No pack**: está no pack (vai para os jogadores), com ou sem cópia no teste;
//! - **Só no teste**: o jogo criou na instância e nunca foi para o pack;
//! - **Só neste computador**: preferência pessoal (`options.txt`, minimapa…), que nunca vai.
//!
//! Funções:
//!
//! - [`overview`]: a lista, com quantas chaves (ou linhas) o teste tem diferentes do pack;
//! - [`compare_places`]: as diferenças de um arquivo, chave a chave, e os dois textos;
//! - [`bring_bytes`] e [`revert_bytes`]: o conteúdo novo do pack (Trazer para o pack) ou do
//!   teste (Voltar ao do pack), de chaves escolhidas ou do arquivo inteiro. Quem grava é o
//!   comando do app (pela [`crate::transaction::PackTransaction`] no pack).
//!
//! O caminho é o mesmo nas duas pastas (o índice do pack na raiz, como o editor já supõe).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;
use warden_configs::{ConfigDocument, ConfigEdit, ConfigFormat, ConfigValue, SemanticChange};

use crate::configs::{ConfigFile, ConfigOrigin, MAX_EDIT_BYTES, Scope, components, list_files};
use crate::{Error, ProjectErrorCode as Code, Result};

/// Acima disso (linhas × linhas), as linhas diferentes são contadas pela posição.
const MAX_LINE_DIFF_CELLS: usize = 4_000_000;

/// Grupo de uma config na seção.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConfigPlace {
    /// Está no pack: vai para os jogadores.
    Pack,
    /// Só na instância de teste: o jogo criou e ninguém trouxe.
    Test,
    /// Preferência pessoal: só neste computador.
    Personal,
}

/// Uma config da lista juntada.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlacedConfig {
    /// Caminho relativo (o mesmo no pack e na instância).
    pub path: String,
    /// Grupo.
    pub place: ConfigPlace,
    /// Existe no pack.
    pub in_pack: bool,
    /// Existe na instância de teste.
    pub in_test: bool,
    /// Tamanho em bytes (do teste, se existe lá; senão do pack).
    pub size: u32,
    /// Última alteração (do teste, se existe lá; senão do pack), em milissegundos Unix.
    pub modified_at_ms: Option<f64>,
    /// Parece binário (sem edição).
    pub binary: bool,
    /// O teste tem valores diferentes do pack (só com os dois; só formatação não conta).
    pub differs: bool,
    /// Quantas chaves diferem (formatos conhecidos) ou quantas linhas (texto). 0 se igual.
    pub differences: u32,
    /// Se as diferenças são por chave (formato conhecido) ou por linha.
    pub by_key: bool,
    /// O arquivo pode ir para o pack ("Trazer para o pack": fica numa pasta que o pack leva).
    pub bringable: bool,
}

/// A lista juntada da seção Configs.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigOverview {
    /// A instância de teste existe (só depois do primeiro teste).
    pub test_available: bool,
    /// As configs, em ordem de caminho.
    pub files: Vec<PlacedConfig>,
    /// Alguma das listas passou de [`crate::configs::MAX_LISTED_FILES`] e foi cortada.
    pub truncated: bool,
    /// Configs pessoais que ainda estão no pack (a migração do ADR-0067 pergunta antes de
    /// tirar).
    pub personal_in_pack: Vec<String>,
}

/// Uma chave diferente entre o teste e o pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KeyDifference {
    /// A chave, como o formulário mostra (`client.quality.distance`).
    pub key: String,
    /// Valor no pack (`None`: a chave não existe lá).
    pub pack: Option<String>,
    /// Valor no teste (`None`: a chave não existe lá).
    pub test: Option<String>,
    /// Existe nos dois lados: dá para trazer ou voltar só esta chave.
    pub per_key: bool,
}

/// As diferenças de um arquivo entre o teste e o pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigComparison {
    /// Caminho.
    pub path: String,
    /// Formato conhecido: as diferenças vêm por chave.
    pub by_key: bool,
    /// Chaves diferentes, na ordem do arquivo do pack (e as novas no fim).
    pub keys: Vec<KeyDifference>,
    /// Linhas diferentes (formato desconhecido; 0 nos conhecidos).
    pub lines: u32,
    /// Texto do pack (sem o BOM), se existe e cabe no editor.
    pub pack_text: Option<String>,
    /// Texto do teste (sem o BOM), se existe e cabe no editor.
    pub test_text: Option<String>,
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::new(Code::Internal, format!("ler a config: {error}"))),
    }
}

/// Sem o BOM do UTF-8.
fn without_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)
}

/// Texto que cabe no editor, sem o BOM.
fn text_of(bytes: Option<&[u8]>) -> Option<String> {
    let bytes = bytes?;
    if bytes.len() as u64 > MAX_EDIT_BYTES || bytes.contains(&0) {
        return None;
    }
    std::str::from_utf8(without_bom(bytes))
        .ok()
        .map(str::to_owned)
}

/// Valor para mostrar (`true`, `128`, `MEDIUM`, `[a, b]`).
fn shown(value: &ConfigValue) -> String {
    value.as_plain_text().unwrap_or_else(|| value.to_string())
}

/// As mudanças de valor do pack para o teste, num formato conhecido. `None`: formato
/// desconhecido ou um dos lados não se lê.
fn semantic(path: &str, pack: &[u8], test: &[u8]) -> Option<(ConfigFormat, Vec<SemanticChange>)> {
    let format = ConfigFormat::detect(Path::new(path), test)
        .or_else(|| ConfigFormat::detect(Path::new(path), pack))?;
    let diff = warden_configs::compare(format, pack, test).ok()?;
    Some((format, diff.changes))
}

/// Linhas do texto, sem o `\r` do fim.
fn lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect()
}

/// Quantas linhas do teste não estão no pack (maior subsequência comum; pela posição quando os
/// textos são grandes demais para isso). Pelo menos 1 se os textos diferem.
pub(crate) fn changed_lines(pack: &str, test: &str) -> u32 {
    if pack == test {
        return 0;
    }
    let old = lines(pack);
    let new = lines(test);
    let count = if old.len().saturating_mul(new.len()) <= MAX_LINE_DIFF_CELLS {
        let mut row = vec![0_u32; new.len() + 1];
        for line in &old {
            let mut diagonal = 0;
            for (index, other) in new.iter().enumerate() {
                let above = row[index + 1];
                row[index + 1] = if line == other {
                    diagonal + 1
                } else {
                    above.max(row[index])
                };
                diagonal = above;
            }
        }
        let common = row[new.len()] as usize;
        old.len().max(new.len()) - common
    } else {
        let same = old.iter().zip(&new).filter(|(a, b)| a == b).count();
        old.len().max(new.len()) - same
    };
    u32::try_from(count.max(1)).unwrap_or(u32::MAX)
}

/// Quantas diferenças e se são por chave. `(0, _)`: iguais nos valores.
fn differences(path: &str, pack: &[u8], test: &[u8]) -> (u32, bool) {
    if pack == test {
        return (0, true);
    }
    if let Some((_, changes)) = semantic(path, pack, test) {
        return (u32::try_from(changes.len()).unwrap_or(u32::MAX), true);
    }
    match (text_of(Some(pack)), text_of(Some(test))) {
        (Some(pack), Some(test)) => (changed_lines(&pack, &test), false),
        // Binário ou grande demais: diferente, sem contar.
        _ => (1, false),
    }
}

/// Se o arquivo pode estar no pack (pasta que o pack leva, fora do que o packwiz gerencia).
/// Configs de mundo (`saves/<mundo>/serverconfig/`) não: vão para `defaultconfigs/` pelo "O que
/// mudou durante o teste".
fn fits_pack(scope: &Scope, path: &str) -> bool {
    components(path).is_ok_and(|parts| {
        !parts
            .first()
            .is_some_and(|first| first.eq_ignore_ascii_case("saves"))
            && scope.is_listed(&parts)
    })
}

/// A lista juntada: o pack (`pack_root`), a instância (`game_dir`, a pasta do jogo) e as
/// configs pessoais (`personal`).
///
/// Erros: só os de disco da leitura dos arquivos que existem nos dois lados.
pub fn overview(
    pack_root: &Path,
    game_dir: &Path,
    personal: &BTreeSet<String>,
) -> Result<ConfigOverview> {
    let pack = list_files(pack_root, ConfigOrigin::Pack)?;
    let test = list_files(game_dir, ConfigOrigin::Instance)?;
    let scope = Scope::new(pack_root, ConfigOrigin::Pack);
    let in_pack: BTreeMap<&str, &ConfigFile> = pack
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let in_test: BTreeMap<&str, &ConfigFile> = test
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let paths: BTreeSet<&str> = in_pack.keys().chain(in_test.keys()).copied().collect();

    let mut files = Vec::with_capacity(paths.len());
    let mut personal_in_pack = Vec::new();
    for path in paths {
        let pack_file = in_pack.get(path).copied();
        let test_file = in_test.get(path).copied();
        let is_personal = personal.contains(path);
        if is_personal && pack_file.is_some() {
            personal_in_pack.push(path.to_owned());
        }
        let place = if is_personal {
            ConfigPlace::Personal
        } else if pack_file.is_some() {
            ConfigPlace::Pack
        } else {
            ConfigPlace::Test
        };
        let shown_file = test_file.or(pack_file);
        let Some(shown_file) = shown_file else {
            continue;
        };
        let (differences, by_key) = match (pack_file, test_file) {
            (Some(pack_file), Some(test_file))
                if !is_personal && (!pack_file.binary || !test_file.binary) =>
            {
                let pack_bytes = read_optional(&pack_root.join(path))?;
                let test_bytes = read_optional(&game_dir.join(path))?;
                match (pack_bytes, test_bytes) {
                    (Some(a), Some(b)) => differences(path, &a, &b),
                    _ => (0, true),
                }
            }
            _ => (0, true),
        };
        files.push(PlacedConfig {
            path: path.to_owned(),
            place,
            in_pack: pack_file.is_some(),
            in_test: test_file.is_some(),
            size: shown_file.size,
            modified_at_ms: shown_file.modified_at_ms,
            binary: shown_file.binary,
            differs: differences > 0,
            differences,
            by_key,
            bringable: !is_personal && fits_pack(&scope, path),
        });
    }
    files.sort_by_cached_key(|file| file.path.to_lowercase());
    Ok(ConfigOverview {
        test_available: test.available,
        files,
        truncated: pack.truncated || test.truncated,
        personal_in_pack,
    })
}

/// Confere o caminho pedido pela interface (relativo, sem `..`) e devolve o normalizado.
fn checked(path: &str) -> Result<String> {
    Ok(components(path)?.join("/"))
}

/// As diferenças de `path` entre o teste (`game_dir`) e o pack (`pack_root`).
///
/// Erros: `INVALID_INPUT` (caminho), disco.
pub fn compare_places(pack_root: &Path, game_dir: &Path, path: &str) -> Result<ConfigComparison> {
    let path = checked(path)?;
    let pack = read_optional(&pack_root.join(&path))?;
    let test = read_optional(&game_dir.join(&path))?;
    let mut comparison = ConfigComparison {
        path: path.clone(),
        by_key: false,
        keys: Vec::new(),
        lines: 0,
        pack_text: text_of(pack.as_deref()),
        test_text: text_of(test.as_deref()),
    };
    let (Some(pack), Some(test)) = (pack, test) else {
        return Ok(comparison);
    };
    if let Some((_, changes)) = semantic(&path, &pack, &test) {
        comparison.by_key = true;
        comparison.keys = changes
            .iter()
            .map(|change| match change {
                SemanticChange::Changed {
                    path,
                    before,
                    after,
                } => KeyDifference {
                    key: path.to_string(),
                    pack: Some(shown(before)),
                    test: Some(shown(after)),
                    per_key: true,
                },
                SemanticChange::Removed { path, value } => KeyDifference {
                    key: path.to_string(),
                    pack: Some(shown(value)),
                    test: None,
                    per_key: false,
                },
                SemanticChange::Added { path, value } => KeyDifference {
                    key: path.to_string(),
                    pack: None,
                    test: Some(shown(value)),
                    per_key: false,
                },
            })
            .collect();
    } else if let (Some(a), Some(b)) = (&comparison.pack_text, &comparison.test_text) {
        comparison.lines = changed_lines(a, b);
    }
    Ok(comparison)
}

/// Copia o valor de cada chave de `keys` de `from` para `into` (as duas existem nos dois
/// lados). Só os bytes desses valores mudam em `into`; o resto (comentários, ordem, outras
/// chaves) fica como está.
///
/// Erros: `CONFIG_NOT_EDITABLE` (formato desconhecido, chave que não existe dos dois lados, ou
/// a edição não pôde ser confirmada).
pub fn copy_keys(path: &str, from: &[u8], into: &[u8], keys: &[String]) -> Result<Vec<u8>> {
    let not_editable =
        |detail: String| Error::new(Code::ConfigNotEditable, detail).param("path", path.to_owned());
    let (format, changes) = semantic(path, into, from)
        .ok_or_else(|| not_editable(format!("{path}: formato sem leitura por chave")))?;
    let wanted: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
    let mut edits = Vec::new();
    let mut found = BTreeSet::new();
    for change in &changes {
        if let SemanticChange::Changed { path, after, .. } = change {
            let key = path.to_string();
            if wanted.contains(key.as_str()) {
                found.insert(key);
                edits.push(ConfigEdit::new(path.clone(), after.clone()));
            }
        }
    }
    if let Some(missing) = wanted.iter().find(|key| !found.contains(**key)) {
        return Err(not_editable(format!(
            "a chave {missing} não é diferente nos dois lados"
        )));
    }
    let document =
        ConfigDocument::parse(format, into).map_err(|error| not_editable(error.to_string()))?;
    let edited = document
        .apply(&edits)
        .map_err(|error| not_editable(error.to_string()))?;
    Ok(edited.into_bytes())
}

/// O conteúdo novo do **pack** para "Trazer para o pack": o arquivo inteiro do teste (sem
/// `keys`) ou só os valores dessas chaves (o resto do arquivo do pack fica igual).
///
/// Erros: `CONFIG_NOT_FOUND` (o teste não tem o arquivo; ou, com `keys`, o pack não tem),
/// `INVALID_INPUT` (caminho que o pack não leva), e os de [`copy_keys`].
pub fn bring_bytes(
    pack_root: &Path,
    game_dir: &Path,
    path: &str,
    keys: Option<&[String]>,
) -> Result<(String, Vec<u8>)> {
    let path = checked(path)?;
    if !fits_pack(&Scope::new(pack_root, ConfigOrigin::Pack), &path) {
        return Err(Error::new(
            Code::InvalidInput,
            format!("{path} não fica numa pasta que o pack leva"),
        )
        .param("field", "path"));
    }
    let test = read_optional(&game_dir.join(&path))?.ok_or_else(|| not_found(&path))?;
    let bytes = match keys {
        None => test,
        Some(keys) => {
            let pack = read_optional(&pack_root.join(&path))?.ok_or_else(|| not_found(&path))?;
            copy_keys(&path, &test, &pack, keys)?
        }
    };
    Ok((path, bytes))
}

/// O conteúdo novo do **teste** para "Voltar ao do pack": o arquivo inteiro do pack (sem
/// `keys`) ou só os valores dessas chaves.
///
/// Erros: `CONFIG_NOT_FOUND` e os de [`copy_keys`].
pub fn revert_bytes(
    pack_root: &Path,
    game_dir: &Path,
    path: &str,
    keys: Option<&[String]>,
) -> Result<(String, Vec<u8>)> {
    let path = checked(path)?;
    let pack = read_optional(&pack_root.join(&path))?.ok_or_else(|| not_found(&path))?;
    let bytes = match keys {
        None => pack,
        Some(keys) => {
            let test = read_optional(&game_dir.join(&path))?.ok_or_else(|| not_found(&path))?;
            copy_keys(&path, &pack, &test, keys)?
        }
    };
    Ok((path, bytes))
}

fn not_found(path: &str) -> Error {
    Error::new(Code::ConfigNotFound, format!("{path} não existe")).param("path", path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, text: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    const DH_PACK: &str = "[client.quality]\n\t#Distância\n\tlodDistance = 256\n\tvertical = \"MEDIUM\"\n[client.ssao]\n\tenableSsao = true\n";
    const DH_TEST: &str = "[client.quality]\n\t#Distância\n\tlodDistance = 128\n\tvertical = \"MEDIUM\"\n[client.ssao]\n\tenableSsao = false\n";

    struct Places {
        _dir: tempfile::TempDir,
        pack: std::path::PathBuf,
        game: std::path::PathBuf,
    }

    fn places() -> Places {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("pack");
        let game = dir.path().join("minecraft");
        write(&pack, "config/DistantHorizons.toml", DH_PACK);
        write(
            &pack,
            "config/crash_assistant/config.toml",
            "[general]\nx = 1\n",
        );
        write(&pack, "options.txt", "renderDistance:12\n");
        write(&game, "config/DistantHorizons.toml", DH_TEST);
        write(
            &game,
            "config/crash_assistant/config.toml",
            "[general]\nx = 1\n",
        );
        write(
            &game,
            "config/create-client.toml",
            "[client]\nenableTooltips = true\n",
        );
        write(&game, "options.txt", "renderDistance:16\n");
        write(
            &game,
            "config/jei/jei-client.ini",
            "[a]\nCenterSearch = false\n",
        );
        write(
            &pack,
            "config/jei/jei-client.ini",
            "[a]\nCenterSearch = true\n",
        );
        Places {
            _dir: dir,
            pack,
            game,
        }
    }

    fn personal() -> BTreeSet<String> {
        ["options.txt".to_owned()].into()
    }

    #[test]
    fn junta_pack_teste_e_pessoais_com_a_diferenca_por_chave() {
        let p = places();
        let view = overview(&p.pack, &p.game, &personal()).unwrap();
        let by_path: BTreeMap<&str, &PlacedConfig> = view
            .files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect();
        assert!(view.test_available);

        let dh = by_path["config/DistantHorizons.toml"];
        assert_eq!(dh.place, ConfigPlace::Pack);
        assert!(dh.in_pack && dh.in_test && dh.differs && dh.by_key);
        assert_eq!(dh.differences, 2);

        let ca = by_path["config/crash_assistant/config.toml"];
        assert_eq!((ca.place, ca.differs), (ConfigPlace::Pack, false));

        let create = by_path["config/create-client.toml"];
        assert_eq!(create.place, ConfigPlace::Test);
        assert!(!create.in_pack && create.bringable);

        let options = by_path["options.txt"];
        assert_eq!(options.place, ConfigPlace::Personal);
        assert!(!options.bringable && !options.differs);
        assert_eq!(view.personal_in_pack, ["options.txt"]);

        // .ini não tem leitura por chave: conta as linhas.
        let jei = by_path["config/jei/jei-client.ini"];
        assert!(jei.differs && !jei.by_key);
        assert_eq!(jei.differences, 1);
    }

    #[test]
    fn sem_instancia_so_o_pack() {
        let p = places();
        fs::remove_dir_all(&p.game).unwrap();
        let view = overview(&p.pack, &p.game, &BTreeSet::new()).unwrap();
        assert!(!view.test_available);
        assert!(
            view.files
                .iter()
                .all(|file| file.place == ConfigPlace::Pack)
        );
    }

    #[test]
    fn compara_por_chave_com_os_dois_textos() {
        let p = places();
        let comparison = compare_places(&p.pack, &p.game, "config/DistantHorizons.toml").unwrap();
        assert!(comparison.by_key);
        assert_eq!(
            comparison.keys,
            [
                KeyDifference {
                    key: "client.quality.lodDistance".into(),
                    pack: Some("256".into()),
                    test: Some("128".into()),
                    per_key: true,
                },
                KeyDifference {
                    key: "client.ssao.enableSsao".into(),
                    pack: Some("true".into()),
                    test: Some("false".into()),
                    per_key: true,
                },
            ]
        );
        assert_eq!(comparison.pack_text.as_deref(), Some(DH_PACK));
        assert_eq!(comparison.test_text.as_deref(), Some(DH_TEST));
        assert!(compare_places(&p.pack, &p.game, "../fora").is_err());
    }

    #[test]
    fn trazer_uma_chave_muda_so_o_valor_dela_no_pack() {
        let p = places();
        let keys = ["client.quality.lodDistance".to_owned()];
        let (path, bytes) =
            bring_bytes(&p.pack, &p.game, "config/DistantHorizons.toml", Some(&keys)).unwrap();
        assert_eq!(path, "config/DistantHorizons.toml");
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            DH_PACK.replace("lodDistance = 256", "lodDistance = 128")
        );
        // Chave igual nos dois lados: recusada, nada a trazer.
        let same = ["client.quality.vertical".to_owned()];
        assert!(bring_bytes(&p.pack, &p.game, "config/DistantHorizons.toml", Some(&same)).is_err());
    }

    #[test]
    fn trazer_o_arquivo_todo_e_voltar_ao_do_pack() {
        let p = places();
        let (_, bytes) = bring_bytes(&p.pack, &p.game, "config/create-client.toml", None).unwrap();
        assert_eq!(bytes, b"[client]\nenableTooltips = true\n");
        let keys = ["client.ssao.enableSsao".to_owned()];
        let (_, bytes) =
            revert_bytes(&p.pack, &p.game, "config/DistantHorizons.toml", Some(&keys)).unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            DH_TEST.replace("enableSsao = false", "enableSsao = true")
        );
        let (_, bytes) = revert_bytes(&p.pack, &p.game, "config/jei/jei-client.ini", None).unwrap();
        assert_eq!(bytes, b"[a]\nCenterSearch = true\n");
        // O teste não tem o arquivo: nada a trazer.
        assert_eq!(
            bring_bytes(&p.pack, &p.game, "config/nao-existe.toml", None)
                .unwrap_err()
                .code,
            Code::ConfigNotFound
        );
        // Mundo (saves/) não vai para o pack por aqui.
        write(&p.game, "saves/M/serverconfig/a.toml", "a = 1\n");
        assert_eq!(
            bring_bytes(&p.pack, &p.game, "saves/M/serverconfig/a.toml", None)
                .unwrap_err()
                .code,
            Code::InvalidInput
        );
    }

    #[test]
    fn linhas_diferentes() {
        assert_eq!(changed_lines("a\nb\nc", "a\nb\nc"), 0);
        assert_eq!(changed_lines("a\nb\nc", "a\nx\nc"), 1);
        assert_eq!(changed_lines("a\nb", "a\nb\nc\nd"), 2);
        assert_eq!(changed_lines("a\r\nb", "a\nb\n"), 1);
    }
}
