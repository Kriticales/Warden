//! O repositório git de um pack (ARCHITECTURE §11; ADR-0015).
//!
//! [`PackRepo`] abre ou cria o repositório na raiz do pack (nunca procura um repositório em
//! pastas acima: um pack dentro de outro repositório não usa o histórico do outro) e confere,
//! antes de cada escrita, que o histórico não está num estado especial deixado por fora
//! (merge, rebase, conflitos, HEAD destacado).
//!
//! Repositórios criados pelo Warden têm uma branch `main` com o ponto inicial ("Pack criado"
//! ou "Pack importado") e as versões salvas. Repositórios importados mantêm a branch atual e
//! o histórico que já tinham.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use git2::{Commit, ErrorCode, Oid, Repository, RepositoryOpenFlags, RepositoryState, Signature};

use crate::error::{Error, GitContext as _, Result};
use crate::moment::Moment;

/// Branch criada pelo Warden em packs novos.
pub const MAIN_BRANCH: &str = "main";

/// E-mail dos commits quando o GitHub não está configurado (ARCHITECTURE §11).
pub const LOCAL_EMAIL: &str = "warden@localhost";

/// Mensagem do ponto inicial de um pack criado pelo Warden.
pub const CREATED_MESSAGE: &str = "Pack criado";

/// Mensagem do ponto inicial de um pack importado sem histórico.
pub const IMPORTED_MESSAGE: &str = "Pack importado";

/// Quem assina os commits, as tags e os pontos de segurança.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// Nome (o autor do pack).
    pub name: String,
    /// E-mail.
    pub email: String,
}

impl Identity {
    /// Identidade de um pack: o nome do autor do `pack.toml` (ou "Warden", se vazio) e o
    /// e-mail `<login>@users.noreply.github.com` quando o GitHub está configurado, senão
    /// [`LOCAL_EMAIL`] (ARCHITECTURE §11).
    #[must_use]
    ///
    /// `<`, `>` e quebras de linha saem do nome e do login (a libgit2 recusa assinaturas com
    /// eles), para um autor como "Ana <ana@x>" não impedir de salvar versões.
    pub fn for_pack(author: &str, github_login: Option<&str>) -> Self {
        let clean = |text: &str| {
            text.chars()
                .filter(|c| !matches!(c, '<' | '>') && !c.is_control())
                .collect::<String>()
                .trim()
                .to_owned()
        };
        let name = clean(author);
        let name = if name.is_empty() {
            "Warden".to_owned()
        } else {
            name
        };
        let login = github_login.map(clean).unwrap_or_default();
        let email = if login.is_empty() {
            LOCAL_EMAIL.to_owned()
        } else {
            format!("{login}@users.noreply.github.com")
        };
        Self { name, email }
    }

    pub(crate) fn signature(&self, when: Moment) -> Result<Signature<'static>> {
        Signature::new(&self.name, &self.email, &when.to_git()).ctx("montar a assinatura")
    }
}

/// Qual ponto inicial gravar ao criar o repositório.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialPoint {
    /// Pack criado pelo Warden ("Pack criado").
    Created,
    /// Pack importado sem histórico ("Pack importado").
    Imported,
}

impl InitialPoint {
    fn message(self) -> &'static str {
        match self {
            Self::Created => CREATED_MESSAGE,
            Self::Imported => IMPORTED_MESSAGE,
        }
    }
}

/// O repositório git de um pack.
pub struct PackRepo {
    pub(crate) repo: Repository,
    pub(crate) root: PathBuf,
}

impl std::fmt::Debug for PackRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackRepo")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl PackRepo {
    /// Abre o repositório cuja árvore de trabalho é exatamente `root`.
    ///
    /// Erros: [`Error::NotVersioned`] se `root` não for a raiz de um repositório (inclusive
    /// quando só uma pasta acima é um repositório); [`Error::RepoNotOwned`] se a libgit2
    /// recusar um repositório de outro usuário.
    pub fn open(root: &Path) -> Result<Self> {
        let repo = match Repository::open_ext(
            root,
            RepositoryOpenFlags::NO_SEARCH,
            std::iter::empty::<&OsStr>(),
        ) {
            Ok(repo) => repo,
            Err(error) if error.code() == ErrorCode::NotFound => {
                return Err(Error::NotVersioned {
                    root: root.to_path_buf(),
                });
            }
            Err(error) if error.code() == ErrorCode::Owner => {
                return Err(Error::RepoNotOwned {
                    root: root.to_path_buf(),
                    source: error,
                });
            }
            Err(error) => return Err(Error::git("abrir o repositório", error)),
        };
        if repo.workdir().is_none() {
            return Err(Error::NotVersioned {
                root: root.to_path_buf(),
            });
        }
        Ok(Self {
            repo,
            root: root.to_path_buf(),
        })
    }

    /// Cria o repositório em `root` com a branch [`MAIN_BRANCH`] e grava o ponto inicial com
    /// todos os arquivos do pack que o `.gitignore` não exclui (ARCHITECTURE §11).
    ///
    /// O repositório nasce com `core.autocrlf = false` e `core.longpaths = true`: quem decide
    /// o fim de linha é o `.gitattributes` do pack (gerado pela P1-01 com `* -text`), não a
    /// configuração global do computador. Os arquivos de controle (`.gitignore`,
    /// `.gitattributes`) são gravados antes por quem cria o pack.
    ///
    /// Erros: [`Error::AlreadyVersioned`] se `root` já for a raiz de um repositório.
    pub fn init(
        root: &Path,
        initial: InitialPoint,
        identity: &Identity,
        when: Moment,
    ) -> Result<Self> {
        match Self::open(root) {
            Ok(_) => {
                return Err(Error::AlreadyVersioned {
                    root: root.to_path_buf(),
                });
            }
            Err(Error::NotVersioned { .. }) => {}
            Err(error) => return Err(error),
        }
        let mut options = git2::RepositoryInitOptions::new();
        options
            .initial_head(MAIN_BRANCH)
            .no_reinit(true)
            .mkpath(false);
        let repo = Repository::init_opts(root, &options).ctx("criar o repositório")?;
        {
            let mut config = repo.config().ctx("ler a configuração do repositório")?;
            config
                .set_bool("core.autocrlf", false)
                .ctx("gravar a configuração do repositório")?;
            config
                .set_bool("core.longpaths", true)
                .ctx("gravar a configuração do repositório")?;
        }
        let pack = Self {
            repo,
            root: root.to_path_buf(),
        };
        {
            let tree = pack.stage_all()?;
            let signature = identity.signature(when)?;
            let tree = pack
                .repo
                .find_tree(tree)
                .ctx("ler a árvore do ponto inicial")?;
            pack.repo
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    initial.message(),
                    &tree,
                    &[],
                )
                .ctx("gravar o ponto inicial")?;
        }
        Ok(pack)
    }

    /// Raiz do pack (a árvore de trabalho do repositório).
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Nome da branch atual (`None` com HEAD destacado).
    pub fn branch_name(&self) -> Result<Option<String>> {
        if self.repo.head_detached().ctx("ler o HEAD")? {
            return Ok(None);
        }
        let head = self.repo.find_reference("HEAD").ctx("ler o HEAD")?;
        let target = String::from_utf8_lossy(head.symbolic_target_bytes().unwrap_or_default());
        Ok(target.strip_prefix("refs/heads/").map(ToOwned::to_owned))
    }

    /// Commit do HEAD (`None` num repositório ainda sem commits).
    pub(crate) fn head_commit(&self) -> Result<Option<Commit<'_>>> {
        match self.repo.head() {
            Ok(head) => Ok(Some(head.peel_to_commit().ctx("ler o commit do HEAD")?)),
            Err(error) if matches!(error.code(), ErrorCode::UnbornBranch | ErrorCode::NotFound) => {
                Ok(None)
            }
            Err(error) => Err(Error::git("ler o HEAD", error)),
        }
    }

    /// Id do commit do HEAD, em hexadecimal (`None` sem commits).
    pub fn head_id(&self) -> Result<Option<String>> {
        Ok(self.head_commit()?.map(|commit| commit.id().to_string()))
    }

    /// Confere que o histórico aceita escrita: sem operação em andamento, sem HEAD destacado
    /// e sem conflitos (ARCHITECTURE §11).
    ///
    /// Erros: [`Error::RepoBusy`], [`Error::DetachedHead`], [`Error::Conflicts`].
    pub fn check_writable(&self) -> Result<()> {
        let state = match self.repo.state() {
            RepositoryState::Clean => None,
            RepositoryState::Merge => Some("merge"),
            RepositoryState::Revert | RepositoryState::RevertSequence => Some("revert"),
            RepositoryState::CherryPick | RepositoryState::CherryPickSequence => {
                Some("cherry-pick")
            }
            RepositoryState::Bisect => Some("bisect"),
            RepositoryState::Rebase
            | RepositoryState::RebaseInteractive
            | RepositoryState::RebaseMerge => Some("rebase"),
            RepositoryState::ApplyMailbox | RepositoryState::ApplyMailboxOrRebase => Some("am"),
        };
        if let Some(state) = state {
            return Err(Error::RepoBusy {
                state: state.to_owned(),
            });
        }
        if self.repo.head_detached().ctx("ler o HEAD")? {
            return Err(Error::DetachedHead);
        }
        let index = self.fresh_index()?;
        if index.has_conflicts() {
            let mut paths = Vec::new();
            for conflict in index.conflicts().ctx("ler os conflitos")? {
                let conflict = conflict.ctx("ler os conflitos")?;
                if let Some(entry) = conflict.our.or(conflict.their).or(conflict.ancestor) {
                    paths.push(String::from_utf8_lossy(&entry.path).into_owned());
                }
            }
            paths.sort();
            paths.dedup();
            return Err(Error::Conflicts { paths });
        }
        Ok(())
    }

    /// O índice do repositório relido do disco. A libgit2 guarda o índice em memória na
    /// abertura; sem reler, uma mudança feita por fora (um `git add`, um conflito) passaria
    /// despercebida.
    pub(crate) fn fresh_index(&self) -> Result<git2::Index> {
        let mut index = self.repo.index().ctx("ler o índice")?;
        index.read(true).ctx("ler o índice")?;
        Ok(index)
    }

    /// Põe no índice todos os arquivos da árvore de trabalho (`git add -A`), grava o índice e
    /// devolve a árvore gravada.
    pub(crate) fn stage_all(&self) -> Result<Oid> {
        let mut index = self.fresh_index()?;
        let result = (|| {
            index
                .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
                .ctx("adicionar os arquivos ao índice")?;
            index.update_all(["*"], None).ctx("atualizar o índice")?;
            let tree = index.write_tree().ctx("gravar a árvore")?;
            index.write().ctx("gravar o índice")?;
            Ok(tree)
        })();
        if result.is_err() {
            // Volta o índice em memória ao que está no disco; se nem isso der, o próximo
            // `fresh_index` relê o arquivo de qualquer forma.
            let _ = index.read(true);
        }
        result
    }

    /// Commit apontado por uma referência, se ela existir.
    pub(crate) fn reference_commit(&self, name: &str) -> Result<Option<Oid>> {
        match self.repo.find_reference(name) {
            Ok(reference) => Ok(Some(
                reference
                    .peel_to_commit()
                    .ctx("ler o commit da referência")?
                    .id(),
            )),
            Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
            Err(error) => Err(Error::git("ler a referência", error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identidade_do_pack() {
        assert_eq!(
            Identity::for_pack("Kriticales", None),
            Identity {
                name: "Kriticales".into(),
                email: LOCAL_EMAIL.into()
            }
        );
        assert_eq!(
            Identity::for_pack("  ", Some("dono")).email,
            "dono@users.noreply.github.com"
        );
        assert_eq!(Identity::for_pack("  ", Some(" ")).name, "Warden");
        assert_eq!(Identity::for_pack("Ana", Some(" ")).email, LOCAL_EMAIL);
        let odd = Identity::for_pack(
            "Ana <ana@x>
",
            Some("<dono>"),
        );
        assert_eq!(odd.name, "Ana ana@x");
        assert_eq!(odd.email, "dono@users.noreply.github.com");
        assert!(odd.signature(Moment::new(0, 0)).is_ok());
        assert_eq!(Identity::for_pack("<>", None).name, "Warden");
    }
}
