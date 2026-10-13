//! As conferências do `publish_plan` como uma lista de checagens plugáveis (ARCHITECTURE
//! §21.7, gancho 6; ADR-0039).
//!
//! Cada checagem recebe o mesmo [`PublishContext`] (a versão e a árvore que vai para o
//! GitHub) e devolve achados do tipo que o app define (a interface os mostra como avisos ou
//! bloqueios). A v1 registra a varredura de segredos, a higiene, os mods da CurseForge, a
//! versão não testada e os problemas; a W-03 (1.1) acrescenta "Segurança dos mods" e
//! "Manutenção dos mods" sem mexer nas outras.

use std::path::Path;
use std::sync::Arc;

use warden_core::CancellationToken;

use super::PublishTree;

/// O que toda checagem recebe.
#[derive(Debug, Clone, Copy)]
pub struct PublishContext<'a> {
    /// Pasta do pack.
    pub root: &'a Path,
    /// Versão a publicar.
    pub version: &'a str,
    /// O conteúdo que vai para o GitHub (`pack.toml`, `index.toml` e os arquivos do índice).
    pub tree: &'a PublishTree,
    /// Cancelamento da operação.
    pub cancel: &'a CancellationToken,
}

/// Um achado que pode impedir a publicação.
pub trait PublishFindingKind {
    /// Impede publicar (os demais são avisos: "Publicar mesmo assim").
    fn blocks(&self) -> bool;
}

/// Uma conferência do `publish_plan`. Falhas próprias (sem rede, por exemplo) viram achados
/// de aviso do tipo do app, nunca erro: uma checagem fora do ar não impede as outras.
#[async_trait::async_trait]
pub trait PublishCheck: Send + Sync {
    /// O tipo dos achados (definido pelo app).
    type Finding: PublishFindingKind + Send;

    /// Nome estável (registros e testes).
    fn id(&self) -> &'static str;

    /// Roda a conferência.
    async fn run(&self, context: &PublishContext<'_>) -> Vec<Self::Finding>;
}

/// Uma lista de checagens com o mesmo tipo de achado.
pub type PublishChecks<F> = Vec<Arc<dyn PublishCheck<Finding = F>>>;

/// Roda as checagens na ordem da lista e junta os achados. Para no cancelamento.
pub async fn run_checks<F>(
    checks: &[Arc<dyn PublishCheck<Finding = F>>],
    context: &PublishContext<'_>,
) -> Vec<F>
where
    F: PublishFindingKind + Send,
{
    let mut findings = Vec::new();
    for check in checks {
        if context.cancel.is_cancelled() {
            break;
        }
        let found = check.run(context).await;
        tracing::debug!(
            checagem = check.id(),
            achados = found.len(),
            "conferência da publicação"
        );
        findings.extend(found);
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    enum Achado {
        Aviso(&'static str),
        Bloqueio,
    }

    impl PublishFindingKind for Achado {
        fn blocks(&self) -> bool {
            matches!(self, Self::Bloqueio)
        }
    }

    struct Fixa(&'static str, bool);

    #[async_trait::async_trait]
    impl PublishCheck for Fixa {
        type Finding = Achado;
        fn id(&self) -> &'static str {
            self.0
        }
        async fn run(&self, context: &PublishContext<'_>) -> Vec<Achado> {
            assert_eq!(context.version, "1.0.0");
            vec![if self.1 {
                Achado::Bloqueio
            } else {
                Achado::Aviso(self.0)
            }]
        }
    }

    #[tokio::test]
    async fn checagens_rodam_em_ordem_e_uma_nova_entra_sem_mexer_nas_outras() {
        let tree = PublishTree::new();
        let cancel = CancellationToken::new();
        let context = PublishContext {
            root: Path::new("."),
            version: "1.0.0",
            tree: &tree,
            cancel: &cancel,
        };
        let mut checks: PublishChecks<Achado> = vec![Arc::new(Fixa("a", false))];
        checks.push(Arc::new(Fixa("seguranca-1.1", true)));
        let found = run_checks(&checks, &context).await;
        assert_eq!(found, vec![Achado::Aviso("a"), Achado::Bloqueio]);
        assert!(found.iter().any(PublishFindingKind::blocks));
        cancel.cancel();
        assert!(run_checks(&checks, &context).await.is_empty());
    }
}
