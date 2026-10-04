# Warden — Padrão de qualidade obrigatório

> Versão do documento: 1.4 (2026-10-04). Tarefa A1; glossário e regra de segredos atualizados na tarefa D2 (decisões do dono, ADR-0025 a ADR-0029); glossário, testes e privacidade das funções avançadas na tarefa D4 (ADR-0030 a ADR-0038); glossário, testes e privacidade do Warden 1.1 "Profissional" na tarefa D5 (ADR-0039 a ADR-0047); desenvolvimento direto no Windows na tarefa D6 (ADR-0048): §13 nova, testes, fim de linha e caminho do `.env`.
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
13. [Desenvolvimento no Windows](#13-desenvolvimento-no-windows)

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

- `.editorconfig`: UTF-8, LF (CRLF só em `.cmd` e `.bat`), indentação de 2 espaços (TS/JSON/YAML/TOML) e 4 (Rust), linha final. Fim de linha no Windows: §13.2.
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
| Unitário Rust | `#[cfg(test)]` no módulo | `cargo nextest` | toda mudança (local no Windows; na CI, Linux em toda mudança e Windows conforme §12) |
| Dourado (golden) | `crates/*/tests/`, snapshots em `snapshots/` | `insta` | toda mudança |
| Integração com packwiz real | `crates/*/tests/packwiz_*.rs` | `cargo nextest` + sidecar (`WARDEN_PACKWIZ_BIN`) | toda mudança |
| Conformidade (packwiz-installer real, Java) | `crates/warden-instance/tests/conformance_*.rs`, `crates/warden-export/tests/conformance_*.rs` | Java 21 (CI: `actions/setup-java`), bootstrap fixado por versão e SHA-256, baixado pelo xtask para o cache | toda mudança no Linux; Windows à noite |
| Rede (APIs reais) | testes marcados `#[ignore = "rede"]` | `cargo xtask test-network` | à noite e manual; local com `.env` |
| Jogo real (matriz de versões) | `crates/warden-launcher/tests/smoke_*.rs` | Linux + Xvfb + Mesa | semanal e manual; antes de cada marco |
| Componente/tela | `apps/desktop/src/**/*.test.tsx` | Vitest + Testing Library + `mockIPC` + `vitest-axe` | toda mudança |
| Ponta a ponta (E2E) | `apps/desktop/e2e/` | WebdriverIO + `tauri-driver` sobre build de debug (`tauri build --debug`), APIs simuladas por servidor local de fixtures, cofre de teste em arquivo e `WARDEN_DATA_ROOT` (§13.5) | local no Windows; na CI, Linux em toda mudança e Windows na `main` e à noite |
| Injeção de falha | testes com a feature `fault-injection` de `warden-core` | `cargo nextest` | toda mudança |
| Desempenho | `crates/*/benches/`, `apps/desktop/e2e/perf/` | `criterion`, medições E2E | marcos e tarefa A-04 |
| Servidor local real (D4) | `crates/warden-server/tests/server_*.rs` | instaladores oficiais + Java, marcados `#[ignore = "servidor"]` | sob demanda e antes do marco M4 |
| IA com servidor simulado (D4) | `crates/warden-ai/tests/` | servidor que imita o Gemini, inclusive a recusa de histórico com *thought signatures* alteradas, e o GitHub | toda mudança; chave real só no teste de rede |
| Segurança dos mods (1.1) | `crates/warden-security/tests/` | jars **sintéticos** gerados por `cargo xtask fixtures-security` (um por regra da lista de sinais, mais jars limpos e reais do corpus da P1-06 para falso positivo); respostas reais gravadas do Modrinth e da CurseForge | toda mudança; `cargo xtask check-signatures` na CI |
| Logs de jogadores (1.1) | `crates/warden-diagnostics/tests/corpus-player/` | logs reais públicos, já redigidos, com a origem (URL da issue) anotada, por formato (Forge 1.7.10, 1.12.2 e moderno, NeoForge, Fabric, Prism, `packwiz.json`) | toda mudança |

Regras:
- **Nada de dados reais do dono.** Testes e E2E nunca tocam nas pastas reais do Warden, nos packs do dono nem no cofre do Windows (§13.6).
- **Testes de integração não pulam em silêncio.** Na CI (`WARDEN_REQUIRE_EXTERNALS=1`), falta de packwiz/Java é falha. Localmente, pulam com aviso explícito na saída.
- **Teste visto falhando.** Correção de bug começa por um teste que falha pela razão certa; o relatório da tarefa mostra a falha antes e o sucesso depois.
- **Sem rede nos testes comuns.** HTTP simulado com `wiremock` e fixtures gravadas de respostas reais (`tests/fixtures/http/`), com data de gravação no nome.
- **Fixtures reais.** Arquivos do packwiz, jars de exemplo, configs e logs de crash vêm de fontes reais (com origem anotada em `FIXTURES.md` da pasta) ou são gerados pelo próprio packwiz (`cargo xtask fixtures-packwiz`). Nunca escritos "de cabeça" quando o objetivo é compatibilidade.
- **Determinismo.** Sem `sleep` para sincronizar; tempo e aleatoriedade injetados. Teste instável é bug P0 da tarefa que o criou.
- **Testes de interface** cobrem para cada tela os estados vazio, carregando, erro e sucesso, e passam no `axe` sem violações sérias.
- **E2E** cobre pelo menos o caminho feliz de cada fluxo P0 e um caminho de erro, pela interface real (clique, digitação), sem chamar comandos diretamente.
- **Guarda de contrato:** teste que lista os comandos registrados no Rust e verifica que cada um é usado por algum código em `src/features/` (equivalente ao `ipcConsumerGuard` de R4); a lista de exceções só pode diminuir.
- **Algoritmos com executor simulado (D4):** a busca do culpado e a nota de saúde são funções puras testadas com executores e dados simulados e com `proptest` (nenhum prefixo sem dependências; um achado a mais nunca aumenta a nota).
- **Nunca malware real (1.1):** nenhum arquivo malicioso real entra no repositório, em fixtures ou em caches de teste. Os sinais são provados com jars sintéticos que reproduzem só o padrão (constantes e instruções), sem código que funcione; os hashes de arquivos maliciosos conhecidos entram como dados, sem o arquivo.
- **Nada de jogo, servidor ou IA de verdade para provar a interface:** E2E usam processos Java de teste que imitam o jogo e o servidor (imprimem os marcadores, travam, criam filhos) e servidores HTTP simulados.

### 4.2 Cobertura mínima (linhas)

Medida com `cargo llvm-cov nextest` e `vitest --coverage` (v8), aplicada por `cargo xtask coverage` na CI Linux.

| Alvo | Mínimo |
|---|---|
| `warden-packwiz`, `warden-configs`, `warden-jarmeta`, `warden-diagnostics`, `warden-catalog`, `warden-bisect`, `warden-mixin`, `warden-import`, `warden-security` (1.1) | 85% |
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
| Nota que resume os problemas do pack (D4) | saúde do pack; faixas Ótimo, Bom, Atenção, Crítico | score, pontuação, health |
| Bisseção automática (D4) | encontrar o mod culpado; busca do culpado; rodada | bisseção, bisect, busca binária |
| Raio-x de mixins (D4) | o que este mod altera no jogo; "alteram o mesmo ponto do jogo" | mixin (na interface, fora da coluna técnica), conflito de mixin, "são incompatíveis" |
| Sessão de perguntas com a IA (D4) | conversa | chat, sessão de IA |
| O que a IA consultou, mostrado na conversa (D4) | Enviado à IA | log de ferramentas, function call |
| Mudança sugerida pela IA (D4) | proposta; botão **Aplicar** | sugestão automática, ação da IA |
| Conjunto de ajustes do teste com nome (D4) | perfil do teste | preset, configuração de teste |
| Servidor dedicado na máquina do usuário (D4) | servidor local; "Testar como servidor" | servidor dedicado, host local |
| Server pack (D4) | pacote para servidor | server pack, pacote de servidor |
| Mods sugeridos ao criar um pack (D4) | mods iniciais | mods padrão, mods obrigatórios |
| Lista curada de mods de otimização (D4) | kit de desempenho | pacote de performance, otimizações |
| spark e Crash Assistant no pack (D4) | ferramenta do jogador | mod utilitário, dependência |
| Lista de testes que travaram (D4) | Travamentos | crashes, histórico de crashes |
| Página aberta por Adicionar (D4) | página de descoberta (na interface, o título continua "Adicionar ao pack") | loja, explorar, marketplace |
| Grafo de dependências (D4) | Ver como: Grafo; "quem precisa de quem" | árvore de dependências, diagrama |
| Abrir pack packwiz ou de outro app (D4) | Abrir ou importar… | Importar pack, Abrir pack existente |
| Checagem de malware dos mods (1.1) | segurança dos mods; "não é um antivírus" | antivírus, proteção, escudo, scan |
| Hash igual ao do arquivo da plataforma (1.1) | confere com o arquivo oficial / não confere com o arquivo oficial | hash válido, íntegro, verificado (sem dizer contra o quê) |
| Assinatura de malware encontrada (1.1) | sinal de programa malicioso conhecido | vírus, ameaça, infectado |
| Padrão heurístico em arquivo fora das plataformas (1.1) | ponto de atenção | suspeito (sozinho), perigoso |
| Liberar um arquivo apontado (1.1) | confiar neste arquivo | ignorar ameaça, permitir, whitelist |
| Mods removidos, arquivados ou parados (1.1) | manutenção dos mods; "removido do Modrinth", "arquivado pelo autor", "arquivo removido" | abandonado (como rótulo), morto, descontinuado |
| Mod parecido sugerido (1.1) | substituto; ação "Procurar substituto" | alternativa (como botão), recomendação |
| Log enviado por quem joga (1.1) | travamento de um jogador; ação "Analisar travamento de um jogador…" | log de terceiros, relatório de crash do usuário |
| Texto do usuário num mod (1.1) | nota | comentário, descrição, anotação |
| Etiqueta do usuário para mods (1.1) | grupo; "Grupos do pack" | tag, categoria, pasta, coleção |
| Materiais iguais em vários mods (1.1) | itens repetidos entre mods; solução: unificar, unificador de itens | duplicatas, conflito de itens |
| Resultado que não é erro nem aviso (1.1) | conselho | dica, sugestão automática |
| Gráfico de tempo e memória por versão (1.1) | desempenho entre versões; "Tempo para abrir", "Memória máxima", "Tempo por tick"; aviso "Esta versão está mais pesada" | benchmark, performance, regressão |

## 9. Segurança e privacidade

1. **Segredos:** chaves e tokens só no armazenamento escolhido pelo usuário: o cofre do sistema (padrão) ou o `.env` da pasta de configuração do Warden (ARCHITECTURE §14, ADR-0025). O conteúdo publicado no GitHub passa por varredura de segredos antes de sair (ARCHITECTURE §11.1). Proibido: escrever em arquivo do repositório, fixture, log, mensagem de erro, saída de teste, commit, argumento de linha de comando de processo filho, URL de remoto git. A chave da CurseForge do dono está em `C:\Users\solel\orca\projects\Warden\.env` (`CURSEFORGE_API_KEY`, entre aspas simples): **não imprimir, não copiar, não registrar**; testes leem do ambiente.
2. **Varredura:** `gitleaks detect` na CI e no checklist de revisão; padrões extras para `$2a$`, `AIza`, `ghp_`, `github_pat_`.
3. **Tipos de segredo:** `secrecy::SecretString` no Rust; no frontend, campos de senha que enviam o valor ao comando `secrets_set` e limpam o estado em seguida; nunca guardar em Zustand, Query cache ou `localStorage`.
4. **PII:** nada é enviado à IA sem o consentimento explícito da conversa (ADR-0030: uma vez por conversa, listando o que a IA pode consultar) e sem passar pela redação (ARCHITECTURE §9.4). O texto inicial mostrado é o texto enviado, e cada envio seguinte aparece na conversa com os bytes exatos. A busca de issues no GitHub envia só o repositório e palavras do erro redigidas, e só com a permissão da conversa. Nada muda no pack por ação da IA sem o clique em Aplicar.
4a. **Rede local (D4):** servidor local, servidor de arquivos do teste pelo link e chamadas ao servidor web do KubeJS só em `127.0.0.1`; o token do KubeJS nunca sai do Rust (ARCHITECTURE §20).
4b. **Warden 1.1:** a checagem de segurança envia só hashes ao Modrinth e à CurseForge, nunca o jar (teste que registra os corpos das requisições); logs de jogadores são baixados só dos hosts da lista da SPEC T30, pelo Rust, e gravados só depois da redação (nome do jogador e da instância incluídos); as métricas de desempenho e a impressão do computador ficam nos dados locais e nunca saem; respostas da CurseForge da manutenção e dos substitutos ficam só em memória; notas e grupos nunca vão para os jogadores nem para o GitHub, exceto as notas que o usuário escolher pôr no resumo da versão.
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

Os comandos são os mesmos no PowerShell do Windows (onde os agentes trabalham) e no Linux da CI (§13.7).

A CI (`.github/workflows/ci.yml`) executa o equivalente a `cargo xtask check` em Linux em todo push de branch e PR; Windows na `main`, em PRs marcados e à noite (decisão pendente D9 da SPEC); E2E Linux em todo PR; testes de rede, conformidade no Windows e smoke do jogo em workflows agendados/manuais.

## 13. Desenvolvimento no Windows

O Warden é desenvolvido direto no Windows nativo; o Linux só roda na CI (ADR-0048). As regras abaixo vêm da prova feita na tarefa D6 nesta máquina (app Tauri 2 mínimo compilado, janela aberta, instalador NSIS gerado) e valem para todos os agentes.

### 13.1 Pré-requisitos

- Já instalados nesta máquina (conferidos em 03/10/2026 e, o Go e o `gh`, na D6): Visual Studio Build Tools 2022 com o componente C++ (MSVC 14.44), WebView2 (154), Rust stable `x86_64-pc-windows-msvc` pelo `rustup` (o `rust-toolchain.toml` fixa a versão), `cargo-tauri`, Node 24, pnpm 12, Go, git com `core.autocrlf=false` e GitHub CLI (`gh`). Java não é pré-requisito: o motor do launcher baixa o Java do Minecraft.
- **Instalar programa no Windows só com autorização do dono.** O que fica no espaço do usuário e é refeito por comando não conta como instalação: `cargo install` feito pelo `cargo xtask setup`, pacotes do pnpm, ferramentas baixadas pelo `xtask` para o cache (como o `msedgedriver`) e as que o próprio `tauri build` baixa para `%LOCALAPPDATA%\tauri\` (NSIS).
- Instalar o próprio Warden para teste é a F0-04; só o dono ou o orquestrador fazem isso.

### 13.2 Fim de linha

- O git do repositório usa `core.autocrlf=false`: não mude. Quem garante o fim de linha é o `.gitattributes` da raiz (ARCHITECTURE §2): `* text=auto eol=lf`; `*.cmd` e `*.bat` com `eol=crlf` (o `cmd.exe` lê rótulos errado com LF); fixtures do packwiz com `-text`, para os bytes não mudarem e os hashes do `index.toml` continuarem valendo.
- O `.editorconfig` pede LF (§2.3) e o `rustfmt.toml` usa `newline_style = "Unix"`; o Prettier usa LF (padrão dele).
- Código que escreve arquivos de texto do Warden usa `\n`. Arquivos do pack mantêm os bytes que já têm (a regra de edição mínima da ARCHITECTURE §6).

### 13.3 Caminhos longos e a pasta `target`

- O `link.exe` da Microsoft não aceita caminhos com mais de 260 caracteres, mesmo com `LongPathsEnabled` ligado no Windows (como está nesta máquina). Na prova da D6, com a pasta `target` a 182 caracteres, o build falhou com `LNK1104` ao criar `build_script_build-<hash>.exe` (caminho de 268 caracteres); com a pasta a 160 caracteres, passou. Dentro da `target` os caminhos chegam a cerca de 135 caracteres.
- **Regra:** o caminho da pasta `target` fica com até 100 caracteres. Os worktrees do Orca (`C:\Users\<usuário>\orca\workspaces\Warden\<nome>\`) já cabem; o `xtask` avisa quando passa. Se passar, defina `CARGO_TARGET_DIR` com um caminho curto **só daquele worktree** (por exemplo `$env:CARGO_TARGET_DIR = 'C:\wt\<nome>'`); uma pasta `target` única para vários worktrees faz os agentes disputarem a trava do cargo.
- `core.longpaths` do git não é necessário: o repositório não versiona caminhos longos, e `node_modules` e `target` não passam pelo git. Se uma fixture precisar de caminho longo, a tarefa liga `core.longpaths` só no próprio worktree e registra no relatório.
- O pnpm já encurta a pasta `node_modules/.pnpm` no Windows; não mude `virtual-store-dir-max-length`.
- No código do app, caminhos do Windows passam pela biblioteca padrão do Rust, que aceita caminhos longos; nada de montar caminho com concatenação de texto (ARCHITECTURE §13).

### 13.4 Tempo de compilação, disco e antivírus

- Medido na prova da D6 (app mínimo, sem o resto do Warden): primeira compilação de debug em 81 s; uma mudança pequena recompila em 8 s; build de release com instalador NSIS em 222 s; a pasta `target` ficou com 3,6 GB (debug e release). O Warden completo será bem maior: limpe a `target` de worktrees encerrados (`cargo clean`), tarefa do orquestrador.
- **Antivírus:** a verificação em tempo real pode deixar a compilação bem mais lenta, porque cada arquivo novo da `target` é examinado. No momento da medição a proteção em tempo real do Defender estava desligada nesta máquina, então o efeito não foi medido. Excluir as pastas `target` da verificação é **decisão do dono**: nenhum agente adiciona exclusões, desliga a proteção ou roda `Add-MpPreference`.
- O sidecar `packwiz.exe` e o instalador não têm assinatura digital (decisão D4): o SmartScreen e antivírus podem estranhar (ADR-0007, `docs/DEV-WINDOWS.md`).
- Avisos conhecidos: o `staticlib`/`cdylib` do modelo do Tauri gera aviso do linker no Windows (por isso a `warden-app` usa só `rlib`, F0-01); a `tauri-cli` 2.11 imprime um aviso de `STATIC_VCRUNTIME` obsoleto que vem dela própria e não falha o build.

### 13.5 Testes ponta a ponta

- **Windows (local e CI Windows):** `tauri-driver` com o `msedgedriver` da **mesma versão** do WebView2 instalado, baixado por `cargo xtask e2e-driver` para um cache fora do git (sem instalar nada). Quando o WebView2 se atualiza, o comando baixa o driver novo.
- **Linux (CI):** `tauri-driver` com o `WebKitWebDriver` do WebKitGTK, sob Xvfb.
- Os E2E rodam sobre build de debug, com `WARDEN_DATA_ROOT` e cofre de teste em arquivo numa pasta temporária (§13.6).

### 13.6 Dados e cofre do dono

- Testes, E2E e `cargo xtask dev` **nunca** leem nem escrevem nas pastas reais do Warden (`%APPDATA%\dev.kriticales.warden\`, `%LOCALAPPDATA%\dev.kriticales.warden\`), nos packs do dono (`Documentos\Warden\`) nem no Gerenciador de Credenciais do Windows. Usam `WARDEN_DATA_ROOT` (pasta temporária, ou `%LOCALAPPDATA%\Warden-dev\<worktree>\` no `dev`) e `WARDEN_SECRET_BACKEND=file:<pasta>`, que só existem em build de debug (ADR-0048, ARCHITECTURE §14).
- O teste com o cofre real do Windows fica marcado `#[ignore = "cofre-real"]` e só roda na CI Windows; o roteiro manual dos marcos confere o cofre real com a versão instalada (F0-04).

### 13.7 Comandos e processos

- Toda automação passa pelo `xtask` (Rust), que funciona no PowerShell e no Linux da CI. Nada de script bash ou PowerShell como passo obrigatório; arquivos `.cmd` só como atalho de duplo clique que chama o `xtask` (F0-04).
- Comandos de verificação nos relatórios e nos documentos são escritos para o PowerShell 7 (`&&` funciona; variável de ambiente com `$env:NOME = 'valor'`; caminhos com espaço entre aspas).
- Testes que iniciam processos resolvem o executável com a extensão do Windows (`.exe`, `.cmd`) e passam argumentos em vetor, nunca por `cmd /c` ou `sh -c` (§9).
