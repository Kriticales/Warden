//! A instância de cada rodada da busca do culpado (ARCHITECTURE §8.2 item 11 e §9.7 item 9;
//! S-R5-3 §6 e §14 item 7; ROADMAP D-12).
//!
//! A busca materializa o pack inteiro **uma vez** numa base (`instances/<pack>/bisect-base/`,
//! com a [`crate::materialize`] de sempre) e, a cada rodada, recria `instances/<pack>/bisect/`
//! só com os itens ligados:
//!
//! - os jars de `mods/` entram por **link físico** a partir da base (0,1–0,2 s num pack de
//!   154 mods no spike); se o link falhar (outro volume, sistema de arquivos sem links), por
//!   cópia. No Windows, `CreateHardLinkW` (o `std::fs::hard_link`) só funciona no mesmo volume
//!   NTFS; apagar o link com a base aberta não mexe na base;
//! - todo o resto (configs, resource packs, `options.txt`) é **copiado** da base a cada rodada:
//!   o jogo regrava configs no lugar, e um link físico levaria a mudança para a base;
//! - o mundo de teste é copiado para `saves/` antes de abrir (sem o `session.lock`), e a pasta
//!   dele existe antes do jogo abrir (mundo inexistente = jogo parado sem log);
//! - `options.txt` com `onboardAccessibility:false` (1.21+) e `config/forge-client.toml` com
//!   `showLoadWarnings = false` (Forge 1.20+), quando pedidos.
//!
//! Nada aqui toca no pack, na instância de teste nem nos mundos dela: a base e a rodada são
//! pastas próprias da busca, apagadas no fim.

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::capture::set_option_lines;
use crate::error::{Error, Result};
use crate::manifest::Manifest;
use crate::materialize::InstanceDirs;

/// Pasta da base (o pack inteiro) dentro de `instances/<pack>/`.
pub const BISECT_BASE_DIR: &str = "bisect-base";
/// Pasta da rodada dentro de `instances/<pack>/`.
pub const BISECT_DIR: &str = "bisect";

/// Pastas da base e da rodada da busca de um pack.
#[must_use]
pub fn bisect_dirs(instance_root: &Path) -> (InstanceDirs, PathBuf) {
    let base = instance_root.join(BISECT_BASE_DIR);
    (
        InstanceDirs {
            game_dir: base.join("minecraft"),
            state_dir: base.join("state"),
        },
        instance_root.join(BISECT_DIR).join("minecraft"),
    )
}

/// O mundo copiado para a rodada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubsetWorld {
    /// A pasta do mundo original (na instância de teste; só lida).
    pub source: PathBuf,
    /// Nome da pasta em `saves/` da rodada.
    pub name: String,
}

/// O que preparar além dos arquivos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubsetOptions {
    /// Itens do pack (caminhos do índice, `mods/x.pw.toml`) que ficam **fora** nesta rodada.
    pub excluded: BTreeSet<String>,
    /// Gravar `onboardAccessibility:false` no `options.txt` (1.21+).
    pub disable_onboarding: bool,
    /// Gravar `showLoadWarnings = false` no `config/forge-client.toml` (Forge 1.20+).
    pub disable_forge_load_warnings: bool,
    /// O mundo de teste.
    pub world: Option<SubsetWorld>,
}

/// O que a preparação fez.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubsetReport {
    /// Arquivos ligados por link físico.
    pub linked: usize,
    /// Arquivos copiados (configs e o que não pôde ser ligado).
    pub copied: usize,
    /// Arquivos de itens desligados que ficaram de fora.
    pub skipped: usize,
}

/// Recria a pasta da rodada (`game_dir`) a partir da base, sem os itens de
/// [`SubsetOptions::excluded`]. Apaga o que a rodada anterior deixou.
pub fn materialize_subset(
    base: &InstanceDirs,
    game_dir: &Path,
    options: &SubsetOptions,
) -> Result<SubsetReport> {
    let manifest = Manifest::load(&base.state_dir)?;
    remove_dir(game_dir)?;
    fs::create_dir_all(game_dir).map_err(|error| Error::io("criar", game_dir, error))?;
    let mut report = SubsetReport::default();
    for (relative, entry) in &manifest.files {
        if options.excluded.contains(&entry.index_path) {
            report.skipped += 1;
            continue;
        }
        let Some(safe) = safe_relative(relative) else {
            continue;
        };
        let source = base.game_dir.join(&safe);
        let target = game_dir.join(&safe);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| Error::io("criar", parent, error))?;
        }
        if relative.starts_with("mods/") && link(&source, &target)? {
            report.linked += 1;
        } else {
            fs::copy(&source, &target).map_err(|error| Error::io("copiar", &source, error))?;
            report.copied += 1;
        }
    }
    if options.disable_onboarding {
        let path = game_dir.join("options.txt");
        let current = read_optional(&path)?;
        let text = set_option_lines(
            current.as_deref(),
            &[("onboardAccessibility".to_owned(), "false".to_owned())],
            "\n",
        );
        fs::write(&path, text).map_err(|error| Error::io("gravar", &path, error))?;
    }
    if options.disable_forge_load_warnings {
        let path = game_dir.join("config").join("forge-client.toml");
        let current = read_optional(&path)?;
        let text = set_forge_toml_value(
            &String::from_utf8_lossy(current.as_deref().unwrap_or_default()),
            "client",
            "showLoadWarnings",
            "false",
        );
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| Error::io("criar", parent, error))?;
        }
        fs::write(&path, text).map_err(|error| Error::io("gravar", &path, error))?;
    }
    if let Some(world) = &options.world {
        let name = safe_relative(&world.name)
            .filter(|name| name.components().count() == 1)
            .ok_or_else(|| Error::Internal(format!("nome de mundo inválido: {}", world.name)))?;
        let target = game_dir.join("saves").join(name);
        copy_world(&world.source, &target)?;
    }
    Ok(report)
}

/// Apaga a pasta (a rodada, ou a busca inteira no fim). Ausente não é erro.
pub fn remove_dir(dir: &Path) -> Result<()> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::io("apagar", dir, error)),
    }
}

/// Link físico; `Ok(false)` quando o sistema não deixa (aí o chamador copia).
fn link(source: &Path, target: &Path) -> Result<bool> {
    match fs::hard_link(source, target) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Err(Error::io("ligar", source, error)),
        Err(error) => {
            tracing::debug!(%error, "link físico indisponível; copiando");
            Ok(false)
        }
    }
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::io("ler", path, error)),
    }
}

/// Caminho relativo seguro (sem `..`, sem raiz).
fn safe_relative(relative: &str) -> Option<PathBuf> {
    let path = PathBuf::from(relative.replace('\\', "/"));
    path.components()
        .all(|part| matches!(part, std::path::Component::Normal(_)))
        .then_some(path)
}

/// Copia o mundo inteiro, sem o `session.lock` (o jogo cria o dele).
fn copy_world(source: &Path, target: &Path) -> Result<()> {
    remove_dir(target)?;
    fs::create_dir_all(target).map_err(|error| Error::io("criar", target, error))?;
    let entries = fs::read_dir(source).map_err(|error| Error::io("ler", source, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| Error::io("ler", source, error))?;
        let name = entry.file_name();
        if name == "session.lock" {
            continue;
        }
        let from = entry.path();
        let to = target.join(&name);
        let kind = entry
            .file_type()
            .map_err(|error| Error::io("ler", &from, error))?;
        if kind.is_dir() {
            copy_world(&from, &to)?;
        } else if kind.is_file() {
            fs::copy(&from, &to).map_err(|error| Error::io("copiar", &from, error))?;
        }
    }
    Ok(())
}

/// Troca (ou acrescenta) `key = value` na seção `[section]` de um TOML do Forge, mantendo o
/// resto do arquivo como está.
#[must_use]
pub fn set_forge_toml_value(text: &str, section: &str, key: &str, value: &str) -> String {
    let header = format!("[{section}]");
    let mut lines: Vec<String> = text.lines().map(ToOwned::to_owned).collect();
    let start = lines.iter().position(|line| line.trim() == header);
    let Some(start) = start else {
        let mut out = text.trim_end().to_owned();
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&header);
        out.push_str("\n\t");
        out.push_str(key);
        out.push_str(" = ");
        out.push_str(value);
        out.push('\n');
        return out;
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.trim_start().starts_with('['))
        .map_or(lines.len(), |(index, _)| index);
    let existing = (start + 1..end).find(|&index| {
        lines[index]
            .split_once('=')
            .is_some_and(|(name, _)| name.trim() == key)
    });
    match existing {
        Some(index) => {
            let indent: String = lines[index]
                .chars()
                .take_while(|character| character.is_whitespace())
                .collect();
            lines[index] = format!("{indent}{key} = {value}");
        }
        None => lines.insert(start + 1, format!("\t{key} = {value}")),
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::manifest::{EntryOrigin, ManifestEntry};
    use crate::test_support::temp_dir;

    fn entry(index_path: &str) -> ManifestEntry {
        ManifestEntry {
            sha256: "0".repeat(64),
            size: 1,
            modified_ns: 0,
            index_path: index_path.into(),
            index_hash: String::new(),
            expected: String::new(),
            origin: EntryOrigin::Pack,
        }
    }

    /// Uma base com dois mods, uma config e um `options.txt` do pack.
    fn base(root: &Path) -> InstanceDirs {
        let (base, _) = bisect_dirs(root);
        let mut manifest = Manifest::default();
        for (file, item, content) in [
            ("mods/a.jar", "mods/a.pw.toml", "jar a"),
            ("mods/b.jar", "mods/b.pw.toml", "jar b"),
            ("config/a.toml", "config/a.toml", "x = 1"),
            ("options.txt", "options.txt", "lang:pt_br\n"),
        ] {
            let path = base.game_dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, content).unwrap();
            manifest.files.insert(file.into(), entry(item));
        }
        fs::create_dir_all(&base.state_dir).unwrap();
        fs::write(
            base.state_dir.join(crate::manifest::MANIFEST_FILE),
            manifest.to_json().unwrap(),
        )
        .unwrap();
        base
    }

    fn world(root: &Path) -> PathBuf {
        let world = root.join("teste/minecraft/saves/Mundo de teste");
        fs::create_dir_all(world.join("region")).unwrap();
        fs::write(world.join("level.dat"), "nbt").unwrap();
        fs::write(world.join("region/r.0.0.mca"), "chunks").unwrap();
        fs::write(world.join("session.lock"), "lock").unwrap();
        world
    }

    /// CA-T25-08 (materialização): a rodada tem só os itens ligados (mods por link físico),
    /// `onboardAccessibility:false`, `showLoadWarnings = false` e o mundo copiado antes de
    /// abrir; a base e o mundo original não mudam.
    #[test]
    fn ca_t25_08_rodada_com_subconjunto_opcoes_e_mundo() {
        let root = temp_dir();
        let base = base(root.path());
        let source = world(root.path());
        let (_, game_dir) = bisect_dirs(root.path());
        let options = SubsetOptions {
            excluded: BTreeSet::from(["mods/b.pw.toml".to_owned()]),
            disable_onboarding: true,
            disable_forge_load_warnings: true,
            world: Some(SubsetWorld {
                source: source.clone(),
                name: "Mundo de teste".into(),
            }),
        };
        let report = materialize_subset(&base, &game_dir, &options).unwrap();
        assert_eq!(report.skipped, 1);
        assert_eq!(report.linked + report.copied, 3);
        assert!(game_dir.join("mods/a.jar").is_file());
        assert!(!game_dir.join("mods/b.jar").exists());
        assert_eq!(
            fs::read_to_string(game_dir.join("options.txt")).unwrap(),
            "lang:pt_br\nonboardAccessibility:false\n"
        );
        assert!(
            fs::read_to_string(game_dir.join("config/forge-client.toml"))
                .unwrap()
                .contains("showLoadWarnings = false")
        );
        let saves = game_dir.join("saves/Mundo de teste");
        assert_eq!(
            fs::read_to_string(saves.join("region/r.0.0.mca")).unwrap(),
            "chunks"
        );
        assert!(!saves.join("session.lock").exists());

        // O jogo regrava a config e o mundo da rodada: a base e o mundo original não mudam.
        fs::write(game_dir.join("config/a.toml"), "x = 2").unwrap();
        fs::write(saves.join("level.dat"), "outro").unwrap();
        assert_eq!(
            fs::read_to_string(base.game_dir.join("config/a.toml")).unwrap(),
            "x = 1"
        );
        assert_eq!(fs::read_to_string(source.join("level.dat")).unwrap(), "nbt");
        assert_eq!(
            fs::read_to_string(base.game_dir.join("options.txt")).unwrap(),
            "lang:pt_br\n"
        );

        // A rodada seguinte recria a pasta: o que a anterior deixou some.
        fs::write(game_dir.join("mods/sobra.jar"), "x").unwrap();
        let next = SubsetOptions::default();
        materialize_subset(&base, &game_dir, &next).unwrap();
        assert!(!game_dir.join("mods/sobra.jar").exists());
        assert!(game_dir.join("mods/b.jar").is_file());
        assert!(!game_dir.join("saves").exists());

        // Apagar a rodada não apaga a base (links físicos).
        remove_dir(&game_dir).unwrap();
        assert_eq!(
            fs::read_to_string(base.game_dir.join("mods/a.jar")).unwrap(),
            "jar a"
        );
        remove_dir(&game_dir).unwrap();
    }

    /// Os jars entram por link físico (mesmo volume): no Linux o arquivo da base passa a ter
    /// dois nomes; no Windows (NTFS), `CreateHardLinkW` faz o mesmo.
    #[test]
    fn jars_por_link_fisico() {
        let root = temp_dir();
        let base = base(root.path());
        let (_, game_dir) = bisect_dirs(root.path());
        let report = materialize_subset(&base, &game_dir, &SubsetOptions::default()).unwrap();
        assert_eq!(report.linked, 2);
        assert_eq!(report.copied, 2);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let meta = fs::metadata(base.game_dir.join("mods/a.jar")).unwrap();
            assert_eq!(meta.nlink(), 2);
        }
        #[cfg(windows)]
        {
            // O mesmo conteúdo pelos dois nomes; apagar o link deixa a base.
            fs::remove_file(game_dir.join("mods/a.jar")).unwrap();
            assert!(base.game_dir.join("mods/a.jar").is_file());
        }
    }

    #[test]
    fn toml_do_forge_troca_ou_acrescenta() {
        assert_eq!(
            set_forge_toml_value("", "client", "showLoadWarnings", "false"),
            "[client]\n\tshowLoadWarnings = false\n"
        );
        let text = "#x\n[client]\n\tshowLoadWarnings = true\n\talwaysSetupTerrainOffThread = false\n[outra]\nshowLoadWarnings = true\n";
        assert_eq!(
            set_forge_toml_value(text, "client", "showLoadWarnings", "false"),
            "#x\n[client]\n\tshowLoadWarnings = false\n\talwaysSetupTerrainOffThread = false\n[outra]\nshowLoadWarnings = true\n"
        );
        assert_eq!(
            set_forge_toml_value(
                "[client]\n\tfoo = 1\n",
                "client",
                "showLoadWarnings",
                "false"
            ),
            "[client]\n\tshowLoadWarnings = false\n\tfoo = 1\n"
        );
        assert_eq!(
            set_forge_toml_value("[general]\nx = 1", "client", "showLoadWarnings", "false"),
            "[general]\nx = 1\n\n[client]\n\tshowLoadWarnings = false\n"
        );
    }

    #[test]
    fn caminhos_e_mundo_invalidos() {
        assert!(safe_relative("../fora").is_none());
        assert!(safe_relative("/abs").is_none());
        assert!(safe_relative("mods/a.jar").is_some());
        let root = temp_dir();
        let base = base(root.path());
        let (_, game_dir) = bisect_dirs(root.path());
        let options = SubsetOptions {
            world: Some(SubsetWorld {
                source: root.path().join("x"),
                name: "../fora".into(),
            }),
            ..SubsetOptions::default()
        };
        assert!(materialize_subset(&base, &game_dir, &options).is_err());
        // Base sem manifesto: rodada vazia, sem erro.
        let empty = InstanceDirs {
            game_dir: root.path().join("vazia/minecraft"),
            state_dir: root.path().join("vazia/state"),
        };
        let report = materialize_subset(&empty, &game_dir, &SubsetOptions::default()).unwrap();
        assert_eq!(report, SubsetReport::default());
    }
}
