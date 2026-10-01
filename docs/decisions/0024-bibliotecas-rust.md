# ADR-0024 — Bibliotecas Rust principais

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão técnica (A1)

## Contexto

Várias crates precisam das mesmas bases (assíncrono, HTTP, erros, TOML, zip, git, cofre). Escolher uma vez evita duplicação e conflitos.

## Decisão

| Uso | Escolha | Motivo |
|---|---|---|
| Assíncrono | `tokio`, `tokio-util` (`CancellationToken`), `async-trait` | Runtime do Tauri; cancelamento cooperativo. |
| Erros | `thiserror` 2 (`anyhow` só em xtask/testes) | Erros tipados com códigos. |
| TOML | `toml` (leitura) e `toml_edit` (edição preservando formato) | Formato packwiz e configs. |
| HTTP | `reqwest` com `rustls`, `governor` | Sem OpenSSL do sistema; limitador por host. |
| Hash | `sha1`, `sha2`, `md-5`; murmur2 próprio | Formatos do packwiz e da CurseForge. |
| Zip | `zip` | Jars e exportação. |
| Git | `git2` (libgit2 vendorizada) | Push com credencial em memória (ADR-0015). |
| Cofre | `keyring`, `secrecy` | ADR-0017. |
| Banco local | `rusqlite` (bundled) | Cache de metadados e índice de downloads. |
| Configs | `jsonc-parser` (cst), `json-five`, parsers próprios | ADR-0013. |
| Logs | `tracing`, `tracing-subscriber`, `tracing-appender` | ADR-0023. |
| Texto | `regex`, `quick-xml`, `encoding_rs` | Logs do jogo. |
| Vigiar pastas | `notify` | Mudanças externas e Downloads. |
| IDs | `ulid`, `uuid` | `PackId`; UUID offline. |
| Windows | `windows` | Job Objects. |
| Testes | `cargo-nextest`, `insta`, `wiremock`, `tempfile`, `proptest`, `criterion`, `cargo-llvm-cov`, `cargo-deny` | QUALITY §4. |

## Alternativas consideradas

- `ureq` (síncrono): não combina com cancelamento e streaming no runtime assíncrono.
- `gix`: ver ADR-0015.
- `serde_yaml`: marcado como descontinuado (R3 §5.6).

## Consequências

- Versões declaradas uma vez em `[workspace.dependencies]`; novas dependências seguem QUALITY §10.
