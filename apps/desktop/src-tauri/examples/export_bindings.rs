//! Gera o `bindings.ts` no caminho recebido como argumento. Chamado por
//! `cargo xtask bindings`; não use direto.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        return ExitCode::from(2);
    };
    match warden_app::export_bindings(&path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // Ferramenta de desenvolvimento: o erro vai para o terminal do xtask.
            #[allow(clippy::print_stderr)]
            {
                eprintln!("falha ao gerar o bindings.ts: {error}");
            }
            ExitCode::FAILURE
        }
    }
}
