//! O índice em memória de uma origem: as entradas de cada arquivo e a busca sobre elas.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::path::Path;

use warden_core::CancellationToken;

use super::extract::{IndexEntry, MAX_INDEXED_BYTES, extract};
use super::matcher::{CompiledQuery, EntryMatch, marked, new_matcher};
use super::mod_id::mod_id;
use super::model::{
    ConfigSearchGroup, ConfigSearchHit, ConfigSearchMod, ConfigSearchResults, SearchQuery,
    SourceFile,
};

/// De quantas em quantas entradas a busca confere o cancelamento.
const CANCEL_CHECK_EVERY: usize = 512;

/// Um arquivo com resultados: a melhor pontuação, o caminho, o arquivo e as entradas que
/// casaram.
type Scored<'a> = (
    u32,
    &'a str,
    &'a FileIndex,
    Vec<(&'a IndexEntry, EntryMatch)>,
);

/// Um arquivo indexado.
#[derive(Debug, Clone)]
pub(crate) struct FileIndex {
    pub(crate) mod_id: Option<String>,
    size: u64,
    modified_ms: Option<f64>,
    content_hash: u64,
    pub(crate) entries: Vec<IndexEntry>,
}

impl FileIndex {
    fn same_stamp(&self, file: &SourceFile) -> bool {
        self.size == file.size && self.modified_ms == file.modified_ms
    }
}

/// O que uma sincronização precisa fazer.
#[derive(Debug, Default)]
pub(crate) struct SyncPlan {
    /// Arquivos a (re)ler.
    pub(crate) read: Vec<SourceFile>,
    /// Arquivos que saíram da origem.
    pub(crate) remove: Vec<String>,
}

/// Um arquivo lido, pronto para entrar no índice (`None`: sai do índice).
pub(crate) struct ReadFile {
    pub(crate) path: String,
    pub(crate) index: Option<FileIndex>,
}

/// O índice de uma origem.
#[derive(Debug, Default)]
pub(crate) struct ConfigIndex {
    files: BTreeMap<String, FileIndex>,
    entries: usize,
}

impl ConfigIndex {
    pub(crate) fn entry_count(&self) -> usize {
        self.entries
    }

    /// Compara a lista da origem com o índice: o que é novo ou mudou de tamanho ou de data é
    /// relido; o que está em `forced` (avisado pelo observador) é relido mesmo com a mesma
    /// data, porque a data do disco pode não mudar numa gravação rápida.
    pub(crate) fn plan(&self, files: &[SourceFile], forced: &BTreeSet<String>) -> SyncPlan {
        let listed: HashMap<&str, &SourceFile> = files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect();
        let mut plan = SyncPlan::default();
        for file in files {
            let current = self.files.get(&file.path);
            let changed = current.is_none_or(|indexed| !indexed.same_stamp(file));
            if changed || forced.contains(&file.path) {
                plan.read.push(file.clone());
            }
        }
        plan.remove = self
            .files
            .keys()
            .filter(|path| !listed.contains_key(path.as_str()))
            .cloned()
            .collect();
        plan
    }

    /// Aplica arquivos lidos. Devolve quantos mudaram de fato (um arquivo relido com o mesmo
    /// conteúdo não conta).
    pub(crate) fn apply(&mut self, batch: Vec<ReadFile>) -> usize {
        let mut changed = 0;
        for read in batch {
            match read.index {
                Some(index) => {
                    let same = self
                        .files
                        .get(&read.path)
                        .is_some_and(|old| old.content_hash == index.content_hash);
                    if !same {
                        changed += 1;
                    }
                    self.insert(read.path, index);
                }
                None => {
                    if self.remove(&read.path) {
                        changed += 1;
                    }
                }
            }
        }
        changed
    }

    fn insert(&mut self, path: String, index: FileIndex) {
        self.entries += index.entries.len();
        if let Some(old) = self.files.insert(path, index) {
            self.entries -= old.entries.len();
        }
    }

    /// Tira um arquivo. Devolve `true` se ele estava no índice.
    pub(crate) fn remove(&mut self, path: &str) -> bool {
        match self.files.remove(path) {
            Some(old) => {
                self.entries -= old.entries.len();
                true
            }
            None => false,
        }
    }

    /// Esvazia o índice (a origem sumiu).
    pub(crate) fn clear(&mut self) -> usize {
        let count = self.files.len();
        self.files.clear();
        self.entries = 0;
        count
    }

    /// Busca. `None` se `cancel` foi acionado no meio (uma busca mais nova chegou).
    pub(crate) fn search(
        &self,
        query: &SearchQuery,
        cancel: &CancellationToken,
    ) -> Option<ConfigSearchResults> {
        let mut results = ConfigSearchResults {
            groups: Vec::new(),
            total_hits: 0,
            total_files: 0,
            truncated: false,
            mods: Vec::new(),
        };
        // Sem texto, só o filtro "Só o que mudou do padrão" lista alguma coisa: todas as chaves
        // alteradas.
        let compiled = CompiledQuery::new(&query.text);
        if compiled.is_none() && !query.only_changed {
            return Some(results);
        }
        let mut matcher = new_matcher();
        let mut mods: BTreeMap<&str, u32> = BTreeMap::new();
        let mut scored: Vec<Scored<'_>> = Vec::new();
        let mut checked = 0usize;
        for (path, file) in &self.files {
            let mut hits = Vec::new();
            for entry in &file.entries {
                checked += 1;
                if checked.is_multiple_of(CANCEL_CHECK_EVERY) && cancel.is_cancelled() {
                    return None;
                }
                if query.only_changed && !entry.changed {
                    continue;
                }
                let found = match &compiled {
                    Some(compiled) => compiled.matches(entry, &mut matcher),
                    None => Some(EntryMatch::whole_key()),
                };
                if let Some(found) = found {
                    hits.push((entry, found));
                }
            }
            if hits.is_empty() {
                continue;
            }
            let count = u32::try_from(hits.len()).unwrap_or(u32::MAX);
            if let Some(id) = file.mod_id.as_deref() {
                *mods.entry(id).or_default() += count;
            }
            if query
                .mod_id
                .as_deref()
                .is_some_and(|wanted| file.mod_id.as_deref() != Some(wanted))
            {
                continue;
            }
            let best = hits.iter().map(|(_, found)| found.score).max().unwrap_or(0);
            scored.push((best, path, file, hits));
        }
        if cancel.is_cancelled() {
            return None;
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        results.mods = mods
            .into_iter()
            .map(|(id, hits)| ConfigSearchMod {
                id: id.to_owned(),
                hits,
            })
            .collect();
        let mut budget = query.max_hits;
        for (_, path, file, mut hits) in scored {
            let total = u32::try_from(hits.len()).unwrap_or(u32::MAX);
            results.total_hits = results.total_hits.saturating_add(total);
            results.total_files += 1;
            if budget == 0 {
                results.truncated = true;
                continue;
            }
            // Os melhores do arquivo, mostrados na ordem das linhas.
            hits.sort_by(|a, b| b.1.score.cmp(&a.1.score).then(a.0.line.cmp(&b.0.line)));
            let keep = hits.len().min(query.max_hits_per_file).min(budget);
            if keep < hits.len() {
                results.truncated = true;
            }
            hits.truncate(keep);
            hits.sort_by_key(|(entry, _)| entry.line);
            budget -= keep;
            results.groups.push(ConfigSearchGroup {
                path: path.to_owned(),
                mod_id: file.mod_id.clone(),
                hits: hits
                    .iter()
                    .map(|(entry, found)| to_hit(entry, found))
                    .collect(),
                total,
            });
        }
        Some(results)
    }
}

fn to_hit(entry: &IndexEntry, found: &EntryMatch) -> ConfigSearchHit {
    ConfigSearchHit {
        line: entry.line,
        field: found.field,
        key: entry.key.as_ref().map(|key| marked(key, &found.key, false)),
        label: entry
            .label
            .as_ref()
            .map(|label| marked(label, &found.label, false)),
        value: entry
            .value
            .as_ref()
            .map(|value| marked(value, &found.value, true)),
        comment: entry
            .comment
            .as_ref()
            .filter(|_| !found.comment.is_empty())
            .map(|comment| marked(comment, &found.comment, true)),
        text: entry
            .text
            .as_ref()
            .map(|text| marked(text, &found.text, true)),
        changed: entry.changed,
    }
}

/// Lê um arquivo do disco para o índice. `None` quando ele não deve estar no índice (binário,
/// grande demais, sumiu ou não pôde ser lido).
pub(crate) fn read_file(root: &Path, file: &SourceFile) -> Option<FileIndex> {
    if file.binary || file.size > MAX_INDEXED_BYTES {
        return None;
    }
    let bytes = std::fs::read(root.join(&file.path)).ok()?;
    if bytes.len() as u64 > MAX_INDEXED_BYTES {
        return None;
    }
    Some(index_bytes(file, &bytes))
}

/// As entradas de um arquivo já lido.
pub(crate) fn index_bytes(file: &SourceFile, bytes: &[u8]) -> FileIndex {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    let (_, entries) = extract(&file.path, bytes);
    FileIndex {
        mod_id: mod_id(&file.path),
        size: file.size,
        modified_ms: file.modified_ms,
        content_hash: hasher.finish(),
        entries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(path: &str, size: u64, modified: f64) -> SourceFile {
        SourceFile {
            path: path.into(),
            size,
            modified_ms: Some(modified),
            binary: false,
        }
    }

    fn lines(count: usize, prefix: &str) -> String {
        use std::fmt::Write as _;

        (0..count).fold(String::new(), |mut text, n| {
            let _ = writeln!(text, "{prefix}{n} = {n}");
            text
        })
    }

    fn index_with(files: &[(&str, &str)]) -> ConfigIndex {
        let mut index = ConfigIndex::default();
        let batch = files
            .iter()
            .map(|(path, text)| ReadFile {
                path: (*path).into(),
                index: Some(index_bytes(
                    &source(path, text.len() as u64, 1.0),
                    text.as_bytes(),
                )),
            })
            .collect();
        index.apply(batch);
        index
    }

    fn search(index: &ConfigIndex, query: &SearchQuery) -> ConfigSearchResults {
        index.search(query, &CancellationToken::new()).unwrap()
    }

    #[test]
    fn plano_le_o_novo_e_o_mudado_e_tira_o_que_sumiu() {
        let index = index_with(&[("config/a.toml", "a = 1\n"), ("config/b.toml", "b = 1\n")]);
        let files = [
            source("config/a.toml", 6, 1.0),
            source("config/b.toml", 6, 2.0),
            source("config/c.toml", 6, 1.0),
        ];
        let plan = index.plan(&files, &BTreeSet::new());
        let read: Vec<&str> = plan.read.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(read, ["config/b.toml", "config/c.toml"]);
        assert!(plan.remove.is_empty());

        let forced = BTreeSet::from(["config/a.toml".to_owned()]);
        let plan = index.plan(&files[..1], &forced);
        assert_eq!(plan.read.len(), 1);
        assert_eq!(plan.remove, ["config/b.toml"]);
    }

    #[test]
    fn aplicar_conta_so_o_que_mudou() {
        let mut index = index_with(&[("config/a.toml", "a = 1\n")]);
        assert_eq!((index.files.len(), index.entry_count()), (1, 1));
        let same = index_bytes(&source("config/a.toml", 6, 9.0), b"a = 1\n");
        assert_eq!(
            index.apply(vec![ReadFile {
                path: "config/a.toml".into(),
                index: Some(same),
            }]),
            0
        );
        let other = index_bytes(&source("config/a.toml", 12, 9.0), b"a = 1\nb = 2\n");
        let gone = ReadFile {
            path: "config/x.toml".into(),
            index: None,
        };
        let changed = index.apply(vec![
            ReadFile {
                path: "config/a.toml".into(),
                index: Some(other),
            },
            gone,
        ]);
        assert_eq!(changed, 1);
        assert_eq!(index.entry_count(), 2);
        assert!(index.remove("config/a.toml"));
        assert_eq!((index.files.len(), index.entry_count()), (0, 0));
        assert_eq!(index.clear(), 0);
    }

    #[test]
    fn agrupa_por_arquivo_ordena_e_filtra_por_mod() {
        let index = index_with(&[
            (
                "config/create-common.toml",
                "#Desliga perto do spawn\ndisableWorldGen = false\n",
            ),
            (
                "config/supplementaries-common.toml",
                "bird_spawn = true\njar_spawn_rate = 0.05\nother = 1\n",
            ),
            ("options.txt", "simulationDistance:8\nlang:pt_br\n"),
        ]);
        let all = search(&index, &SearchQuery::new("spawn"));
        let paths: Vec<&str> = all.groups.iter().map(|g| g.path.as_str()).collect();
        // A chave vale mais que o comentário: o arquivo com "spawn" na chave vem antes.
        assert_eq!(
            paths,
            [
                "config/supplementaries-common.toml",
                "config/create-common.toml"
            ]
        );
        assert_eq!((all.total_hits, all.total_files), (3, 2));
        let lines: Vec<u32> = all.groups[0].hits.iter().map(|h| h.line).collect();
        assert_eq!(lines, [1, 2]);
        let create = &all.groups[1].hits[0];
        assert_eq!(create.comment.as_ref().unwrap().marked(), ["spawn"]);
        assert_eq!(create.value.as_ref().unwrap().plain(), "false");
        let mods: Vec<(&str, u32)> = all.mods.iter().map(|m| (m.id.as_str(), m.hits)).collect();
        assert_eq!(mods, [("create", 1), ("supplementaries", 2)]);

        let only = search(
            &index,
            &SearchQuery::new("spawn").with_mod(Some("create".into())),
        );
        assert_eq!(only.groups.len(), 1);
        assert_eq!(only.total_hits, 1);
        assert_eq!(
            only.mods.len(),
            2,
            "o filtro não esconde os outros mods da lista"
        );

        assert!(search(&index, &SearchQuery::new("  ")).groups.is_empty());
        assert_eq!(
            search(&index, &SearchQuery::new("pt_br")).groups[0].path,
            "options.txt"
        );
    }

    #[test]
    fn so_o_que_mudou_do_padrao() {
        // C-06 (CA-T12-09 na busca): só as chaves diferentes do padrão que o arquivo informa.
        let index = index_with(&[
            (
                "config/create-server.toml",
                "[a]\n\t# Default: 20\n\t# Range: > 5\n\tspawnTimer = 40\n\t# Default: 9\n\tspawnMax = 9\n\tspawnFree = 3\n",
            ),
            (
                "config/badmobs.cfg",
                "general {\n    # Liga [default: false]\n    B:spawnRemove=true\n}\n",
            ),
            ("options.txt", "spawnSomething:8\n"),
        ]);
        let all = search(&index, &SearchQuery::new("spawn"));
        assert_eq!(all.total_hits, 5);
        let changed = search(&index, &SearchQuery::new("spawn").only_changed(true));
        let keys: Vec<(&str, String, bool)> = changed
            .groups
            .iter()
            .flat_map(|g| {
                g.hits
                    .iter()
                    .map(|h| (g.path.as_str(), h.key.as_ref().unwrap().plain(), h.changed))
            })
            .collect();
        assert_eq!(
            keys,
            [
                ("config/badmobs.cfg", "general.spawnRemove".to_owned(), true),
                ("config/create-server.toml", "a.spawnTimer".to_owned(), true),
            ]
        );
        // Sem texto: todas as alteradas; sem o filtro, o texto vazio não lista nada.
        assert_eq!(
            search(&index, &SearchQuery::new("").only_changed(true)).total_hits,
            2
        );
        assert_eq!(search(&index, &SearchQuery::new("")).total_hits, 0);
        let marked: Vec<bool> = all
            .groups
            .iter()
            .flat_map(|g| g.hits.iter().map(|h| h.changed))
            .collect();
        assert_eq!(marked.iter().filter(|c| **c).count(), 2);
    }

    #[test]
    fn limites_cortam_e_avisam() {
        let many = lines(80, "spawn");
        let index = index_with(&[("config/a.toml", &many), ("config/b.toml", "spawn = 1\n")]);
        let mut query = SearchQuery::new("spawn");
        let result = search(&index, &query);
        assert!(result.truncated);
        assert_eq!(result.total_hits, 81);
        let a = result
            .groups
            .iter()
            .find(|g| g.path == "config/a.toml")
            .unwrap();
        assert_eq!((a.hits.len(), a.total), (50, 80));
        query.max_hits = 10;
        let small = search(&index, &query);
        let shown: usize = small.groups.iter().map(|g| g.hits.len()).sum();
        assert_eq!(shown, 10);
        assert_eq!(small.total_hits, 81);
    }

    #[test]
    fn busca_cancelada_nao_devolve_nada() {
        let many = lines(2_000, "k");
        let index = index_with(&[("config/a.toml", &many)]);
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(index.search(&SearchQuery::new("k"), &cancel).is_none());
    }

    #[test]
    fn le_do_disco_e_pula_binarios_e_grandes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.toml"), "a = 1\n").unwrap();
        let file = source("a.toml", 6, 1.0);
        assert_eq!(read_file(dir.path(), &file).unwrap().entries.len(), 1);
        let binary = SourceFile {
            binary: true,
            ..file.clone()
        };
        assert!(read_file(dir.path(), &binary).is_none());
        let huge = SourceFile {
            size: MAX_INDEXED_BYTES + 1,
            ..file.clone()
        };
        assert!(read_file(dir.path(), &huge).is_none());
        assert!(read_file(dir.path(), &source("sumiu.toml", 1, 1.0)).is_none());
    }
}
