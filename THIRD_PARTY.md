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
| `serde_json` | 1.0.151 | MIT OR Apache-2.0 | `warden-app`, `xtask` | JSON (`settings.json`, relatório do `llvm-cov`, testes do contrato). |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 | `warden-app` | Erros tipados (ADR-0024). |
| `anyhow` | 1.0.104 | MIT OR Apache-2.0 | `xtask` | Erros da ferramenta de desenvolvimento (QUALITY §2.1: só no xtask e em testes). |
| `cargo_metadata` | 0.23.1 | MIT | `xtask` | Grafo de dependências (`check-deps`), pasta `target`, crates do workspace. |
| `clap` | 4.6.7 | MIT OR Apache-2.0 | `xtask` | Argumentos de `cargo xtask`. |
| `pulldown-cmark` | 0.13.4 | MIT | `xtask` | Leitura de Markdown no `check-docs`. |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 | `xtask` | Pastas temporárias (`bindings --check` e testes). |
| `toml` | 1.1.6 | MIT OR Apache-2.0 | `xtask` | Leitura de `xtask/coverage.toml`. |
| `which` | 8.0.6 | MIT | `xtask` | Localizar `pnpm.cmd`/`node` no `PATH` do Windows. |
| `base64` | 0.23.1 | MIT OR Apache-2.0 | `xtask` | Decodificar a chave embutida do packwiz para provar que ela não está no sidecar (F0-03). |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 | `xtask`, `warden-packwiz` | SHA-256 dos patches e dos executáveis no registro do `build-packwiz` (F0-03). Mesma versão que o Tauri já usa. Na `warden-packwiz` (P1-01): hashes `sha256` (índice, links) e `sha512` (Modrinth). |
| `proptest` | 1.11.0 | MIT OR Apache-2.0 | `xtask`, `warden-core`, `warden-secrets`, `warden-packwiz`, `warden-configs`, `warden-jarmeta` (testes) | Testes de propriedade dos leitores de cabeçalho PE/ELF, do commit e da versão do Go (F0-03), do `resolve_inside` e do leitor de `.env` (F0-05), de ida e volta e entradas aleatórias do formato packwiz (P1-01) e dos parsers de config (C-01). |
| `ulid` | 3.0.0 | MIT | `warden-core` | `PackId` e `OperationId` ordenáveis (ARCHITECTURE §13). |
| `tokio-util` | 0.7.19 | MIT | `warden-core` | `CancellationToken` (ARCHITECTURE §15). |
| `junction` | 2.1.0 | MIT | `warden-core` (testes, Windows) | Criar junções do NTFS nos testes do `resolve_inside`, sem `cmd /c mklink`. |
| `secrecy` | 0.10.3 | MIT OR Apache-2.0 | `warden-secrets` | `SecretString`: o `Debug` não mostra o valor (QUALITY §9). |
| `keyring-core` | 1.0.0 | MIT OR Apache-2.0 | `warden-secrets` | Cofre do sistema (ADR-0025); sucessor mantido do `keyring` 3. |
| `windows-native-keyring-store` | 1.1.0 | MIT OR Apache-2.0 | `warden-secrets` (Windows) | Gerenciador de Credenciais do Windows para o `keyring-core`. |
| `zbus-secret-service-keyring-store` | 1.0.1 | MIT OR Apache-2.0 | `warden-secrets` (Linux) | Secret Service do Linux para o `keyring-core` (só compila na CI). |
| `tracing` | 0.1.44 | MIT | `warden-secrets`, `warden-app` | Registros com contexto (ARCHITECTURE §16). |
| `tracing-subscriber` | 0.3.23 | MIT | `warden-app` | Filtro recarregável, formato do arquivo e ponte da fachada `log` (`tracing-log`). |
| `tracing-appender` | 0.2.5 | MIT | `warden-app` | Arquivo diário com 14 dias de retenção, gravado fora da thread do app. |
| `tokio` | 1.53.2 | MIT | `warden-app` | Travas por pack (`RwLock` justo), cancelamento e testes de concorrência (ARCHITECTURE §15). |
| `async-trait` | 0.1.92 | MIT OR Apache-2.0 | `warden-app` | Trait `SecretTester` (testar chaves) como objeto dinâmico. |
| `tauri-plugin-single-instance` | 2.5.2 | Apache-2.0 OR MIT | `warden-app` | Instância única do app (ADR-0019). |
| `tauri-plugin-log` | 2.10.0 | Apache-2.0 OR MIT | `warden-app` | Registros do frontend no mesmo arquivo (ARCHITECTURE §16). |
| `tauri-plugin-opener` | 2.7.0 | Apache-2.0 OR MIT | `warden-app` | Abrir links `https:` no navegador (ARCHITECTURE §20). |
| `tauri-plugin-dialog` | 2.8.1 | Apache-2.0 OR MIT | `warden-app` | Aviso de erro interno no pânico; diálogos nativos acionados pelo Rust (ARCHITECTURE §4.1). |
| `toml_edit` | 0.25.15 | MIT OR Apache-2.0 | `warden-packwiz`, `warden-configs` | Leitura de TOML e edição mínima de `pack.toml`, `index.toml` e `.pw.toml` (ARCHITECTURE §6.2); nas configs, posição de cada valor para editar só os bytes dele (ADR-0013). |
| `regex` | 1.13.1 | MIT OR Apache-2.0 | `warden-packwiz` | Matcher do `.packwizignore` com a semântica do packwiz (expressões geradas como no go-gitignore). |
| `sha1` | 0.10.7 | MIT OR Apache-2.0 | `warden-packwiz`; `warden-jarmeta` (testes) | Hash `sha1` (CurseForge); confere os jars baixados nos testes de rede da `warden-jarmeta`. |
| `md-5` | 0.10.6 | MIT OR Apache-2.0 | `warden-packwiz` | Hash `md5` (CurseForge sem `sha1`). |
| `hex` | 0.4.3 | MIT OR Apache-2.0 | `warden-packwiz` | Hashes em hexadecimal minúsculo, como o packwiz grava. |
| `jsonc-parser` | 0.34.0 | MIT | `warden-configs` | Leitura de JSON, JSONC e JSON5 com posições e comentários (ADR-0013). |
| `zip` | 8.6.0 | MIT | `warden-jarmeta` | Leitura de jars sem extrair (só `deflate`, via `flate2` + `zlib-rs`). |
| `insta` | 1.49.0 | Apache-2.0 | `warden-jarmeta` (testes) | Dourados do corpus de jars (QUALITY §4.1). |
| `ureq` | 3.4.2 | MIT OR Apache-2.0 | `warden-jarmeta` (testes) | Download dos jars reais nos testes de rede (`rede_*`). |
| `reqwest` | 0.13.5 | MIT OR Apache-2.0 | `warden-http` | Cliente HTTP (ARCHITECTURE §17), sem os recursos padrão: só `rustls-no-provider` (o TLS é montado pela `warden-http`). Mesma versão que o Tauri já usa. |
| `rustls` | 0.23.45 | Apache-2.0 OR ISC OR MIT | `warden-http` | TLS com o provedor `ring` (já usado pelo `ureq` do `xtask`), sem OpenSSL nem `aws-lc` no Windows. |
| `rustls-platform-verifier` | 0.7.1 | MIT OR Apache-2.0 | `warden-http` | Verificação de certificados pelo sistema (Windows e Linux), a mesma que o `reqwest` usa. |
| `governor` | 0.10.4 | MIT | `warden-http` | Limitador por servidor (GCRA), com o relógio injetado do cliente (ARCHITECTURE §17). |
| `httpdate` | 1.0.3 | MIT OR Apache-2.0 | `warden-http` | `Retry-After` em forma de data HTTP. |
| `bytes` | 1.12.1 | MIT | `warden-http` | Pedaços do corpo das respostas (tipo do `reqwest`). |
| `url` | 2.5.8 | MIT OR Apache-2.0 | `warden-http`, `warden-modrinth` | Links validados e montagem de consultas. |
| `wiremock` | 0.6.5 | MIT OR Apache-2.0 | `warden-http`, `warden-modrinth` (testes) | Servidor HTTP simulado: 429, retomada com `Range`, limitador por servidor (QUALITY §4.1). |
| `rusqlite` | 0.40.2 | MIT | `warden-modrinth` | Cache `metadata.sqlite` (ARCHITECTURE §13), com o SQLite embutido (`bundled`, domínio público). |
| `windows` | 0.62.2 | MIT OR Apache-2.0 | `warden-packwiz-cli` (Windows) | Job Object com `KILL_ON_JOB_CLOSE`: cancelar o packwiz mata o processo e os filhos (ADR-0024). |
| `rustix` | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | `warden-packwiz-cli` (Linux) | `SIGKILL` no grupo de processos do packwiz, sem `unsafe`. |
| `git2` | 0.21.0 | MIT OR Apache-2.0 | `warden-versioning` | Git embutido (ADR-0015): histórico de versões, tags, pontos de segurança e restauração. Sem as features `https` e `ssh` na V-01. |
| `libgit2-sys` (transitiva, `vendored-libgit2`) | 0.18.8+1.9.7 | MIT OR Apache-2.0 (a crate); a libgit2 1.9.7 compilada junto é GPL-2.0 com exceção de linkagem | `warden-versioning` | A libgit2 é compilada com o compilador C do MSVC (o mesmo que o Rust já exige no Windows) e vai dentro do executável: o computador do usuário não precisa de git instalado. Inclui zlib (`libz-sys`, Zlib). |
| `semver` | 1.0.28 | MIT OR Apache-2.0 | `warden-versioning` | Números de versão `SemVer` do pack: validação, ordem e sugestão (SPEC T16). Mesma versão que o Tauri já usa. |
| `encoding_rs` | 0.8.42 | (Apache-2.0 OR MIT) AND BSD-3-Clause | `warden-diagnostics` | Recuo para Windows-1252 nas linhas de log que não são UTF-8 (ARCHITECTURE §7.4) e variantes de codificação dos nomes na redação (D-02). |
| `unicode-normalization` | 0.1.25 | MIT OR Apache-2.0 | `warden-diagnostics` | Formas NFC/NFD dos nomes com acento na redação de dados pessoais (D-02). |
| `quick-xml` | 0.42.0 | MIT | `warden-catalog` | Leitura do `maven-metadata.xml` do Forge (lista de versões). Mesma versão que já vinha pelo `plist` do Tauri. |

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
| `crates/warden-packwiz/src/encode.rs` | `github.com/BurntSushi/toml` v1.5.0, `encode.go` | MIT | Regras de escrita de TOML que o packwiz usa (ordem das chaves, linhas em branco, aspas, escapes, decimais), portadas para gravar arquivos idênticos aos do packwiz. |
| `crates/warden-packwiz/src/ignore.rs` | `github.com/sabhiram/go-gitignore` commit `525f6e181f06`, `ignore.go`; padrões embutidos de `packwiz/packwiz` `ef87d96`, `core/index.go` | MIT | Tradução de cada linha do `.packwizignore` em expressão regular, portada linha a linha. |
| `crates/warden-packwiz/src/hash.rs` | `github.com/aviddiviner/go-murmur` `b9740d71e571`, `murmur2.go`; `packwiz/packwiz` `ef87d96`, `curseforge/murmur2` e `core/hash.go` | MIT | MurmurHash2 de 32 bits e a variante da CurseForge (sem espaços em branco, semente 1). |
| `crates/warden-packwiz/src/naming.rs`, `metafile.rs`, `pack.rs`, `index.rs` | `packwiz/packwiz` `ef87d96`: `core/mod.go` (`SlugifyName`), `core/pack.go`, `core/indexfiles.go`, `modrinth/modrinth.go` (`getSide`), `url/install.go` | MIT | Regras do formato portadas: slug de nome, migração de `pack-format`, normalização do índice, nome do arquivo de um link. |
| `crates/warden-jarmeta/tests/data/flexver_test_vectors.txt` | `test/test_vectors.txt` de [unascribed/FlexVer](https://github.com/unascribed/FlexVer) (branch `trunk`, baixado em 2026-10-04) | CC0-1.0 | Vetores oficiais de comparação do FlexVer, sem alteração. |
| `crates/warden-diagnostics/tests/corpus/codex-*/` | `test/data/` de [aternosorg/codex-minecraft](https://github.com/aternosorg/codex-minecraft), commit `d7fb6a30b8dbe9d9f73c97e4ad7927a45c3173e9` (Copyright (c) 2019-2025 Aternos GmbH) | MIT | Logs públicos do mclo.gs usados no corpus da análise pós-crash; nomes de pessoas trocados por fictícios (`tests/corpus/FIXTURES.md`); licença em `tests/corpus/LICENSE-codex-minecraft.txt`. |
| `crates/warden-diagnostics/tests/corpus/github-*/` | logs citados em issues públicas do GitHub (links em `tests/corpus/FIXTURES.md`) | sem licença declarada; trechos curtos de log usados só como dado de teste | Queda nativa no driver de vídeo e jar Fabric no NeoForge. |

## Sidecar do packwiz

| Origem | Versão | Licença | Para quê |
|---|---|---|---|
| [packwiz/packwiz](https://github.com/packwiz/packwiz) | commit em [`third_party/packwiz/COMMIT`](third_party/packwiz/COMMIT) (`ef87d96`, 2026-09-06) | MIT ([`third_party/packwiz/LICENSE`](third_party/packwiz/LICENSE)) | Refresh, validação e exportação do pack ([ADR-0007](docs/decisions/0007-sidecar-packwiz.md)). Compilado por `cargo xtask build-packwiz` com os patches de [`third_party/packwiz/patches/`](third_party/packwiz/patches/) e embutido como sidecar do Tauri. As dependências Go do packwiz vêm do `go.sum` do commit. |

## Comportamento portado (reescrito em Rust a partir da leitura, sem cópia literal)

| Arquivo | Origem | Licença | O quê |
|---|---|---|---|
| `crates/warden-jarmeta/src/version/flexver.rs` | `FlexVerComparator.java` de [unascribed/FlexVer](https://github.com/unascribed/FlexVer) | CC0-1.0 | Algoritmo FlexVer. |
| `crates/warden-jarmeta/src/version/fabric.rs` | `SemanticVersionImpl`, `VersionParser`, `VersionPredicateParser`, `VersionComparisonOperator` e os casos de `VersionParsingTests.java` do [fabric-loader](https://github.com/FabricMC/fabric-loader), commit `c75cac1` | Apache-2.0 | Versões e predicados do Fabric; os casos de teste foram portados como testes Rust. |
| `crates/warden-jarmeta/src/version/maven.rs` | `ComparableVersion`, `VersionRange`, `Restriction` e os casos de `ComparableVersionTest`/`VersionRangeTest` do [maven-artifact](https://github.com/apache/maven) 3.6.0, 3.6.3, 3.8.5, 3.8.8 e 3.9.9 (tags `maven-*`); cópia antiga das mesmas classes no FML do [MinecraftForge](https://github.com/MinecraftForge/MinecraftForge) (branches `1.7.10` e `1.12.x`) | Apache-2.0; LGPL-2.1-only (FML) | Versões e faixas Maven em cada variante que o Forge e o NeoForge usam; os casos de teste foram portados. |
| `crates/warden-jarmeta/src/version/mod_annotation.rs` | `DependencyParser.java` do [MinecraftForge](https://github.com/MinecraftForge/MinecraftForge), branch `1.12.x` | LGPL-2.1-only | Gramática de `@Mod.dependencies`. |
| `crates/warden-diagnostics/data/log-patterns.toml` | classes de problema de `src/Analysis/Problem/` (CrashReport, Fabric, Forge, Vanilla) do [aternosorg/codex-minecraft](https://github.com/aternosorg/codex-minecraft), commit `d7fb6a30b8dbe9d9f73c97e4ad7927a45c3173e9` (Copyright (c) 2019-2025 Aternos GmbH) | MIT | Padrões de log da análise pós-crash: as expressões foram reescritas para a crate `regex` e para linhas limpas; a origem de cada padrão está no campo `origin`. Os demais padrões vêm da pesquisa R2 §6.3 (mensagens-fonte dos loaders) e do spike S-R5-3. |
