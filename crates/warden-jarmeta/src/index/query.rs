//! Consultas do índice de um pack.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use super::{IndexedJar, JarIndex, MixinConfigEntry};
use crate::model::{DescriptorKind, Loader};
use crate::version::flexver;

/// Posição de um jar no índice do pack: item e jar dentro dele.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct JarRef {
    item: usize,
    jar: usize,
}

/// Quem é dono de algo: o item do pack, o jar (de cima ou embutido) e o mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JarOwner<'a> {
    /// O item do pack ([`PackJar::item`](super::PackJar::item)).
    pub item: &'a str,
    /// Caminho do jar dentro do item (vazio = o jar de cima).
    pub jar: &'a str,
    /// O mod do jar (o principal para o loader do pack), se o jar declara algum.
    pub mod_id: Option<&'a str>,
}

/// Resposta de uma consulta de pacote.
///
/// Cópias do mesmo mod embutidas em itens diferentes (o `fabric-api-base` que vem dentro do
/// Sodium, do Iris e da Fabric API) não são ambiguidade: o loader carrega uma só, a de versão
/// maior, e é ela que a consulta devolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ownership<'a> {
    /// Um só mod tem o pacote (a cópia que o loader carrega, se houver várias).
    Unique(JarOwner<'a>),
    /// Mods diferentes têm o pacote. Se todos estão no mesmo item, o dono escolhido é o jar de
    /// topo desse item (o mod que o usuário adicionou); entre itens diferentes não há escolha.
    Ambiguous {
        /// O jar de topo, quando os candidatos são todos do mesmo item.
        chosen: Option<JarOwner<'a>>,
        /// Um candidato por mod (a cópia carregada).
        candidates: Vec<JarOwner<'a>>,
    },
}

impl<'a> Ownership<'a> {
    /// O dono a usar, quando há um.
    #[must_use]
    pub fn owner(&self) -> Option<JarOwner<'a>> {
        match self {
            Self::Unique(owner) => Some(*owner),
            Self::Ambiguous { chosen, .. } => *chosen,
        }
    }
}

/// Dono de uma config de mixin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixinOwner<'a> {
    /// Item, jar e mod.
    pub owner: JarOwner<'a>,
    /// Descritor que declarou a config.
    pub declared_in: DescriptorKind,
    /// O arquivo da config existe no jar.
    pub present: bool,
}

/// Índices dos jars de um pack, com as consultas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackIndex {
    loader: Option<Loader>,
    items: Vec<(String, JarIndex)>,
    packages: BTreeMap<String, Vec<JarRef>>,
    mixins: BTreeMap<String, Vec<(JarRef, usize)>>,
}

impl PackIndex {
    /// Junta índices já montados (`(item, índice)`), na ordem dada.
    #[must_use]
    pub fn new(items: Vec<(String, JarIndex)>, loader: Option<Loader>) -> Self {
        let mut packages: BTreeMap<String, Vec<JarRef>> = BTreeMap::new();
        let mut mixins: BTreeMap<String, Vec<(JarRef, usize)>> = BTreeMap::new();
        for (item, (_, index)) in items.iter().enumerate() {
            for (jar, indexed) in index.jars.iter().enumerate() {
                for package in &indexed.packages {
                    packages
                        .entry(package.clone())
                        .or_default()
                        .push(JarRef { item, jar });
                }
            }
            for (position, entry) in index.mixin_configs.iter().enumerate() {
                let jar = usize::try_from(entry.jar).unwrap_or(usize::MAX);
                if jar < index.jars.len() {
                    mixins
                        .entry(entry.config.clone())
                        .or_default()
                        .push((JarRef { item, jar }, position));
                }
            }
        }
        Self {
            loader,
            items,
            packages,
            mixins,
        }
    }

    /// Loader do pack usado nas escolhas.
    #[must_use]
    pub fn loader(&self) -> Option<Loader> {
        self.loader
    }

    /// Os itens e seus índices, na ordem de montagem.
    #[must_use]
    pub fn items(&self) -> &[(String, JarIndex)] {
        &self.items
    }

    /// Quantos pacotes distintos o pack tem.
    #[must_use]
    pub fn package_count(&self) -> usize {
        self.packages.len()
    }

    /// Dono de um pacote (`net.fabricmc.fabric.api.util`; aceita `/`).
    #[must_use]
    pub fn package_owner(&self, package: &str) -> Option<Ownership<'_>> {
        let refs = self.packages.get(&package.trim().replace('/', "."))?;
        Some(self.ownership(refs))
    }

    /// Dono da classe (`me.jellysquid.mods.sodium.client.SodiumClientMod`, com `$` de classe
    /// interna ou `/`), pelo pacote dela.
    #[must_use]
    pub fn class_owner(&self, class_name: &str) -> Option<Ownership<'_>> {
        let normalized = class_name.trim().replace('/', ".");
        let (package, _) = normalized.rsplit_once('.')?;
        self.package_owner(package)
    }

    /// Pacotes que mods diferentes têm (cópias do mesmo mod não contam), com os candidatos.
    #[must_use]
    pub fn ambiguous_packages(&self) -> Vec<(&str, Ownership<'_>)> {
        self.packages
            .iter()
            .filter(|(_, refs)| refs.len() > 1)
            .map(|(package, refs)| (package.as_str(), self.ownership(refs)))
            .filter(|(_, ownership)| matches!(ownership, Ownership::Ambiguous { .. }))
            .collect()
    }

    /// Todas as configs de mixin conhecidas, em ordem.
    pub fn mixin_configs(&self) -> impl Iterator<Item = &str> {
        self.mixins.keys().map(String::as_str)
    }

    /// Todos os jars que declaram a config, sem repetir o mesmo jar.
    #[must_use]
    pub fn mixin_owners(&self, config: &str) -> Vec<MixinOwner<'_>> {
        self.mixin_refs(config)
            .into_iter()
            .map(|(jar_ref, entry)| self.mixin_owner_of(jar_ref, entry))
            .collect()
    }

    /// O dono da config de mixin. Com mais de um jar declarando, valem só os descritores que o
    /// loader do pack lê; cópias do mesmo mod ficam com a que o loader carrega; mods diferentes
    /// no mesmo item ficam com o primeiro; mods diferentes em itens diferentes não têm dono.
    #[must_use]
    pub fn mixin_owner(&self, config: &str) -> Option<MixinOwner<'_>> {
        let all = self.mixin_refs(config);
        let relevant: Vec<_> = all
            .iter()
            .copied()
            .filter(|(_, entry)| declared_for(entry.declared_in, self.loader))
            .collect();
        let candidates = if relevant.is_empty() { all } else { relevant };
        let refs: Vec<JarRef> = candidates.iter().map(|(r, _)| *r).collect();
        let loaded = self.loaded_copies(&refs);
        let chosen = match loaded.as_slice() {
            [single] => *single,
            [first, rest @ ..] if rest.iter().all(|r| r.item == first.item) => *first,
            _ => return None,
        };
        let (jar_ref, entry) = candidates.into_iter().find(|(r, _)| *r == chosen)?;
        Some(self.mixin_owner_of(jar_ref, entry))
    }

    fn mixin_refs(&self, config: &str) -> Vec<(JarRef, &MixinConfigEntry)> {
        let mut out: Vec<(JarRef, &MixinConfigEntry)> = Vec::new();
        for (jar_ref, position) in self.mixins.get(config.trim()).into_iter().flatten() {
            if out.iter().any(|(r, _)| r == jar_ref) {
                continue;
            }
            if let Some(entry) = self
                .items
                .get(jar_ref.item)
                .and_then(|(_, index)| index.mixin_configs.get(*position))
            {
                out.push((*jar_ref, entry));
            }
        }
        out
    }

    fn mixin_owner_of<'a>(
        &'a self,
        jar_ref: JarRef,
        entry: &'a MixinConfigEntry,
    ) -> MixinOwner<'a> {
        let mut owner = self.owner(jar_ref);
        if let Some(declared) = entry.mod_id.as_deref() {
            owner.mod_id = Some(declared);
        }
        MixinOwner {
            owner,
            declared_in: entry.declared_in,
            present: entry.present,
        }
    }

    fn jar(&self, jar_ref: JarRef) -> Option<(&str, &IndexedJar)> {
        let (item, index) = self.items.get(jar_ref.item)?;
        Some((item.as_str(), index.jars.get(jar_ref.jar)?))
    }

    fn owner(&self, jar_ref: JarRef) -> JarOwner<'_> {
        let Some((item, jar)) = self.jar(jar_ref) else {
            return JarOwner {
                item: "",
                jar: "",
                mod_id: None,
            };
        };
        JarOwner {
            item,
            jar: &jar.path,
            mod_id: self.mod_of(jar_ref),
        }
    }

    /// O mod do jar ou, num jar sem mod (biblioteca embutida, `FMLModType: GAMELIBRARY`), o
    /// do jar mais próximo que o contém; sem nenhum, o mod do item ([`Self::top_owner`]).
    fn mod_of(&self, jar_ref: JarRef) -> Option<&str> {
        let (_, index) = self.items.get(jar_ref.item)?;
        let mut current = index.jars.get(jar_ref.jar);
        // O caminho de pais é finito: cada jar aponta para um anterior a ele.
        for _ in 0..=index.jars.len() {
            let Some(jar) = current else {
                break;
            };
            if let Some(id) = jar.primary_mod(self.loader) {
                return Some(id);
            }
            current = jar
                .parent
                .and_then(|p| usize::try_from(p).ok())
                .and_then(|p| index.jars.get(p));
        }
        self.top_owner(jar_ref.item).and_then(|owner| owner.mod_id)
    }

    /// Dono do jar de topo de um item: o mod principal dele ou, num jar que só carrega outros
    /// (Sinytra Connector 1.20.1, Kotlin for Forge), o primeiro embutido com mod.
    fn top_owner(&self, item: usize) -> Option<JarOwner<'_>> {
        let (name, index) = self.items.get(item)?;
        let mod_id = index
            .jars
            .iter()
            .find_map(|jar| jar.primary_mod(self.loader));
        Some(JarOwner {
            item: name,
            jar: "",
            mod_id,
        })
    }

    /// Uma referência por mod: entre cópias do mesmo mod (mesma identidade), a de versão maior
    /// (`FlexVer`; no empate, a primeira), que é a que o loader carrega. Jars sem mod nem artefato
    /// são cada um a sua própria identidade.
    fn loaded_copies(&self, refs: &[JarRef]) -> Vec<JarRef> {
        let mut out: Vec<(Option<&str>, Option<&str>, JarRef)> = Vec::new();
        for jar_ref in refs {
            let identity = self
                .jar(*jar_ref)
                .and_then(|(_, jar)| jar.identity(self.loader));
            let (id, version) = identity.map_or((None, None), |(id, v)| (Some(id), v));
            match out
                .iter_mut()
                .find(|(other, _, _)| id.is_some() && *other == id)
            {
                Some(slot) => {
                    if newer(version, slot.1) {
                        slot.1 = version;
                        slot.2 = *jar_ref;
                    }
                }
                None => out.push((id, version, *jar_ref)),
            }
        }
        out.into_iter().map(|(_, _, r)| r).collect()
    }

    fn ownership(&self, refs: &[JarRef]) -> Ownership<'_> {
        let loaded = self.loaded_copies(refs);
        if let [single] = loaded.as_slice() {
            return Ownership::Unique(self.owner(*single));
        }
        let same_item = loaded.windows(2).all(|w| w[0].item == w[1].item);
        Ownership::Ambiguous {
            chosen: same_item
                .then(|| loaded.first().and_then(|r| self.top_owner(r.item)))
                .flatten(),
            candidates: loaded.iter().map(|r| self.owner(*r)).collect(),
        }
    }
}

/// `candidate` é versão maior que `current` (sem versão perde para qualquer versão).
fn newer(candidate: Option<&str>, current: Option<&str>) -> bool {
    match (candidate, current) {
        (Some(a), Some(b)) => flexver::compare(a, b) == Ordering::Greater,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// O descritor é lido pelo loader (o manifesto vale para Forge e NeoForge; sem loader, todos).
fn declared_for(kind: DescriptorKind, loader: Option<Loader>) -> bool {
    match loader {
        None => true,
        Some(Loader::Fabric) => kind == DescriptorKind::FabricModJson,
        Some(Loader::Quilt) => matches!(
            kind,
            DescriptorKind::QuiltModJson | DescriptorKind::FabricModJson
        ),
        Some(Loader::Forge) => matches!(
            kind,
            DescriptorKind::ModsToml | DescriptorKind::Manifest | DescriptorKind::McmodInfo
        ),
        Some(Loader::NeoForge) => matches!(
            kind,
            DescriptorKind::NeoForgeModsToml | DescriptorKind::ModsToml | DescriptorKind::Manifest
        ),
    }
}
