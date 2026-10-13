//! Reconhece o que a pessoa escolheu pelo conteúdo, nunca pela extensão (SPEC T24, "Onde";
//! ARCHITECTURE §12.2).

use std::path::Path;

use crate::archive::{Source, display_name};
use crate::error::{Error, ImportErrorCode as Code, Result};
use crate::model::SourceKind;

/// O formato e a subpasta onde ele começa (zip de instância do Prism com a instância dentro de
/// uma pasta).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Detected {
    pub(crate) kind: SourceKind,
    pub(crate) root: String,
}

fn is_prism(source: &Source) -> bool {
    source.has_file("instance.cfg") && source.has_file("mmc-pack.json")
}

/// Reconhece uma origem já aberta.
pub(crate) fn detect_source(source: &mut Source, is_dir: bool) -> Result<Detected> {
    let found = |kind| {
        Ok(Detected {
            kind,
            root: String::new(),
        })
    };
    if is_dir {
        if source.has_file("pack.toml") {
            return found(SourceKind::Packwiz);
        }
        if is_prism(source) {
            return found(SourceKind::Prism);
        }
        if source.has_file("minecraftinstance.json") {
            return found(SourceKind::CurseforgeApp);
        }
    } else {
        if source.has_file("modrinth.index.json") {
            return found(SourceKind::Mrpack);
        }
        if source.has_file("manifest.json") {
            return found(SourceKind::CurseforgeZip);
        }
        if is_prism(source) {
            return found(SourceKind::Prism);
        }
        // Zip exportado pelo Prism/MultiMC: a instância fica dentro de uma pasta.
        let tops: std::collections::BTreeSet<String> = source
            .entries()?
            .into_iter()
            .filter_map(|entry| entry.name.split('/').next().map(str::to_owned))
            .collect();
        if let [top] = tops.into_iter().collect::<Vec<_>>().as_slice() {
            let has = |name: &str| source.has_file(&format!("{top}/{name}"));
            if has("instance.cfg") && has("mmc-pack.json") {
                return Ok(Detected {
                    kind: SourceKind::Prism,
                    root: top.clone(),
                });
            }
        }
    }
    Err(unknown(source.file_name()))
}

fn unknown(file_name: &str) -> Error {
    Error::new(
        Code::UnknownFormat,
        format!("{file_name} não é um modpack que o Warden importa"),
    )
    .with_param("name", file_name)
}

/// Abre a origem (zip ou pasta) e reconhece o formato.
///
/// Erros: [`Code::UnknownFormat`], [`Code::Unreadable`], [`Code::UnsafePath`],
/// [`Code::TooLarge`].
pub(crate) fn open(path: &Path) -> Result<(Source, Detected)> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        Error::new(
            Code::Unreadable,
            format!("{}: {}", display_name(path), error.kind()),
        )
        .with_param("name", display_name(path))
    })?;
    let is_dir = metadata.is_dir();
    let mut source = if is_dir {
        Source::open_dir(path)?
    } else {
        match Source::open_zip(path) {
            Ok(source) => source,
            // Arquivo que nem é zip: não é um modpack.
            Err(error) if error.code() == Code::Unreadable => {
                return Err(unknown(&display_name(path)).with_detail(error.to_string()));
            }
            Err(error) => return Err(error),
        }
    };
    let detected = detect_source(&mut source, is_dir)?;
    Ok((source, detected))
}

/// O formato da origem em `path` (arquivo ou pasta).
///
/// Erros: os de [`open`].
pub fn detect(path: &Path) -> Result<SourceKind> {
    open(path).map(|(_, detected)| detected.kind)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::fs::{self, File};
    use std::io::Write as _;

    use super::*;

    fn zip_with(path: &Path, names: &[&str]) {
        let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
        for name in names {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"{}").unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn reconhece_pelo_conteudo() {
        let temp = tempfile::tempdir().unwrap();
        let t = temp.path();
        // A extensão não importa.
        zip_with(&t.join("a.zip"), &["modrinth.index.json"]);
        assert_eq!(detect(&t.join("a.zip")).unwrap(), SourceKind::Mrpack);
        zip_with(&t.join("b.mrpack"), &["manifest.json", "overrides/x"]);
        assert_eq!(
            detect(&t.join("b.mrpack")).unwrap(),
            SourceKind::CurseforgeZip
        );
        zip_with(
            &t.join("c.zip"),
            &[
                "Inst/instance.cfg",
                "Inst/mmc-pack.json",
                "Inst/.minecraft/x",
            ],
        );
        let (_, detected) = open(&t.join("c.zip")).unwrap();
        assert_eq!(detected.kind, SourceKind::Prism);
        assert_eq!(detected.root, "Inst");
        zip_with(&t.join("d.zip"), &["outra/coisa.txt"]);
        assert_eq!(
            detect(&t.join("d.zip")).unwrap_err().code(),
            Code::UnknownFormat
        );
        fs::write(t.join("e.mrpack"), b"texto").unwrap();
        assert_eq!(
            detect(&t.join("e.mrpack")).unwrap_err().code(),
            Code::UnknownFormat
        );

        let dir = t.join("pasta");
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(detect(&dir).unwrap_err().code(), Code::UnknownFormat);
        fs::write(dir.join("minecraftinstance.json"), b"{}").unwrap();
        assert_eq!(detect(&dir).unwrap(), SourceKind::CurseforgeApp);
        fs::write(dir.join("instance.cfg"), b"").unwrap();
        fs::write(dir.join("mmc-pack.json"), b"{}").unwrap();
        assert_eq!(detect(&dir).unwrap(), SourceKind::Prism);
        fs::write(dir.join("pack.toml"), b"").unwrap();
        assert_eq!(detect(&dir).unwrap(), SourceKind::Packwiz);
    }
}
