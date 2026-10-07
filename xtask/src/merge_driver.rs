//! Driver de merge do `bindings.ts` (ARCHITECTURE §4.1; `.gitattributes`).
//!
//! O `bindings.ts` é gerado e versionado. Quando duas branches mexem no mesmo trecho, o git
//! resolve sozinho com a versão de quem recebe o merge (`git merge-file --ours` só decide os
//! trechos em conflito; o resto das duas lados entra). O resultado pode ficar desatualizado, e
//! isso é seguro: `cargo xtask bindings --check` (parte do `check` e da CI) falha até alguém
//! rodar `cargo xtask bindings` e incluir o arquivo no commit do merge.
//!
//! O driver mora na configuração local do repositório (o git não versiona isso), então o
//! `cargo xtask setup` o instala.

use anyhow::Result;

use crate::util::Cmd;

/// Nome usado em `.gitattributes` (`merge=bindings`).
const NAME: &str = "bindings";
/// Comando do driver: `%A` (nosso, recebe o resultado), `%O` (base), `%B` (deles).
const DRIVER: &str = "git merge-file --ours %A %O %B";

/// Instala (ou confere) o driver na configuração local do repositório.
pub fn ensure() -> Result<()> {
    let key = format!("merge.{NAME}.driver");
    let current = Cmd::new("git")
        .args(["config", "--local", "--get", &key])
        .read()
        .unwrap_or_default();
    if current.trim() == DRIVER {
        return Ok(());
    }
    Cmd::new("git")
        .args(["config", "--local", &key, DRIVER])
        .run()?;
    Cmd::new("git")
        .args([
            "config",
            "--local",
            &format!("merge.{NAME}.name"),
            "bindings.ts gerado: fica o nosso, depois rode cargo xtask bindings",
        ])
        .run()
}
