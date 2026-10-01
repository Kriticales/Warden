# Warden — Padrão de qualidade obrigatório

> Versão do documento: 1.1 (2026-10-01). Tarefa A1; glossário e regra de segredos atualizados na tarefa D2 (decisões do dono, ADR-0025 a ADR-0029).
> Vale para **todos** os agentes e para o orquestrador. Uma entrega que não cumpre este documento não é integrada.
> Referências: `ARCHITECTURE.md` (estrutura), `SPEC.md` (critérios de aceite), `ROADMAP.md` (tarefas), `docs/decisions/` (ADRs).

## Sumário

1. [Princípios](#1-princípios)
2. [Formatação e lint](#2-formatação-e-lint)
3. [Tratamento de erros](#3-tratamento-de-erros)
4. [Testes](#4-testes)
5. [Definição de pronto](#5-definição-de-pronto)
6. [Checklist de revisão do orquestrador](#6-checklist-de-revisão-do-orquestrador)
7. [Commits e branches](#7-commits-e-branches)
8. [Idioma, textos e glossário (pt-BR)](#8-idioma-textos-e-glossário-pt-br)
9. [Segurança e privacidade](#9-segurança-e-privacidade)
10. [Dependências e código de terceiros](#10-dependências-e-código-de-terceiros)
11. [Código e documentação](#11-código-e-documentação)
12. [Comandos de verificação](#12-comandos-de-verificação)

---

## 1. Princípios

1. **Funciona de verdade.** Comportamento que envolve packwiz, Java, o jogo ou APIs externas só está provado quando foi exercitado contra a coisa real (packwiz compilado do commit fixado, packwiz-installer real, jogo real na matriz de versões, APIs reais nos testes de rede). Mocks complementam, não substituem.
2. **Honestidade.** Relatórios separam "verifiquei executando" de "inferi lendo". Teste pulado, flaky ou desativado é declarado no relatório, nunca escondido.
3. **Fluxo básico antes de enfeite.** Nenhum esforço em animação, tema ou meta-ferramenta enquanto um fluxo P0 da mesma área não estiver provado (R4 §4.1 item 12).
4. **Fatias verticais.** Funcionalidade = domínio + comando + tela + teste ponta a ponta. Backend sem tela ou tela sem backend não conta como entrega (R4 §2.4).
5. **O pack é sagrado.** Nenhum código escreve no pack fora da `PackTransaction` (ARCHITECTURE §6.5) e nada que não seja conteúdo do pack.
6. **Pequeno e revisável.** Commits pequenos, cada um compilando e passando a verificação rápida.

## 2. Formatação e lint

### 2.1 Rust

- Formatação: `cargo fmt --all`. `rustfmt.toml` com `edition = "2024"`, `max_width = 100`, `newline_style = "Unix"`, `use_field_init_shorthand = true` (só opções estáveis).
- Lint: `cargo clippy --workspace --all-targets --all-features -- -D warnings` (qualquer aviso falha).
- `[workspace.lints]` (herdado por todas as crates com `[lints] workspace = true`):

```toml
[workspace.lints.rust]
unsafe_code = "deny"                 # exceção só com #[allow(unsafe_code)] + comentário SAFETY (ARCHITECTURE §7.4)
missing_docs = "warn"                # itens públicos documentados
unreachable_pub = "warn"
rust_2018_idioms = { level = "warn", priority = -1 }

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
unimplemented = "deny"
dbg_macro = "deny"
print_stdout = "deny"
print_stderr = "deny"
large_futures = "warn"
module_name_repetitions = "allow"
missing_errors_doc = "allow"
```

- `clippy.toml`: `allow-unwrap-in-tests = true`, `allow-expect-in-tests = true`, `allow-panic-in-tests = true`.
- `#[allow(...)]` só com comentário explicando o motivo, no menor escopo possível. `#[allow(clippy::all)]` e `#[allow(warnings)]` são proibidos.
- `xtask` não herda `[workspace.lints]` (declara os próprios) e pode usar `anyhow` e `println!`; nenhuma crate de domínio pode.

### 2.2 TypeScript e React

- Formatação: Prettier (`printWidth: 100`, `singleQuote: true`, `trailingComma: "all"`, `semi: true`). `pnpm format:check` na CI.
- `tsconfig.json`: `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noImplicitOverride`, `noImplicitReturns`, `noFallthroughCasesInSwitch`, `noUnusedLocals`, `noUnusedParameters`, `verbatimModuleSyntax`, `isolatedModules`, `skipLibCheck: false` para o código do app.
- ESLint (flat config): `typescript-eslint` `strictTypeChecked` + `stylisticTypeChecked`, `eslint-plugin-react-hooks` (recommended), `eslint-plugin-jsx-a11y` (strict), `eslint-plugin-i18next` (`no-literal-string` em JSX), e:
  - `@typescript-eslint/no-explicit-any`: erro; `@typescript-eslint/no-non-null-assertion`: erro;
  - `@typescript-eslint/switch-exhaustiveness-check`: erro;
  - `@typescript-eslint/no-floating-promises`, `no-misused-promises`: erro;
  - `@typescript-eslint/consistent-type-imports`: erro;
  - `no-console`: erro (usar o logger que encaminha ao `tauri-plugin-log`);
  - `no-restricted-imports`: `@tauri-apps/api/core` (`invoke`) proibido fora de `src/lib/ipc/`;
  - `react/no-danger` (de `eslint-plugin-react`): erro, com exceção configurada só para o arquivo do componente `SafeHtml`.
- `pnpm lint` com `--max-warnings 0`.
- Arquivos gerados (`bindings.ts`, `routeTree.gen.ts`) são excluídos do lint e da formatação, nunca editados à mão.

### 2.3 Outros

- `.editorconfig`: UTF-8, LF, indentação de 2 espaços (TS/JSON/YAML/TOML) e 4 (Rust), linha final.
- Workflows do GitHub validados com `actionlint`.
- Markdown dos docs: títulos em sentence case, links relativos válidos (`cargo xtask check-docs` verifica links internos).

## 3. Tratamento de erros

1. **Nada de pânico em produção.** Sem `unwrap`/`expect`/`panic!`/índice que possa estourar em código que roda no app. Invariantes impossíveis viram erro `INTERNAL` com contexto.
2. **Erro tipado por crate** (`thiserror`), com contexto suficiente para agir: caminho, URL, hash esperado/obtido, código de saída, versão.
3. **Código estável** para tudo que chega à interface (ARCHITECTURE §5). Todo código novo tem frase pt-BR em `src/i18n/errors/<domínio>.ts`; o TypeScript falha se faltar.
4. **Frase = o que aconteceu + o que fazer.** Ex.: "A CurseForge recusou a chave. Confira a chave em Configurações." Nunca "Erro desconhecido" sem ação; nunca a mensagem crua da ferramenta como frase principal (ela vai em "Detalhes técnicos").
5. **Nunca engolir erro.** `let _ = ...` em `Result` só com comentário justificando. Erros não fatais viram `Warning` no canal da operação ou no relatório.
6. **Novas tentativas** só em operações idempotentes (leituras HTTP, downloads com hash). Escritas no pack nunca são repetidas automaticamente.
7. **Falha deixa o estado anterior.** Toda operação que escreve tem caminho de reversão testado (injeção de falha, §4.2).
8. **Saída de ferramentas externas** (packwiz, Java) entra no `detail`, truncada e sem segredos.
9. **Frontend:** todo `useQuery`/`useMutation` tem estado de erro renderizado com `ErrorPanel`; nenhuma promessa sem tratamento; erros inesperados no React caem num *error boundary* por rota com "Tentar de novo".

## 4. Testes

### 4.1 Tipos de teste

| Tipo | Onde | Ferramenta | Quando roda |
|---|---|---|---|
| Unitário Rust | `#[cfg(test)]` no módulo | `cargo nextest` | toda mudança no Linux; Windows conforme §12 |
| Dourado (golden) | `crates/*/tests/`, snapshots em `snapshots/` | `insta` | toda mudança |
| Integração com packwiz real | `crates/*/tests/packwiz_*.rs` | `cargo nextest` + sidecar (`WARDEN_PACKWIZ_BIN`) | toda mudança |
| Conformidade (packwiz-installer real, Java) | `crates/warden-instance/tests/conformance_*.rs`, `crates/warden-export/tests/conformance_*.rs` | Java 21 (CI: `actions/setup-java`), bootstrap fixado por versão e SHA-256, baixado pelo xtask para o cache | toda mudança no Linux; Windows à noite |
| Rede (APIs reais) | testes marcados `#[ignore = "rede"]` | `cargo xtask test-network` | à noite e manual; local com `.env` |
| Jogo real (matriz de versões) | `crates/warden-launcher/tests/smoke_*.rs` | Linux + Xvfb + Mesa | semanal e manual; antes de cada marco |
| Componente/tela | `apps/desktop/src/**/*.test.tsx` | Vitest + Testing Library + `mockIPC` + `vitest-axe` | toda mudança |
| Ponta a ponta (E2E) | `apps/desktop/e2e/` | WebdriverIO + `tauri-driver` sobre build de debug (`tauri build --debug`), APIs simuladas por servidor local de fixtures, cofre de teste em arquivo | Linux em toda mudança; Windows na `main` e à noite |
| Injeção de falha | testes com a feature `fault-injection` de `warden-core` | `cargo nextest` | toda mudança |
| Desempenho | `crates/*/benches/`, `apps/desktop/e2e/perf/` | `criterion`, medições E2E | marcos e tarefa A-04 |

Regras:
- **Testes de integração não pulam em silêncio.** Na CI (`WARDEN_REQUIRE_EXTERNALS=1`), falta de packwiz/Java é falha. Localmente, pulam com aviso explícito na saída.
- **Teste visto falhando.** Correção de bug começa por um teste que falha pela razão certa; o relatório da tarefa mostra a falha antes e o sucesso depois.
- **Sem rede nos testes comuns.** HTTP simulado com `wiremock` e fixtures gravadas de respostas reais (`tests/fixtures/http/`), com data de gravação no nome.
- **Fixtures reais.** Arquivos do packwiz, jars de exemplo, configs e logs de crash vêm de fontes reais (com origem anotada em `FIXTURES.md` da pasta) ou são gerados pelo próprio packwiz (`cargo xtask fixtures-packwiz`). Nunca escritos "de cabeça" quando o objetivo é compatibilidade.
- **Determinismo.** Sem `sleep` para sincronizar; tempo e aleatoriedade injetados. Teste instável é bug P0 da tarefa que o criou.
- **Testes de interface** cobrem para cada tela os estados vazio, carregando, erro e sucesso, e passam no `axe` sem violações sérias.
- **E2E** cobre pelo menos o caminho feliz de cada fluxo P0 e um caminho de erro, pela interface real (clique, digitação), sem chamar comandos diretamente.
- **Guarda de contrato:** teste que lista os comandos registrados no Rust e verifica que cada um é usado por algum código em `src/features/` (equivalente ao `ipcConsumerGuard` de R4); a lista de exceções só pode diminuir.

### 4.2 Cobertura mínima (linhas)

Medida com `cargo llvm-cov nextest` e `vitest --coverage` (v8), aplicada por `cargo xtask coverage` na CI Linux.

| Alvo | Mínimo |
|---|---|
| `warden-packwiz`, `warden-configs`, `warden-jarmeta`, `warden-diagnostics`, `warden-catalog` | 85% |
| Demais crates de domínio | 75% |
| `warden-app` | sem mínimo numérico (coberto por E2E e guardas de contrato) |
| `apps/desktop/src/lib/**`, `src/features/*/lib/**` | 85% |
| `src/features/*/hooks/**` | 70% |
| Componentes React | sem mínimo numérico; estados obrigatórios do §4.1 |

Cobertura não substitui casos: cada critério de aceite da SPEC coberto pela tarefa tem um teste nomeado com o código do critério (`ca_t08_01_...` em Rust, `it('CA-T08-01: ...')` em TS) ou um roteiro manual no relatório quando automatizar não é viável (ex.: abrir no Windows real).

## 5. Definição de pronto

Uma tarefa está pronta quando **todos** os itens valem:

1. Critérios de aceite da tarefa (ROADMAP) e da SPEC que ela cobre, atendidos e demonstrados (teste nomeado ou roteiro manual executado).
2. `cargo xtask check` passa localmente (§12) e a tarefa informa a saída resumida.
3. Testes do tipo exigido pela tarefa existem e passam; integração contra packwiz/Java real quando a tarefa toca nisso.
4. Funcionalidade alcançável pela interface (se a tarefa tiver parte visível), com estados vazio/carregando/erro, textos pt-BR do catálogo e navegação por teclado.
5. Nenhum arquivo fora da posse da tarefa alterado, exceto os registros acréscimo-apenas (ARCHITECTURE §2) e o que a tarefa declarar explicitamente.
6. `bindings.ts` regenerado se comandos/tipos mudaram.
7. Documentação atualizada: ADR novo se a tarefa tomou decisão técnica relevante; ARCHITECTURE/SPEC se mudou contrato (com aviso ao orquestrador); comentários de módulo.
8. Sem `TODO`/`FIXME` sem número de tarefa (`// TODO(P1-12): ...`).
9. Relatório final no envelope `SUPERSET_WORKER_DONE`, com o que foi verificado executando e o que não foi.

## 6. Checklist de revisão do orquestrador

Antes de integrar uma branch:

- [ ] Escopo: a branch só toca a posse declarada da tarefa; nada de mudanças oportunistas.
- [ ] `cargo xtask check` reexecutado pelo orquestrador no worktree da branch, sobre a `main` atual.
- [ ] CI verde (Linux; Windows quando a tarefa é de launcher, processo, caminhos ou empacotamento).
- [ ] Cada critério de aceite: teste correspondente lido e executado; para o que é manual, o roteiro foi seguido.
- [ ] Testes de integração realmente rodaram contra o packwiz/Java reais (verificar no log que não foram pulados).
- [ ] Pasta do pack: nenhum caminho de código escreve no pack fora da `PackTransaction`; nenhum arquivo auxiliar criado lá (procurar por `write`, `create`, `rename` novos).
- [ ] Erros: códigos novos têm tradução; nenhuma mensagem crua na interface; nenhum `unwrap` fora de testes.
- [ ] Segredos e privacidade: nenhuma chave/token em código, fixture, log, mensagem de erro ou commit (rodar `gitleaks detect` na branch); nada de PII enviado a terceiros sem consentimento.
- [ ] Textos: pt-BR correto, acentuação, glossário (§8), sem português europeu.
- [ ] Interface: estados vazio/carregando/erro presentes; nenhum botão morto; acessível por teclado.
- [ ] Concorrência: comandos recebem `PackId`; mutações pegam a trava; operações longas canceláveis.
- [ ] Dependências novas justificadas, com licença registrada e `cargo deny` verde.
- [ ] Commits pequenos, mensagens no padrão (§7), com a linha `Co-Authored-By`.
- [ ] Relatório honesto: diferenças entre o pedido e o entregue estão declaradas.

## 7. Commits e branches

### 7.1 Branches

- Uma branch por tarefa: `<tipo>/<id-da-tarefa>-<descrição-curta>` em minúsculas, sem acento. Ex.: `feat/p1-09-adicionar-modrinth`, `fix/l-03-preserve-ignorado`, `spike/s1-motor-launcher`, `docs/spec`.
- Agentes trabalham só no próprio worktree e branch. **Não fazem merge, rebase sobre `main` publicado, nem push.** Quem integra é o orquestrador.
- A `main` sempre compila e passa na CI.

### 7.2 Mensagens de commit

- Formato Conventional Commits, em português do Brasil:

```
<tipo>(<escopo>): <resumo no presente, minúsculo, sem ponto final, até 72 caracteres>

<corpo opcional: por que a mudança existe, o que foi verificado; linhas até 100 caracteres>

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
```

- Tipos: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `build`, `ci`, `chore`.
- Escopo: nome da crate sem o prefixo (`packwiz`, `launcher`, `app`), `ui/<feature>` para o frontend, `xtask`, `ci`, `docs`.
- Ex.: `feat(packwiz): grava .pw.toml do Modrinth idêntico ao do packwiz`.
- Cada commit compila e passa `cargo xtask check --fast`. Commits "wip" não entram na branch final.
- Arquivo gerado (`bindings.ts`) entra no mesmo commit da mudança que o gerou.

## 8. Idioma, textos e glossário (pt-BR)

### 8.1 Regras

1. Toda a interface em **português do Brasil**, com acentuação correta. Tratamento por "você".
2. Nenhum texto visível escrito direto no JSX: tudo em `src/i18n/pt-BR/<área>.ts` (o ESLint bloqueia).
3. Frases curtas, sentence case ("Salvar versão", não "Salvar Versão"), sem ponto de exclamação, sem jargão não explicado.
4. Botões com verbo no infinitivo que diz o que acontece ("Adicionar 3 mods", "Trazer selecionados para o pack"); evitar "OK" e "Sim/Não" em confirmações.
5. Plural com i18next (`_one`/`_other`); números, datas e tamanhos com `Intl` em `pt-BR` ("1,5 MB", "1 de outubro de 2026").
6. Mensagens de erro: o que aconteceu + o que fazer (§3).
7. Proibido português europeu: "ficheiro", "ecrã", "utilizador", "registar", "a carregar", "transferir" (como download), "partilhar", "aceder". Em botões e nomes de ação, "Salvar", nunca "Guardar" (em texto descritivo, "fica guardado" é aceitável).
8. Documentação e comentários de código em pt-BR; identificadores, nomes de crates, tipos e funções em inglês; termos técnicos no original.
9. Toda tarefa que escreve texto de interface recebe este glossário.
10. O ícone de brilhinho (✦/sparkles) marca o que é IA e só isso; nunca como decoração (ADR-0026).
11. Nada de frases de marketing ("Eleve seus modpacks", "Experiência perfeita", "Desbloqueie"), exclamações, excesso de travessões ou títulos que dizem o óbvio: texto direto e útil, como numa ferramenta de trabalho.

### 8.2 Glossário

| Conceito | Usar | Não usar |
|---|---|---|
| Modpack | pack (ou modpack em textos explicativos) | pacote de mods |
| Mod | mod | modificação |
| Resource pack | resource pack | pacote de recursos, pacote de texturas |
| Shader pack | shader | sombreador |
| Mod loader | loader | carregador (só na explicação do glossário) |
| Arquivo de configuração de mod | config (plural: configs) | configuração, quando se refere ao arquivo |
| Preferências do app | Configurações | Definições, Preferências |
| Instância do launcher | instância de teste | perfil, instalação |
| Executar o jogo para testar | Testar | Jogar, Iniciar |
| Download | baixar / download | transferir |
| Enviar uma versão final ao GitHub para os jogadores | publicar versão | enviar ao GitHub, fazer push, upload |
| Versão salva pronta para os jogadores | versão final | release, versão estável |
| Endereço do `pack.toml` publicado | link do pack | URL raw, link do raw |
| Commit + tag (local) | salvar versão | commit, confirmar, publicar |
| Histórico git | histórico | log, commits |
| Saída ao vivo do jogo | console | terminal |
| Arquivos de log do jogo | log (do jogo) | registro do jogo |
| Logs do Warden | registros | logs do app |
| Crash | travou / travamento; "crash report" para o arquivo | quebrou, falhou geral |
| Seção com os achados do diagnóstico | Problemas | Diagnóstico (como nome de seção), Erros |
| Diagnóstico feito pela IA | Diagnóstico com IA (com o ícone ✦) | assistente, IA mágica |
| Etapa que leva o pack para a instância | Copiar o pack para o teste | sincronizar, sync |
| Revisão das mudanças feitas no jogo | O que mudou durante o teste | diff, sincronização reversa |
| Tela inicial do app | Meus packs | Início, Modpacks, Dashboard |
| Nome, autor e descrição do pack | Informações do pack (ação "Editar informações") | Ajustes do pack, Configurações do pack |
| Memória, Java e argumentos do teste | Ajustes do teste neste computador | Configurações do pack |
| Arquivo opcional com as chaves | arquivo .env | arquivo de segredos, config de chaves |
| Side do packwiz | lado: "Cliente e servidor", "Só cliente", "Só servidor" | ambos, side |
| Pin | fixar versão | travar, pinar |
| Optional mod | opcional | facultativo |
| Snapshot de segurança | ponto de segurança | backup, snapshot |
| Index do packwiz | índice | index |
| Chave de API | chave | token (exceto o do GitHub, que é "token") |

## 9. Segurança e privacidade

1. **Segredos:** chaves e tokens só no armazenamento escolhido pelo usuário: o cofre do sistema (padrão) ou o `.env` da pasta de configuração do Warden (ARCHITECTURE §14, ADR-0025). O conteúdo publicado no GitHub passa por varredura de segredos antes de sair (ARCHITECTURE §11.1). Proibido: escrever em arquivo do repositório, fixture, log, mensagem de erro, saída de teste, commit, argumento de linha de comando de processo filho, URL de remoto git. A chave da CurseForge do dono está em `/home/solel/.superset/projects/Warden/.env` (`CURSEFORGE_API_KEY`): **não imprimir, não copiar, não registrar**; testes leem do ambiente.
2. **Varredura:** `gitleaks detect` na CI e no checklist de revisão; padrões extras para `$2a$`, `AIza`, `ghp_`, `github_pat_`.
3. **Tipos de segredo:** `secrecy::SecretString` no Rust; no frontend, campos de senha que enviam o valor ao comando `secrets_set` e limpam o estado em seguida; nunca guardar em Zustand, Query cache ou `localStorage`.
4. **PII:** nada é enviado a terceiros (IA) sem consentimento explícito por envio e sem passar pela redação (ARCHITECTURE §9.4). O texto mostrado é o texto enviado.
5. **Registros:** sem segredos; sem corpo de respostas da CurseForge; sem ambiente de processos filhos.
6. **Entrada externa** (APIs, jars, zips, logs, configs) é não confiável: limites de tamanho, parsers tolerantes que não entram em pânico (testes com `proptest`/entradas malformadas), proteção contra *zip slip* e *path traversal*, HTML/Markdown higienizado.
7. **Processos:** argv sempre em vetor, nunca via shell; ambiente limpo para o sidecar; nenhum comando genérico exposto à interface.
8. **Tauri:** capabilities mínimas e CSP (ARCHITECTURE §20); qualquer ampliação exige ADR.
9. **Legal:** nada da Mojang é redistribuído (só baixado dos servidores oficiais na máquina do usuário); sem contorno de autenticação; aviso "não oficial" visível (ADR-0010).

## 10. Dependências e código de terceiros

1. Nova dependência (crate ou pacote npm) precisa de: justificativa no commit; manutenção ativa (versão nos últimos 12 meses) ou maturidade comprovada; licença registrada.
2. Licenças: o Warden é privado e nunca distribuído (ADR-0004), então GPL/LGPL são permitidas. Mesmo assim, cada dependência e cada trecho copiado/portado é registrado.
3. `cargo deny check` (licenças, avisos de segurança, fontes, duplicatas) verde. `pnpm audit --prod` sem vulnerabilidades altas/críticas.
4. **Código copiado ou portado** (ex.: padrões do codex-minecraft, trechos do theseus/portablemc): comentário no topo do arquivo com origem (URL), licença e commit, e entrada em `THIRD_PARTY.md` na raiz.
5. Binários externos (packwiz, packwiz-installer-bootstrap, Java) fixados por commit/versão e hash.
6. Atualização de dependências é tarefa própria, com changelog lido e testes completos.

## 11. Código e documentação

- Itens públicos das crates com doc comment (`///`) em pt-BR explicando o que fazem e os erros possíveis; cada crate com doc de módulo (`//!`) explicando a responsabilidade e os limites.
- Funções puras sempre que possível (construtores de argv, regras de diagnóstico, sugestão SemVer, mapeamentos): fáceis de testar.
- Nada de estado global mutável além do estado do app em `warden-app`.
- Nomes que dizem o que a coisa é; sem abreviações obscuras.
- Componentes React pequenos; lógica fora do JSX em hooks ou `lib/`.
- Mudança de contrato (comando, evento, código de erro, formato em disco) atualiza ARCHITECTURE no mesmo branch e é destacada no relatório.

## 12. Comandos de verificação

| Comando | O que faz |
|---|---|
| `cargo xtask setup` | Instala dependências locais do projeto (pnpm install, compila o sidecar do packwiz, baixa o bootstrap do packwiz-installer para o cache). |
| `cargo xtask check` | Gate completo local: `cargo fmt --check`, `cargo clippy ... -D warnings`, `cargo nextest run --workspace`, `cargo test --doc`, `cargo deny check`, `cargo xtask check-deps`, `cargo xtask bindings --check`, `pnpm -C apps/desktop format:check`, `lint`, `typecheck`, `test`. |
| `cargo xtask check --fast` | Formatação, clippy, testes unitários das crates alteradas, lint e typecheck. Usado a cada commit. |
| `cargo xtask coverage` | Cobertura com os mínimos do §4.2. |
| `cargo xtask test-network` | Testes contra APIs reais (carrega o `.env` do repositório principal sem imprimir valores). |
| `pnpm -C apps/desktop e2e` | Testes ponta a ponta com o app real. |
| `cargo xtask bindings` | Regenera `apps/desktop/src/lib/ipc/bindings.ts`. |

A CI (`.github/workflows/ci.yml`) executa o equivalente a `cargo xtask check` em Linux em todo push de branch e PR; Windows na `main`, em PRs marcados e à noite (decisão pendente D9 da SPEC); E2E Linux em todo PR; testes de rede, conformidade no Windows e smoke do jogo em workflows agendados/manuais.
