//! Cache do raio-x: um **arquivo binário único** por pack em `cache/xray/` (ARCHITECTURE §9.6;
//! S-R5-1 §7: o JSON por jar do protótipo deu 160–360 ms com cache).
//!
//! O que se lê de um jar só depende dos bytes dele e do loader, então cada leitura fica
//! guardada pelo hash do jar (SHA-256 do tamanho e do diretório central do zip, que traz o CRC
//! de cada arquivo; `pack::jar_hash`). Mesmo assim, abrir 300 jars a cada vez custaria mais que
//! o resto; por isso o arquivo também lembra o hash de cada item com o tamanho e a data:
//! enquanto os dois não mudam, o jar não é relido. Mudou, o hash é refeito e, se for o mesmo
//! conteúdo, a leitura guardada continua valendo.
//!
//! Formato (versão [`FORMAT`]): cabeçalho `WXRAY`, versão, loader; a tabela "caminho → tamanho,
//! data, hash"; as leituras por hash. Textos repetidos (nomes de classes, configs) vão uma vez
//! só e depois por número. Arquivo ilegível, truncado, de outra versão ou de outro loader é
//! ignorado (tudo é relido e o arquivo, regravado); falhar ao gravar não impede o raio-x.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use warden_jarmeta::Loader;

use crate::model::{
    At, Change, ChangeKind, DeclaredDep, DepKind, IssueKind, JarScan, MixinClass, ScanIssue,
    ScannedMod,
};

/// Versão do formato. Mudou o que se lê de um jar (regras de leitura, campos), sobe.
pub const FORMAT: u32 = 1;
const MAGIC: &[u8; 5] = b"WXRAY";
/// Nome da pasta do cache dentro de `cache/`.
pub const CACHE_DIR_NAME: &str = "xray";

/// O que o cache sabe de um arquivo pelo caminho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    /// Tamanho em bytes.
    pub len: u64,
    /// Data de modificação, em nanossegundos desde 1970.
    pub modified: u128,
}

/// O conteúdo do cache de um pack.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CacheData {
    /// Caminho (como o chamador o identifica) → carimbo e SHA-256.
    pub files: BTreeMap<String, (FileStamp, String)>,
    /// SHA-256 → leitura.
    pub scans: BTreeMap<String, JarScan>,
}

/// A pasta do cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrayCache {
    dir: PathBuf,
}

impl XrayCache {
    /// Cache na pasta dada (normalmente `<dados>/cache/xray`).
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Cache em `<cache>/xray`.
    #[must_use]
    pub fn in_cache_dir(cache_dir: &Path) -> Self {
        Self::new(cache_dir.join(CACHE_DIR_NAME))
    }

    /// Arquivo de um pack; `None` se a chave não é um nome seguro (letras, números, `-`, `_`).
    #[must_use]
    pub fn path_for(&self, key: &str) -> Option<PathBuf> {
        let safe = !key.is_empty()
            && key.len() <= 64
            && key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'));
        safe.then(|| self.dir.join(format!("{key}.bin")))
    }

    /// O cache do pack, se existir e servir para o loader.
    #[must_use]
    pub fn load(&self, key: &str, loader: Loader) -> Option<CacheData> {
        let bytes = std::fs::read(self.path_for(key)?).ok()?;
        decode(&bytes, loader)
    }

    /// Grava (atomicamente).
    ///
    /// # Errors
    ///
    /// Chave inválida ou falha de disco.
    pub fn store(
        &self,
        key: &str,
        loader: Loader,
        data: &CacheData,
    ) -> Result<(), warden_core::CoreError> {
        let path = self.path_for(key).ok_or_else(|| {
            warden_core::CoreError::io(
                "gravar",
                &self.dir,
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "chave do cache inválida"),
            )
        })?;
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| warden_core::CoreError::io("criar a pasta", &self.dir, e))?;
        warden_core::atomic_write(&path, &encode(loader, data))
    }
}

fn loader_code(loader: Loader) -> u8 {
    match loader {
        Loader::Fabric => 0,
        Loader::Quilt => 1,
        Loader::Forge => 2,
        Loader::NeoForge => 3,
    }
}

/// Codifica o cache.
#[must_use]
pub fn encode(loader: Loader, data: &CacheData) -> Vec<u8> {
    let mut w = Writer::default();
    w.out.extend_from_slice(MAGIC);
    w.u32(FORMAT);
    w.out.push(loader_code(loader));
    w.len(data.files.len());
    for (path, (stamp, sha)) in &data.files {
        w.str(path);
        w.u64(stamp.len);
        w.u128(stamp.modified);
        w.str(sha);
    }
    w.len(data.scans.len());
    for (sha, scan) in &data.scans {
        w.str(sha);
        w.len(scan.mods.len());
        for module in &scan.mods {
            w.module(module);
        }
    }
    w.out
}

/// Decodifica; `None` se o arquivo não serve (outro formato, outro loader, truncado).
#[must_use]
pub fn decode(bytes: &[u8], loader: Loader) -> Option<CacheData> {
    let mut r = Reader {
        bytes,
        pos: 0,
        strings: Vec::new(),
    };
    if r.take(MAGIC.len())? != MAGIC || r.u32()? != FORMAT || r.u8()? != loader_code(loader) {
        return None;
    }
    let mut data = CacheData::default();
    for _ in 0..r.len()? {
        let path = r.str()?;
        let len = r.u64()?;
        let modified = r.u128()?;
        let sha = r.str()?;
        data.files.insert(path, (FileStamp { len, modified }, sha));
    }
    for _ in 0..r.len()? {
        let sha = r.str()?;
        let count = r.len()?;
        let mut mods = Vec::with_capacity(count.min(4096));
        for _ in 0..count {
            mods.push(r.module()?);
        }
        data.scans.insert(sha, JarScan { mods });
    }
    (r.pos == bytes.len()).then_some(data)
}

#[derive(Default)]
struct Writer {
    out: Vec<u8>,
    strings: HashMap<String, u64>,
}

impl Writer {
    fn varint(&mut self, mut value: u64) {
        loop {
            let byte = u8::try_from(value & 0x7f).unwrap_or(0);
            value >>= 7;
            if value == 0 {
                self.out.push(byte);
                return;
            }
            self.out.push(byte | 0x80);
        }
    }

    fn u32(&mut self, value: u32) {
        self.varint(u64::from(value));
    }

    fn u64(&mut self, value: u64) {
        self.varint(value);
    }

    fn u128(&mut self, value: u128) {
        self.varint(u64::try_from(value >> 64).unwrap_or(0));
        self.varint(u64::try_from(value & u128::from(u64::MAX)).unwrap_or(0));
    }

    fn len(&mut self, value: usize) {
        self.varint(u64::try_from(value).unwrap_or(u64::MAX));
    }

    fn i32(&mut self, value: i32) {
        // zigzag
        let zigzag = (i64::from(value) << 1) ^ (i64::from(value) >> 63);
        self.varint(u64::try_from(zigzag).unwrap_or(0));
    }

    fn opt_i32(&mut self, value: Option<i32>) {
        match value {
            None => self.out.push(0),
            Some(v) => {
                self.out.push(1);
                self.i32(v);
            }
        }
    }

    fn bool(&mut self, value: bool) {
        self.out.push(u8::from(value));
    }

    /// Texto: 0 + bytes na primeira vez; depois, o número dele + 1.
    fn str(&mut self, text: &str) {
        if let Some(id) = self.strings.get(text).copied() {
            self.varint(id + 1);
            return;
        }
        let id = u64::try_from(self.strings.len()).unwrap_or(u64::MAX);
        self.strings.insert(text.to_owned(), id);
        self.varint(0);
        self.len(text.len());
        self.out.extend_from_slice(text.as_bytes());
    }

    fn strs(&mut self, list: &[String]) {
        self.len(list.len());
        for text in list {
            self.str(text);
        }
    }

    fn module(&mut self, m: &ScannedMod) {
        self.str(&m.path);
        self.str(&m.mod_id);
        self.str(&m.version);
        self.bool(m.connector);
        self.len(m.deps.len());
        for dep in &m.deps {
            self.str(&dep.id);
            self.bool(dep.kind == DepKind::Required);
        }
        self.strs(&m.packages);
        self.strs(&m.configs);
        self.len(m.classes.len());
        for class in &m.classes {
            self.class(class);
        }
        self.len(m.other_annotations.len());
        for (name, count) in &m.other_annotations {
            self.str(name);
            self.u32(*count);
        }
        self.len(m.issues.len());
        for issue in &m.issues {
            self.out.push(issue_code(issue.kind));
            self.str(&issue.file);
        }
        self.strs(&m.plugin_strings);
        self.strs(&m.disables);
        self.u32(m.nested_count);
        self.bool(m.mixin_squared);
        self.len(m.names.len());
        for (runtime, named) in &m.names {
            self.str(runtime);
            self.str(named);
        }
    }

    fn class(&mut self, c: &MixinClass) {
        self.str(&c.name);
        self.str(&c.config);
        self.bool(c.plugin);
        self.i32(c.priority);
        self.opt_i32(c.default_require);
        self.strs(&c.targets);
        self.len(c.changes.len());
        for change in &c.changes {
            self.out.push(change.kind.code());
            self.str(&change.target_class);
            self.str(&change.method);
            self.str(&change.method_named);
            self.str(&change.at.value);
            self.str(&change.at.target);
            self.str(&change.selector);
            self.opt_i32(change.require);
        }
        self.strs(&c.refs);
    }
}

const ISSUES: [IssueKind; 7] = [
    IssueKind::UnreadableJar,
    IssueKind::MissingConfig,
    IssueKind::InvalidConfig,
    IssueKind::MissingClass,
    IssueKind::UnreadableClass,
    IssueKind::NotAMixin,
    IssueKind::TooLarge,
];

fn issue_code(kind: IssueKind) -> u8 {
    u8::try_from(ISSUES.iter().position(|k| *k == kind).unwrap_or(0)).unwrap_or(0)
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    strings: Vec<String>,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let slice = self.bytes.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    fn varint(&mut self) -> Option<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.u8()?;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }

    fn u32(&mut self) -> Option<u32> {
        u32::try_from(self.varint()?).ok()
    }

    fn u64(&mut self) -> Option<u64> {
        self.varint()
    }

    fn u128(&mut self) -> Option<u128> {
        let high = u128::from(self.varint()?);
        let low = u128::from(self.varint()?);
        Some((high << 64) | low)
    }

    /// Tamanho de lista: nunca mais que os bytes que sobram (arquivo corrompido não aloca
    /// absurdos).
    fn len(&mut self) -> Option<usize> {
        let value = usize::try_from(self.varint()?).ok()?;
        (value <= self.bytes.len() - self.pos).then_some(value)
    }

    fn i32(&mut self) -> Option<i32> {
        let raw = self.varint()?;
        let value = i64::try_from(raw >> 1).ok()? ^ -i64::try_from(raw & 1).ok()?;
        i32::try_from(value).ok()
    }

    #[allow(clippy::option_option)] // fora: arquivo ruim; dentro: o valor opcional
    fn opt_i32(&mut self) -> Option<Option<i32>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(self.i32()?)),
            _ => None,
        }
    }

    fn bool(&mut self) -> Option<bool> {
        match self.u8()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    fn str(&mut self) -> Option<String> {
        let tag = self.varint()?;
        if tag == 0 {
            let len = self.len()?;
            let text = std::str::from_utf8(self.take(len)?).ok()?.to_owned();
            self.strings.push(text.clone());
            return Some(text);
        }
        self.strings.get(usize::try_from(tag - 1).ok()?).cloned()
    }

    fn strs(&mut self) -> Option<Vec<String>> {
        let count = self.len()?;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(self.str()?);
        }
        Some(out)
    }

    fn module(&mut self) -> Option<ScannedMod> {
        let path = self.str()?;
        let mod_id = self.str()?;
        let version = self.str()?;
        let connector = self.bool()?;
        let mut deps = Vec::new();
        for _ in 0..self.len()? {
            let id = self.str()?;
            let kind = if self.bool()? {
                DepKind::Required
            } else {
                DepKind::Optional
            };
            deps.push(DeclaredDep { id, kind });
        }
        let packages = self.strs()?;
        let configs = self.strs()?;
        let mut classes = Vec::new();
        for _ in 0..self.len()? {
            classes.push(self.class()?);
        }
        let mut other_annotations = BTreeMap::new();
        for _ in 0..self.len()? {
            let name = self.str()?;
            other_annotations.insert(name, self.u32()?);
        }
        let mut issues = Vec::new();
        for _ in 0..self.len()? {
            let kind = *ISSUES.get(usize::from(self.u8()?))?;
            issues.push(ScanIssue {
                kind,
                file: self.str()?,
            });
        }
        let plugin_strings = self.strs()?;
        let disables = self.strs()?;
        let nested_count = self.u32()?;
        let mixin_squared = self.bool()?;
        let mut names = Vec::new();
        for _ in 0..self.len()? {
            let runtime = self.str()?;
            names.push((runtime, self.str()?));
        }
        Some(ScannedMod {
            path,
            mod_id,
            version,
            connector,
            deps,
            packages,
            configs,
            classes,
            other_annotations,
            issues,
            plugin_strings,
            disables,
            nested_count,
            mixin_squared,
            names,
        })
    }

    fn class(&mut self) -> Option<MixinClass> {
        let name = self.str()?;
        let config = self.str()?;
        let plugin = self.bool()?;
        let priority = self.i32()?;
        let default_require = self.opt_i32()?;
        let targets = self.strs()?;
        let mut changes = Vec::new();
        for _ in 0..self.len()? {
            let kind = ChangeKind::from_code(self.u8()?)?;
            let target_class = self.str()?;
            let method = self.str()?;
            let method_named = self.str()?;
            let value = self.str()?;
            let target = self.str()?;
            let selector = self.str()?;
            let require = self.opt_i32()?;
            changes.push(Change {
                kind,
                target_class,
                method,
                method_named,
                at: At { value, target },
                selector,
                require,
            });
        }
        let refs = self.strs()?;
        Some(MixinClass {
            name,
            config,
            plugin,
            priority,
            default_require,
            targets,
            changes,
            refs,
        })
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn sample() -> CacheData {
        let change = Change {
            kind: ChangeKind::Redirect,
            target_class: "net/minecraft/class_1".into(),
            method: "method_2".into(),
            method_named: "tick".into(),
            at: At {
                value: "INVOKE".into(),
                target: "La;b()V".into(),
            },
            selector: String::new(),
            require: Some(-3),
        };
        let module = ScannedMod {
            path: "META-INF/jars/a.jar".into(),
            mod_id: "lithium".into(),
            version: "0.11.4".into(),
            connector: true,
            deps: vec![
                DeclaredDep {
                    id: "x".into(),
                    kind: DepKind::Required,
                },
                DeclaredDep {
                    id: "y".into(),
                    kind: DepKind::Optional,
                },
            ],
            packages: vec!["a/b".into()],
            configs: vec!["l.mixins.json".into()],
            classes: vec![MixinClass {
                name: "a/b/M".into(),
                config: "l.mixins.json".into(),
                plugin: true,
                priority: i32::MIN,
                default_require: None,
                targets: vec!["net/minecraft/class_1".into()],
                changes: vec![
                    change.clone(),
                    Change {
                        require: None,
                        ..change
                    },
                ],
                refs: vec!["a/b/M".into()],
            }],
            other_annotations: [("Accessor".to_owned(), 7)].into(),
            issues: vec![ScanIssue {
                kind: IssueKind::TooLarge,
                file: "x".into(),
            }],
            plugin_strings: vec!["ç".into()],
            disables: vec!["lithium:mixin.a".into()],
            nested_count: 3,
            mixin_squared: true,
            names: vec![("method_2".into(), "tick".into())],
        };
        let mut data = CacheData::default();
        data.files.insert(
            "mods/a.jar".into(),
            (
                FileStamp {
                    len: 10,
                    modified: u128::MAX - 1,
                },
                "f".repeat(64),
            ),
        );
        data.scans.insert(
            "f".repeat(64),
            JarScan {
                mods: vec![module.clone(), module],
            },
        );
        data
    }

    #[test]
    fn ida_e_volta() {
        let data = sample();
        let bytes = encode(Loader::Fabric, &data);
        assert_eq!(decode(&bytes, Loader::Fabric), Some(data));
        assert_eq!(decode(&bytes, Loader::Forge), None, "outro loader");
        for cut in 0..bytes.len() {
            assert_eq!(
                decode(&bytes[..cut], Loader::Fabric),
                None,
                "truncado em {cut}"
            );
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(decode(&longer, Loader::Fabric), None);
    }

    #[test]
    fn grava_le_e_recusa_chave_estranha() {
        let dir = tempfile::tempdir().unwrap();
        let cache = XrayCache::in_cache_dir(dir.path());
        assert_eq!(cache.load("pack_1", Loader::Fabric), None);
        cache.store("pack_1", Loader::Fabric, &sample()).unwrap();
        assert_eq!(cache.load("pack_1", Loader::Fabric), Some(sample()));
        for bad in ["", "../x", "a/b", "a.b", &"a".repeat(65)] {
            assert_eq!(cache.path_for(bad), None, "{bad}");
            assert!(cache.store(bad, Loader::Fabric, &sample()).is_err());
        }
    }

    proptest! {
        #[test]
        fn bytes_quaisquer_nao_derrubam(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = decode(&bytes, Loader::Fabric);
            let mut with_header = b"WXRAY\x01\x00".to_vec();
            with_header.extend(&bytes);
            let _ = decode(&with_header, Loader::Fabric);
        }

        #[test]
        fn inteiros_ida_e_volta(value in any::<i32>(), big in any::<u128>()) {
            let mut w = Writer::default();
            w.i32(value);
            w.u128(big);
            let mut r = Reader { bytes: &w.out, pos: 0, strings: Vec::new() };
            prop_assert_eq!(r.i32(), Some(value));
            prop_assert_eq!(r.u128(), Some(big));
        }
    }
}
