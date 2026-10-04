# Código e dependências de terceiros

Registro das dependências diretas do Warden e de todo trecho copiado ou portado, com a licença (QUALITY §10). O Warden é privado e nunca distribuído ([ADR-0004](docs/decisions/0004-uso-privado-e-licencas.md)); mesmo assim, tudo fica registrado aqui.

Uma seção por origem (registro acréscimo-apenas, ROADMAP §1). As versões exatas ficam no `Cargo.lock` e no `pnpm-lock.yaml`; as listadas abaixo são as fixadas quando a dependência entrou. As dependências transitivas do Rust são conferidas pelo `cargo deny check` (licenças permitidas em [`deny.toml`](deny.toml)); os avisos completos de terceiros exibidos no app vêm da A-02.

## Crates Rust (crates.io)

| Crate | Versão | Licença | Usada em | Para quê |
|---|---|---|---|---|
| `tauri` | 2.12.1 | Apache-2.0 OR MIT | `warden-app` | App desktop (ADR-0002). |
| `tauri-build` | 2.7.1 | Apache-2.0 OR MIT | `warden-app` (build) | Contexto e recursos do app no build. |
| `specta` | =2.0.0-rc.25 | MIT | `warden-app` | Tipos Rust → TypeScript (ADR-0018). Versão RC, fixada com `=`. |
| `specta-typescript` | =0.0.12 | MIT | `warden-app` | Exportação do `bindings.ts`. |
| `tauri-specta` | =2.0.0-rc.25 | MIT | `warden-app` | Comandos tipados e geração do `bindings.ts` (ADR-0018). Versão RC, fixada com `=`. |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | `warden-app`, `xtask` | Serialização. |
| `serde_json` | 1.0.151 | MIT OR Apache-2.0 | `warden-app` (testes), `xtask` | JSON (relatório do `llvm-cov`, testes do contrato). |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 | `warden-app` | Erros tipados (ADR-0024). |
| `anyhow` | 1.0.104 | MIT OR Apache-2.0 | `xtask` | Erros da ferramenta de desenvolvimento (QUALITY §2.1: só no xtask e em testes). |
| `cargo_metadata` | 0.23.1 | MIT | `xtask` | Grafo de dependências (`check-deps`), pasta `target`, crates do workspace. |
| `clap` | 4.6.7 | MIT OR Apache-2.0 | `xtask` | Argumentos de `cargo xtask`. |
| `pulldown-cmark` | 0.13.4 | MIT | `xtask` | Leitura de Markdown no `check-docs`. |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 | `xtask` | Pastas temporárias (`bindings --check` e testes). |
| `toml` | 1.1.6 | MIT OR Apache-2.0 | `xtask` | Leitura de `xtask/coverage.toml`. |
| `which` | 8.0.6 | MIT | `xtask` | Localizar `pnpm.cmd`/`node` no `PATH` do Windows. |
| `base64` | 0.23.1 | MIT OR Apache-2.0 | `xtask` | Decodificar a chave embutida do packwiz para provar que ela não está no sidecar (F0-03). |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 | `xtask` | SHA-256 dos patches e dos executáveis no registro do `build-packwiz` (F0-03). Mesma versão que o Tauri já usa. |
| `proptest` | 1.11.0 | MIT OR Apache-2.0 | `xtask`, `warden-core`, `warden-secrets` (testes) | Testes de propriedade dos leitores de cabeçalho PE/ELF, do commit e da versão do Go (F0-03), do `resolve_inside` e do leitor de `.env` (F0-05). |
| `ulid` | 3.0.0 | MIT | `warden-core` | `PackId` e `OperationId` ordenáveis (ARCHITECTURE §13). |
| `tokio-util` | 0.7.19 | MIT | `warden-core` | `CancellationToken` (ARCHITECTURE §15). |
| `junction` | 2.1.0 | MIT | `warden-core` (testes, Windows) | Criar junções do NTFS nos testes do `resolve_inside`, sem `cmd /c mklink`. |
| `secrecy` | 0.10.3 | MIT OR Apache-2.0 | `warden-secrets` | `SecretString`: o `Debug` não mostra o valor (QUALITY §9). |
| `keyring-core` | 1.0.0 | MIT OR Apache-2.0 | `warden-secrets` | Cofre do sistema (ADR-0025); sucessor mantido do `keyring` 3. |
| `windows-native-keyring-store` | 1.1.0 | MIT OR Apache-2.0 | `warden-secrets` (Windows) | Gerenciador de Credenciais do Windows para o `keyring-core`. |
| `zbus-secret-service-keyring-store` | 1.0.1 | MIT OR Apache-2.0 | `warden-secrets` (Linux) | Secret Service do Linux para o `keyring-core` (só compila na CI). |
| `tracing` | 0.1.44 | MIT | `warden-secrets`, `warden-app` | Registros com contexto (ARCHITECTURE §16). |

## Ferramentas instaladas por `cargo xtask setup` (crates.io, `cargo install --locked`)

| Ferramenta | Versão | Licença | Para quê |
|---|---|---|---|
| `cargo-nextest` | 0.9.146 | Apache-2.0 OR MIT | Testes Rust (QUALITY §4). |
| `cargo-deny` | 0.20.2 | MIT OR Apache-2.0 | Licenças, avisos de segurança, fontes e duplicatas. |
| `cargo-llvm-cov` | 0.9.1 | Apache-2.0 OR MIT | Cobertura (QUALITY §4.2). |
| `tauri-cli` | 2.12.1 | Apache-2.0 OR MIT | `cargo tauri dev` e `cargo tauri build`. |
| `tauri-driver` | 2.1.0 | Apache-2.0 OR MIT | E2E com WebdriverIO (F0-06). |

## Pacotes npm (`apps/desktop`)

| Pacote | Versão | Licença | Para quê |
|---|---|---|---|
| `react`, `react-dom` | 19.3.0 | MIT | Interface (ADR-0002). |
| `@tanstack/react-router` | 1.170.41 | MIT | Rotas por arquivo (ADR-0020). |
| `@tanstack/react-query` | 5.104.1 | MIT | Dados do backend (ADR-0020). |
| `i18next` | 26.4.2 | MIT | Catálogo de textos pt-BR (ADR-0016). |
| `react-i18next` | 17.0.15 | MIT | i18next no React. |
| `@tauri-apps/api` | 2.12.1 | Apache-2.0 OR MIT | IPC com o Rust; `mocks` nos testes. Mesma versão menor do `tauri`. |
| `vite` | 8.3.2 | MIT | Servidor de desenvolvimento e build. |
| `@vitejs/plugin-react` | 6.1.1 | MIT | React no Vite. |
| `@tanstack/router-plugin` | 1.168.42 | MIT | Gera `routeTree.gen.ts` no Vite. |
| `@tanstack/router-cli` | 1.167.40 | MIT | Gera `routeTree.gen.ts` antes de typecheck, lint e testes (`tsr generate`). |
| `typescript` | 6.0.3 | Apache-2.0 | Tipos. Fica no 6.0 porque o `typescript-eslint` 8.71 aceita só TypeScript < 6.1. |
| `eslint` | 9.39.5 | MIT | Lint. Fica no 9 porque `eslint-plugin-jsx-a11y` e `eslint-plugin-react` declaram suporte só até o ESLint 9. |
| `@eslint/js` | 9.39.5 | MIT | Regras recomendadas do ESLint. |
| `typescript-eslint` | 8.71.0 | MIT | `strictTypeChecked` e `stylisticTypeChecked` (QUALITY §2.2). |
| `eslint-plugin-react` | 7.37.5 | MIT | `react/no-danger` e regras do React. |
| `eslint-plugin-react-hooks` | 7.1.1 | MIT | Regras dos hooks. |
| `eslint-plugin-jsx-a11y` | 6.10.2 | MIT | Acessibilidade (strict). |
| `eslint-plugin-i18next` | 6.1.5 | ISC | Proíbe texto solto em JSX. |
| `globals` | 17.13.0 | MIT | Globais do navegador e do Node no ESLint. |
| `prettier` | 3.9.9 | MIT | Formatação. |
| `vitest` | 5.0.3 | MIT | Testes de componente. |
| `@vitest/coverage-v8` | 5.0.3 | MIT | Cobertura do frontend. |
| `@testing-library/react` | 16.3.3 | MIT | Testes de componente. |
| `@testing-library/dom` | 10.4.2 | MIT | Dependência par do Testing Library. |
| `jsdom` | 30.1.1 | MIT | DOM nos testes. |
| `@types/node` | 24.19.1 | MIT | Tipos do Node 24 (arquivos de configuração). |
| `@types/react`, `@types/react-dom` | 19.3.0 | MIT | Tipos do React. |

## Trechos copiados

| Arquivo | Origem | Licença | O quê |
|---|---|---|---|
| `apps/desktop/src-tauri/windows-app-manifest.xml` | `tauri-build` 2.7.1, `src/windows-app-manifest.xml` ([tauri-apps/tauri](https://github.com/tauri-apps/tauri)) | Apache-2.0 OR MIT | Manifesto do Windows que pede o Common Controls v6, embutido pelo linker em todos os alvos da `warden-app` (inclusive testes). |

## Sidecar do packwiz

| Origem | Versão | Licença | Para quê |
|---|---|---|---|
| [packwiz/packwiz](https://github.com/packwiz/packwiz) | commit em [`third_party/packwiz/COMMIT`](third_party/packwiz/COMMIT) (`ef87d96`, 2026-09-06) | MIT ([`third_party/packwiz/LICENSE`](third_party/packwiz/LICENSE)) | Refresh, validação e exportação do pack ([ADR-0007](docs/decisions/0007-sidecar-packwiz.md)). Compilado por `cargo xtask build-packwiz` com os patches de [`third_party/packwiz/patches/`](third_party/packwiz/patches/) e embutido como sidecar do Tauri. As dependências Go do packwiz vêm do `go.sum` do commit. |
