//! Resolução das dependências de "Adicionar" (SPEC T09; CA-T09-01, CA-T09-02, CA-T09-04 e
//! CA-T09-05).
//!
//! Parte dos itens escolhidos e segue as dependências declaradas pela fonte, nível a nível:
//!
//! - **obrigatórias** em cadeia (as de uma opcional também, ligadas a ela); a versão é a que a
//!   dependência pede, se servir para o pack, ou a mais nova compatível do canal;
//! - **opcionais** só dos itens escolhidos (informativas, desmarcadas);
//! - a mesma dependência pedida por vários itens vira **um** nó, com todos em `required_by`,
//!   mesmo quando cada um a pede por uma fonte (o `MezzConfig` do Modrinth e o da CurseForge
//!   são o mesmo nó, achado pelo SHA-1 ou pelo nome; fica o escolhido e, entre iguais, o do
//!   Modrinth, ADR-0027); um item escolhido nunca reaparece como opcional;
//! - o que já está no pack (pelo projeto, pelo projeto equivalente achado pelo hash ou pelo
//!   nome) não entra de novo e aparece em "Já no pack";
//! - **incompatibilidades** declaradas nos dois sentidos: as que os itens novos declaram (com o
//!   pack e entre si) e as que os itens do pack declaram contra os novos.
//!
//! Limites: [`MAX_DEPTH`] níveis e [`MAX_NODES`] itens (uma cadeia maior que isso é defeito
//! nos dados da fonte; o que passar fica registrado no log).

use std::collections::{BTreeMap, HashMap, HashSet};

use warden_core::CancellationToken;

use crate::Result;
use crate::add::plan::{
    AddPlan, ConflictParty, NodeRole, PackState, PlanConflict, PlanDuplicate, PlanInstalled,
    PlanMissing, PlanNode, PlanTarget,
};
use crate::add::{
    AddChoice, AddSource, AddSources, ChannelPolicy, DependencyKind, SideNote, SourceProject,
    SourceVersion, curseforge, source_off,
};
use crate::dedup::{CrossIdentity, DuplicateMatch, Identity, PackItem, PackItems, same_across};
use crate::search::SourceId;
use crate::side::SideChoice;
use crate::{Error, ProjectErrorCode as Code};

/// Níveis de dependência seguidos.
pub const MAX_DEPTH: usize = 12;

/// Itens no plano, no máximo.
pub const MAX_NODES: usize = 200;

/// Acha, pelo SHA-1 dos arquivos, o projeto equivalente noutra fonte de cada item do pack (o
/// JEI da CurseForge é o `u6dRKJwZ` do Modrinth). Uma requisição por fonte, só se houver
/// hashes; uma fonte que não responde só deixa de ajudar (o nome ainda serve).
pub(crate) async fn find_equivalents(
    sources: &AddSources,
    items: &mut PackItems,
    cancel: &CancellationToken,
) -> Result<()> {
    for source in sources.all() {
        let id = source.id();
        let hashes: Vec<String> = items
            .items()
            .iter()
            .filter(|item| item.source != Some(id))
            .filter_map(|item| item.sha1.clone())
            .collect();
        if hashes.is_empty() {
            continue;
        }
        match source.versions_by_sha1(&hashes, cancel).await {
            Ok(found) => {
                let matches: Vec<(String, String)> = items
                    .items()
                    .iter()
                    .filter(|item| item.source != Some(id))
                    .filter_map(|item| {
                        let version = found.get(item.sha1.as_deref()?)?;
                        Some((item.path.clone(), id.key(&version.project_id)))
                    })
                    .collect();
                for (path, key) in matches {
                    items.set_equivalent(&path, key);
                }
            }
            Err(error) => {
                tracing::warn!(%error, fonte = id.label(), "não foi possível conferir os hashes do pack");
            }
        }
    }
    Ok(())
}

/// Uma dependência ainda por resolver.
#[derive(Debug, Clone)]
struct Edge {
    /// Chave do item que declara.
    parent: String,
    source: SourceId,
    project_id: Option<String>,
    version_id: Option<String>,
    role: NodeRole,
}

/// Incompatibilidade declarada por um item do plano.
#[derive(Debug, Clone)]
struct Declared {
    by: String,
    source: SourceId,
    project_id: Option<String>,
    version_id: Option<String>,
}

/// Um item com projeto e versão a caminho de virar nó: um escolhido, ou uma dependência com
/// todos os itens do nível que a pedem.
#[derive(Debug, Clone)]
struct Candidate {
    key: String,
    role: NodeRole,
    project: SourceProject,
    version: SourceVersion,
    picked: bool,
    edges: Vec<Edge>,
}

struct Resolver<'a> {
    sources: &'a AddSources,
    pack: &'a PackState,
    policy: ChannelPolicy,
    cancel: &'a CancellationToken,
    nodes: Vec<PlanNode>,
    index: HashMap<String, usize>,
    /// Chave de um projeto equivalente a um nó (o mesmo mod noutra fonte) → chave do nó.
    aliases: HashMap<String, String>,
    sha1: HashMap<String, String>,
    /// Chaves do mesmo projeto noutras fontes, achadas pelo SHA-1, por nó.
    equivalents: HashMap<String, Vec<String>>,
    /// Versões achadas pelo SHA-1 noutra fonte: (fonte consultada, SHA-1) → versão.
    by_sha1: HashMap<(SourceId, String), SourceVersion>,
    /// SHA-1 já consultados (achados ou não), para não perguntar de novo.
    checked_sha1: HashSet<String>,
    projects: HashMap<String, SourceProject>,
    installed: Vec<PlanInstalled>,
    missing: Vec<PlanMissing>,
    declared: Vec<Declared>,
}

/// Monta o plano (sem gravar nada).
pub(crate) async fn resolve(
    sources: &AddSources,
    pack: &PackState,
    policy: ChannelPolicy,
    choices: &[AddChoice],
    cancel: &CancellationToken,
) -> Result<AddPlan> {
    let mut resolver = Resolver {
        sources,
        pack,
        policy,
        cancel,
        nodes: Vec::new(),
        index: HashMap::new(),
        aliases: HashMap::new(),
        sha1: HashMap::new(),
        equivalents: HashMap::new(),
        by_sha1: HashMap::new(),
        checked_sha1: HashSet::new(),
        projects: HashMap::new(),
        installed: Vec::new(),
        missing: Vec::new(),
        declared: Vec::new(),
    };
    let chosen = resolver.chosen(choices).await?;
    let mut queue = resolver.settle_all(chosen).await?;
    let mut depth = 0;
    while !queue.is_empty() {
        if depth >= MAX_DEPTH || resolver.nodes.len() >= MAX_NODES {
            tracing::warn!(
                restantes = queue.len(),
                "cadeia de dependências longa demais; o resto fica de fora"
            );
            break;
        }
        queue = resolver.level(queue).await?;
        depth += 1;
    }
    let duplicates = resolver.duplicates();
    let conflicts = resolver.conflicts().await?;
    let target = &pack.target;
    Ok(AddPlan {
        target: PlanTarget {
            minecraft: target.minecraft.clone(),
            loader: target.loaders.first().map(|loader| loader.key().to_owned()),
        },
        nodes: resolver.nodes,
        installed: resolver.installed,
        missing: resolver.missing,
        conflicts,
        duplicates,
        pack_item_count: pack.item_count,
    })
}

impl Resolver<'_> {
    fn source(&self, id: SourceId) -> Result<&dyn AddSource> {
        self.sources.get(id).ok_or_else(|| source_off(id))
    }

    /// A chave do nó que responde por `key` (ela mesma, ou o nó equivalente noutra fonte).
    fn canonical(&self, key: &str) -> String {
        let mut key = key.to_owned();
        // Os apelidos sempre apontam para um nó vivo; o limite só protege de um ciclo.
        for _ in 0..4 {
            match self.aliases.get(&key) {
                Some(next) if *next != key => key.clone_from(next),
                _ => break,
            }
        }
        key
    }

    /// Busca, em lote, os projetos ainda desconhecidos.
    async fn load_projects(&mut self, source: SourceId, ids: &[String]) -> Result<()> {
        let missing: Vec<String> = unique(
            ids.iter()
                .filter(|id| !self.projects.contains_key(&source.key(id)))
                .cloned(),
        );
        if missing.is_empty() {
            return Ok(());
        }
        let found = self.source(source)?.projects(&missing, self.cancel).await?;
        for project in found {
            self.projects.insert(source.key(&project.id), project);
        }
        Ok(())
    }

    /// Os itens escolhidos: projeto e versão de cada um.
    async fn chosen(&mut self, choices: &[AddChoice]) -> Result<Vec<Candidate>> {
        let mut seen = HashSet::new();
        let choices: Vec<&AddChoice> = choices
            .iter()
            .filter(|choice| seen.insert(choice.source.key(&choice.project_id)))
            .collect();
        let mut explicit: HashMap<String, SourceVersion> = HashMap::new();
        for source in [SourceId::Modrinth, SourceId::Curseforge] {
            let of_source: Vec<&&AddChoice> = choices
                .iter()
                .filter(|choice| choice.source == source)
                .collect();
            if of_source.is_empty() {
                continue;
            }
            let ids: Vec<String> = of_source.iter().map(|c| c.project_id.clone()).collect();
            self.load_projects(source, &ids).await?;
            let versions: Vec<String> = of_source
                .iter()
                .filter_map(|choice| choice.version_id.clone())
                .collect();
            if !versions.is_empty() {
                for version in self
                    .source(source)?
                    .versions(&versions, self.cancel)
                    .await?
                {
                    explicit.insert(version.id.clone(), version);
                }
            }
        }
        let pack = self.pack;
        let mut candidates = Vec::new();
        for choice in choices {
            let key = choice.source.key(&choice.project_id);
            if let Some(item) = pack.items.same_source(choice.source, &choice.project_id) {
                self.installed(&key, item, true, None);
                continue;
            }
            let Some(project) = self.projects.get(&key).cloned() else {
                self.add_missing(&key, &choice.project_id, NodeRole::Chosen, None);
                continue;
            };
            if project.kind.is_none() {
                return Err(Error::new(
                    Code::InvalidInput,
                    format!("{} não é mod, resource pack nem shader", project.title),
                )
                .param("field", "kind"));
            }
            let (version, picked) = match &choice.version_id {
                Some(id) => {
                    let version = explicit
                        .get(id)
                        .filter(|version| version.project_id == project.id)
                        .cloned()
                        .ok_or_else(|| {
                            Error::new(
                                Code::InvalidInput,
                                format!("a versão {id} não é de {}", project.title),
                            )
                            .param("field", "versionId")
                        })?;
                    (Some(version), false)
                }
                None => (self.default_version(&project).await?, true),
            };
            let Some(version) = version else {
                self.add_missing(&key, &project.title, NodeRole::Chosen, None);
                continue;
            };
            candidates.push(Candidate {
                key,
                role: NodeRole::Chosen,
                project,
                version,
                picked,
                edges: Vec::new(),
            });
        }
        Ok(candidates)
    }

    /// A versão padrão de um projeto para o pack.
    async fn default_version(&self, project: &SourceProject) -> Result<Option<SourceVersion>> {
        let versions = self
            .source(project.source)?
            .compatible_versions(project, &self.pack.target, self.cancel)
            .await?;
        Ok(self.policy.pick(&versions).cloned())
    }

    /// Os candidatos de um nível viram nós (ou se juntam a um nó equivalente, ou contam como
    /// já no pack). Devolve as dependências dos nós novos.
    async fn settle_all(&mut self, candidates: Vec<Candidate>) -> Result<Vec<Edge>> {
        self.look_up_sha1(&candidates).await;
        let mut next = Vec::new();
        for candidate in &candidates {
            next.extend(self.settle(candidate));
        }
        Ok(next)
    }

    /// Procura, em lote, o arquivo de cada candidato nas outras fontes pelo SHA-1 (o `MezzConfig`
    /// da CurseForge é o `m-mezz` do Modrinth; e o Modrinth informa o lado). Uma fonte que não
    /// responde só deixa de ajudar: o nome ainda serve.
    async fn look_up_sha1(&mut self, candidates: &[Candidate]) {
        let mut found_projects: Vec<(SourceId, String)> = Vec::new();
        for source in self.sources.all() {
            let id = source.id();
            let hashes: Vec<String> = unique(
                candidates
                    .iter()
                    .filter(|candidate| candidate.project.source != id)
                    .filter_map(|candidate| file_sha1(&candidate.version))
                    .filter(|sha1| {
                        !self
                            .checked_sha1
                            .contains(&format!("{}:{sha1}", id.label()))
                    }),
            );
            if hashes.is_empty() {
                continue;
            }
            for sha1 in &hashes {
                self.checked_sha1.insert(format!("{}:{sha1}", id.label()));
            }
            match source.versions_by_sha1(&hashes, self.cancel).await {
                Ok(found) => {
                    for (sha1, version) in found {
                        found_projects.push((id, version.project_id.clone()));
                        self.by_sha1.insert((id, sha1), version);
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, fonte = id.label(), "não foi possível conferir os hashes do plano");
                }
            }
        }
        // O projeto achado informa o lado quando a versão não informa.
        for source in [SourceId::Modrinth, SourceId::Curseforge] {
            let ids: Vec<String> = found_projects
                .iter()
                .filter(|(id, _)| *id == source)
                .map(|(_, project)| project.clone())
                .collect();
            if !ids.is_empty()
                && let Err(error) = self.load_projects(source, &ids).await
            {
                tracing::warn!(%error, "projetos achados pelo hash indisponíveis");
            }
        }
    }

    /// As versões do mesmo arquivo nas outras fontes, achadas pelo SHA-1.
    fn same_file(&self, version: &SourceVersion) -> Vec<&SourceVersion> {
        let Some(sha1) = file_sha1(version) else {
            return Vec::new();
        };
        self.sources
            .all()
            .map(AddSource::id)
            .filter(|id| *id != version.source)
            .filter_map(|id| self.by_sha1.get(&(id, sha1.clone())))
            .collect()
    }

    /// O lado que outra fonte informa para o mesmo arquivo (a CurseForge não informa o lado;
    /// o Modrinth, sim), quando ela sabe.
    fn side_elsewhere(&self, version: &SourceVersion) -> Option<(SideChoice, Option<SideNote>)> {
        self.same_file(version).into_iter().find_map(|found| {
            if found.side_note != Some(SideNote::Unknown) {
                return Some((found.side, found.side_note));
            }
            self.projects
                .get(&found.source.key(&found.project_id))
                .and_then(|project| project.side)
                .filter(|(_, note)| *note != Some(SideNote::Unknown))
        })
    }

    /// Um candidato vira nó, junta-se ao nó equivalente que já está no plano ou conta como já
    /// no pack. Devolve as dependências do que entrou.
    fn settle(&mut self, candidate: &Candidate) -> Vec<Edge> {
        let equivalents: Vec<String> = self
            .same_file(&candidate.version)
            .into_iter()
            .map(|found| found.source.key(&found.project_id))
            .collect();
        let sha1 = file_sha1(&candidate.version);
        let project = &candidate.project;
        if candidate.role != NodeRole::Chosen {
            // O mesmo mod no pack por outra fonte ou link: conta como já no pack. (Um
            // escolhido nessa situação vira duplicado, com a opção de substituir.)
            let pack = self.pack;
            let identity = Identity {
                author: "",
                slug: &project.slug,
                title: &project.title,
            };
            let in_pack = equivalents
                .iter()
                .find_map(|key| pack.items.by_key(key))
                .or_else(|| {
                    pack.items
                        .equivalent(project.source, &project.id, identity, sha1.as_deref())
                        .map(|(item, _)| item)
                });
            if let Some(item) = in_pack {
                for edge in &candidate.edges {
                    if edge.role == NodeRole::Required {
                        self.installed(&candidate.key, item, false, Some(&edge.parent));
                    }
                }
                return Vec::new();
            }
        }
        let identity = CrossIdentity {
            source: Some(project.source),
            project_id: Some(&project.id),
            sha1: sha1.as_deref(),
            equivalents: &equivalents,
            slug: &project.slug,
            title: &project.title,
        };
        if let Some(position) = self.equivalent_node(&identity) {
            return self.merge(position, candidate, equivalents);
        }
        let key = candidate.key.clone();
        let next = self.push_node(candidate, Vec::new(), Vec::new());
        self.equivalents.insert(key.clone(), equivalents);
        for edge in &candidate.edges {
            self.link_known(edge, &key);
        }
        next
    }

    /// O nó do plano que é este projeto por outra fonte (o escolhido primeiro, depois o mais
    /// confiável: projeto, hash, nome).
    fn equivalent_node(&self, identity: &CrossIdentity<'_>) -> Option<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(position, node)| {
                let equivalents = self
                    .equivalents
                    .get(&node.key)
                    .map_or(&[][..], Vec::as_slice);
                let other = CrossIdentity {
                    source: Some(node.source),
                    project_id: Some(&node.project_id),
                    sha1: self.sha1.get(&node.key).map(String::as_str),
                    equivalents,
                    slug: &node.slug,
                    title: &node.title,
                };
                let how = same_across(identity, &other)?;
                Some(((node.role != NodeRole::Chosen, how), position))
            })
            .min()
            .map(|(_, position)| position)
    }

    /// Junta o candidato ao nó equivalente: fica o escolhido; entre iguais, o do Modrinth
    /// (ADR-0027). Os pais dos dois ficam no nó que fica.
    fn merge(
        &mut self,
        position: usize,
        candidate: &Candidate,
        equivalents: Vec<String>,
    ) -> Vec<Edge> {
        let existing = &self.nodes[position];
        let rank = |role: NodeRole, source: SourceId| {
            (role != NodeRole::Chosen, source != SourceId::Modrinth)
        };
        let replace =
            rank(candidate.role, candidate.project.source) < rank(existing.role, existing.source);
        let mut next = Vec::new();
        let kept = if replace {
            let old = existing.key.clone();
            let role = stronger(existing.role, candidate.role);
            let required_by = existing.required_by.clone();
            let optional_for = existing.optional_for.clone();
            let key = candidate.key.clone();
            self.index.remove(&old);
            self.sha1.remove(&old);
            self.equivalents.remove(&old);
            let candidate_role = Candidate {
                role,
                ..candidate.clone()
            };
            // O nó vai para o fim e volta para a posição do antigo.
            next = self.push_node(&candidate_role, required_by, optional_for);
            let node = self
                .nodes
                .pop()
                .unwrap_or_else(|| unreachable!("o nó acabou de entrar"));
            self.nodes[position] = node;
            self.index.insert(key.clone(), position);
            self.equivalents.insert(key.clone(), equivalents);
            self.rekey(&old, &key);
            key
        } else {
            let key = existing.key.clone();
            self.aliases.insert(candidate.key.clone(), key.clone());
            key
        };
        for edge in &candidate.edges {
            self.link_known(edge, &kept);
        }
        next
    }

    /// A chave `old` saiu do plano em favor de `new`: os pais, as incompatibilidades e os
    /// apelidos passam a apontar para `new`.
    fn rekey(&mut self, old: &str, new: &str) {
        let swap = |list: &mut Vec<String>| {
            if list.iter().any(|key| key == old) {
                list.retain(|key| key != old);
                push_unique(list, new);
            }
        };
        for node in &mut self.nodes {
            swap(&mut node.required_by);
            swap(&mut node.optional_for);
            node.required_by.retain(|key| key != &node.key);
            node.optional_for.retain(|key| key != &node.key);
        }
        for entry in &mut self.installed {
            swap(&mut entry.required_by);
        }
        for entry in &mut self.missing {
            swap(&mut entry.required_by);
        }
        for declared in &mut self.declared {
            if declared.by == old {
                new.clone_into(&mut declared.by);
            }
        }
        for target in self.aliases.values_mut() {
            if target == old {
                new.clone_into(target);
            }
        }
        self.aliases.insert(old.to_owned(), new.to_owned());
    }

    /// Acrescenta o nó do candidato (com estes pais) e devolve as dependências dele.
    fn push_node(
        &mut self,
        candidate: &Candidate,
        required_by: Vec<String>,
        optional_for: Vec<String>,
    ) -> Vec<Edge> {
        let Candidate {
            key,
            role,
            project,
            version,
            picked,
            ..
        } = candidate;
        let role = *role;
        let kind = project.kind.unwrap_or(crate::search::ProjectKind::Mod);
        let file_name = version
            .file
            .as_ref()
            .map(|file| file.name.clone())
            .unwrap_or_default();
        if let Some(sha1) = file_sha1(version) {
            self.sha1.insert(key.clone(), sha1);
        }
        // Versão sem `environment`: vale o lado do projeto, se ele informar; na falta dele, o
        // que outra fonte informa para o mesmo arquivo.
        let (side, side_note) = match version.side_note {
            Some(SideNote::Unknown) => project
                .side
                .or_else(|| self.side_elsewhere(version))
                .unwrap_or((version.side, version.side_note)),
            _ => (version.side, version.side_note),
        };
        let version_number = match project.source {
            SourceId::Curseforge => curseforge::readable_number(version, &project.title),
            SourceId::Modrinth => version.number.clone(),
        };
        let node = PlanNode {
            key: key.clone(),
            role,
            source: project.source,
            project_id: project.id.clone(),
            slug: project.slug.clone(),
            title: project.title.clone(),
            icon_url: project.icon_url.clone(),
            kind,
            version_id: version.id.clone(),
            version_number,
            channel: version.channel,
            outside_channel: *picked && !self.policy.allows(version.channel),
            compatible: version.compatible(project.kind, &self.pack.target),
            file_name,
            side,
            side_note,
            required_by,
            optional_for,
        };
        self.index.insert(key.clone(), self.nodes.len());
        self.nodes.push(node);
        let mut edges = Vec::new();
        for dependency in &version.dependencies {
            match dependency.kind {
                DependencyKind::Incompatible => self.declared.push(Declared {
                    by: key.clone(),
                    source: project.source,
                    project_id: dependency.project_id.clone(),
                    version_id: dependency.version_id.clone(),
                }),
                DependencyKind::Required | DependencyKind::Optional => {
                    // Opcionais só dos itens escolhidos (informativas).
                    if dependency.kind == DependencyKind::Optional && role != NodeRole::Chosen {
                        continue;
                    }
                    edges.push(Edge {
                        parent: key.clone(),
                        source: project.source,
                        project_id: dependency.project_id.clone(),
                        version_id: dependency.version_id.clone(),
                        role: if dependency.kind == DependencyKind::Required {
                            NodeRole::Required
                        } else {
                            NodeRole::Optional
                        },
                    });
                }
            }
        }
        edges
    }

    /// Resolve um nível de dependências e devolve o próximo.
    async fn level(&mut self, edges: Vec<Edge>) -> Result<Vec<Edge>> {
        let (mut edges, pinned) = self.pin_versions(edges).await?;
        // O Modrinth primeiro: entre duas fontes com o mesmo mod, fica o dele (ADR-0027).
        edges.sort_by_key(|edge| edge.source != SourceId::Modrinth);
        for source in [SourceId::Modrinth, SourceId::Curseforge] {
            let ids: Vec<String> = edges
                .iter()
                .filter(|edge| edge.source == source)
                .filter_map(|edge| edge.project_id.clone())
                .filter(|id| !self.index.contains_key(&self.canonical(&source.key(id))))
                .collect();
            if !ids.is_empty() {
                self.load_projects(source, &ids).await?;
            }
        }
        let mut candidates: Vec<Candidate> = Vec::new();
        for edge in edges {
            let Some(project_id) = edge.project_id.clone() else {
                tracing::debug!(pai = %edge.parent, "dependência sem projeto nem versão conhecida");
                continue;
            };
            let key = edge.source.key(&project_id);
            if key == edge.parent || self.link_known(&edge, &key) {
                continue;
            }
            // A mesma dependência pedida por outro item deste nível.
            if let Some(known) = candidates.iter_mut().find(|known| known.key == key) {
                known.role = stronger(known.role, edge.role);
                known.edges.push(edge);
                continue;
            }
            if let Some(candidate) = self.candidate(edge, &project_id, key, &pinned).await? {
                candidates.push(candidate);
            }
        }
        self.settle_all(candidates).await
    }

    /// Busca, em lote, as versões exatas pedidas; dependências que só informam a versão
    /// ganham o projeto dela.
    async fn pin_versions(
        &self,
        edges: Vec<Edge>,
    ) -> Result<(Vec<Edge>, HashMap<String, SourceVersion>)> {
        let mut pinned: HashMap<String, SourceVersion> = HashMap::new();
        for source in [SourceId::Modrinth, SourceId::Curseforge] {
            let ids: Vec<String> = unique(
                edges
                    .iter()
                    .filter(|edge| edge.source == source)
                    .filter_map(|edge| edge.version_id.clone()),
            );
            if ids.is_empty() {
                continue;
            }
            for version in self.source(source)?.versions(&ids, self.cancel).await? {
                pinned.insert(version.id.clone(), version);
            }
        }
        let edges = edges
            .into_iter()
            .map(|mut edge| {
                if edge.project_id.is_none()
                    && let Some(version) = edge.version_id.as_ref().and_then(|id| pinned.get(id))
                {
                    edge.project_id = Some(version.project_id.clone());
                }
                edge
            })
            .collect();
        Ok((edges, pinned))
    }

    /// Liga a dependência ao que já se sabe dela (nó do plano, inclusive o equivalente noutra
    /// fonte, já no pack, sem versão). Devolve se ela já era conhecida.
    fn link_known(&mut self, edge: &Edge, key: &str) -> bool {
        let node_key = self.canonical(key);
        let parent = self.canonical(&edge.parent);
        if let Some(&position) = self.index.get(&node_key) {
            if node_key == parent {
                // O mod pede a si mesmo pela outra fonte: nada a ligar.
                return true;
            }
            let node = &mut self.nodes[position];
            if edge.role == NodeRole::Required && node.role == NodeRole::Optional {
                node.role = NodeRole::Required;
            }
            link(node, edge.role, &parent);
            return true;
        }
        if let Some(entry) = self.installed.iter_mut().find(|entry| entry.key == key) {
            if edge.role == NodeRole::Required {
                push_unique(&mut entry.required_by, &parent);
            }
            return true;
        }
        if let Some(entry) = self.missing.iter_mut().find(|entry| entry.key == key) {
            if edge.role == NodeRole::Required && entry.role == NodeRole::Optional {
                entry.role = NodeRole::Required;
            }
            push_unique(&mut entry.required_by, &parent);
            return true;
        }
        let pack = self.pack;
        let in_pack = pack
            .items
            .same_source(edge.source, edge.project_id.as_deref().unwrap_or_default())
            .or_else(|| pack.items.by_equivalent_key(key));
        if let Some(item) = in_pack {
            if edge.role == NodeRole::Required {
                self.installed(key, item, false, Some(&parent));
            }
            return true;
        }
        false
    }

    /// Uma dependência ainda desconhecida: projeto e versão (ou "sem versão"). O candidato
    /// ainda pode ser o mesmo mod de um nó ou do pack por outra fonte ([`Resolver::settle`]).
    async fn candidate(
        &mut self,
        edge: Edge,
        project_id: &str,
        key: String,
        pinned: &HashMap<String, SourceVersion>,
    ) -> Result<Option<Candidate>> {
        let parent = Some(edge.parent.as_str());
        let Some(project) = self.projects.get(&key).cloned() else {
            self.add_missing(&key, project_id, edge.role, parent);
            return Ok(None);
        };
        if project.kind.is_none() {
            self.add_missing(&key, &project.title, edge.role, parent);
            return Ok(None);
        }
        let wanted = edge
            .version_id
            .as_ref()
            .and_then(|id| pinned.get(id))
            .filter(|version| {
                version.project_id == project.id
                    && version.compatible(project.kind, &self.pack.target)
            })
            .cloned();
        let (version, picked) = match wanted {
            Some(version) => (Some(version), false),
            None => (self.default_version(&project).await?, true),
        };
        let Some(version) = version else {
            self.add_missing(&key, &project.title, edge.role, parent);
            return Ok(None);
        };
        Ok(Some(Candidate {
            key,
            role: edge.role,
            project,
            version,
            picked,
            edges: vec![edge],
        }))
    }

    fn installed(&mut self, key: &str, item: &PackItem, chosen: bool, parent: Option<&str>) {
        if let Some(entry) = self.installed.iter_mut().find(|entry| entry.key == key) {
            entry.chosen |= chosen;
            if let Some(parent) = parent {
                push_unique(&mut entry.required_by, parent);
            }
            return;
        }
        self.installed.push(PlanInstalled {
            key: key.to_owned(),
            title: item.name.clone(),
            path: item.path.clone(),
            pack_source: item.source,
            chosen,
            required_by: parent.map(|p| vec![p.to_owned()]).unwrap_or_default(),
        });
    }

    fn add_missing(&mut self, key: &str, title: &str, role: NodeRole, parent: Option<&str>) {
        self.missing.push(PlanMissing {
            key: key.to_owned(),
            title: title.to_owned(),
            role,
            required_by: parent.map(|p| vec![p.to_owned()]).unwrap_or_default(),
        });
    }

    /// Itens escolhidos que já estão no pack por outra fonte (ou link): oferecer substituir.
    fn duplicates(&self) -> Vec<PlanDuplicate> {
        self.nodes
            .iter()
            .filter(|node| node.role == NodeRole::Chosen)
            .filter_map(|node| {
                let identity = Identity {
                    author: "",
                    slug: &node.slug,
                    title: &node.title,
                };
                let sha1 = self.sha1.get(&node.key).map(String::as_str);
                let by_hash = self
                    .equivalents
                    .get(&node.key)
                    .into_iter()
                    .flatten()
                    .find_map(|key| self.pack.items.by_key(key))
                    .map(|item| (item, DuplicateMatch::Project));
                let (item, matched_by) = by_hash.or_else(|| {
                    self.pack
                        .items
                        .equivalent(node.source, &node.project_id, identity, sha1)
                })?;
                Some(PlanDuplicate {
                    key: node.key.clone(),
                    existing_path: item.path.clone(),
                    existing_title: item.name.clone(),
                    existing_source: item.source,
                    matched_by,
                })
            })
            .collect()
    }

    /// Incompatibilidades declaradas, nos dois sentidos.
    async fn conflicts(&mut self) -> Result<Vec<PlanConflict>> {
        let mut found: BTreeMap<(String, String), PlanConflict> = BTreeMap::new();
        // Declaradas só por versão: a versão diz o projeto.
        for source in [SourceId::Modrinth, SourceId::Curseforge] {
            let ids: Vec<String> = unique(
                self.declared
                    .iter()
                    .filter(|d| d.source == source && d.project_id.is_none())
                    .filter_map(|d| d.version_id.clone()),
            );
            if ids.is_empty() {
                continue;
            }
            let versions = self.source(source)?.versions(&ids, self.cancel).await?;
            for declared in &mut self.declared {
                if declared.source == source
                    && declared.project_id.is_none()
                    && let Some(version) = versions
                        .iter()
                        .find(|version| Some(&version.id) == declared.version_id.as_ref())
                {
                    declared.project_id = Some(version.project_id.clone());
                }
            }
        }
        for declared in &self.declared {
            let Some(project_id) = &declared.project_id else {
                continue;
            };
            let key = declared.source.key(project_id);
            let Some(by) = self.party_of_node(&declared.by) else {
                continue;
            };
            if let Some(other) = self.party_of_node(&key) {
                insert_conflict(&mut found, by, other, false, &declared.by);
            } else if let Some(item) = self
                .pack
                .items
                .same_source(declared.source, project_id)
                .or_else(|| self.pack.items.by_equivalent_key(&key))
            {
                insert_conflict(&mut found, by, pack_party(&key, item), true, &declared.by);
            }
        }
        // Itens do pack que declaram incompatibilidade com os novos.
        for source in self.sources.all() {
            let id = source.id();
            let pack_versions: Vec<(&PackItem, String)> = self
                .pack
                .items
                .items()
                .iter()
                .filter(|item| item.source == Some(id))
                .filter_map(|item| Some((item, item.version_id.clone()?)))
                .collect();
            if pack_versions.is_empty() || self.nodes.is_empty() {
                continue;
            }
            let ids: Vec<String> = pack_versions.iter().map(|(_, v)| v.clone()).collect();
            let versions = match source.versions(&ids, self.cancel).await {
                Ok(versions) => versions,
                Err(error) => {
                    tracing::warn!(%error, "não foi possível ler as versões do pack para conferir incompatibilidades");
                    continue;
                }
            };
            for version in versions {
                let Some((item, _)) = pack_versions.iter().find(|(_, v)| *v == version.id) else {
                    continue;
                };
                let item_key = id.key(&version.project_id);
                for dependency in &version.dependencies {
                    if dependency.kind != DependencyKind::Incompatible {
                        continue;
                    }
                    let Some(project_id) = &dependency.project_id else {
                        continue;
                    };
                    if let Some(node) = self.party_of_node(&id.key(project_id)) {
                        insert_conflict(
                            &mut found,
                            node,
                            pack_party(&item_key, item),
                            true,
                            &item_key,
                        );
                    }
                }
            }
        }
        Ok(found.into_values().collect())
    }

    fn party_of_node(&self, key: &str) -> Option<ConflictParty> {
        let node = &self.nodes[*self.index.get(&self.canonical(key))?];
        Some(ConflictParty {
            key: node.key.clone(),
            title: node.title.clone(),
            path: None,
        })
    }
}

fn pack_party(key: &str, item: &PackItem) -> ConflictParty {
    ConflictParty {
        key: key.to_owned(),
        title: item.name.clone(),
        path: Some(item.path.clone()),
    }
}

/// Guarda a incompatibilidade uma vez por par (A×B e B×A são a mesma).
fn insert_conflict(
    found: &mut BTreeMap<(String, String), PlanConflict>,
    item: ConflictParty,
    other: ConflictParty,
    with_pack: bool,
    declared_by: &str,
) {
    if item.key == other.key {
        return;
    }
    let pair = if item.key < other.key {
        (item.key.clone(), other.key.clone())
    } else {
        (other.key.clone(), item.key.clone())
    };
    found.entry(pair).or_insert_with(|| PlanConflict {
        item,
        other,
        with_pack,
        declared_by: declared_by.to_owned(),
        reason: None,
    });
}

/// O SHA-1 do arquivo de uma versão, em minúsculas.
fn file_sha1(version: &SourceVersion) -> Option<String> {
    version
        .file
        .as_ref()
        .and_then(|file| file.sha1.as_deref())
        .filter(|sha1| !sha1.is_empty())
        .map(str::to_ascii_lowercase)
}

/// O papel mais forte: escolhido, depois obrigatória, depois opcional.
fn stronger(a: NodeRole, b: NodeRole) -> NodeRole {
    let weight = |role| match role {
        NodeRole::Chosen => 2,
        NodeRole::Required => 1,
        NodeRole::Optional => 0,
    };
    if weight(b) > weight(a) { b } else { a }
}

fn link(node: &mut PlanNode, role: NodeRole, parent: &str) {
    match role {
        NodeRole::Required => push_unique(&mut node.required_by, parent),
        NodeRole::Optional => push_unique(&mut node.optional_for, parent),
        NodeRole::Chosen => {}
    }
}

fn push_unique(list: &mut Vec<String>, value: &str) {
    if !list.iter().any(|known| known == value) {
        list.push(value.to_owned());
    }
}

fn unique(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.clone()))
        .collect()
}
