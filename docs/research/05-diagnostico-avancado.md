# R5A — Diagnóstico avançado: bisseção, IA com ferramentas, raio-x de mixins, console e desempenho

> Tarefa R5A, 01/10/2026. Pesquisa para o pedido do dono de um diagnóstico "mais avançado que o MCDoctor.ai".
> Base: `docs/SPEC.md` (T13, T14), `docs/ARCHITECTURE.md` §7.4 e §9, `docs/design/ESTRUTURA.md` (estrutura aprovada), R1 §4.4, R2 §6, R3 §4, S1 §3.4. O que já está nesses documentos é citado, não repetido.
> Experimentos fora do repositório em `~/.local/share/warden-r5/` (protótipo `mixin-xray` em Rust e jars reais baixados do Modrinth).

**Legenda:** **[verificado]** = executei ou consultei a API e vi o resultado; **[código]** = li o código-fonte; **[doc]** = documentação oficial; **[inferência]** = conclusão minha, ainda não testada.

## Sumário

0. [Resumo](#0-resumo)
1. [O que já existe: MCDoctor.ai e as ferramentas de referência](#1-o-que-já-existe-mcdoctorai-e-as-ferramentas-de-referência)
2. [Mods que vêm por padrão: spark e mod de crash report](#2-mods-que-vêm-por-padrão-spark-e-mod-de-crash-report)
3. [Função 1 — Busca automática do mod culpado (bisseção)](#3-função-1--busca-automática-do-mod-culpado-bisseção)
4. [Função 2 — IA "médico" com ferramentas e chat](#4-função-2--ia-médico-com-ferramentas-e-chat)
5. [Função 3a — Raio-x de mixins](#5-função-3a--raio-x-de-mixins)
6. [Função 3b/3c — Grafo de dependências e nota de saúde](#6-função-3b3c--grafo-de-dependências-e-nota-de-saúde)
7. [Função 4 — Console inteligente, desempenho, spark automático e histórico de travamentos](#7-função-4--console-inteligente-desempenho-spark-automático-e-histórico-de-travamentos)
8. [Onde cada função fica na estrutura aprovada](#8-onde-cada-função-fica-na-estrutura-aprovada)
9. [Dependências entre as funções e esforço](#9-dependências-entre-as-funções-e-esforço)
10. [Implicações para o Warden](#10-implicações-para-o-warden)
11. [Questões para o dono](#11-questões-para-o-dono)
12. [Fontes](#12-fontes)

---

## 0. Resumo

- **O MCDoctor.ai é um analisador de um arquivo por vez na nuvem**, com um LLM não divulgado, cobrança por "tokens" e um changelog cheio de correções para diagnósticos inventados. Ele não vê a pasta `mods`, os jars, as configs nem o histórico, e não consegue testar. O Warden supera isso por **ter a instância na mão**: lê os jars, testa sozinho, compara versões e só usa a IA como camada de cima, citando evidências (§1.1).
- **Busca automática do culpado (bisseção):** viável e diferencial. Proposta: busca por **prefixos em ordem topológica** (respeita dependências sem cálculo extra), assinatura do travamento para saber se "reproduziu", minimização para pares de mods, mundos descartáveis, servidor dedicado para versões antigas e para problemas no mundo, resultado parcial ao cancelar. Cerca de 8 rodadas para 200 mods (≈ 20–25 min) **[inferência]**. Nenhuma ferramenta atual junta "respeita dependências" e "decide sozinha" em todos os loaders (§3).
- **IA "médico" com ferramentas:** Gemini com function calling (família 3.x; *thought signatures* obrigatórias), ~20 ferramentas de leitura e propostas que só valem com o clique do usuário, com **evidência conferida por código**. Custa cerca de US$ 0,20 por conversa no `gemini-3.8-flash` (preço até dez/2026). Exige rever a regra de consentimento "a cada envio" (§4, questão 3).
- **Raio-x de mixins:** protótipo em Rust **[verificado]** leu 1.989 classes de mixin de 14 mods reais em 25–67 ms por conjunto. Mostrou que o **risco existe, mas a maioria das sobreposições é compatibilidade intencional**: os dois "riscos altos" do conjunto Fabric (Sodium × Iris, Lithium × ModernFix) são falsos positivos práticos. Na v1: inventário e avisos explicáveis, nunca bloqueio. Existe uma ferramenta Rust MIT muito próxima (intermed), que vale um spike (§5).
- **Grafo e nota de saúde:** baratos, porque reaproveitam o diagnóstico P0. A nota precisa ser explicável ("o que tirou pontos") e não pode prometer que o pack funciona (§6).
- **Console e desempenho:** atribuir linhas a mods é viável (índice de pacotes com 2 ambiguidades em 844 pacotes **[verificado]**; frames de stack com jar ou módulo; nomes de handler do Mixin no Fabric). A memória do jogo pode ser lida **sem JDK e sem mexer no jogo** via `hsperfdata` + API do Windows. Tempo por mod existe pronto no Forge 1.12.2, com `TRACE` no Forge/NeoForge moderno, e não existe no Fabric (§7).
- **Mods padrão:** **spark** sim (GPL-3.0, Modrinth de 1.16.5 a 26.3; builds antigos na CurseForge para 1.7.10 e 1.12.2). Mod de crash report: o **Crash Assistant** é o único que cobre 1.7.10 a 26.3 em todos os loaders, mas tem **licença com cláusula de não concorrência** e envia metadados ao autor por padrão. Recomendo incluí-lo por referência (permitido) com o envio desligado, sem ler nem depender do código e dos formatos dele, e deixar o dono decidir (§2, questão 1).
- **Estrutura:** nenhuma seção nova. Três ajustes pequenos (E1–E3, §8).

## 1. O que já existe: MCDoctor.ai e as ferramentas de referência

### 1.1 MCDoctor.ai

| Item | Achado |
|---|---|
| Quem opera | Starlancer Srl (Bucareste), marca McHostingPro / Minecraft-Hosting.pro **[verificado: /privacy, /terms]** |
| Como funciona | Upload ou colagem do log, ou link de mclo.gs/hastebin/pastebin. Etapas declaradas: limpeza → detecção da plataforma → extração dos erros → "Minecraft-trained AI" → relatório "Cause → Fixes → Notes", com confiança ("confirmed/likely") **[verificado: /how-it-works, /changelog]** |
| Modelo | Não divulgado. Modo Standard (~10 s) e Advanced ("deeper model", ~30 s), com seletor de sintoma (travamento, inicialização, lag, conexão, mundo, mods) **[verificado: /prices]** |
| Regras | "Log clustering" e "smart trimming" para logs acima de 1,5 MB (v1.4). As versões 2.0.6–2.0.8 corrigem diagnósticos inventados: culpar *warnings*, sugerir mod inexistente, tratar log cortado como crash **[verificado: /changelog]** |
| Entrada | `latest.log`, `debug.log`, crash reports, Fabric/Forge/NeoForge, Paper, *watchdog*, `hs_err_pid`, `.gz`/`.zip` **[verificado: /faq]** |
| Preço | Grátis com conta: 5 Standard + 1 Advanced por mês; planos de US$ 4,99 a 19,99; API para empresas **[verificado: /prices]** |
| Privacidade | Contraditória: a página inicial diz "never stored permanently"; o FAQ e a política falam em 90 dias e numa versão anonimizada guardada para *dataset* **[verificado]** |
| Integrações | Mod de servidor (Fabric/Forge/NeoForge, All Rights Reserved), plugin Bukkit, extensão para Pterodactyl **[verificado: API do Modrinth]** |

**Limites do MCDoctor que o Warden não tem [inferência]:**

| MCDoctor.ai | Warden (proposta) |
|---|---|
| Um arquivo por vez; às vezes pede outro log | Lê todos os artefatos da sessão (ARCHITECTURE §9.3) e o histórico de sessões |
| Não vê os jars, as configs nem a lista real de mods | Lê metadados, dependências, mixins e configs de cada jar |
| Não reproduz o problema | Testa de novo e faz a busca do culpado (§3) |
| Não sabe o que mudou desde a última versão que funcionava | Compara versões salvas (git) |
| LLM decide; regras só pré-processam | Regras determinísticas decidem o que der; a IA só entra para o resto e precisa citar evidência conferida |
| Nuvem, conta e pagamento | Local; a IA é opcional e usa a chave do próprio usuário |
| Foco em servidor/hospedagem | Foco em quem monta o pack |

O que vale copiar do MCDoctor: o **seletor de sintoma** ("trava ao abrir", "trava no mundo", "lag", "não conecta"), que orienta a análise e deveria virar o primeiro passo da conversa com a IA (§4) e da busca do culpado (§3.4); a **confiança explícita**; e o "smart trimming" de logs gigantes (o Warden já tem o orçamento de 200 KB na ARCHITECTURE §9.5).

Fontes: https://mcdoctor.ai/, /how-it-works, /prices, /faq, /privacy, /terms, /changelog, https://api.modrinth.com/v2/project/mcdoctor.ai.

### 1.2 Ferramentas de diagnóstico e licenças (o que o R1/R2 não cobriram)

| Ferramenta | O que é | Licença | Estado (01/10/2026) | Uso pelo Warden |
|---|---|---|---|---|
| **Crash Assistant** (KostromDan) | Mod; janela pós-crash num **processo separado** que espera a JVM do jogo terminar (pega travamentos antes da janela do jogo abrir); ~30 análises de log e ~13 de `hs_err`; upload para mclo.gs só ao clicar; compara a lista de mods do jogador com a do pack; *scripts* JEXL para o autor do pack | **KostromDan's Modded Minecraft License 1.1.3**: uso em modpacks liberado (§1), mas **§3 Non-Compete** proíbe usar o código ou o conhecimento obtido dele para criar um "Competing Work" (inclui "replaces specific functionality"); §4.2: obra concorrente só pode detectar a presença ou usar API/documentação pública | 1.11.14 (20/09/2026), 1.691 versões, 19,3 M downloads | Mod padrão para o **jogador** (§2.2). **Não ler o código nem os formatos próprios dele.** O R2 §6.4 dizia só "All Rights Reserved"; a cláusula de não concorrência é mais forte. Não é parecer jurídico. |
| **Not Enough Crashes** | Mod; volta ao menu em vez de fechar, lista mods suspeitos | MIT | 4.4.9 (28/09/2026), ativo | Não recomendado por padrão (§2.2) |
| **mclo.gs + codex-minecraft** (Aternos) | Serviço e biblioteca PHP de análise de logs | MIT | codex-minecraft v5.2.0 (30/06/2026), último commit 10/08/2026; 69 arquivos em `src/Analysis/Problem/` | Porte das regras com atribuição (já decidido, ADR-0014). `POST /1/analyse` analisa sem salvar; limites 10 MiB, 25.000 linhas, 60 req/min **[verificado]**. O Warden **não** envia logs ao mclo.gs (ADR-0014). |
| **spark** (lucko) | Profiler de CPU/memória/TPS | GPL-3.0-only (viewer também GPL-3.0) | 1.10.187 (19/09/2026) | Mod padrão (§2.1) e fonte de perfis (§7.4) |
| **MixinTrace / MixinTrace Reloaded** | Acrescenta "Mixins in Stacktrace" ao crash report | MIT | Reloaded ativo (04/2026) | Complemento opcional (§2.2) |
| **Neruina** | Suspende a entidade/bloco que causa *ticking crash* em vez de travar o mundo | MIT | ativo (07/2026) | Opcional; muda o comportamento do jogo, então não entra por padrão |
| **StackDeobfuscator** | Traduz `class_xxx` nos stack traces | LGPL-3.0 | obsoleto a partir de 26.x (jogo sem ofuscação) | Não |
| Ferramentas de bisseção | §3.9 | — | — | Ideias |
| Detectores de mixin (intermed, ModLint…) | §5.7 | — | — | intermed: spike |

Fontes: https://raw.githubusercontent.com/KostromDan/Crash-Assistant/1.19-1.20.1/LICENSE.md, https://modrinth.com/mod/crash-assistant, https://github.com/natanfudge/Not-Enough-Crashes, https://github.com/aternosorg/codex-minecraft, https://api.mclo.gs/1/limits, https://github.com/lucko/spark, https://modrinth.com/mod/mixintrace-reloaded, https://modrinth.com/mod/neruina.

**Recursos nativos dos loaders** (o Warden já lê os arquivos que eles geram, ARCHITECTURE §9.3): Fabric API acrescenta "Fabric Mods" em árvore ao crash report; o Fabric Loader mostra a janela de erro de dependências num **subprocesso Java** (`FabricGuiEntry`), desligável com `-Dfabric.noGui` **[código]**, o que interessa à busca do culpado (sem janela esperando clique); Forge escreve "Suspected Mods:"; Forge/NeoForge modernos gravam `crash-*-fml.txt`.

## 2. Mods que vêm por padrão: spark e mod de crash report

### 2.1 spark

| Faixa | Disponibilidade | Fonte |
|---|---|---|
| Forge 1.16.5 → 26.3 | Modrinth | **[verificado]** `/v2/project/spark/version` (119 versões) |
| Fabric 1.17.1 → 26.3 | Modrinth; **exige Fabric API** (`fabric-api-base`, `command-api-v2`, `lifecycle-events-v1`, não embutidos) | idem; `spark-fabric/fabric.mod.json` **[código]** |
| NeoForge 1.20.3 → 26.3 | Modrinth | idem |
| Forge 1.12.2 | só CurseForge: spark **1.6.3** (11/2021), sem manutenção | API CurseForge, mod 361579 **[verificado]** |
| Forge 1.7.10 | só CurseForge: `spark-1.10.19-forge1710.jar` (12/2022) | idem |

`allowModDistribution = true` na CurseForge, então o download pelo packwiz funciona sem passo manual.

**Recomendação:** incluir o spark por padrão em todo pack novo, lado "cliente e servidor", como **sugestão marcada no assistente Criar pack** (o usuário pode desmarcar), puxando a Fabric API quando for Fabric. Para 1.7.10 e 1.12.2, oferecer a versão antiga da CurseForge com o aviso "versão antiga, sem atualizações". Licença GPL-3.0 não é problema: o pack só **referencia** o arquivo.

**Cuidados de privacidade e higiene:**
- `/spark profiler upload` envia ao `spark-usercontent.lucko.me` o nome e o UUID de quem rodou o comando, CPU, RAM, SO, os **argumentos da JVM** (com redação parcial), a lista de mods e o `server.properties` filtrado **[código: `spark.proto`, `ServerConfigProvider.java`]**. É uma ação do jogador; o Warden nunca faz upload.
- `--save-to-file` grava em **`config/spark/*.sparkprofile`** e `*.sparkheap` **[código: `SparkPlatform.java`]**. Como `config/` é conteúdo do pack, **esses arquivos precisam entrar na lista "sempre ignorados na captura"** (ARCHITECTURE §8.4) e no `.packwizignore` padrão, senão aparecem em "O que mudou durante o teste" e podem ir para o pack. O mesmo vale para `config/spark/tmp*` **[inferência]**.
- O spark aceita configuração por propriedade de sistema `-Dspark.<chave>` e variável `SPARK_<CHAVE>` **[código: `RuntimeConfiguration.java`]**: o Warden pode ajustar o spark **só nos testes**, sem gravar nada no pack.

### 2.2 Mod de crash report

O pedido do dono ("Crash report") é para o **jogador** que recebe o pack: dentro do Warden, quem analisa é o próprio Warden. Os candidatos:

| Critério | Crash Assistant | Not Enough Crashes | Só MixinTrace (Reloaded) | Nenhum |
|---|---|---|---|---|
| Cobertura | **1.7.10 → 26.3**, Forge, Fabric, Quilt, NeoForge, Legacy Fabric | Fabric 1.16.5 → 26.3, NeoForge 1.20.6 → 26.3, **Forge só 1.16.5–1.20.4**; nada para 1.7.10/1.12.2 | Fabric/Quilt 1.16.1–1.21.1 (original), Forge/NeoForge 1.19–1.21.11 e Fabric 1.21.2–26.1.1 (Reloaded) | — |
| O que o jogador ganha | Janela clara após o travamento, mesmo antes da janela do jogo; upload de logs com um clique; avisos de driver/memória; diferença entre a lista de mods dele e a do pack | Tela de travamento, volta ao menu, mods suspeitos | Só informação extra no crash report | Crash report padrão do jogo |
| Riscos | Licença com não concorrência; jar de ~3,4 MB; **envia metadados a `api.kostromdan.dev` por padrão desde a 1.11.12** (configs `general.send_uploaded_logs_data_to_kostromdan_dev` e `general.wrap_link`) | "Voltar ao menu" deixa o jogo em estado quebrado (issues #41, #138, #191, #194); token do GitHub embutido no código (issue #217) | Quase nenhum | Jogador sem ajuda |
| Licença | Proprietária (MML 1.1.3) | MIT | MIT | — |

**Recomendação [inferência]:**
1. **Padrão: Crash Assistant**, incluído por referência (packwiz baixa da página oficial, que é o que a licença pede), com uma config inicial gerada pelo Warden que **desliga o envio de metadados ao autor e o encurtador de links**, mantém `piracy.enabled=false` (o launcher do Warden é offline) e preenche o link de suporte do pack, se houver. Ele é o único que cobre todas as faixas que o Warden promete.
2. **Regras de convivência por causa da licença:** o Warden só **detecta a presença** do mod e escreve as configs documentadas na página dele; **não lê o código**, não lê os arquivos próprios que ele gera (ex.: `logs/crash_assistant/`) e não copia análises dele. O diagnóstico do Warden continua baseado nos arquivos padrão do jogo e no codex-minecraft (MIT).
3. **Nos testes dentro do Warden**, a janela do Crash Assistant duplicaria (e atrapalharia) o resultado do teste e a busca do culpado. Proposta: o Warden marca spark e Crash Assistant como **"ferramentas do jogador"**; nos testes normais e na bisseção, o Crash Assistant **não é copiado para a instância**; no **Testar como o jogador recebe** ele entra, para conferir a experiência real. Decisão do dono (questão 2).
4. **Plano B** (se o dono não quiser a licença do Crash Assistant): nenhum mod de crash + **MixinTrace Reloaded** onde existir (MIT, leve). O Not Enough Crashes fica como opção manual, com `disableReturnToMainMenu = true`.

| Faixa | Mod de crash padrão | Complemento opcional | spark |
|---|---|---|---|
| Forge 1.7.10 | Crash Assistant | — | 1.10.19 (CurseForge, antigo) |
| Forge 1.12.2 | Crash Assistant (evitar VanillaFix, abandonado) | — | 1.6.3 (CurseForge, antigo) |
| Forge 1.16.5 / Fabric 1.16.5 | Crash Assistant | MixinTrace (Fabric) | Modrinth (Fabric a partir de 1.17.1) |
| 1.18–1.20.1 (Forge, Fabric) | Crash Assistant | MixinTrace / Reloaded | Modrinth |
| NeoForge 1.20.2+ e 1.21.x | Crash Assistant | MixinTrace Reloaded | Modrinth (NeoForge a partir de 1.20.3) |
| 26.x | Crash Assistant | — (sem ofuscação, StackDeobfuscator inútil) | Modrinth |

**Esforço:** **P** (lista de "itens sugeridos" por faixa em dados versionados, como os kits de performance do R1 §2.3.5, mais a config inicial do Crash Assistant e as regras de ignorar).

## 3. Função 1 — Busca automática do mod culpado (bisseção)

### 3.1 O problema e o método manual de hoje

Quando o pack trava e nem as regras determinísticas (ARCHITECTURE §9.2–9.3) nem o log apontam o culpado, a comunidade usa a "busca binária" manual: tirar metade dos mods, testar, repetir com a metade culpada. É lento (cada rodada exige mover arquivos e abrir o jogo), fácil de errar (tirar uma biblioteca quebra metade do pack com um erro diferente) e ninguém anota o que já testou. O Warden tem tudo para automatizar isso: conhece o grafo de dependências (metadados dos jars, R3 §4), controla o launcher, lê o log ao vivo e sabe classificar o resultado de uma sessão (ARCHITECTURE §7.4).

### 3.2 Algoritmo proposto: busca por prefixos em ordem topológica

**Ideia central [inferência, desenho próprio]:** em vez de "cortar o pack ao meio" (que obriga a recalcular dependências a cada corte e pode gerar metades desequilibradas), o Warden ordena os mods numa **ordem topológica do grafo de dependências obrigatórias** (dependências antes de quem depende delas) e testa **prefixos** dessa lista. Todo prefixo de uma ordem topológica é automaticamente "fechado para baixo": se um mod está ligado, suas dependências obrigatórias também estão. Isso elimina o principal erro da bisseção manual (desligar uma biblioteca e ver um erro diferente) sem nenhum cálculo extra.

Passo a passo:

1. **Montar o grafo** a partir do diagnóstico completo (ARCHITECTURE §9.2): arestas `depends`/`required` (Fabric, Forge, NeoForge), `provides`, jar-in-jar (já embutido, não é aresta), dependências da API (Modrinth `dependencies[].dependency_type = required`, CurseForge `relationType = 3`). Ciclos (raros, mas existem com dependências mútuas) viram um só nó: o Warden liga ou desliga o grupo inteiro.
2. **Separar o que nunca entra na busca:** o loader e a Fabric API/bibliotecas exigidas por quase todos ficam sempre ligados *se* todos os mods dependem deles (não há como testar sem eles); mods de ferramenta do Warden (spark, mod de crash report, §2) ficam **desligados** durante a busca, para não interferirem, a menos que sejam suspeitos.
3. **Ordenar** em ordem topológica estável (desempate: bibliotecas primeiro, depois mods por ordem alfabética do id, para o resultado ser reproduzível).
4. **Rodada 0 — reproduzir com o pack inteiro.** Confirma que o problema acontece e grava a **assinatura** dele (§3.4). Se não reproduzir em até 3 tentativas: parar e dizer ("O problema não apareceu em 3 testes seguidos. A busca só funciona com problemas que se repetem.").
5. **Rodada 1 — controle sem mods** (só loader e o que é sempre ligado). Se já falha, o culpado **não é um mod**: Java, driver de vídeo, memória, argumentos de JVM ou config global. O Warden para e mostra isso, que é uma resposta valiosa.
6. **Busca binária no tamanho do prefixo.** Com `lo` = maior prefixo que passou e `hi` = menor que falhou, testa `mid = (lo+hi)/2` até `hi = lo + 1`. O mod na posição `hi` é o **último membro necessário** da combinação que falha.
7. **Combinações de 2+ mods (variante do *delta debugging*).** Fixado o mod `X` encontrado no passo 6, o Warden repete a busca só entre os mods *antes* de `X`, com `X` (e suas dependências) sempre ligado. Se o prefixo vazio + `X` já falha, `X` sozinho basta. Senão, encontra `Y`, o parceiro. Repete até o conjunto mínimo (na prática, 1 a 3 mods). É a mesma ideia do *ddmin* de Zeller e Hildebrandt (minimizar a entrada que falha), adaptada para respeitar dependências.
8. **Confirmação final:** testa só o conjunto mínimo encontrado (deve falhar) e o pack inteiro sem ele (deve passar). Se as duas confirmações batem, a confiança é **alta**; se a segunda falha com a mesma assinatura, existe outro culpado independente e o Warden oferece continuar a busca sem o primeiro.

**Por que prefixos e não metades arbitrárias:** com metades, o fechamento de dependências pode puxar quase o pack inteiro (ex.: 80% dos mods dependem de Architectury e Cloth Config), e a "metade" vira 90%. Com prefixos topológicos, cada teste tem exatamente o tamanho escolhido, e o número de rodadas é o da busca binária pura. A desvantagem é a suposição de **monotonicidade** (adicionar mods não faz um problema sumir), discutida em §3.6.

### 3.3 Número de testes e tempo esperado

| Pack (mods candidatos) | 1 culpado | Par de mods | Com controles (rodadas 0, 1 e confirmação) |
|---|---|---|---|
| 50 | 6 | ~11 | +4 |
| 100 | 7 | ~13 | +4 |
| 200 | 8 | ~15 | +4 |
| 400 | 9 | ~17 | +4 |

`⌈log₂ n⌉` para um culpado; para um par, `⌈log₂ n⌉ + ⌈log₂ (posição do parceiro)⌉` **[inferência, cálculo]**.

Tempo por rodada = preparar a instância (segundos, com links físicos dos jars do cache, §3.5) + abrir o jogo até o marcador de sucesso ou de falha. O S1 mediu 9–15 s até o menu com packs mínimos no Linux e 25–30 s no Windows com GPU real (S1 §3.4, **[verificado]** lá). Packs grandes levam minutos para carregar **[inferência; sem medição própria]**: os prefixos têm em média metade do pack, então uma rodada custa em torno de metade do carregamento completo. Estimativa para um pack de 200 mods que abre em 3 min: 12 rodadas × ~1,5–2 min ≈ **20–25 min** para um culpado, ~40 min para um par. Com problema intermitente (§3.6), multiplicar pelo número de repetições.

O Warden mostra no início a estimativa ("cerca de 12 rodadas, ~25 min") e, durante a busca, "Rodada 5 de ~12 · 25 mods suspeitos restantes".

### 3.4 Como o Warden sabe que "reproduziu o problema"

A rodada 0 grava uma **assinatura** do problema, que é comparada a cada rodada:

| Tipo de problema | Assinatura | Como é detectado |
|---|---|---|
| Travou ao abrir | tipo de exceção + mod/classe da primeira linha de stack trace do pacote do jogo ou de mod + regra do catálogo (ex.: `MIXIN_APPLY_FAILED` com o arquivo `.mixins.json`) | crash report novo, `crash-*-fml.txt`, `hs_err_pid`, código de saída (ARCHITECTURE §7.4) |
| Tela de erro do loader (Forge/NeoForge/Fabric) | código da regra do catálogo (R2 §6.3) + mods citados | linha do log; o jogo fica aberto na tela de erro, então o Warden encerra o processo ao ver o padrão |
| Congelou | "sem nova linha de log por N s depois do último marcador" | relógio; N = 3× o maior intervalo visto na rodada de controle, mínimo 120 s; no servidor dedicado, o *watchdog* do próprio jogo gera crash ("A single server tick took 60.00 seconds") |
| Mensagem no log (erro sem travar) | expressão escolhida pelo usuário ou sugerida pelo Warden a partir da linha selecionada no console | o usuário marca "procurar esta linha" |
| Travou ao entrar no mundo | igual a "travou", mas só depois do marcador de "mundo carregado" | ver abaixo |

Resultados de cada rodada: **passou** (chegou ao marcador de sucesso e ficou estável por 20 s), **falhou igual** (mesma assinatura), **falhou diferente** (outra assinatura) e **tempo esgotado**. "Falhou diferente" **não** conta como reprodução: é tratado como o `git bisect skip` (rodada inconclusiva). O caso mais comum de "falhou diferente" é uma **dependência não declarada** (`NoClassDefFoundError`/`ClassNotFoundException`, padrão 35 do R2 §6.3): o Warden mapeia o pacote da classe que faltou para o jar dono (índice de pacotes, §7.1), acrescenta a aresta ao grafo, avisa ("O mod X usa Y sem declarar a dependência") e refaz a rodada.

**Marcadores de sucesso.** No cliente, o padrão é **chegar ao menu principal** (marcadores por faixa do S1 §3.4: atlas de texturas criado + `Sound engine started`), porque a maioria dos travamentos acontece no carregamento. Para problemas dentro do mundo:
- **Servidor dedicado** (P1 da v1, função 7 do R5B): é o melhor ambiente de bisseção para tudo que não é renderização. Sobe sozinho, cria o mundo, imprime `Done (X.XXXs)!` e aceita `stop` pela entrada padrão. O mundo pode ser recriado a cada rodada com a mesma semente (`level-seed`), o que reproduz travamentos de geração de mundo. Mods só de cliente ficam de fora automaticamente pelo `side`.
- **Cliente 1.20+ (desde o snapshot 23w14a):** o argumento `--quickPlaySingleplayer <pasta do mundo>` abre um mundo existente direto do menu; com `--quickPlayPath <arquivo>`, o jogo grava um JSON (`quickPlayWorld`, `lastPlayedTime`) quando o jogador entra no mundo, um sinal estruturado de "entrou" **[código, client 26.3; regras `has_quick_plays_support` no JSON de versão do 23w14a, https://minecraft.wiki/w/Quick_Play]**. O Warden copia o **mundo de teste** escolhido para uma pasta descartável a cada rodada.
- **Cliente 1.6.1 a 1.19.4:** não há argumento para mundo local, mas existem `--server <host> --port <n>` (removidos no 23w14a), que entram direto num servidor. O Warden sobe um **servidor dedicado local** (`online-mode=false`) e lança o cliente apontando para ele. Isso testa os dois lados de uma vez **[inferência; https://minecraft.wiki/w/Java_Edition_client_command_line_arguments]**.
- **Sem servidor dedicado disponível:** **modo assistido** (§3.7).
- **Marcador de "entrou no mundo"**, igual de 1.7.10 a 26.3: `logged in with entity id` (thread do servidor integrado ou dedicado); servidor pronto: `Done (` … `)! For help, type "help"`. `Preparing spawn area`/`Preparing start region` mudaram ou sumiram em 26.x **[código: cadeias extraídas dos jars 1.7.10, 1.20.1 e 26.3 pelo subagente]**.
- **EULA do servidor:** o servidor só sobe com `eula=true` em `eula.txt`. Aceitar em nome do usuário sem perguntar não é aceitável; o padrão do mercado (ex.: a imagem itzg/docker-minecraft-server exige `EULA=TRUE` explícito) é pedir o aceite. Proposta: diálogo único com o link https://aka.ms/MinecraftEULA na primeira vez que o Warden for subir um servidor; o aceite fica nas Configurações. Limite: o servidor não exercita mixins de cliente (renderização, interface).

**Mundos.** A busca nunca usa os mundos da instância de teste nem os do usuário: cada rodada recebe uma **cópia descartável** (apagada no fim), porque abrir um mundo sem os mods que o criaram apaga blocos e itens desses mods ("missing registry entries"). No Forge 1.12.2, a tela "faltam mods neste mundo" bloqueia o carregamento; a propriedade `-Dfml.queryResult=confirm` responde sozinha **[código: `StartupQuery.java` do Forge 1.12.x lê `System.getProperty("fml.queryResult")` e aceita `confirm` ou `cancel`, https://github.com/MinecraftForge/MinecraftForge/blob/1.12.x/src/main/java/net/minecraftforge/fml/common/StartupQuery.java]**. Em versões novas, telas de confirmação de mundo (mods ausentes, recursos experimentais) podem travar a automação; nesses casos o Warden usa o servidor dedicado ou o modo assistido.

### 3.5 Isolamento, velocidade e cancelamento

- **Instância própria da busca** em `instances/<pack-id>/bisect/` (dados locais, ARCHITECTURE §13), separada da instância de teste. O pack e a instância de teste **nunca** são alterados. Configs vêm do pack a cada rodada; mundos, de cópias.
- **Trocar os mods é barato:** os jars já estão no cache por SHA-256 (`cache/downloads/`); cada rodada recria `mods/` com **links físicos** (NTFS aceita, no mesmo volume) e cai para cópia quando o cache estiver em outro volume **[inferência]**.
- **Um jogo por vez** (regra da T13): a busca ocupa o launcher. O botão do cabeçalho vira "● Buscando o culpado: ver progresso".
- **Cancelar** a qualquer momento: encerra o processo (Job Object, ARCHITECTURE §7.4), apaga a instância da busca e guarda o **resultado parcial**: "O culpado está entre estes 12 mods" (útil por si só e enviado à IA, se o usuário quiser).
- **Pausar entre rodadas** (útil em notebook): o estado da busca (ordem, `lo`, `hi`, resultados) fica em `instances/<pack-id>/state/bisect.json` e pode continuar depois.

### 3.6 Quando a busca não funciona (e como o Warden diz isso)

| Situação | O que acontece | O que o Warden faz e diz |
|---|---|---|
| Problema intermitente (às vezes trava, às vezes não) | um "passou" pode ser sorte | Na rodada 0, repete até 3 vezes e estima a taxa de reprodução *p*. Cada "passou" é repetido `⌈ln 0,05 / ln(1−p)⌉` vezes (p = 50% → 5 vezes) para ter 95% de confiança. Mostra o tempo extra antes de começar. Abaixo de ~30% de reprodução, recomenda não fazer a busca. |
| Resultados contraditórios (um prefixo maior passa depois de um menor falhar) | quebra a suposição de monotonicidade: existe um mod que **corrige** o problema de outro (patches de compatibilidade) ou o problema é aleatório | Para e explica: "Os testes se contradizem: o mod X parece *evitar* o problema. Isso acontece com mods de compatibilidade." Mostra a tabela das rodadas. |
| Precisa de uma ação no jogo (abrir um inventário, ir ao Nether) | o Warden não sabe jogar | **Modo assistido** (§3.7). |
| Problema de desempenho (lag), não travamento | não há "falhou" binário | P2: limiar sobre uma métrica (MSPT do servidor via spark, §7.4). Na v1, fora. |
| O culpado é uma config, um script KubeJS ou um resource pack | a busca só liga/desliga mods | Dizer que a busca terminou sem culpado entre os mods. Bisseção de configs e scripts fica para P2. |
| O pack sem mods já falha (rodada 1) | não é mod | Mostrar: "Mesmo sem mods o jogo trava. A causa está no Java, no driver de vídeo ou nos ajustes do teste." com links para Ajustes do teste. |

Resultado final mostrado ao usuário (texto de exemplo, no tom do QUALITY §8):

> **Culpado encontrado: Iris (com Sodium)**
> O jogo trava ao abrir quando o Iris está ligado junto com o Sodium. Sem o Iris, abriu normalmente; só com o Iris e o Sodium, travou com a mesma mensagem.
> Confiança: alta (confirmado em 2 testes de controle) · 13 rodadas · 22 min
> [Desligar Iris no pack] [Ver o que o Iris altera no jogo] [✦ Perguntar à IA] [Ver as rodadas]

"Ver as rodadas" abre a tabela: rodada, mods ligados (contagem e lista), resultado, assinatura e link para o log de cada uma. "Ver o que o Iris altera" leva ao raio-x de mixins (§5) já filtrado nos dois mods.

### 3.7 Modo assistido

Para problemas que exigem uma ação no jogo, o Warden faz o mesmo algoritmo, mas cada rodada termina com uma pergunta: "Abra o jogo, faça o que causa o problema e diga o que aconteceu: **Aconteceu** · **Não aconteceu** · **Não deu para testar**". Detecção automática continua ligada (se travar, a rodada é marcada sozinha). É o que os jogadores fazem à mão hoje, com o Warden cuidando das dependências, das cópias de mundo e das anotações.

### 3.8 Viabilidade, riscos e esforço

- **Viabilidade:** alta. Tudo reaproveita peças P0 (grafo do diagnóstico completo, materialização, supervisor de processo, catálogo de padrões). O algoritmo em si é pequeno; o trabalho está na **detecção confiável de sucesso/falha por faixa de versão** e no servidor dedicado.
- **Riscos:** marcadores de log diferentes entre versões (mitigação: os mesmos marcadores da matriz L-05 do ROADMAP); dependências não declaradas (mitigação: aresta inferida, §3.4); tempo longo em packs grandes (mitigação: estimativa honesta, pausa e resultado parcial); mods que exigem *download* manual da CurseForge (já estão no cache depois do primeiro teste).
- **Esforço:** **M** (núcleo automático no cliente até o menu + modo assistido); **+M** para o servidor dedicado (que o R5B já trata como função própria).
- **Depende de:** diagnóstico completo (grafo), índice de pacotes (§7.1), catálogo de padrões (ARCHITECTURE §9.3), supervisor de processo. **Alimenta:** IA (ferramenta `get_bisect_result`), histórico de travamentos.

### 3.9 O que já existe

| Ferramenta | Licença / estado | Como funciona | Respeita dependências? | Automatiza o teste? |
|---|---|---|---|---|
| **mod-bisect-tool** (Qendolin, Go) | MPL-2.0, ativo (09/2026) | Busca guiada: o usuário abre o jogo e responde; acha conflitos de 2+ mods e tem "continuar busca" para conflitos independentes | **Sim** (liga as dependências obrigatórias, trata jar-in-jar) | Não |
| FabricBinarySearchTool (skycatminepokie) | MIT, **arquivado** | Desliga metade dos mods renomeando para `.disabled` | Sim | Não; acha só 1 culpado |
| **Prism Launcher PR #5855** | GPL-3.0, **fechado sem merge em 22/09/2026**; a issue #1924 segue aberta | Botão "Bisect mods" no estilo ddmin; mods "travados"; detecta respostas inconsistentes | Sim | Semi: lança sozinho, mas quem julga é o usuário |
| **Quilt Bisect** | sem licença no repositório (Modrinth: ARR); só Quilt 1.20.1–1.20.2, parado desde 2024 | Busca binária com "seções" para conflitos n-ários; relança o jogo em subprocesso, entra num mundo sozinho, dá "OK" após N ticks, faz backup do mundo | **Não** (admite que pode culpar o dependente) | **Sim**, o mais automatizado |
| emendator | GPL-3.0, 06/2026, só Fabric | Análise estática + servidor em Docker + bisseção em ~log₂N boots | ? | Sim, só para travamento no carregamento; não cobre mixins de cliente |
| Scope Launcher | MIT | "Automated Crash Bisect" | ? | Lança sozinho [inferência] |

HMCL, ATLauncher e o Modrinth App não têm bisseção **[inferência por ausência]**. Fontes: https://github.com/Qendolin/mod-bisect-tool, https://github.com/skycatminepokie/FabricBinarySearchTool, https://github.com/PrismLauncher/PrismLauncher/pull/5855, https://github.com/anonymous123-code/quilt-bisect, https://github.com/RemiAsselin42/emendator, https://github.com/lonestill/scope-launcher.

**Base teórica:** *ddmin* (Zeller e Hildebrandt, "Simplifying and Isolating Failure-Inducing Input", IEEE TSE 28(2), 2002, https://www.st.cs.uni-saarland.de/papers/tse2002/) usa três resultados (falha, passa, **não resolvido**) e chega a um conjunto 1-mínimo (tirar qualquer elemento faz o problema sumir); `git bisect run` usa o código 125 para "pular" (https://git-scm.com/docs/git-bisect). A proposta da §3.2 combina os dois: prefixos fechados por dependência, "falhou diferente" como *skip*, e minimização para pares.

**O que o Warden faria que ninguém faz junto:** respeitar dependências (como o mod-bisect-tool) **e** automatizar o veredito por assinatura do travamento (como o Quilt Bisect, mas em todos os loaders e versões), com servidor dedicado para versões antigas, mundos descartáveis, resultado parcial ao cancelar e integração com o raio-x e a IA. Nenhum código é reaproveitável por licença ou linguagem, exceto ideias; o algoritmo é pequeno.

## 4. Função 2 — IA "médico" com ferramentas e chat

### 4.1 O que muda em relação ao desenho atual

Hoje (ARCHITECTURE §9.5, SPEC T14) a IA recebe **um pacote fixo** (resumo do pack, achados, até 200 KB de log) e devolve **uma resposta** em JSON. O MCDoctor.ai faz algo parecido, só que pior: analisa um arquivo por vez, sem ver a pasta `mods`, os jars ou as configs, e o próprio changelog dele registra correções para diagnósticos inventados ("culpar *warnings*", "sugerir instalar mod que não existe", "confundir log cortado com crash", versões 2.0.6–2.0.8) **[verificado, https://mcdoctor.ai/changelog]**.

A proposta é a IA virar um **agente com ferramentas**: em vez de receber tudo de uma vez, ela **pergunta ao Warden** o que precisa (ler um trecho do log, ver os metadados de um jar, comparar com a última versão que funcionava, buscar issues do mod) e responde **sempre citando a evidência** que as ferramentas devolveram. O usuário conversa (chat), e qualquer mudança no pack vira uma **proposta** que só é aplicada quando ele clica.

Vantagens sobre o pacote fixo: manda menos dados (só o que a IA pediu), alcança coisas que não cabem num pacote (configs, histórico, changelogs entre duas versões, issues) e permite perguntas de seguimento ("e se eu atualizar o Create em vez de remover?").

### 4.2 A API: function calling do Gemini (estado em 01/10/2026)

Fatos levantados na documentação oficial **[doc, verificado em 01/10/2026]**:

| Tema | O que vale | Fonte |
|---|---|---|
| Endpoint | `POST /v1beta/models/{id}:generateContent` com `x-goog-api-key` continua "fully supported". O Google agora **recomenda** a nova *Interactions API* (`/v1beta/interactions`, GA em junho/2026) para projetos novos. | https://ai.google.dev/gemini-api/docs/migrate-to-interactions |
| Declaração de ferramentas | `tools: [{ functionDeclarations: [{ name, description, parametersJsonSchema }] }]` (JSON Schema; `parameters` no subconjunto OpenAPI é a alternativa, as duas são exclusivas). | https://ai.google.dev/api/generate-content |
| Modos | `toolConfig.functionCallingConfig.mode`: `AUTO`, `ANY`, `NONE`, `VALIDATED` (valida a chamada com decodificação restrita); `allowedFunctionNames` com `ANY`/`VALIDATED`. | https://ai.google.dev/api/caching#FunctionCallingConfig |
| Chamadas paralelas e em sequência | Suportadas nos 3.x e 2.5. | https://ai.google.dev/gemini-api/docs/generate-content/function-calling |
| Devolver o resultado | conteúdo `role: "user"` com `functionResponse { id, name, response }`; no Gemini 3, repetir o `id` exato da chamada; erro vai em `response.error`. | idem |
| *Thought signatures* | **Obrigatórias** no function calling do Gemini 3 ("Missing signatures will result in a 400 error"). Regra prática: reenviar o `content` do modelo **inteiro e intacto**, sem fundir partes. | https://ai.google.dev/gemini-api/docs/generate-content/thought-signatures |
| Quantas ferramentas | Sem máximo documentado; recomendação oficial de manter **10–20 ativas**. Declarações contam como tokens de entrada. | function-calling |
| Ferramentas + saída em JSON | Suportado nos modelos **Gemini 3**. `responseSchema` está *deprecated*; usar `responseFormat`/`responseJsonSchema`. | https://ai.google.dev/gemini-api/docs/generate-content/structured-output |
| Temperatura | Para Gemini 3, o guia recomenda **manter 1.0** (valores baixos podem causar repetição em laço), contrariando a página genérica que sugere 0. | https://ai.google.dev/gemini-api/docs/generate-content/gemini-3 |
| Modelos | Família 3.x estável: `gemini-3.8-flash` (recomendado, 02/09/2026), 3.7/3.6/3.5-flash, `gemini-3.5-flash-lite`; `gemini-3.1-pro-preview` (sem plano gratuito). Todos com 1.048.576 tokens de entrada, 65.536 de saída, function calling e thinking. `gemini-3.1-flash-lite` será desligado a partir de 07/05/2027. 2.5 com acesso restrito a quem já usava. | https://ai.google.dev/gemini-api/docs/models, /deprecations |
| Plano gratuito | Limites RPM/TPM/RPD **não estão mais na documentação** (só no AI Studio); valem por projeto. Termos: no plano gratuito, "Google uses the content you submit… human reviewers may read, annotate, and process your API input and output… Do not submit sensitive, confidential, or personal information". No pago, o conteúdo não é usado para melhorar produtos. | https://ai.google.dev/gemini-api/docs/rate-limits, https://ai.google.dev/gemini-api/terms |

**Consequências para o desenho atual [inferência]:**
- A ARCHITECTURE §9.5 já escolhe o modelo por `models.list`, sem nome fixo: continua certo. A preferência passa a ser "o Flash estável mais novo com `generateContent` e function calling" (hoje `gemini-3.8-flash`), com `gemini-3.5-flash-lite` como opção econômica.
- **Ficar no `generateContent` sem estado.** A Interactions API com `store: true` guarda os dados no servidor do Google (1 dia no gratuito, 55 no pago), o que contraria a postura de privacidade. Sem estado, o Warden reenvia o histórico a cada rodada, e isso preserva as *thought signatures* naturalmente.
- Trocar `responseSchema` (ARCHITECTURE §9.5) por `responseFormat`/`responseJsonSchema` na resposta final.
- Adotar `VALIDATED` nas rodadas de ferramenta e tratar `finishReason = MALFORMED_FUNCTION_CALL` como erro recuperável (uma nova tentativa).

### 4.3 Ferramentas: o que a IA pode consultar e o que só pode propor

Princípio: **a IA nunca altera nada**. Ferramentas de leitura respondem na hora; ferramentas de ação só criam uma **proposta** que aparece como cartão no chat, com a diferença exata e o botão **Aplicar**. Aplicar passa pelos mesmos fluxos do app (diálogo de dependências, salvar config com diferença etc.) e cria um ponto de segurança (T17) para desfazer.

**Leitura (sem confirmação a cada chamada; tudo passa pela redação da §9.4):**

| Ferramenta | Devolve | Evidência citável |
|---|---|---|
| `get_pack_overview()` | MC, loader e versões, Java, memória, número de itens, nota de saúde (§6.2), último teste | `pack:overview` |
| `list_mods(filter?)` | id, nome, versão, lado, fonte, se é biblioteca, se tem mixins; filtro por texto/categoria | `mod:<id>` |
| `get_mod_details(mod_id)` | metadados do jar (dependências, faixas, `provides`, jar-in-jar, Java exigido), dados da API (links, categorias, data da versão), achados que o envolvem | `jar:<id>/<arquivo>`, `api:<fonte>/<id>` |
| `get_findings(kind?)` | achados determinísticos do pré-teste e do último travamento, já com evidência | `finding:<código>#n` |
| `list_sessions()` / `get_session(id)` | testes anteriores: resultado, duração, assinatura do travamento | `session:<id>` |
| `search_log(session, query, context=5)` | linhas que casam (texto ou regex), com número de linha, no máximo 40 trechos | `log:<session>/<arquivo>#L<n>` |
| `read_log(session, file, from, to)` | trecho de até 300 linhas | idem |
| `get_crash_report(session)` | crash report inteiro (redigido) | `crash:<session>` |
| `search_configs(query)` / `read_config(path, from?, to?)` | busca em todas as configs e leitura de um arquivo | `config:<path>#L<n>` |
| `get_history(limit)` / `diff_versions(a, b)` | versões salvas e "o que mudou" entre duas (mods +/−/↑, configs alteradas) | `version:<a>..<b>` |
| `get_mixin_report(mod_id?)` | inventário e sobreposições do raio-x (§5) | `mixin:<id>` |
| `get_dependents(mod_id)` / `why_in_pack(mod_id)` | consultas do grafo (§6.1) | `graph:<id>` |
| `get_bisect_result()` | última busca do culpado, com as rodadas | `bisect:<id>` |
| `get_mod_changelog(mod_id, from, to)` | changelogs das versões entre `from` e `to` (Modrinth `GET /v2/project/{id}/version` com `include_changelog`; CurseForge `GET /v1/mods/{modId}/files/{fileId}/changelog`, em HTML convertido para texto) | `changelog:<id>@<versão>` |
| `search_mod_issues(mod_id, query)` | até 5 issues (título, estado, data, URL, trecho) do repositório do mod no GitHub, achado por `source_url`/`issues_url` do Modrinth ou `links.sourceUrl`/`issuesUrl` da CurseForge | `issue:<owner>/<repo>#<n>` |

**Propostas (sempre exigem o clique do usuário):** `propose_add_mod`, `propose_remove_mod`, `propose_update_mod(to_version)`, `propose_change_version`, `propose_set_side`, `propose_edit_config(path, key, value)`, `propose_test_settings(memory, java, jvm_args)`, `propose_run_test()`, `propose_start_bisect()`. Cada proposta precisa citar pelo menos uma evidência; sem evidência, o Warden recusa a proposta e devolve erro à IA.

São ~22 ferramentas; para ficar na faixa recomendada de 10–20 ativas, o Warden declara **por etapa**: na primeira rodada, as de visão geral e log; as de changelog/issues e as propostas entram quando a IA já identificou mods suspeitos (`allowedFunctionNames`) **[inferência]**.

**Issues e changelogs, detalhes [verificado pela pesquisa]:**
- Modrinth: `issues_url`/`source_url` vêm no projeto (ex.: Sodium → `https://github.com/CaffeineMC/sodium/issues`); limite de 300 req/min por IP e *User-Agent* identificável obrigatório (https://docs.modrinth.com/api/). Nos 100 projetos mais baixados, 92 têm `source_url` no GitHub.
- GitHub: `GET /search/issues?q=repo:OWNER/REPO+is:issue+<termos>` funciona sem token, mas com **10 buscas/min** e 60 req/h no restante da API, por IP; com token, 30 buscas/min e 5.000 req/h (https://docs.github.com/en/rest/search/search). O Warden já terá o token do GitHub para publicar (ADR-0028), então usa-o quando existir, e guarda as respostas em cache.
- Repositórios no GitLab/Codeberg ou sem fonte: a ferramenta devolve só o link, e a IA diz que não conseguiu consultar.
- Texto de issues, changelogs e logs é **dado não confiável** (pode conter instruções escondidas, o "prompt injection indireto" do OWASP LLM01, https://genai.owasp.org/llmrisk/llm01-prompt-injection/): vai delimitado e marcado como dado, e nenhuma ferramenta tem efeito sem o clique do usuário.

### 4.4 Como evitar que a IA invente

1. **Evidência obrigatória e conferida por código.** A resposta final é JSON (`responseFormat`) com `achados[] { afirmacao, evidencias[] { id, citacao }, confianca }` e `propostas[]`. O Rust confere que cada `id` foi devolvido por uma ferramenta **nesta conversa** e que a `citacao` aparece literalmente no resultado daquela ferramenta. Afirmação sem evidência válida aparece riscada como "não verificado" (ou é removida, a decidir no protótipo).
2. **Nomes conferidos:** todo mod citado precisa existir no pack ou na API (evita "instale o mod X" inventado, o erro que o MCDoctor corrigiu na 2.0.8).
3. **"Não sei" é resposta válida:** o *system prompt* manda dizer que não encontrou a causa e sugerir a busca do culpado (§3) quando as evidências não bastam.
4. **Determinístico primeiro:** os achados das regras entram como fatos; a IA não pode contradizê-los sem citar evidência mais forte.
5. **Limites:** no máximo 8 rodadas de ferramenta por pergunta e um teto de tokens; ao estourar, o Warden força `mode: NONE` e pede o resumo do que já sabe.
6. **Instruções do Google para *grounding*:** "rely only on the facts that are directly mentioned in that context" e "verify your claims by quoting the exact applicable information" (https://ai.google.dev/gemini-api/docs/prompting-strategies).

### 4.5 Privacidade e consentimento (precisa de decisão do dono)

A regra atual (ADR-0014, CA-T14-03) é "mostrar o texto exato antes de cada envio". Com ferramentas, cada rodada envia um resultado novo, e pedir confirmação a cada uma tornaria o chat inutilizável. Proposta **[inferência]**:

- **Ao começar uma conversa**, um consentimento único que lista **o que a IA poderá consultar** (pack e configs; logs e crash reports, sempre redigidos; busca em issues do GitHub) e mostra o texto inicial exato. Opções: **Começar** / **Cancelar**, e uma caixa "Não deixar a IA ler configs" para quem quiser.
- **Cada resultado de ferramenta aparece no chat** num bloco recolhido "Enviado à IA: trecho do latest.log, linhas 1.200–1.260", que é byte a byte o que foi enviado (mantém o espírito do CA-T14-03).
- **Propostas sempre com clique.**
- Aviso do plano gratuito com o texto real dos termos do Google (revisores humanos podem ler).

Isso muda a ADR-0014; está nas questões (§11).

### 4.6 Histórico de conversas por pack

- Cada conversa vira `ai/<pack-id>/<ULID>.json` (já previsto na ARCHITECTURE §9.5 para respostas), agora com as mensagens, as chamadas de ferramenta, os resultados **como enviados** (já redigidos), modelo, tokens (`usageMetadata`) e as propostas aplicadas. Nunca na pasta do pack.
- "Respostas anteriores" da seção vira **Conversas**: título automático (a causa provável), data, o que foi analisado, situação ("resolvido por: remover X"), **Continuar** e **Apagar**.
- Ao continuar uma conversa antiga, o Warden avisa se o pack mudou desde então ("3 mods foram atualizados desde esta conversa") e oferece começar uma nova.

### 4.7 Custo e tamanho

Estimativa para uma sessão típica (30 mil tokens de contexto inicial, 5 rodadas de ferramenta com ~5 mil tokens de resultado e ~2 mil de saída cada, reenviando o histórico): **~220 mil tokens de entrada e ~10 mil de saída** **[inferência, cálculo da pesquisa]**.

| Modelo | Custo aproximado por sessão |
|---|---|
| `gemini-3.8-flash`, preço promocional até 31/12/2026 (US$ 0,75 entrada / US$ 3,75 saída por milhão) | ~US$ 0,20 (≈ US$ 0,09 se o cache implícito acertar) |
| `gemini-3.8-flash` a partir de 2027 (US$ 1,50 / US$ 7,50) | ~US$ 0,41 |
| `gemini-3.5-flash-lite` (US$ 0,30 / US$ 2,50) | ~US$ 0,09 |
| `gemini-3.1-pro-preview` | ~US$ 0,56 |
| Plano gratuito | US$ 0, com limites e uso dos dados pelo Google |

Preços em https://ai.google.dev/gemini-api/docs/pricing. Para baratear: colocar o contexto estável (visão geral do pack, achados) **no começo** para aproveitar o cache implícito (mínimo de 4.096 tokens nos 3.x), resultados de ferramenta curtos (trechos, não arquivos inteiros) e `countTokens` antes de enviar para mostrar o tamanho no consentimento. O chat mostra "Esta conversa: ~180 mil tokens".

### 4.8 Viabilidade, riscos e esforço

- **Viabilidade:** alta. É um laço HTTP em Rust (sem SDK oficial, o que a ARCHITECTURE já assume), mais as ferramentas, que são consultas a dados que o Warden já tem.
- **Riscos:** custo no plano pago; mudança de API (a própria doc moveu o `generateContent` para outra seção); *thought signatures* em chamadas paralelas (relatos da comunidade de erro 400, não verificado); injeção de instruções via issues; respostas convincentes e erradas (mitigadas pela conferência de evidência).
- **Esforço:** **G** no total: laço com ferramentas e conferência de evidência (M), ~20 ferramentas de leitura (M, a maioria fina sobre o que existe), propostas aplicáveis (M, reusa fluxos), chat e conversas (M).
- **Depende de:** diagnóstico P0, redação §9.4, histórico (git), raio-x e bisseção (opcionais: viram ferramentas quando existirem).

## 5. Função 3a — Raio-x de mixins

### 5.1 O que é um mixin e por que ele causa conflitos

Mixin (SpongePowered) é o mecanismo que quase todos os mods modernos usam para alterar o código do próprio Minecraft: cada mod declara uma ou mais **configs de mixin** (`*.mixins.json`) que listam classes de mixin; cada classe de mixin diz, por anotação, **qual classe do jogo** altera (`@Mixin(Alvo.class)` ou `targets = "..."`) e **como** (injetar código no começo de um método, trocar uma chamada, substituir o método inteiro etc.). Quando dois mods mexem no mesmo lugar do jogo, a ordem e o tipo das alterações decidem se tudo convive, se um anula o outro em silêncio ou se o jogo trava ao carregar (padrões 23–25 do R2 §6.3).

O raio-x de mixins é uma **análise estática** (sem abrir o jogo) que lê os jars do pack e responde: "quais mods alteram as mesmas partes do jogo, e de que forma?".

### 5.2 Onde ficam os dados dentro dos jars

| Loader | Onde o mod declara as configs de mixin | Jar dentro de jar | Nomes nos alvos |
|---|---|---|---|
| Fabric / Quilt | `fabric.mod.json` → `"mixins": ["x.mixins.json", {"config": "...", "environment": "client"}]` | `META-INF/jars/*.jar` (listados em `"jars"`) | *intermediary* (`net/minecraft/class_761`, `method_3251`); textos traduzidos pelo **refmap** |
| Forge (1.13+) | `META-INF/MANIFEST.MF` → `MixinConfigs: a.json,b.json` (linhas quebradas a cada 72 bytes) | `META-INF/jarjar/*.jar` + `META-INF/jarjar/metadata.json` | SRG (`m_47505_`) via refmap |
| NeoForge | `META-INF/neoforge.mods.toml` → `[[mixins]] config = "..."` (e, em versões antigas, o `MixinConfigs` do manifesto) | `META-INF/jarjar/*.jar` | Mojang (`Biome#getTemperature`) a partir de 1.20.5: sem refmap |
| Forge 1.12.2 / 1.7.10 | MixinBooter / UniMixins: `MANIFEST.MF` ou registro em código (`IEarlyMixinLoader`) | raro | MCP/SRG antigos; parte das configs só é registrada em código **[inferência]** |
| Minecraft 26.x | igual ao loader | igual | **nomes originais da Mojang**: o jogo deixou de ser ofuscado a partir dos *snapshots* de 26.1 (anúncio da Mojang, 29/10/2025), então refmaps e tabelas de nomes deixam de ser necessários |

Cada config de mixin (`*.mixins.json`) traz `package`, as listas `mixins`/`client`/`server`, `priority` (padrão 1000), `refmap`, `plugin` (classe que pode **ligar ou desligar mixins em tempo de execução**), `injectors.defaultRequire` e `compatibilityLevel`. O refmap (`*.refmap.json`) tem `mappings: { "<classe do mixin>": { "<texto no código>": "<nome em tempo de execução>" } }`.

O JSON diz *quais* classes de mixin existem; o bytecode diz *o alvo e o tipo* de cada alteração. As anotações ficam em **dois atributos diferentes** do `.class`, e é preciso ler os dois **[código: SpongePowered/Mixin; confirmado no protótipo e no bench do subagente]**:
- `@Mixin` (alvos e prioridade), `@Pseudo`, `@Implements`, `@Intrinsic`, `@Dynamic`, `@Group` têm retenção `CLASS` → atributo **`RuntimeInvisibleAnnotations`**;
- `@Inject`, `@Redirect`, `@Overwrite`, `@ModifyArg(s)`, `@ModifyVariable`, `@ModifyConstant`, `@At`, `@Shadow`, `@Accessor`, `@Invoker` e os do MixinExtras (`@WrapOperation`, `@ModifyExpressionValue`, `@WrapMethod`) têm retenção `RUNTIME` → atributo **`RuntimeVisibleAnnotations`**;
- `@Local`, `@Share` e `@Cancellable` (MixinExtras) são anotações de parâmetro.

No Sodium Fabric 1.20.1: 92 `@Mixin` invisíveis e 0 visíveis; 117 injetores visíveis e 0 invisíveis.

### 5.3 Experimento: protótipo em Rust com jars reais **[verificado]**

Protótipo descartável `~/.local/share/warden-r5/mixin-xray` (Rust, crates `cafebabe` 0.9 para ler `.class`, `zip` 8 e `serde_json`). Ele:
1. lê `fabric.mod.json`, `mods.toml`/`neoforge.mods.toml` e `MANIFEST.MF` para achar as configs de mixin;
2. desce recursivamente nos jars aninhados (`META-INF/jars/`, `META-INF/jarjar/`);
3. lê cada config e o refmap;
4. abre cada classe de mixin, extrai `@Mixin` (alvos e prioridade) e, em cada método, as anotações `@Overwrite`, `@Inject`, `@Redirect`, `@ModifyArg(s)`, `@ModifyVariable`, `@ModifyConstant` e as do MixinExtras (`@WrapOperation`, `@ModifyExpressionValue`, `@ModifyReturnValue`, `@WrapWithCondition`, `@WrapMethod`), com o método-alvo e o ponto de injeção (`@At`), traduzidos pelo refmap;
5. cruza alvos entre mods e classifica o risco.

Jars usados (versões mais recentes do Modrinth em 01/10/2026):

| Conjunto | Mods | Jars lidos (com aninhados) | Configs | Classes de mixin | Alterações extraídas | Tempo |
|---|---|---|---|---|---|---|
| Fabric 1.20.1 | Sodium 0.5.13, Lithium 0.11.4, Iris 1.7.6, Fabric API 0.92.12, ModernFix 5.25.2 | 68 (63 aninhados, quase todos módulos da Fabric API) | 80 | 899 | 1.410 | **62–67 ms** |
| NeoForge 1.21.1 | Sodium 0.8.13, Iris 1.8.14-beta.1, ModernFix 5.27.24, Lithium 0.15.4, FerriteCore 7.0.3 | 16 (11) | 34 | 698 | 1.080 | 39 ms |
| Forge 1.20.1 | Embeddium 0.3.31, Oculus 1.8.0, ModernFix 5.27.85, FerriteCore 6.0.1 | 9 (5) | 23 | 392 | 641 | 25 ms |

Zero erros de leitura de classe em 1.989 classes. Contagem de anotações no conjunto Fabric: 855 `@Inject`, 229 `@Redirect`, 150 `@Overwrite`, 52 `@ModifyVariable`, 38 `@ModifyArg`, 4 `@ModifyConstant`, 15 `@WrapOperation`, 209 `@Accessor`, 147 `@Shadow`.

Resultado do cruzamento no conjunto Fabric 1.20.1: **83 classes do jogo** alteradas por 2+ mods e **86 métodos** alterados por 2+ mods, classificados pelo protótipo em 2 de risco alto, 5 médio e 79 baixo (o protótipo tratava `@ModifyConstant` como médio; a regra final da §5.5 o trata como `@Redirect`, o que não muda estes números, porque nenhum `@ModifyConstant` coincidiu). Exemplos reais:

```text
- ALTO: @Overwrite de 2+ mods [condicional: todos os mixins têm plugin]
    net/minecraft/class_630#method_22703            (ModelPart#render)
    sodium:Overwrite@ModelPartMixin[1000,plugin]{}
    iris:Overwrite@ModelPartMixin[1000,plugin]{}
- ALTO: @Overwrite de 2+ mods [condicional: todos os mixins têm plugin]
    net/minecraft/class_1959#method_21740           (Biome#getTemperature)
    lithium:Overwrite@BiomeMixin[1000,plugin]{}
    modernfix:Overwrite@BiomeMixin[1000,plugin]{}
- MÉDIO: @Redirect + outro injetor no mesmo ponto
    net/minecraft/class_1088$class_7778#method_45873
    fabric-model-loading-api-v1:Redirect@ModelLoaderBakerImplMixin[1000]{INVOKE:...class_1100;method_4753(...)}
    modernfix:WrapOperation@ModelBakerImplMixin[600,plugin]{INVOKE:...class_1100;method_4753(...)}
```

No conjunto NeoForge 1.21.1 (nomes da Mojang, legíveis sem tradução): 3 altos (Lithium × ModernFix em `Biome#getTemperature`; FerriteCore × Lithium em `PalettedContainer#acquire` e `#release`) e 1 médio (Sodium substitui `MultiPartBakedModel#getQuads` inteiro, e o FerriteCore troca chamadas `Map.get`/`Map.put` dentro desse mesmo método). No Forge 1.20.1: 11 classes em comum e nenhum método em comum.

### 5.4 O que o experimento ensinou

1. **Ler tudo é rápido e viável em Rust.** Dezenas de milissegundos para cinco mods grandes; um pack de 300 mods deve ficar na casa de 1–3 s **[inferência por extrapolação]**, rodando junto com a passagem completa do diagnóstico e com cache por hash do jar.
2. **Os dois "riscos altos" do conjunto Fabric são falsos positivos práticos.** Sodium + Iris e Lithium + ModernFix são combinações usadas em milhões de instalações. O que o experimento mostrou nos próprios jars:
   - o `ModelPartMixin` do Iris fica em `net/irisshaders/iris/compat/sodium/mixin/`, referencia classes do Sodium e é ligado por `IrisSodiumCompatMixinPlugin`, que chama `isModLoaded`: é **compatibilidade intencional**, não conflito **[verificado, cadeias do bytecode]**;
   - o `BiomeMixin` do ModernFix é a opção `mixin.perf.remove_biome_temperature_cache` ("Removes the biome temperature cache **as Lithium does**", no `en_us.json` do próprio mod), e `ModernFixEarlyConfig` contém a cadeia `lithium`, o que indica que a opção é desligada com o Lithium presente **[verificado: cadeias; a regra exata não foi lida no código-fonte]**.
3. **O campo `plugin` muda tudo.** Sodium, Lithium, ModernFix, FerriteCore e Iris usam plugins de mixin que ligam/desligam mixins conforme opções e mods presentes. Em 13 dos 86 métodos compartilhados (Fabric) e 31 de 38 (NeoForge), **todos** os mixins envolvidos têm plugin. A análise estática não sabe o que o plugin decide.
4. **"Mesmo método" não basta; é preciso "mesmo ponto".** A primeira versão do protótipo marcava como alto qualquer par de `@Redirect` no mesmo método; a maioria mirava chamadas **diferentes** dentro do método. Comparar o `@At` (tipo + alvo) reduziu os altos de 9 para 2.
5. **A ordem importa e é calculável:** prioridade menor aplica antes (padrão 1000, em `@Mixin(priority)` ou no `priority` da config). O Iris usa 999 e 1010 em configs diferentes justamente para se encaixar antes/depois do Sodium. O raio-x deve mostrar a ordem prevista.
6. **Refmap faltando = nomes não comparáveis.** O Sodium 0.5 (Fabric) não declara refmap e escreve os alvos já em *intermediary*; o FerriteCore no Forge 1.20.1 deixa textos sem tradução. Sem refmap, o Warden precisaria das tabelas de nomes (Mojang → intermediary/SRG) para comparar; em NeoForge ≥ 1.20.5 e em 26.x isso não é necessário.
7. **Detalhe de implementação:** o `mods.toml` precisa de um parser TOML de verdade (o protótipo leu `modernfix" #mandatory` por cortar o comentário à mão).

### 5.5 Regras de risco propostas (explicáveis)

| Situação no mesmo método-alvo, mods diferentes | Risco | Por quê |
|---|---|---|
| `@Overwrite` de 2+ mods | **Alto** | Só um corpo sobrevive: o de **maior prioridade** vence; com prioridade igual, o primeiro fica e o outro é pulado com o aviso `Method overwrite conflict for … previously written by … Skipping method.` Não trava: a falha é **silenciosa** (uma funcionalidade some). |
| `@Redirect` ou `@ModifyConstant` de 2+ mods no **mesmo** `@At` | **Alto** | Os dois "reivindicam" a instrução: o primeiro fica, o rival de prioridade menor ou igual é pulado com `@Redirect conflict. Skipping …` (o `ModifyConstantInjector` herda do `RedirectInjector`, mesma regra). Vira erro (`InvalidInjectionException`) quando o anterior é `@Final`. |
| `@Overwrite` de um + injeção **no corpo** (`INVOKE`, `FIELD`, `CONSTANT`, `NEW`, `JUMP`) de outro | **Médio** | O Mixin faz primeiro o *merge* de todos os mixins da classe (inclusive `@Overwrite`) e só depois roda os injetores, que operam **sobre o corpo já substituído**. Se o ponto sumiu e `require`/`defaultRequire ≥ 1`, vira `Critical injection failure` (travamento). |
| `@Redirect` de um + outro injetor no mesmo `@At` | **Médio** | Depende da ordem de aplicação. |
| `@ModifyVariable`/`@ModifyArg(s)` de 2+ mods no mesmo `@At` | **Médio** | Encadeiam (não reivindicam a instrução), mas o resultado depende da ordem; conflito lógico, raramente travamento. |
| `@Overwrite` + injeção `HEAD`/`RETURN`/`TAIL` | Baixo | Esses pontos existem em qualquer corpo. |
| Só `@Inject`, `@WrapOperation`, `@ModifyExpressionValue`, `@WrapMethod` | Baixo | Feitos para coexistir (o MixinExtras foi criado para isso). |

**Rebaixadores** (diminuem um nível e dizem por quê):
- todos os mixins envolvidos têm `plugin` → "pode ser desligado pelo próprio mod";
- o mixin de um mod referencia classes do outro mod, ou está numa config/pacote de compatibilidade (`compat`), ou o mod declara o outro como dependência/recomendação → "compatibilidade intencional";
- o par aparece na lista curada de "pares conhecidamente compatíveis" (`data/mixin-known-compatible.toml`, mesmo mecanismo dos dados curados do ARCHITECTURE §9.2).

**Elevadores:**
- o par aparece em `known-conflicts.toml` ou o último travamento citou um dos `.mixins.json` envolvidos → alto, com o trecho do log como evidência;
- mixin com `require`/`defaultRequire ≥ 1` no ponto disputado.

### 5.6 O que é realista na v1

| Camada | Conteúdo | Prioridade |
|---|---|---|
| **A. Inventário** | Para cada mod: quantas configs, classes de mixin e alterações; quais classes do jogo altera; se usa plugin. Mostrado nos detalhes do mod ("Altera 52 partes do jogo; 3 também alteradas por outros mods"). | **P1** (barato, útil para a IA e para o usuário curioso) |
| **B. Sobreposições com regras da §5.5** | Achados `W_MIXIN_OVERLAP` (aviso, nunca erro) em Problemas, com rebaixadores e a lista curada. **Nunca bloqueiam o teste.** | **P1** |
| **C. Ligação com o travamento** | Quando o log cita um `.mixins.json` ou uma classe de mixin, mapear para o mod (índice config → mod) e mostrar as outras alterações no mesmo alvo. | **P0** (a parte "mapear `<x>.mixins.json` para o mod" já está prevista no padrão 23 do R2 §6.3; o resto é P1) |
| **D. Tradução de nomes sem refmap** (tabelas Mojang/intermediary/SRG) | Comparar mods que usam convenções diferentes. | P2 |
| **E. Simulação da aplicação** (rodar o próprio Mixin em modo de auditoria) | O próprio Mixin tem `-Dmixin.debug.export` e `mixin.checks`; rodar um "teste a seco" num processo Java isolado daria a resposta exata. | P2 (exige Java e muito trabalho) |

**Crate recomendada:** `cafebabe` (licença 0BSD; decodifica anotações visíveis, invisíveis e de parâmetro; sem teto de versão de classe; modo rápido sem bytecode com `ParseOptions::parse_bytecode(false)`) **[verificado no protótipo]**. Alternativas e licenças na §5.7.

**Riscos e limites:** falsos positivos (mitigados pelos rebaixadores e pela linguagem: "alteram o mesmo ponto", nunca "são incompatíveis"); falsos negativos com nomes não traduzidos (camada D); configs registradas só em código (1.7.10/1.12.2); mixins em alvos de **outros mods** (não só do jogo: o Oculus altera classes do Embeddium) entram naturalmente, porque o alvo é só um nome de classe.

**Esforço:** A+C = **P**; B com rebaixadores e lista curada = **M**; D = M; E = G.

### 5.7 Ferramentas existentes e bibliotecas

**Detectores de conflito de mixin** (pesquisa do subagente, 01/10/2026):

| Ferramenta | Licença | Abordagem | Uso pelo Warden |
|---|---|---|---|
| **intermed** (jarettr, Rust, criado em 06/2026) | **MIT** | Estática: lê configs e bytecode, trabalha por "ponto de aplicação" (handler → alvo → ponto), ordena por prioridade efetiva, classifica papéis (2× `@Redirect` = conflito quase certo; `@WrapOperation` em cadeia = ok), checa `side`/ambiente, refmap por loader, e tem interpretador abstrato para cancelamentos incondicionais. Usa cafebabe + noak + zip. | **A referência mais próxima.** Estudar e, se a qualidade se confirmar, **reaproveitar como dependência ou portar com atribuição** (MIT). Avaliar maturidade (projeto de 4 meses) num spike. https://github.com/jarettr/intermed |
| **ModLint** (Basinity, Java/ASM) | MIT | Estática; marca sobreposição "potencial" só para `@Overwrite`, `@Redirect`, `@ModifyConstant`, `@ModifyArg(s)` e **ignora `@Inject`**; percorre jar-in-jar; validado contra ~700 jars sem falso "alto", segundo o autor. | Referência das regras (coincide com a §5.5). https://github.com/Basinity/ModLint |
| emendator | GPL-3.0 | Estática em Python + boot de servidor em Docker + bisseção; só Fabric. | Só leitura de ideias (GPL). https://github.com/RemiAsselin42/emendator |
| Mixin Conflict Helper (isXander) | LGPL-3.0, parado desde 2022 | Em tempo de execução: captura a exceção de injeção e mostra os culpados. | Não. |
| MixinTrace / MixinTrace Reloaded | MIT | Em tempo de execução: lista os mixins presentes no stack trace. | Candidato a mod padrão complementar (§2). |
| MixinSquared | — | Permite a um mod **cancelar ou alterar mixins de outro** (`MixinCanceller`) em código: a análise estática não prevê o efeito. | Detectar presença e rebaixar a confiança do raio-x. |

**Propriedades de depuração do próprio Mixin** (úteis para a camada E e para um "teste com diagnóstico de mixins" no menu ▾): `mixin.debug.export` (grava as classes transformadas em `.mixin.out/`), `mixin.debug.verbose`, `mixin.debug.countInjections`, `mixin.checks`, `mixin.dumpTargetOnFailure` **[código: `MixinEnvironment.java`]**.

**Leitores de `.class` em Rust** (crates.io em 01/10/2026; bench do subagente com 1.803 classes do 1.7.10, 7.762 do servidor 26.3 e mods reais, zero erros em todos):

| Crate | Licença | Última versão | Anotações | Observação |
|---|---|---|---|---|
| **cafebabe** | 0BSD | 0.9.0 (06/2025) | as três decodificadas | **recomendada**; sem teto de versão de classe (Java 6 a 25 testados) |
| noak | MIT/Apache-2.0 | 0.7.0 (07/2026) | sim, decodificação preguiçosa; também escreve | fallback (como no intermed) |
| ristretto_classfile | Apache-2.0/MIT | 0.34.0 (09/2026) | sim; lê, escreve, verifica | **rejeita versão de classe > 71 (Java 27)**: risco futuro |
| classfile-parser | MIT | 0.3.8 (10/2024) | não decodifica anotações | descartar |

Zip: `zip` (zip-rs/zip2, MIT, 8.6.0) com `default-features = false, features = ["deflate"]`; jar aninhado = ler em memória e abrir com `Cursor` (feito no protótipo). Achado do subagente: no Sodium NeoForge 26.3 o jar externo não tem nenhum mixin; as 88 classes `@Mixin` estão no jar aninhado em `META-INF/jarjar/`, então a descida recursiva é obrigatória.

## 6. Função 3b/3c — Grafo de dependências e nota de saúde

### 6.1 Grafo de dependências

**Dados (todos já previstos no modelo normalizado do R3 §4.5):** nós = itens do pack + dependências embutidas (jar-in-jar, marcadas como "dentro de X"); arestas tipadas:

| Aresta | Fonte | Visual |
|---|---|---|
| obrigatória (`depends`, `required`, `mandatory`) | jar; API | linha cheia |
| opcional / recomendada (`recommends`, `suggests`, `optional`) | jar; API | tracejada |
| incompatível (`breaks`, `incompatible`, `conflicts`, `discouraged`) | jar; API; lista curada | vermelha |
| embutida (jar-in-jar) | jar | o nó fica dentro do nó pai |
| fornece (`provides`) | jar | rótulo no nó |
| inferida (dependência não declarada, vinda da bisseção ou do log) | §3.4 | pontilhada, com "inferida" |
| mixins no mesmo ponto (risco médio/alto) | §5 | opcional, desligada por padrão |

**Visualização [inferência; bibliotecas a validar no protótipo de interface]:** Cytoscape.js (MIT) aguenta centenas de nós e tem layouts hierárquicos (dagre, MIT) e por forças (fcose, MIT); React Flow (`@xyflow/react`, MIT) é melhor para poucos nós com conteúdo rico. Para 100–400 mods, Cytoscape.js. Um grafo de 300 nós inteiro é ilegível; por isso o padrão é o **grafo focado**: um mod no centro, com o que ele exige à esquerda e quem depende dele à direita, até 2 níveis, e o grafo inteiro só como opção, com bibliotecas agrupadas.

**Perguntas que o grafo responde (e que viram texto, não só desenho):**
- "Se eu remover X, o que quebra?" (dependentes obrigatórios transitivos; o diálogo de remover da T06 já precisa disso);
- "Por que Y está no pack?" (cadeia até um mod escolhido pelo usuário: "Y é exigido por Create, que você adicionou");
- **bibliotecas órfãs**: dependência que nenhum mod usa mais (sobrou de um mod removido) → informação "pode ser removida";
- mods que mais "seguram" o pack (muitos dependentes), úteis para ordenar a bisseção.

**Esforço:** dados = já existem no diagnóstico (P0); consultas "quem depende / por que está aqui / órfãs" = **P**; desenho interativo = **M**.

### 6.2 Nota de saúde do pack

Uma nota só é útil se for **explicável** e **estável** (não muda sozinha sem motivo). Proposta: começa em 100 e cada item tira pontos, com teto por categoria, para que um único problema não esconda os outros. Tudo determinístico, sem IA.

| Categoria (teto) | Regra | Pontos |
|---|---|---|
| Problemas do pré-teste (−40) | cada erro / cada aviso / informação | −15 / −3 / 0 |
| Último teste (−25) | travou / não testado depois da última alteração / nunca testado | −20 / −5 / −10 |
| Travamentos recentes (−10) | travamentos distintos (por assinatura) nas últimas 10 sessões | −3 cada |
| Mixins (−10) | sobreposição alta não rebaixada / média | −4 / −1 |
| Manutenção (−10) | mod sem versão nova há mais de 18 meses para a versão do MC do pack, projeto arquivado, versão alpha/beta | −1 / −3 / −1 cada |
| Distribuição (−10) | mod da CurseForge com download bloqueado (jogador terá trabalho) | −2 cada |
| Desempenho (−5) | tempo de carregamento 50% acima do último teste bem-sucedido; memória configurada abaixo da sugestão da T13 | −3 / −2 |

Faixas: **90–100 Ótimo**, **75–89 Bom**, **50–74 Atenção**, **abaixo de 50 Crítico**. A tela mostra a nota, a faixa e a lista "O que tirou pontos", cada linha com link para o problema, o mod ou o teste. Itens que o usuário mandou ignorar (T14, P1) não tiram pontos, mas aparecem riscados ("ignorado por você").

Cuidados: a nota **não** é uma promessa de que o pack funciona; o texto diz "Resumo dos problemas conhecidos". As regras e os pesos ficam num arquivo de dados versionado (`data/health-score.toml`), com teste de propriedade (adicionar um erro nunca aumenta a nota). **Esforço:** **P** (é uma agregação do que o diagnóstico já produz).

## 7. Função 4 — Console inteligente, desempenho, spark automático e histórico de travamentos

### 7.1 Atribuir cada linha de log a um mod

Sinais disponíveis, do mais forte ao mais fraco:

| Sinal | Onde aparece | Exemplo | Fonte |
|---|---|---|---|
| Jar no frame do stack trace | Forge 1.17+ | `at pkg.Cls.m(Cls.java:99) ~[ColdSweat-2.2-b04d.jar%23197!/:2.2-b04d] {re:mixin,pl:mixin:APP:mixins.oculus.json:…}` (inclui os mixins aplicados na classe) | issues reais citadas pela pesquisa **[verificado]** |
| Módulo no frame | NeoForge 1.20.5+ | `at TRANSFORMER/epicfight_dd@21.17.0.1/net…` (modid@versão) | idem |
| Nome do *handler* de mixin | Fabric (fork FabricMC do Mixin) | `handler$<uid>$<modid>$<método>`: o modid está no nome do método | **[código]** `MethodMapper.java` |
| "Mixins in Stacktrace" | crash reports NeoForge; MixinTrace | lista de configs de mixin | **[verificado]** |
| Nome do *logger* | Forge 1.12.2 (`getLogger(modid)`), Fabric (exemplo oficial usa o modid) | `[Sodium]`, `[iris]` | **[código: MDKs]** |
| Nome do logger = nome da classe | Forge/NeoForge 1.13+ (`LogUtils.getLogger()`) | `[me.jellysquid.mods.sodium.client.SodiumClientMod]` | idem → precisa do índice de pacotes |
| Índice pacote → mod | todos | pacote da classe → jar → modid | **[verificado no experimento abaixo]** |

**Experimento [verificado]:** índice de pacotes dos 5 jars Fabric da §5.3, descendo nos jars aninhados: **844 pacotes, só 2 ambíguos** (ambos entre módulos da própria Fabric API, ex.: `net/fabricmc/fabric/api/util`), montado em 40 ms em Python. Quando um pacote é ambíguo, o Warden atribui ao jar de topo (o mod que o usuário adicionou), como o mod-bisect-tool faz com jar-in-jar. O mesmo índice resolve `NoClassDefFoundError` (§3.4) e o "mixin config → mod" do padrão 23 do R2 §6.3.

**Desenho do console [inferência]:** um seletor "Mostrar: Linha a linha · Agrupado por mod · Só problemas" (sem abas):
- **Normalização:** cada linha vira um *modelo* trocando números, coordenadas, UUIDs, hex e caminhos por marcadores (`Missing texture for {id} at {n}`), e o hash do modelo agrupa repetições: "×1.284, primeira às 18:02:16, última às 18:04:51".
- **Stack trace = um item só**, recolhido, com a primeira linha "de mod" em destaque (pula frames do Java, do jogo e do loader).
- **Agrupado por mod:** tabela mod → erros, avisos, repetições, com link para o item do pack. "Jogo/loader" e "Desconhecido" são grupos próprios.
- **Ruído conhecido:** catálogo curado de linhas que assustam mas são inofensivas (ex.: avisos de refmap ou de som ausente), marcado como "comum, geralmente inofensivo", no mesmo formato de dados do catálogo de padrões.
- Nada disso altera o log gravado: o `output.log` da sessão continua bruto (ARCHITECTURE §7.4).

Não há ferramenta aberta que deduplique logs do Minecraft; o mais próximo é o parser do Prism (GPL-3.0, leitura só para ideias), que lê os eventos XML do log4j no stdout **[código, via pesquisa]**.

**Saída estruturada pelo stdout (P2, com cuidado):** uma config log4j própria com `PatternLayout` delimitado daria logger e milissegundos em toda linha. Mas no **NeoForge, passar qualquer config de log4j faz sumir o `debug.log`** (o FML usa a config do launcher quando ela existe) **[código: `Entrypoint.java` do FancyModLoader]**. A ARCHITECTURE §7.4 já evita isso para o NeoForge; qualquer mudança aqui precisa ser por faixa e validada na matriz L-05.

**Esforço:** índice = P; console agrupado com normalização = M; ruído conhecido = P (dados).

### 7.2 Tempo de carregamento (total e por mod)

| Loader | O que existe sem instalar nada | Como o Warden obtém | Fonte |
|---|---|---|---|
| Forge 1.7.10 | `Sending event … to mod …` / `Sent event…` em `fml-client-latest.log`, com segundos | diferença entre pares | **[código]** `LoadController.java` |
| **Forge 1.12.2** | **`Bar Step: <fase> - <mod> took X.XXXs`** e `Bar Finished: … took Xs` no `debug.log` (padrão) | leitura direta: tabela pronta por mod e por fase | **[código]** `ProgressManager.java` |
| Forge 1.16–1.20.x | `Loading mod instance`/`Loaded mod instance`, `Firing event for modid X`/`Fired event…` só em `TRACE` | passar `-Dforge.logging.debugFile.level=trace` e casar pares por thread | **[código]** `FMLModContainer.java`, `log4j2.xml` |
| NeoForge | as mesmas linhas em `TRACE`; também `Mod '{}' took {} to run a deferred task.` (≥ 1 s, WARN, sem ligar nada) | idem; a construção é paralela, então casar por thread | **[código]** FancyModLoader |
| Fabric | **nada por mod** | perfil da inicialização (P2): agente do spark ou JFR, agrupando amostras por jar | **[código]** `SystemProperties.java` |
| Todos | tempo total | do início do processo até o marcador de "carregou" (S1) e até "entrou no mundo"; o ModernFix ainda imprime `Game took N seconds to start` | supervisor + log |

O que esses números medem: só o código do mod nas fases do loader. Mixins aplicados durante o carregamento de classes e a recarga de recursos (texturas, modelos) ficam de fora, e costumam ser a maior parte do tempo em packs grandes **[inferência, com aviso do próprio Loading Profiler]**. O Warden mostra "o que mais demorou" com essa ressalva.

Ligar `TRACE` aumenta muito o `debug.log`; proposta: só quando o usuário escolher **Testar com perfil de desempenho** (§7.4). O tempo total entra em todo teste e vira tendência por versão do pack ("1 min 42 s, 18 s a mais que a versão 1.4.2").

**Esforço:** total = P; por mod em 1.12.2 = P; Forge/NeoForge com TRACE = P–M; Fabric = M (P2).

### 7.3 Memória ao vivo

**RAM do processo (Windows):** `GetProcessMemoryInfo` com `PROCESS_MEMORY_COUNTERS_EX` (`WorkingSetSize`, `PrivateUsage`) pelo handle que o supervisor já tem; o crate `sysinfo` expõe os mesmos valores **[código]**. O Job Object (ARCHITECTURE §7.4) dá os picos (`PeakProcessMemoryUsed`).

**Heap da JVM sem JDK:**
- Os **JREs Temurin 8/17/21/25 não trazem `jcmd`, `jstat` nem a Attach API** (`jdk.attach` e `jdk.jcmd` ausentes em `java --list-modules`) **[verificado pela pesquisa: download e inspeção]**. JMX remoto abre uma porta que executa código e não tem cliente viável em Rust: descartado.
- **`hsperfdata`**: a JVM HotSpot publica contadores num arquivo mapeado em memória, ligado por padrão (`UsePerfData`) do Java 8 ao 25. No Windows, `%TEMP%\hsperfdata_<usuário>\<pid>` com um *file mapping* nomeado `hsperfdata_<usuário>_<pid>`; é exatamente o que o `jstat` lê **[código: `perfMemory_windows.cpp`]**.
- **Experimento (Linux, JDK 21) [verificado pela pesquisa]:** arquivo de 32 KB, cabeçalho `0xcafec0c0`, 187 contadores; um leitor de ~30 linhas em Python bateu com `jstat -gc`.
- Contadores: heap usada = soma de `sun.gc.generation.*.space.*.used`; máximo = `-Xmx` (que o Warden passou) ou o `maxCapacity` de uma geração (no G1 cada geração informa o heap inteiro: **não somar**); coletas = `sun.gc.collector.N.invocations`/`time`; também `sun.gc.metaspace.used` e `java.cls.loadedClasses`.
- Crates existentes são imaturas (`jmon-rs`) ou GPL (`hsperf`): escrever um leitor próprio (~150 linhas).
- **Riscos:** `-XX:+PerfDisableSharedMem` (presente nas "flags do Aikar", muito copiadas) e `-XX:-UsePerfData` desligam isso, então o Warden avisa ou remove essas flags dos argumentos do teste; OpenJ9 não tem `hsperfdata`; nome de usuário com acento no Windows (procurar `hsperfdata_*\<pid>` em vez de montar o nome).
- **GC log (opcional):** `-Xlog:gc:file=logs/gc.log:uptime,level,tags:filecount=3,filesize=5m` (Java 9+; caminho relativo por causa do `:` de `C:\`) dá a linha do tempo das pausas: `Pause Young (Normal) (G1 Evacuation Pause) 24M->5M(252M) 4.006ms` **[verificado pela pesquisa]**.

**Proposta de tela [inferência]:** faixa acima do console com "Memória do jogo 3,1 / 6 GB · RAM do processo 7,4 GB · Coletas: 214 (1,2 s)" e um gráfico pequeno dos últimos minutos. Achado automático `W_LOW_HEAP` quando a heap **depois das coletas** fica acima de ~90% do máximo por mais de 1 minuto ("O jogo está quase sem memória. Aumente para 8 GB em Ajustes do teste."), e `I_HEAP_OVERSIZED` quando nunca passa de 40% com mais de 8 GB configurados. Amostragem a cada 1 s.

**Esforço:** RAM do processo = P; `hsperfdata` = P–M (spike curto no Windows com Java 8, 17, 21 e 25); achados de memória = P.

### 7.4 spark automático e perfil de desempenho

| Situação | Como disparar o spark sem o usuário digitar | Viável? |
|---|---|---|
| Servidor dedicado | escrever `spark profiler start --timeout 60 --save-to-file` na entrada padrão do servidor | **Sim** **[inferência: o console do servidor lê o stdin]** |
| Cliente | não há console; o comando `/sparkc` é só pelo chat | Não diretamente |
| Cliente, alternativa 1 | **agente *standalone* do spark** (`-javaagent:spark-…-standalone-agent.jar=start`), desde 01/2025: começa a perfilar antes de tudo (inclui a inicialização); controle por um servidor SSH local; exige Java 11+ | Sim, mas complexo (cliente SSH em Rust, `russh`) |
| Cliente, alternativa 2 | **JFR** da própria JVM: `-XX:StartFlightRecording=filename=…,dumponexit=true,settings=profile` (Java 11+ e 8u262+); o `jfr.exe` vem nos JREs 17+ e o crate `jfrs` (Apache-2.0) lê o formato | Sim; agrupar amostras por jar com o índice da §7.1 |

Formato do spark: `.sparkprofile`/`.sparkheap` são **protobuf puro** (sem gzip), com `.proto` públicos em `spark-common/src/main/proto/spark/`; compilam em Rust com `prost`. `WindowStatistics` já traz TPS, MSPT, CPU, entidades e chunks por janela **[código]**. O **spark-viewer abre arquivo local no navegador** (FilePicker com arrastar e soltar; o processamento é no navegador) **[código: `FilePicker.tsx`]**, então "Abrir no visualizador do spark" não exige upload; embutir o viewer no Warden traria a GPL-3.0 para essa parte (evitar).

**Proposta por prioridade [inferência]:**
- **P1:** o Warden **lê os `.sparkprofile` que o usuário salvar** (`/sparkc profiler stop --save-to-file`) na instância de teste, mostra um resumo no resultado do teste (MSPT/TPS por janela, "mods que mais usaram CPU" agrupando os frames por jar) e oferece abrir no visualizador do spark. No servidor dedicado, **Testar com perfil de desempenho** dispara o spark sozinho pelo stdin.
- **P2:** perfil automático do **cliente** via JFR (inclui o tempo de inicialização no Fabric) e bisseção por desempenho (MSPT acima de um limite como "falhou").

### 7.5 Histórico de travamentos

As sessões já são guardadas (`instances/<id>/state/sessions/`, ARCHITECTURE §7.4). O que falta:
- **Assinatura** de cada travamento (a mesma da §3.4: regra do catálogo + tipo de exceção + primeiro frame de mod normalizado, sem números de linha nem endereços), gravada no `session.json`.
- **Versão do pack** (commit) e lista resumida de mods no momento do teste, para responder "desde quando isso acontece?" e "sumiu depois de qual versão?".
- **Lista em Problemas → Travamentos:** agrupada por assinatura ("Travou 3 vezes com a mesma causa · última em 01/10 · versões 1.4.1–1.4.3"), com o diagnóstico, a busca do culpado e as conversas com a IA ligadas àquela assinatura, e o estado ("Não voltou a acontecer desde a versão 1.4.4").
- **Retenção:** sessões sem travamento são podadas (ex.: últimas 30); sessões com travamento ficam até um limite de espaço (ex.: 500 MB por pack), com aviso antes de apagar. Tudo em dados locais, nunca no pack.

**Esforço:** **P** (assinatura + lista), aproveitando as sessões que já existem.

## 8. Onde cada função fica na estrutura aprovada

Regra seguida: **nenhuma seção nova e nenhuma aba dentro de aba** (ESTRUTURA §1). Tudo cabe nas 6 seções, no botão ▶ Testar e na tela do teste, usando seletores, painéis laterais e listas.

| Função | Onde fica | Como se chega |
|---|---|---|
| Busca do culpado (§3) | Ação, não lugar. Roda **na tela do teste** (é um "teste em várias rodadas"), com o indicador de rodadas no lugar das 5 etapas. O botão do cabeçalho vira "● Buscando o culpado: ver progresso". | Botão **Encontrar o mod culpado** no resultado "O jogo travou" (T13), em Problemas → travamento, no menu ▾ do Testar ("Encontrar o mod culpado…", para problemas sem travamento: modo assistido) e por proposta da IA. Resultado final fica no histórico de travamentos. |
| IA com ferramentas e chat (§4) | Seção **✦ Diagnóstico com IA**, que deixa de ser "formulário + resposta" e vira **conversas**: lista de conversas à esquerda (ou acima), conversa aberta à direita. A página "Resposta da IA" deixa de existir: a resposta é a própria conversa. | Igual à ESTRUTURA (M12): seção do menu e **✦ Pedir ajuda à IA** no resultado do travamento, que abre uma conversa já com aquele travamento. |
| Raio-x de mixins (§5) | **Mods → detalhes do item** (painel lateral): bloco "O que este mod altera no jogo" com a contagem e as sobreposições, e **Ver todas as alterações**, que abre a lista completa no mesmo painel. Sobreposições de risco viram achados `W_MIXIN_OVERLAP` em **Problemas**. | Pelos detalhes do mod, pelos achados e pelo resultado da busca do culpado. |
| Grafo de dependências (§6.1) | **Mods**: seletor "Ver como: Lista · Grafo" (modo de exibição, permitido pela regra 3). Nos detalhes do item, "Depende de" / "Usado por" / "Por que está no pack". | Seletor no topo de Mods. |
| Nota de saúde (§6.2) | Topo de **Problemas** ("Saúde do pack: 82 · Bom · ver o que tirou pontos") e coluna em **Meus packs**. | Sempre visível em Problemas. |
| Console inteligente (§7.1) | **Tela do teste**, no console: seletor "Mostrar: Linha a linha · Agrupado por mod · Só problemas". | Durante e depois do teste ("Ver último teste"). |
| Monitor de desempenho (§7.2–7.3) | **Tela do teste**: faixa compacta acima do console (memória do jogo, RAM do processo, tempo até carregar); no resultado, "Carregamento: 1 min 42 s · o que mais demorou". | Automático em todo teste. |
| spark automático (§7.4) | **Menu ▾ do Testar → Testar com perfil de desempenho**; o relatório aparece no resultado do teste. | Ação explícita. |
| Histórico de travamentos (§7.5) | **Problemas**: o bloco "Último travamento" vira **Travamentos** (lista das sessões que travaram, agrupadas pela assinatura: "Travou 3 vezes com a mesma causa"). | Problemas. |

**Mudanças mínimas na estrutura** (para o dono aprovar):
- **E1.** A descrição de **Problemas** passa de "O que pode impedir o jogo de abrir" para **"Saúde do pack, problemas e travamentos"**, porque a seção ganha a nota e a lista de travamentos.
- **E2.** A página **Resposta da IA** (ESTRUTURA §2.1) é substituída pela **conversa**; a descrição da seção vira "Conversar com a IA sobre um problema do pack".
- **E3.** O menu ▾ do Testar ganha **Encontrar o mod culpado…** e **Testar com perfil de desempenho**.

## 9. Dependências entre as funções e esforço

```
Diagnóstico P0 (metadados dos jars, grafo, catálogo de padrões, redação)
 ├── Índice de pacotes e de configs de mixin (P) ──┬── Console agrupado por mod (M)
 │                                                 ├── Mixin citado no log → mod (P0, P)
 │                                                 └── Raio-x de mixins (A+B: M)
 ├── Grafo: consultas (P) ── Grafo desenhado (M)
 ├── Nota de saúde (P)  ← usa achados, testes, raio-x, atualizações
 ├── Supervisor de processo P0 ──┬── Monitor de RAM/heap (P–M)
 │                               ├── Tempo de carregamento (P)
 │                               ├── Histórico de travamentos com assinatura (P)
 │                               └── Busca do culpado (M) ── + servidor dedicado (M, R5B)
 └── IA com ferramentas (G) ← fica mais forte com tudo acima (cada função vira uma ferramenta)
spark por padrão (P) ── perfil automático (M)
```

| Função | Esforço | Depende de | Risco principal |
|---|---|---|---|
| Assinatura de travamento + histórico | P | supervisor, catálogo de padrões | assinaturas instáveis entre versões (mitigar normalizando números e endereços) |
| Índice pacote → mod e config de mixin → mod | P | leitura de jars (P0) | jars aninhados que repetem pacotes (raro: 2 de 844 pacotes no experimento, §7.1) |
| Console agrupado | M | índice | linhas sem pacote nem logger identificável |
| Monitor de memória e tempo de carregamento | P–M | supervisor | ver §7.3 |
| Nota de saúde | P | achados | parecer "promessa"; mitigar com texto |
| Grafo (consultas / desenho) | P / M | diagnóstico | legibilidade com 300 nós |
| Raio-x de mixins (A+C / B / D / E) | P / M / M / G | leitura de jars | falsos positivos (§5.4) |
| Busca do culpado (cliente / + servidor) | M / +M | supervisor, grafo, índice, assinatura | tempo e marcadores por versão |
| IA com ferramentas | G | quase tudo acima | custo, privacidade, alucinação |
| spark por padrão / perfil automático | P / M | — | comandos só no cliente via chat (§7.4) |
| Mod de crash report por padrão | P | — | licença (§2.2) |

## 10. Implicações para o Warden

### 10.1 Lista priorizada

**P0 — entra na v1, junto com o diagnóstico que já é P0** (barato e é a base de tudo):
1. **Índice pacote → mod e config de mixin → mod**, montado na passagem completa do diagnóstico, com cache por hash do jar (§7.1, §5.6 camada C).
2. **Assinatura de travamento** gravada em cada sessão e **lista de Travamentos** em Problemas (§7.5).
3. **Higiene:** acrescentar `config/spark/*.sparkprofile`, `config/spark/*.sparkheap` e `config/spark/tmp*` aos "sempre ignorados na captura" (ARCHITECTURE §8.4) e ao `.packwizignore` padrão (§2.1).
4. **Itens sugeridos por faixa** no assistente Criar pack: spark e o mod de crash report escolhido pelo dono, com a config inicial do Crash Assistant (envio ao autor desligado) e a regra "ferramenta do jogador" na instância de teste (§2).
5. **RAM do processo** e **tempo total de carregamento** em todo teste (§7.2, §7.3).
6. **Consultas do grafo** (quem depende, por que está no pack, bibliotecas órfãs), que o diálogo de remover da T06 já precisa (§6.1).

**P1 — entra na v1, depois do núcleo** (as funções que o dono pediu, na ordem de valor por esforço):
7. **Nota de saúde** em Problemas e em Meus packs (§6.2).
8. **Console agrupado** (normalização, repetições, stack traces recolhidos, agrupado por mod, "Só problemas") (§7.1).
9. **Memória da JVM via `hsperfdata`** e achados de memória (§7.3), depois do spike S-R5-2.
10. **Busca do culpado** no cliente (até o menu) com modo assistido (§3); com **servidor dedicado** quando a função 7 do R5B existir.
11. **IA com ferramentas e conversas**, substituindo o pacote fixo da ARCHITECTURE §9.5 (que já era P1) (§4), depois do spike S-R5-4 e da decisão sobre consentimento.
12. **Raio-x de mixins**, camadas A e B (inventário e avisos explicáveis, nunca bloqueio) (§5.6), depois do spike S-R5-1.
13. **Tempo por mod** em Forge 1.12.2 e Forge/NeoForge com `TRACE` (§7.2) e leitura dos `.sparkprofile` salvos + spark automático no servidor dedicado (§7.4).
14. **Grafo desenhado** em Mods ("Ver como: Grafo") (§6.1).

**P2 — depois da v1:**
15. Raio-x camadas D (tradução de nomes sem refmap) e E (aplicação a seco do Mixin).
16. Perfil automático do cliente (JFR ou agente do spark) e tempo por mod no Fabric.
17. Bisseção de configs/scripts e bisseção por desempenho.
18. Saída estruturada do log4j por faixa (§7.1).

### 10.2 Spikes recomendados antes de implementar

| Spike | Pergunta | Esforço |
|---|---|---|
| **S-R5-1** intermed | O intermed (Rust, MIT) pode ser usado como biblioteca ou portado para o raio-x? Qualidade em 3 packs reais (Fabric, Forge, NeoForge) e taxa de falsos positivos com os rebaixadores da §5.5. | P |
| **S-R5-2** memória sem JDK no Windows | Ler `hsperfdata` pelo *file mapping* nomeado com Java 8, 17, 21 e 25 no Windows, inclusive com usuário acentuado, e comparar com o `jstat` de um JDK. | P |
| **S-R5-3** marcadores da busca | Para cada faixa da matriz L-05: marcadores de sucesso no cliente, `--quickPlaySingleplayer` (1.20+), `--server/--port` com servidor local (< 1.20), `-Dfml.queryResult=confirm` (1.12.2), `-Dfabric.noGui`, e tempo médio de rodada num pack de ~150 mods. | M |
| **S-R5-4** laço de ferramentas do Gemini | Laço `generateContent` sem estado com 10–20 ferramentas, *thought signatures*, chamadas paralelas, `VALIDATED` e resposta final em JSON com conferência de evidência, contra um servidor simulado e depois com uma chave real. | M |

### 10.3 Mudanças em documentos existentes (se o dono aprovar)

- **SPEC T13/T14:** acrescentar busca do culpado, console agrupado, faixa de memória, perfil de desempenho, travamentos em Problemas, nota de saúde; IA como conversa com ferramentas.
- **ARCHITECTURE §9.5:** `responseSchema` → `responseFormat`; laço de ferramentas; histórico de conversas. **§8.4:** arquivos do spark. **§7.4:** leitura de `hsperfdata`; aviso sobre `-XX:+PerfDisableSharedMem`.
- **ADR-0014:** consentimento por conversa com transparência por resultado (se aprovado).
- **ESTRUTURA:** E1–E3 (§8).
- **R2 §6.4:** a licença do Crash Assistant tem cláusula de não concorrência (mais restritiva do que "All Rights Reserved").

## 11. Questões para o dono

1. **Mod de crash report padrão.** O mais completo é o **Crash Assistant** (funciona em todas as versões e loaders que o Warden suporta e mostra uma janela clara para o jogador quando o jogo trava). Mas a licença dele proíbe usar o código dele para fazer algo "concorrente", e ele manda alguns dados ao autor por padrão (o Warden desligaria isso). Prefere:
   - **(a) Crash Assistant** (recomendado), com o envio de dados desligado; o Warden só o inclui no pack e nunca usa nada do código dele;
   - **(b) nenhum mod de crash**, só um complemento leve (MixinTrace) que melhora o relatório de erro;
   - **(c) Not Enough Crashes**, que é livre, mas não funciona em 1.7.10 nem em 1.12.2 e às vezes deixa o jogo num estado estranho depois de um erro.
2. **Durante os seus testes no Warden**, o Crash Assistant abriria a janela dele por cima do resultado do Warden. Posso deixá-lo **fora dos testes normais** e **dentro do "Testar como o jogador recebe"**? (Recomendado: sim.)
3. **IA conversando com acesso ao pack.** Hoje a regra é mostrar o texto exato e perguntar **antes de cada envio**. Numa conversa, a IA consulta várias coisas seguidas (um trecho do log, depois uma config). Prefere:
   - **(a) perguntar uma vez no começo da conversa**, mostrando o que a IA poderá consultar, e deixar cada coisa enviada visível no chat (recomendado);
   - **(b) perguntar a cada consulta** (mais seguro, mas a conversa fica lenta e cheia de cliques).
   Em qualquer caso, nada muda no pack sem você clicar em **Aplicar**.
4. **A IA pode procurar problemas parecidos nas páginas dos mods no GitHub?** Isso envia ao GitHub o nome do mod e algumas palavras do erro (não o log). Recomendado: sim.
5. **Servidor de teste.** Para a busca do culpado funcionar em versões antigas e para problemas dentro do mundo, o Warden precisa abrir um servidor do Minecraft no seu computador, e isso exige aceitar a EULA da Mojang. Tudo bem o Warden pedir esse aceite **uma vez** (com o link da EULA) e lembrar dele?
6. **Tempo da busca do culpado.** Num pack grande ela pode levar de 20 a 40 minutos, e o computador fica abrindo e fechando o jogo sozinho (não dá para testar outra coisa ao mesmo tempo). Tudo bem, desde que dê para pausar e cancelar a qualquer momento?
7. **Nota de saúde.** Quer ver a nota (ex.: "82 · Bom") também na lista **Meus packs**, ou só dentro do pack, em Problemas?
8. **O spark entra marcado por padrão** em todo pack novo (pode desmarcar no assistente)? Nas versões 1.7.10 e 1.12.2 só existe uma versão antiga dele.

## 12. Fontes

Fontes primárias consultadas (cada seção cita as suas; aqui o consolidado). Pesquisa feita com apoio de quatro subagentes de pesquisa em 01/10/2026; os fatos marcados **[verificado]** por eles vêm de chamadas reais às APIs ou de download e inspeção de arquivos.

**Diagnóstico e mods de crash**
- https://mcdoctor.ai/ (/how-it-works, /prices, /faq, /privacy, /terms, /changelog, /reviews) · https://api.modrinth.com/v2/project/mcdoctor.ai
- https://github.com/KostromDan/Crash-Assistant · https://raw.githubusercontent.com/KostromDan/Crash-Assistant/1.19-1.20.1/LICENSE.md · https://modrinth.com/mod/crash-assistant
- https://github.com/natanfudge/Not-Enough-Crashes (issues #41, #138, #191, #194, #217) · https://modrinth.com/mod/notenoughcrashes
- https://github.com/aternosorg/codex-minecraft · https://github.com/aternosorg/mclogs · https://api.mclo.gs/1/limits
- https://modrinth.com/mod/mixintrace-reloaded · https://github.com/comp500/mixintrace · https://modrinth.com/mod/neruina
- https://github.com/FabricMC/fabric-loader/blob/master/src/main/java/net/fabricmc/loader/impl/gui/FabricGuiEntry.java

**spark e desempenho**
- https://github.com/lucko/spark (`SparkPlatform.java`, `SamplerModule.java`, `RuntimeConfiguration.java`, `BackgroundSamplerManager.java`, `spark-common/src/main/proto/spark/`) · https://github.com/lucko/spark-viewer (`FilePicker.tsx`)
- https://spark.lucko.me/docs/Command-Usage · https://spark.lucko.me/docs/Configuration · https://spark.lucko.me/docs/Standalone-Agent · https://spark.lucko.me/download
- https://api.modrinth.com/v2/project/spark/version · CurseForge mod 361579
- https://github.com/MinecraftForge/MinecraftForge/blob/1.12.x/src/main/java/net/minecraftforge/fml/common/ProgressManager.java · https://github.com/MinecraftForge/MinecraftForge/blob/1.12.x/src/main/java/net/minecraftforge/fml/common/StartupQuery.java
- https://github.com/neoforged/FancyModLoader · https://github.com/FabricMC/Mixin/blob/main/src/main/java/org/spongepowered/asm/mixin/transformer/MethodMapper.java
- https://github.com/openjdk/jdk21u/blob/master/src/hotspot/os/windows/perfMemory_windows.cpp · https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex2 · https://github.com/GuillaumeGomez/sysinfo · https://crates.io/crates/jfrs

**Bisseção**
- https://github.com/Qendolin/mod-bisect-tool · https://github.com/skycatminepokie/FabricBinarySearchTool · https://github.com/PrismLauncher/PrismLauncher/pull/5855 · https://github.com/PrismLauncher/PrismLauncher/issues/1924 · https://github.com/anonymous123-code/quilt-bisect · https://github.com/RemiAsselin42/emendator · https://github.com/lonestill/scope-launcher
- https://www.st.cs.uni-saarland.de/papers/tse2002/ · https://www.debuggingbook.org/html/DeltaDebugger.html · https://git-scm.com/docs/git-bisect
- https://minecraft.wiki/w/Quick_Play · https://minecraft.wiki/w/Java_Edition_client_command_line_arguments · https://minecraft.wiki/w/Server.properties · https://aka.ms/MinecraftEULA · https://github.com/itzg/docker-minecraft-server

**Mixins**
- https://github.com/SpongePowered/Mixin (`Mixin.java`, `MixinApplicatorStandard.java`, `RedirectInjector.java`, `ModifyConstantInjector.java`, `InjectionInfo.java`, `MixinConfig.java`, `MixinEnvironment.java`)
- https://github.com/LlamaLad7/MixinExtras/wiki · https://github.com/Bawnorton/MixinSquared
- https://wiki.fabricmc.net/documentation:fabric_mod_json_spec · https://github.com/neoforged/JarJar · https://github.com/CleanroomMC/MixinBooter · https://github.com/LegacyModdingMC/UniMixins
- https://github.com/jarettr/intermed · https://github.com/Basinity/ModLint · https://github.com/isXander/MixinConflictHelper
- https://crates.io/crates/cafebabe · https://crates.io/crates/noak · https://crates.io/crates/ristretto_classfile · https://crates.io/crates/classfile-parser · https://crates.io/crates/zip
- https://www.minecraft.net/en-us/article/removing-obfuscation-in-java-edition · https://fabricmc.net/2025/10/31/obfuscation.html

**IA e APIs**
- https://ai.google.dev/gemini-api/docs/function-calling · https://ai.google.dev/gemini-api/docs/generate-content/function-calling · https://ai.google.dev/gemini-api/docs/generate-content/thought-signatures · https://ai.google.dev/gemini-api/docs/generate-content/gemini-3 · https://ai.google.dev/gemini-api/docs/generate-content/structured-output · https://ai.google.dev/gemini-api/docs/generate-content/tool-combination · https://ai.google.dev/gemini-api/docs/migrate-to-interactions
- https://ai.google.dev/api/generate-content · https://ai.google.dev/api/caching · https://ai.google.dev/api/models · https://ai.google.dev/api/tokens
- https://ai.google.dev/gemini-api/docs/models · https://ai.google.dev/gemini-api/docs/deprecations · https://ai.google.dev/gemini-api/docs/pricing · https://ai.google.dev/gemini-api/docs/rate-limits · https://ai.google.dev/gemini-api/terms · https://ai.google.dev/gemini-api/docs/prompting-strategies
- https://docs.modrinth.com/api/ · https://docs.curseforge.com/rest-api/ · https://docs.github.com/en/rest/search/search · https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api · https://docs.github.com/en/rest/releases/releases
- https://genai.owasp.org/llmrisk/llm01-prompt-injection/

**Experimentos próprios** (fora do repositório): `~/.local/share/warden-r5/mixin-xray/` (protótipo Rust), `~/.local/share/warden-r5/xray-*.txt` (saídas), `~/.local/share/warden-r5/jars/` (14 jars do Modrinth).
