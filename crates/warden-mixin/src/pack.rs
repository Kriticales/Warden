//! O raio-x de um pack: lê os jars em paralelo (com o cache), cruza e monta o relatório de
//! cada mod ("O que este mod altera no jogo", SPEC T07).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use warden_jarmeta::{Limits, Loader};

use crate::cache::{CacheData, FileStamp, XrayCache};
use crate::model::{ChangeKind, JarScan, ScanIssue};
use crate::overlap::{CrossContext, ItemScan, Overlap, Risk, cross, kept_mods};
use crate::scan::scan_jar_file;

/// Um jar do pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackJar {
    /// Item do pack (caminho do metafile ou do arquivo, relativo ao pack).
    pub item: String,
    /// Onde o jar está.
    pub path: PathBuf,
}

/// Números da leitura (registro e medição de desempenho).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    /// Jars do pack.
    pub jars: usize,
    /// Jars lidos agora (os outros vieram do cache).
    pub scanned: usize,
    /// Jars cujo hash ([`jar_hash`]) foi calculado agora.
    pub hashed: usize,
    /// Tempo total da leitura, em milissegundos.
    pub millis: u64,
}

/// Os jars lidos.
#[derive(Debug, Clone, Default)]
pub struct PackScan {
    /// Item → leitura, na ordem dos itens.
    pub items: Vec<(String, JarScan)>,
    /// Números.
    pub stats: ScanStats,
}

/// Onde guardar o cache do pack.
#[derive(Debug, Clone, Copy)]
pub struct CacheSlot<'a> {
    /// A pasta.
    pub cache: &'a XrayCache,
    /// O nome do arquivo (o id do pack).
    pub key: &'a str,
}

fn stamp(path: &Path) -> Option<FileStamp> {
    let meta = std::fs::metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_file)?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    Some(FileStamp {
        len: meta.len(),
        modified,
    })
}

/// O hash que identifica o conteúdo de um jar: SHA-256 do tamanho e do diretório central do zip.
///
/// O diretório central lista cada arquivo do jar com o CRC-32 e os tamanhos; mudou qualquer byte
/// de qualquer arquivo, ele muda. Ler só o fim do jar (dezenas de KB) em vez do jar inteiro é o
/// que deixa a primeira leitura de um pack de 900 MB abaixo dos 3 s (sem SHA na CPU, o hash do
/// arquivo inteiro custava 2 s). Zip64 ou fim de arquivo estranho: o hash do arquivo inteiro.
fn jar_hash(path: &Path, limits: &Limits) -> Option<String> {
    use std::io::{Seek, SeekFrom};
    const EOCD: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let tail_len = len.min(65_557);
    file.seek(SeekFrom::Start(len - tail_len)).ok()?;
    let mut tail = Vec::with_capacity(usize::try_from(tail_len).unwrap_or(0));
    (&mut file).take(tail_len).read_to_end(&mut tail).ok()?;
    let directory = tail
        .windows(4)
        .rposition(|w| w == EOCD)
        .and_then(|at| tail.get(at + 12..at + 20))
        .map(|fields| {
            let size = u64::from(u32::from_le_bytes([
                fields[0], fields[1], fields[2], fields[3],
            ]));
            let offset = u64::from(u32::from_le_bytes([
                fields[4], fields[5], fields[6], fields[7],
            ]));
            (size, offset)
        })
        .filter(|(size, offset)| {
            *size != u64::from(u32::MAX)
                && *offset != u64::from(u32::MAX)
                && offset.saturating_add(*size) <= len
                && *size <= limits.max_nested_jar_bytes
        });
    let mut hasher = Sha256::new();
    hasher.update(len.to_le_bytes());
    if let Some((size, offset)) = directory {
        file.seek(SeekFrom::Start(offset)).ok()?;
        let mut directory = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
        (&mut file).take(size).read_to_end(&mut directory).ok()?;
        hasher.update(b"cd");
        hasher.update(&directory);
    } else {
        file.seek(SeekFrom::Start(0)).ok()?;
        let mut reader =
            std::io::BufReader::with_capacity(1 << 16, file.take(limits.max_file_bytes));
        let mut buf = vec![0u8; 1 << 16];
        loop {
            let read = reader.read(&mut buf).ok()?;
            if read == 0 {
                break;
            }
            hasher.update(&buf[..read]);
        }
    }
    Some(
        hasher
            .finalize()
            .iter()
            .fold(String::with_capacity(64), |mut out, byte| {
                use std::fmt::Write as _;
                let _ = write!(out, "{byte:02x}");
                out
            }),
    )
}

/// Um jar lido por uma thread.
struct Done {
    index: usize,
    key: String,
    stamp: Option<FileStamp>,
    sha: Option<String>,
    scan: JarScan,
    scanned: bool,
    hashed: bool,
}

/// Lê os jars do pack em paralelo (até 8 threads), reaproveitando o cache.
#[must_use]
pub fn scan_pack(
    jars: &[PackJar],
    loader: Loader,
    slot: Option<CacheSlot<'_>>,
    limits: &Limits,
) -> PackScan {
    let started = Instant::now();
    let with_cache = slot.is_some();
    let old = slot
        .and_then(|s| s.cache.load(s.key, loader))
        .unwrap_or_default();
    let threads = std::thread::available_parallelism()
        .map_or(2, std::num::NonZero::get)
        .clamp(1, 8);
    let chunk = jars.len().div_ceil(threads).max(1);

    let results: Vec<Done> = std::thread::scope(|scope| {
        let handles: Vec<_> = jars
            .chunks(chunk)
            .enumerate()
            .map(|(part, list)| {
                let old = &old;
                scope.spawn(move || {
                    let mut out = Vec::new();
                    for (offset, jar) in list.iter().enumerate() {
                        let index = part * chunk + offset;
                        let key = jar.item.clone();
                        let stamp = stamp(&jar.path);
                        let mut hashed = false;
                        let sha = match (stamp, old.files.get(&key)) {
                            (Some(now), Some((before, sha))) if now == *before => Some(sha.clone()),
                            // Sem cache, o hash não serve para nada.
                            (Some(_), _) if with_cache => {
                                hashed = true;
                                jar_hash(&jar.path, limits)
                            }
                            _ => None,
                        };
                        let cached = sha.as_ref().and_then(|s| old.scans.get(s)).cloned();
                        let (scan, scanned) = match cached {
                            Some(scan) => (scan, false),
                            None => (scan_jar_file(&jar.path, loader, limits), true),
                        };
                        out.push(Done {
                            index,
                            key,
                            stamp,
                            sha,
                            scan,
                            scanned,
                            hashed,
                        });
                    }
                    out
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    });

    let mut stats = ScanStats {
        jars: jars.len(),
        ..ScanStats::default()
    };
    let mut fresh = CacheData::default();
    let mut ordered: Vec<Option<JarScan>> = vec![None; jars.len()];
    let mut changed = false;
    for done in results {
        stats.scanned += usize::from(done.scanned);
        stats.hashed += usize::from(done.hashed);
        changed |= done.scanned || done.hashed;
        if let (Some(stamp), Some(sha)) = (done.stamp, &done.sha) {
            fresh.files.insert(done.key, (stamp, sha.clone()));
            fresh.scans.insert(sha.clone(), done.scan.clone());
        }
        if let Some(slot) = ordered.get_mut(done.index) {
            *slot = Some(done.scan);
        }
    }
    // Jar que saiu do pack sai do cache.
    changed |= fresh.files.len() != old.files.len();
    if let Some(slot) = slot
        && changed
        && let Err(error) = slot.cache.store(slot.key, loader, &fresh)
    {
        tracing::warn!(%error, "não deu para gravar o cache do raio-x");
    }
    stats.millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    PackScan {
        items: jars
            .iter()
            .zip(ordered)
            .map(|(jar, scan)| (jar.item.clone(), scan.unwrap_or_default()))
            .collect(),
        stats,
    }
}

/// O raio-x do pack inteiro.
#[derive(Debug, Clone, Default)]
pub struct PackXray {
    /// Os jars lidos.
    pub scan: PackScan,
    /// Sobreposições.
    pub overlaps: Vec<Overlap>,
    /// O pack tem o `MixinSquared`: um mod pode cancelar mixins de outro, e o raio-x perde
    /// confiança.
    pub mixin_squared: bool,
    /// Opções desligadas no cruzamento (pack + descritores).
    pub disabled: BTreeSet<String>,
}

impl PackXray {
    /// Cruza os jars lidos.
    #[must_use]
    pub fn build(scan: PackScan, context: &CrossContext) -> Self {
        let items: Vec<ItemScan<'_>> = scan
            .items
            .iter()
            .map(|(item, jar)| ItemScan {
                item,
                mods: &jar.mods,
            })
            .collect();
        let kept = kept_mods(&items);
        let overlaps = cross(&kept, context);
        let mixin_squared = kept.iter().any(|k| k.module.mixin_squared);
        let mut disabled = context.disabled.clone();
        for k in &kept {
            disabled.extend(k.module.disables.iter().cloned());
        }
        drop(items);
        Self {
            scan,
            overlaps,
            mixin_squared,
            disabled,
        }
    }

    /// "O que este mod altera no jogo" de um item (`None`: o item não é um jar do pack).
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn mod_report(&self, item: &str) -> Option<ModReport> {
        let (_, jar) = self.scan.items.iter().find(|(i, _)| i == item)?;
        let items: Vec<ItemScan<'_>> = self
            .scan
            .items
            .iter()
            .map(|(item, jar)| ItemScan {
                item,
                mods: &jar.mods,
            })
            .collect();
        let kept = kept_mods(&items);
        let mine: Vec<_> = kept.iter().filter(|k| k.item == item).collect();

        let mut configs = Vec::new();
        let mut classes = 0usize;
        let mut disabled_classes = 0usize;
        let mut priorities = BTreeSet::new();
        let mut counts: BTreeMap<ChangeKind, u32> = BTreeMap::new();
        let mut points: BTreeMap<(String, String), PointRow> = BTreeMap::new();
        let mut connector = false;
        let mut mod_ids = Vec::new();
        for k in &mine {
            connector |= k.module.connector;
            if !k.module.classes.is_empty() || k.module.path.is_empty() {
                mod_ids.push(k.module.mod_id.clone());
            }
            configs.extend(k.module.configs.iter().cloned());
            for class in &k.module.classes {
                if crate::options::is_disabled(
                    &k.module.mod_id,
                    &class.name,
                    &class.config,
                    &self.disabled,
                ) {
                    disabled_classes += 1;
                    continue;
                }
                classes += 1;
                priorities.insert(class.priority);
                for change in &class.changes {
                    *counts.entry(change.kind).or_default() += 1;
                    let row = points
                        .entry((change.target_class.clone(), change.method.clone()))
                        .or_insert_with(|| PointRow {
                            target_class: change.target_class.clone(),
                            method: change.method.clone(),
                            class_named: String::new(),
                            method_named: change.method_named.clone(),
                            kinds: Vec::new(),
                            others: Vec::new(),
                            risk: None,
                        });
                    if !row.kinds.contains(&change.kind) {
                        row.kinds.push(change.kind);
                    }
                }
            }
        }
        let mut overlaps: Vec<Overlap> = self
            .overlaps
            .iter()
            .filter(|o| o.items.iter().any(|i| i == item))
            .cloned()
            .collect();
        overlaps.sort_by(|a, b| {
            b.risk
                .cmp(&a.risk)
                .then_with(|| a.target_class.cmp(&b.target_class))
                .then_with(|| a.method.cmp(&b.method))
        });
        for overlap in &overlaps {
            if let Some(row) =
                points.get_mut(&(overlap.target_class.clone(), overlap.method.clone()))
            {
                row.others = overlap
                    .items
                    .iter()
                    .filter(|i| *i != item)
                    .cloned()
                    .collect();
                row.risk = Some(overlap.risk);
                row.class_named.clone_from(&overlap.class_named);
                if !overlap.method_named.is_empty() {
                    row.method_named.clone_from(&overlap.method_named);
                }
            }
        }
        // Nomes legíveis que os refmaps do pack ensinam, também para os pontos sem disputa.
        let mut names: BTreeMap<&str, &str> = BTreeMap::new();
        for k in &kept {
            for (runtime, named) in &k.module.names {
                names.entry(runtime.as_str()).or_insert(named.as_str());
            }
        }
        for row in points.values_mut() {
            if row.class_named.is_empty()
                && let Some(named) = names.get(row.target_class.as_str())
            {
                row.class_named = (*named).to_owned();
            }
            if let Some(named) = names.get(row.method.as_str()) {
                row.method_named = (*named).to_owned();
            }
        }
        let mut points: Vec<PointRow> = points.into_values().collect();
        points.sort_by(|a, b| {
            b.risk
                .cmp(&a.risk)
                .then_with(|| b.others.len().cmp(&a.others.len()))
                .then_with(|| (&a.target_class, &a.method).cmp(&(&b.target_class, &b.method)))
        });
        let shared = points.iter().filter(|p| !p.others.is_empty()).count();
        let issues: Vec<ScanIssue> = jar
            .mods
            .iter()
            .flat_map(|m| m.issues.iter())
            .filter(|i| !matches!(i.kind, crate::model::IssueKind::MissingConfig))
            .cloned()
            .collect();
        configs.sort();
        configs.dedup();
        Some(ModReport {
            item: item.to_owned(),
            mod_ids,
            connector,
            configs,
            class_count: count(classes),
            disabled_class_count: count(disabled_classes),
            change_count: counts.values().copied().sum(),
            counts: counts
                .into_iter()
                .map(|(kind, count)| KindCount { kind, count })
                .collect(),
            point_count: count(points.len()),
            shared_point_count: count(shared),
            priorities: priorities.into_iter().collect(),
            overlaps,
            points,
            issues,
            mixin_squared: self.mixin_squared,
        })
    }
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Quantas alterações de um tipo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KindCount {
    /// Tipo.
    pub kind: ChangeKind,
    /// Quantas.
    pub count: u32,
}

/// Uma parte do jogo que o mod altera (classe + método), para "Ver todas as alterações".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PointRow {
    /// Classe (em tempo de execução).
    pub target_class: String,
    /// Método (em tempo de execução).
    pub method: String,
    /// Classe com nome legível, quando conhecido.
    pub class_named: String,
    /// Método com nome legível, quando conhecido.
    pub method_named: String,
    /// Tipos de alteração que o mod faz ali.
    pub kinds: Vec<ChangeKind>,
    /// Outros itens que alteram o mesmo método.
    pub others: Vec<String>,
    /// Risco da sobreposição, se houver outros.
    pub risk: Option<Risk>,
}

/// "O que este mod altera no jogo".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModReport {
    /// O item.
    pub item: String,
    /// Os mods do item que alteram o jogo (o de cima e os embutidos com mixins).
    pub mod_ids: Vec<String>,
    /// Mod Fabric num pack Forge/NeoForge (Sinytra Connector): nomes não comparados com os dos
    /// mods nativos.
    pub connector: bool,
    /// Configs de mixin.
    pub configs: Vec<String>,
    /// Classes de mixin ativas.
    pub class_count: u32,
    /// Classes de mixin desligadas por opções.
    pub disabled_class_count: u32,
    /// Alterações.
    pub change_count: u32,
    /// Alterações por tipo.
    pub counts: Vec<KindCount>,
    /// Partes do jogo (métodos) alteradas.
    pub point_count: u32,
    /// Partes também alteradas por outros mods.
    pub shared_point_count: u32,
    /// Prioridades das classes (menor aplica antes; padrão 1000).
    pub priorities: Vec<i32>,
    /// Sobreposições com outros mods, do maior risco para o menor.
    pub overlaps: Vec<Overlap>,
    /// Todas as partes alteradas.
    pub points: Vec<PointRow>,
    /// Problemas de leitura (sem as configs que ficam noutro jar).
    pub issues: Vec<ScanIssue>,
    /// O pack tem o `MixinSquared`.
    pub mixin_squared: bool,
}
