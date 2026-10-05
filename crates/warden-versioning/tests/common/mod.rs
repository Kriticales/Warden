//! Auxiliares dos testes: packs em pastas temporárias, com os arquivos de controle que a P1-01
//! gera (`.gitignore` e `.gitattributes`) e metafiles escritos pelo codificador da
//! `warden-packwiz` (provado byte a byte contra o packwiz real).
//!
//! Nenhum teste toca em repositório real do dono: tudo nasce em `tempfile::tempdir()`.

#![allow(dead_code, unreachable_pub)]
// cada arquivo de teste usa uma parte
// Auxiliares de teste: falhar com pânico reprova o teste que chamou.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};
use warden_packwiz::hygiene::{GITATTRIBUTES_TEMPLATE, gitignore_template};
use warden_packwiz::{CurseForgeFile, HashFormat, Metafile, ModrinthFile, PackManifest, Side};
use warden_versioning::{Identity, InitialPoint, Moment, PackRepo};

/// 2026-10-01T12:00:00-03:00.
pub const T0: i64 = 1_790_866_800;

pub fn at(minutes: i64) -> Moment {
    Moment::new(T0 + minutes * 60, -180)
}

pub fn identity() -> Identity {
    Identity::for_pack("Autor de Teste", None)
}

pub struct TestPack {
    pub dir: tempfile::TempDir,
    pub root: PathBuf,
}

impl TestPack {
    /// Pasta com `pack.toml`, `index.toml`, `.gitignore` e `.gitattributes`, sem repositório.
    /// O nome da pasta tem espaço e acento, como `Documentos\Warden\Meu pack ação`.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Meu pack ação");
        fs::create_dir(&root).unwrap();
        let pack = Self { dir, root };
        pack.write(".gitignore", gitignore_template().as_bytes());
        pack.write(".gitattributes", GITATTRIBUTES_TEMPLATE.as_bytes());
        pack.write_pack_toml("0.1.0", "1.21.1", &[("fabric", "0.16.9")]);
        pack.write("index.toml", b"hash-format = \"sha256\"\n");
        pack
    }

    /// Pack já com repositório e ponto inicial.
    pub fn versioned() -> (Self, PackRepo) {
        let pack = Self::new();
        let repo = PackRepo::init(&pack.root, InitialPoint::Created, &identity(), at(0)).unwrap();
        (pack, repo)
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    pub fn read(&self, relative: &str) -> Vec<u8> {
        fs::read(self.path(relative)).unwrap()
    }

    pub fn remove(&self, relative: &str) {
        fs::remove_file(self.path(relative)).unwrap();
    }

    pub fn exists(&self, relative: &str) -> bool {
        self.path(relative).exists()
    }

    pub fn write_pack_toml(&self, version: &str, minecraft: &str, loaders: &[(&str, &str)]) {
        let mut manifest = PackManifest::new("Pack de Teste", minecraft);
        version.clone_into(&mut manifest.version);
        "Autor de Teste".clone_into(&mut manifest.author);
        let versions = manifest.versions.get_or_insert_with(BTreeMap::new);
        for (loader, loader_version) in loaders {
            versions.insert((*loader).to_owned(), (*loader_version).to_owned());
        }
        self.write("pack.toml", manifest.to_toml_string().as_bytes());
    }

    /// Metafile do Modrinth em `mods/<slug>.pw.toml`.
    pub fn add_modrinth(&self, slug: &str, title: &str, project: &str, version: &str, side: Side) {
        self.add_modrinth_in("mods", slug, title, project, version, side);
    }

    pub fn add_modrinth_in(
        &self,
        folder: &str,
        slug: &str,
        title: &str,
        project: &str,
        version: &str,
        side: Side,
    ) {
        let file = ModrinthFile {
            title: title.to_owned(),
            project_id: project.to_owned(),
            version_id: version.to_owned(),
            filename: format!("{slug}-{version}.jar"),
            url: format!("https://cdn.modrinth.com/data/{project}/versions/{version}/{slug}.jar"),
            hash_format: HashFormat::Sha512,
            hash: "ab".repeat(64),
        };
        let metafile = Metafile::modrinth(&file, side);
        self.write(
            &format!("{folder}/{slug}.pw.toml"),
            metafile.to_toml_string().as_bytes(),
        );
    }

    /// Metafile da CurseForge em `mods/<slug>.pw.toml`.
    pub fn add_curseforge(&self, slug: &str, name: &str, project: u32, file: u32, side: Side) {
        let file = CurseForgeFile {
            name: name.to_owned(),
            project_id: project,
            file_id: file,
            filename: format!("{slug}-{file}.jar"),
            hash_format: HashFormat::Sha1,
            hash: "cd".repeat(20),
        };
        let metafile = Metafile::curseforge(&file, side);
        self.write(
            &format!("mods/{slug}.pw.toml"),
            metafile.to_toml_string().as_bytes(),
        );
    }

    /// SHA-256 de todos os arquivos da pasta (fora `.git`), inclusive os ignorados.
    pub fn hashes(&self) -> BTreeMap<String, String> {
        let mut files = BTreeMap::new();
        collect(&self.root, &self.root, &mut files);
        files
    }

    /// Pastas (fora `.git`), para conferir que nenhuma sobrou ou sumiu.
    pub fn dirs(&self) -> Vec<String> {
        let mut dirs = Vec::new();
        collect_dirs(&self.root, &self.root, &mut dirs);
        dirs.sort();
        dirs
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

fn collect(root: &Path, dir: &Path, files: &mut BTreeMap<String, String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue; // apagado no meio da listagem (antivírus)
        };
        if kind.is_dir() {
            collect(root, &path, files);
        } else if let Ok(bytes) = fs::read(&path) {
            files.insert(relative(root, &path), hex::encode(Sha256::digest(bytes)));
        }
    }
}

fn collect_dirs(root: &Path, dir: &Path, dirs: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            dirs.push(relative(root, &path));
            collect_dirs(root, &path, dirs);
        }
    }
}

/// Repositório git aberto direto pela `git2`, para conferir o que a crate gravou.
pub fn raw(pack: &TestPack) -> git2::Repository {
    git2::Repository::open(&pack.root).unwrap()
}

/// Arquivos alterados segundo o `git status` da libgit2 (ignorados fora).
pub fn git_status(pack: &TestPack) -> Vec<String> {
    let repo = raw(pack);
    let mut options = git2::StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut options)).unwrap();
    statuses
        .iter()
        .map(|entry| entry.path().unwrap().to_owned())
        .collect()
}
