# S-R5-3 — Marcadores e mecanismos da busca do culpado (spike)

> Tarefa S-R5-3 do Warden (onda 0). Data: 2026-10-04. Branch: `Kriticales/spike-s-r5-3-marcadores`.
> Código descartável em `spikes/s-r5-3/` nessa branch (ver o README de lá); golden logs em
> `spikes/s-r5-3/fixtures/`.
>
> Convenção: **[verificado]** = executei e vi o resultado; **[código]** = li no código-fonte;
> **[inferência]** = conclusão minha, não testada.

## 1. Resumo

- **A busca do culpado funciona de ponta a ponta no Windows** **[verificado]**: num pack real de
  154 mods (Create: Complete 2.0.0, NeoForge 1.21.1) com um conflito de 2 mods plantado, o
  algoritmo da R5A achou o par certo em **20 rodadas e 26 min 55 s**, com confirmação (só o par
  falha; o pack sem o culpado passa).
- **Marcadores levantados para 20 combinações** (Forge 1.7.10 a 26.2, NeoForge 1.20.1 a 26.2, Fabric
  1.16.5 a 26.3, Quilt 1.20.1, vanilla 26.3), todos chegando ao menu e ao mundo no Windows com
  janela real (§3). Golden logs em `spikes/s-r5-3/fixtures/`.
- **Mecanismos** (§4): Quick Play (`--quickPlaySingleplayer` e `--quickPlayPath`) funciona em
  1.20.1, 1.21.1, 26.2 e 26.3, mas a **tela de boas-vindas de acessibilidade bloqueia o Quick Play
  em 1.21+** até ser fechada; `onboardAccessibility:false` no `options.txt` resolve.
  `--server/--port` com servidor local funciona de 1.7.10 a 1.19.2 e é ignorado no 1.20.1.
  `-Dfml.queryResult=confirm` responde sozinho à pergunta do Forge 1.12.2. `-Dfabric.noGui=true`
  faz o Fabric sair sozinho em vez de abrir a janela de erro. A tela de avisos do Forge 1.20.1
  também bloqueia o Quick Play (`showLoadWarnings = false`).
- **Cenários de falha reais** (§5): 23 situações em Fabric, NeoForge, Forge 1.20.1 e Forge 1.12.2, com
  o sinal no log, o tempo e se o jogo fecha. Os pontos que mudam a D-12: no NeoForge/Forge a tela de
  erro de carregamento **dispara os marcadores de "pronto"**; a falha ao entrar no mundo **não gera
  crash** (`Couldn't place player in world`); travamento e falta de memória no NeoForge deixam o
  jogo vivo e calado; duplicados e mods de outro loader passam em silêncio nos loaders modernos.
- **Tempo de rodada** (§6): ~69 s até o mundo com o pack inteiro (cache frio 74 s + ~35 s de
  instalação do NeoForge); ~90 s por rodada com 20 s de estabilidade. Prefixos de metade do pack
  custam **quase o mesmo** que o pack inteiro. A estimativa da R5A estava otimista; a revisada está
  na §8 (rodada ≈ W + 20 s; 1 culpado = ⌈log₂ n⌉ + 4 rodadas; par = ⌈log₂ n⌉ + ⌈log₂ p⌉ + 5).
- **Linux não verificado** (sem WSL); o que a CI precisa conferir está na §10.
- **Validade:** algumas rodadas tiveram clique humano na tela de boas-vindas; foram identificadas
  pelo `options.txt` e descartadas (§9).

## 2. Ambiente e método

| Item | Valor |
|---|---|
| Máquina | Windows 11 Pro 10.0.26300 (nativo, sem WSL), Intel Core i7-10700K (16 threads), 31,9 GB de RAM, AMD Radeon RX 7600, SSD SATA |
| Rust | 1.98.1 MSVC; `CARGO_TARGET_DIR=C:\wt\s-r5-3\target` (caminho curto, QUALITY §13.3) |
| Motor | `portablemc` 5.0.5 (o adaptador `engine.rs` do S1, com a variante `Quilt` acrescentada) |
| Javas | baixados pelo motor (runtimes da Mojang): 8u51 (`jre-legacy`), 17.0.15 (`java-runtime-gamma`), 21.0.7 (`java-runtime-delta`) e o Java 25 dos 26.x; um JDK Temurin 21.0.12 baixado para a pasta de dados só para compilar o mod de teste |
| Áudio | `ALSOFT_DRIVERS=null` (o OpenAL abre "No Output") |
| Dados | `C:\wt\s-r5-3\data` (fora do repositório; apagado no fim, §11) |
| Jogador | perfil offline `WardenTest`, sem conta e sem login |

O protótipo `bisect-proto` (Rust) instala com o portablemc, monta a linha de comando e é dono do
processo: põe o jogo num **Job Object** do Windows com `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, lê
stdout e stderr em duas threads, registra a **primeira ocorrência** de cada marcador do catálogo
(`markers.rs`) com o tempo desde o início do processo, e encerra o jogo com `TerminateJobObject`
assim que o objetivo (menu, mundo ou saída do processo) é atingido e o período de estabilidade
termina. Com `--server-dir`, sobe antes um servidor dedicado local (com a entrada padrão aberta),
espera o `Done (`, abre o cliente e encerra o servidor com `stop`. Os roteiros em Python
(`scripts/`) preparam servidores, mundos e mods e rodam a matriz, os cenários de falha e a busca
do culpado. Comandos exatos no README de `spikes/s-r5-3/`.

Os tempos são medidos do *spawn* do processo Java até a linha do log (relógio do supervisor, não o
carimbo do log). Toda execução abriu a janela real do jogo na tela do dono (GPU real, sem servidor
gráfico virtual).

## 3. Matriz: cliente pronto e "entrou no mundo" por faixa

Todas as linhas abaixo são **[verificado]** no Windows desta máquina, com o loader sem mods (só o
que o próprio loader traz). "Pronto" = todos os marcadores de menu vistos; "mundo" = primeira linha
de entrada no mundo (no cliente com Quick Play; no servidor local nas versões anteriores a 1.20).
Tempos em segundos desde o *spawn* do cliente; quando houve mais de uma rodada válida, a faixa
mostra o menor e o maior valor. As rodadas contaminadas por clique humano (§9) estão fora.

| Combinação | Java | Como entra no mundo | Pronto | Mundo | Linha que identifica o loader |
|---|---|---|---|---|---|
| Forge 1.7.10-10.13.4.1614 | 8u51 | servidor Forge local + `--server/--port` | 13,7 | 15,9 | `Forge Mod Loader version 7.99.40.1614 for Minecraft 1.7.10 loading` |
| Forge 1.12.2-14.23.5.2860 | 8u51 | idem | 16,0–18,7 | 18,4–21,5 | `Forge Mod Loader version 14.23.5.2860 for Minecraft 1.12.2 loading` |
| Forge 1.16.5-36.2.34 | 8u51 | idem | 15,9–17,6 | 20,0–21,2 | `Launching target 'fmlclient'`, `Forge mod loading, version 36.2.34` |
| Fabric 1.16.5 (loader 0.19.5) | 8u51 | servidor Fabric local + `--server/--port` | 11,4–13,4 | 12,1 | `Loading Minecraft 1.16.5 with Fabric Loader 0.19.5` |
| Forge 1.18.2-40.3.0 | 17.0.15 | servidor Forge local | 34,3 | 38,3 | `Launching target 'forgeclient'`, `Forge mod loading, version 40.3.0` |
| Fabric 1.18.2 | 17.0.15 | servidor Fabric local | 25,3 | 25,4 | `Loading Minecraft 1.18.2 with Fabric Loader 0.19.5` |
| Forge 1.19.2-43.5.0 | 17.0.15 | servidor Forge local | 28,3 | 32,5 | `Launching target 'forgeclient'`, `Forge mod loading, version 43.5.0` |
| Fabric 1.19.2 | 17.0.15 | servidor Fabric local | 20,0–27,5 | 24,1–31,0 | `Loading Minecraft 1.19.2 with Fabric Loader 0.19.5` (ver a queda nativa em §5.3) |
| Forge 1.20.1-47.4.10 | 17.0.15 | Quick Play | 20,1–29,9 | 28,2–42,5 | `Launching target 'forgeclient'`, `Forge mod loading, version 47.4.10` |
| NeoForge 1.20.1-47.1.106 | 17.0.15 | Quick Play | 25,2–37,0 | 34,1–48,0 | `NeoForge mod loading, version 47.1.106` |
| Fabric 1.20.1 | 17.0.15 | Quick Play | 14,3–23,6 | 23,8–34,2 | `Loading Minecraft 1.20.1 with Fabric Loader 0.19.5` |
| Quilt 1.20.1 (loader 0.24.0) | 17.0.15 | Quick Play | 16,2–30,6 | 28,1–41,1 | `Loading Minecraft 1.20.1 with Quilt Loader 0.24.0` |
| Forge 1.21.1-52.1.0 | 21.0.7 | Quick Play + `onboardAccessibility:false` | 19,3 | 24,5 | `Forge mod loading, version 52.1.0` (sem `Launching target`) |
| NeoForge 21.1.252 (1.21.1) | 21.0.7 | Quick Play + `onboardAccessibility:false` | 17,9–24,2 | 26,2–32,5 | `Launching target 'forgeclient'`, `NeoForge mod loading, version 21.1.252` |
| Fabric 1.21.1 | 21.0.7 | Quick Play + `onboardAccessibility:false` | 10,6–11,3 | 15,3–16,6 | `Loading Minecraft 1.21.1 with Fabric Loader 0.19.5` |
| Forge 26.2-65.1.0 | 25.0.1 | Quick Play + `onboardAccessibility:false` | 19,0 | 27,9 | `Forge mod loading, version 65.1.0` |
| NeoForge 26.2.0.88 | 25.0.1 | Quick Play + `onboardAccessibility:false` | 18,4 | 27,0 | `NeoForge mod loading, version 26.2.0.88` |
| Fabric 26.2 | 25.0.1 | Quick Play + `onboardAccessibility:false` | 12,4 | 21,4 | `Loading Minecraft 26.2 with Fabric Loader 0.19.5` |
| Vanilla 26.3 | 25.0.1 | Quick Play + `onboardAccessibility:false` | 15,0–15,3 | 22,2–24,1 | (nenhuma) |
| Fabric 26.3 | 25.0.1 | Quick Play + `onboardAccessibility:false` | 11,3–12,0 | 17,2–18,1 | `Loading Minecraft 26.3 with Fabric Loader 0.19.5` |

Golden logs de cada linha em `spikes/s-r5-3/fixtures/matriz/<combinação>-cliente.log` (e
`-servidor.log` para as faixas com servidor local). Os servidores dedicados de teste subiram em
6–78 s (o primeiro arranque do servidor Fabric baixa o jar do jogo).

### 3.1 Marcadores de "cliente pronto"

| Faixa | Todos precisam aparecer | Observações **[verificado]** |
|---|---|---|
| 1.7.10 Forge | `Forge Mod Loader has successfully loaded`, o atlas **definitivo** `Created: <L>x<A> textures/blocks-atlas` (o que **não** é `16x16`) e `Sound engine started` | O FML cria um atlas vazio `16x16 textures/blocks-atlas` antes de carregar os mods; casar só "blocks-atlas" dá pronto cedo demais. O `Sound engine started` aparece duas vezes (antes e depois da recarga de recursos). O `---- Minecraft Crash Report ----` do `SplashProgress` ("THIS IS NOT A ERROR") continua aparecendo e não é falha (golden `mecanismos/forge-1.7.10-falso-crash-report-splash.log`). |
| 1.8–1.12.2 Forge | `Forge Mod Loader has successfully loaded`, `Created: <L>x<A> textures-atlas`, `Sound engine started` | No 1.12.2 o atlas vem **antes** do `successfully loaded`; o pronto é o último dos três. |
| 1.13–1.19.4 (Forge e Fabric) | `Created: <L>x<A>x<M> minecraft:textures/atlas/blocks.png-atlas`, `Sound engine started` | O tamanho do atlas varia (1024x512, 1024x1024, 2048x2048 no 26.x): casar só o sufixo `blocks.png-atlas`. |
| 1.20+ (todos) e 26.x | idem | No NeoForge e no Forge modernos o log vira texto puro depois que o FML assume (o início é XML do log4j); o parser precisa dos dois formatos, como o S1 já tinha visto. |

**Falso positivo grave [verificado]:** no NeoForge (21.1.252) e no Forge (1.20.1), quando o
carregamento dos mods falha (dependência faltando, versão errada etc.), o jogo **continua** e
mostra a tela de erro de carregamento, e os marcadores de pronto (`Sound engine started` e o atlas)
**aparecem mesmo assim**. Nos cenários `neo-dep-faltando`, `neo-dep-versao`, `neo-outro-loader` e
`forge-dep-faltando` o protótipo declarou "pronto". O Warden precisa tratar como falha, **com
prioridade sobre o pronto**, as linhas `Error during pre-loading phase` (FATAL),
`Missing or unsupported mandatory dependencies` e `Cowardly refusing to send event ... to a broken mod state` (§5).

### 3.2 Marcadores de "entrou no mundo"

| Onde | Marcador | Faixas |
|---|---|---|
| Cliente (servidor integrado, Quick Play) | `<jogador>[local:E:<hex>] logged in with entity id <n> at (x, y, z)` e, logo depois, `<jogador> joined the game` | 1.20.1, 1.21.1, 26.2, 26.3, em todos os loaders **[verificado]** |
| Arquivo do `--quickPlayPath` | JSON gravado ~0,3–1 s **depois** do `logged in` | 1.20+ **[verificado]** (§4.1) |
| Servidor dedicado local | `<jogador>[/127.0.0.1:<porta>] logged in with entity id ...` e `<jogador> joined the game` no log **do servidor** | 1.7.10, 1.12.2, 1.16.5, 1.18.2, 1.19.2 (Forge e Fabric) **[verificado]** |
| Cliente conectado a servidor (sinal secundário) | `Connecting to 127.0.0.1, <porta>`; Forge 1.7.10/1.12.2: `Injecting existing block and item data` / `registry data into this client instance` + `Holder lookups applied`; Forge 1.16.5+: `Connected to a modded server.` | **[verificado]** No cliente Fabric pré-1.20 não há linha de entrada; só o servidor confirma. |
| Servidor pronto | `Done (<s>s)! For help, type "help"` (1.7.10/1.12.2 terminam com `or "?"`; no 1.7.10/1.12.2 com Java 8u51 o decimal saiu com vírgula: `Done (3,611s)!`) | todas **[verificado]** |

`Preparing spawn area` e `Preparing start region` ainda existem em 1.20.1, 1.21.1 e no servidor
26.x que usei, mas não servem de marcador de entrada (aparecem antes). Não procurei em 26.3 cliente.

## 4. Mecanismos

### 4.1 Quick Play (1.20+)

- `--quickPlaySingleplayer <pasta do mundo>` abre direto o mundo copiado para `saves/<pasta>` em
  1.20.1 (Forge, NeoForge, Fabric, Quilt), 1.21.1 (Forge, NeoForge, Fabric), 26.2 (Forge,
  NeoForge, Fabric) e 26.3 (vanilla, Fabric) **[verificado]**. O mundo de teste foi gerado por um
  servidor dedicado vanilla da mesma versão (`level-seed=warden`) e copiado sem `session.lock`.
- `--quickPlayPath <arquivo>` (relativo à pasta do jogo) grava, depois da entrada, um JSON como
  `[{"type":"singleplayer","id":"wardentest","name":"world","lastPlayedTime":"2026-10-04T23:26:38.256697400Z","gamemode":"survival"}]`
  **[verificado]** (golden `mecanismos/quickplay-path-*.json`). É um sinal estruturado, mas chega
  0,3–1 s depois do `logged in`: o log continua sendo o sinal principal.
- **Os dois argumentos juntos** funcionam; o portablemc só põe um modo de Quick Play por vez
  (`set_quick_play`) **[código]**, então o Warden acrescenta os argumentos ao `LaunchPlan` ele mesmo.
- **Mundo inexistente** (`--quickPlaySingleplayer naoexiste`): o jogo fica parado depois do menu e
  **não escreve nada no log** (60 s observados) **[verificado]**. O Warden confere a pasta antes de
  abrir e trata "nenhuma linha depois do pronto, sem `Starting integrated minecraft server`" como
  erro de preparo.
- `--quickPlayMultiplayer 127.0.0.1:<porta>` entrou no servidor vanilla local em 13,1 s (Fabric
  1.20.1) **[verificado]**; `--server/--port` no 1.20.1 é **ignorado** (o jogo fica no menu)
  **[verificado]**, como a R5A dizia.
- **Tela de boas-vindas de acessibilidade (1.21.1 e 26.x) bloqueia o Quick Play.** Em instância
  sem `options.txt`, o jogo mostra essa tela na primeira abertura e só segue para o mundo depois
  que ela é fechada. Em **todas** as rodadas em que ninguém fechou a tela, o jogo ficou parado até
  o tempo-limite (45, 240 e 300 s; NeoForge 1.21.1, Forge 1.21.1, NeoForge/Forge/Fabric 26.2,
  vanilla 26.3); com `onboardAccessibility:false` gravado no `options.txt` antes de abrir, **todas**
  entraram no mundo em tempo normal **[verificado]**. No 1.20.1 a tela não bloqueia: o Quick Play
  entrou nas 12 rodadas sem a opção, sem `options.txt` gravado **[verificado]**. Ao fechar a tela, o
  jogo grava `onboardAccessibility:false`, o que permitiu separar as rodadas em que alguém clicou
  (§9).
- **Mundo salvo com um mod que não está mais na instância** (Comforts): NeoForge 21.1.252 entrou
  no mundo pelo Quick Play e só avisou no log (`The following mods have version differences that
  were not resolved: comforts (version 9.0.5+1.21.1 -> MISSING) ... Things may not work well.`);
  Fabric 1.20.1 entrou sem dizer nada **[verificado]**. Nenhuma tela de confirmação apareceu.
  Pela interface normal (menu "Um jogador") não testei.

### 4.2 `--server/--port` com servidor local (antes de 1.20)

Funciona em 1.7.10, 1.12.2, 1.16.5, 1.18.2 e 1.19.2, Forge e Fabric **[verificado]**: o cliente
conecta 0,6–4,2 s depois do pronto. O servidor de teste: `online-mode=false`, `server-ip=127.0.0.1`,
porta própria, `eula=true` (§2), encerrado com `stop` pela entrada padrão em 1,3–7,5 s, código 0,
sem precisar forçar **[verificado]**. O instalador do Forge (`--installServer`) funcionou com o
Java 8u51 da Mojang (1.7.10, 1.12.2, 1.16.5) e com o Java 17 (1.18.2, 1.19.2); o NeoForge 21.1.252
com o Java 21 **[verificado]**. O portablemc converte `QuickPlay::Multiplayer` em `--server/--port`
nas versões antigas (`fix_legacy_quick_play`) **[código]**; usei os argumentos diretos.

### 4.3 `-Dfml.queryResult=confirm` (Forge 1.12.2)

Mundo criado num servidor Forge 1.12.2 com o Comforts; depois o Comforts saiu. **[verificado]**

- Sem a propriedade: `Forge Mod Loader detected missing registry entries.` e o servidor **para**
  esperando no console: `Run the command /fml confirm or or /fml cancel to proceed.` /
  `Alternatively start the server with -Dfml.queryResult=confirm or -Dfml.queryResult=cancel to
  preselect the answer.` (90 s de espera sem progresso).
- Com a propriedade: `Using fml.queryResult confirm to answer the following query:` e o servidor
  sobe (`Done` em 24,8 s).

Golden: `mecanismos/forge-1.12.2-servidor-pergunta-mundo-sem-mod.log` e
`mecanismos/forge-1.12.2-servidor-queryResult-confirm.log`. No cliente 1.12.2 o mesmo
`StartupQuery` vira uma tela **[código, R5A]**, mas não há como abrir um mundo local sem clicar
nessa versão, então não testei no cliente.

### 4.4 `-Dfabric.noGui` (Fabric)

Com uma dependência faltando (Comforts sem a Fabric API), o log é o mesmo com e sem a
propriedade (`Mod resolution failed`, `Incompatible mods found!` e a lista
`Mod 'Comforts' (comforts) 6.4.0+1.20.1 requires any version of fabric-api, which is missing!`)
**[verificado]**:

- **com** `-Dfabric.noGui=true`: o processo **sai sozinho** com código 1 em 1,6–2,1 s;
- **sem** a propriedade: abre a janela de erro do Fabric e o processo fica vivo até ela ser
  fechada. Na rodada sem ninguém mexendo, ficou 40 s até o tempo-limite; em duas rodadas com o dono
  usando a máquina, saiu 6 e 23 s depois do erro, o que é compatível com a janela fechada por ele
  (§9). O Job Object mostrou **um** processo só (a janela não abriu num segundo `java`, ao
  contrário do que a R5A supunha) **[verificado]**.
- O mesmo vale para o Java errado no Fabric (`fab-java8`): sem `noGui`, o
  `UnsupportedClassVersionError` aos 9,6 s deixou o processo vivo até o tempo-limite.

### 4.5 Tela de avisos de carregamento do Forge (1.20.1)

Achado fora do previsto **[verificado]**: um aviso de carregamento (aqui
`Missing metadata in pack mod:wardenfalhas`, um jar sem `pack.mcmeta`) faz o Forge 1.20.1 abrir a
tela de avisos depois do menu, e essa tela **bloqueia o Quick Play** sem nenhuma linha específica
no log (parado até o tempo-limite em 3 rodadas). Com `showLoadWarnings = false` em
`config/forge-client.toml`, o mesmo jar entrou no mundo em 26,4 s. Golden:
`falhas/forge-avisos-bloqueiam-quickplay.log`. O NeoForge 21.1.252 com o mesmo tipo de jar não
bloqueou (os cenários `neo-entrar` e `neo-travar` chegaram ao servidor integrado).

### 4.6 Processo e encerramento no Windows

- Job Object com `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` e `TerminateJobObject`: encerramento em
  0,0–0,6 s (código 1) em todas as rodadas; quando o próprio protótipo foi interrompido no meio de
  uma série, nenhum `java` ficou para trás **[verificado]**.
- Os sinais de falha que **não** fecham o jogo (tela de erro do NeoForge/Forge, janela do Fabric
  sem `noGui`, `Couldn't place player in world`, falta de memória no NeoForge, travamento) exigem
  que o Warden encerre o processo ao ver o sinal; esperar a saída do processo não funciona.

## 5. Cenários de falha

### 5.1 Como foram provocados

Mods reais do Modrinth (Comforts, Supplementaries/Moonlight, Quark/AutoRegLib, Fabric API antiga)
para dependência, versão, loader e duplicado; e um mod de teste próprio, `wardenfalhas`
(`spikes/s-r5-3/testmod/`, compilado para Fabric, NeoForge e Forge), para as falhas de código, com
o modo escolhido por `-Dwarden.falha=...`:

- `mixin`: `@Inject` com `require = 1` num método inexistente do cliente (o que acontece com um mod
  feito para outra versão do jogo ou do mod que ele altera);
- `iniciar`: exceção no construtor do cliente;
- `entrar`: exceção no construtor do jogador no servidor (ao entrar no mundo);
- `travar`: laço infinito no mesmo ponto (o jogo para de responder sem fechar);
- `par:<classe>`: como `entrar`, mas só se a classe de outro mod existir (conflito de 2 mods; usado
  na bisseção, §7).

### 5.2 Sinal, tempo e como o Warden diferencia **[verificado]**

Tempo = do *spawn* até a primeira linha-sinal. "Fecha?" = o processo sai sozinho.

| Cenário | Loader | Sinal no log | Tempo | Fecha? | Como o Warden diferencia |
|---|---|---|---|---|---|
| Dependência faltando | Fabric 1.20.1 | `Incompatible mods found!` + `requires any version of fabric-api, which is missing!` | 8,9–10,7 | só com `noGui` | regra de dependência (catálogo), antes do menu |
| Dependência faltando | NeoForge 21.1.252 | `Missing or unsupported mandatory dependencies:` / `Mod ID: 'moonlight', Requested by: 'supplementaries', Expected range: ...` (3,7 s); depois `Error during pre-loading phase: Mod supplementaries requires moonlight 1.21-3.6.4 or above` (FATAL, 15,4 s) | 3,7 | não (tela de erro; o "pronto" aparece) | `Missing or unsupported...`/`Error during pre-loading phase` = falha, mesmo com pronto |
| Dependência faltando | Forge 1.20.1 | `Missing or unsupported mandatory dependencies:` (7,9 s); `Error during pre-loading phase` + `ModLoadingException: Mod supplementaries requires moonlight 1.20-2.16.26 or above` (35,2 s) | 7,9 | não (tela de erro; o "pronto" aparece) | idem |
| Dependência faltando | Forge 1.12.2 | `MissingModsException: Mod quark (Quark) requires [autoreglib@[1.3-32,)]` | 12,5 | não (tela de erro do FML) | `MissingModsException` |
| Versão errada da dependência | Fabric 1.20.1 | `requires version 0.92.0+1.20.1 or later of mod 'Fabric API' (fabric-api), but only the wrong version is present: 0.83.0+1.20.1!` | 9,8 | só com `noGui` | `Incompatible mods found!` |
| Versão errada da dependência | NeoForge 21.1.252 | **nenhum** aviso de versão: `1.21.1-3.6.3` passa na faixa `[1.21-3.6.4,]`; a falha vem depois como `NoClassDefFoundError: net/mehvahdjukaar/moonlight/...` e a lista `Mod loading has failed` | 19,4 | não | trata como dependência não declarada (R5A §3.4); a comparação de versões do NeoForge aceita o que não deveria |
| Mod para outra versão do jogo | Fabric 1.20.1 | `requires any version between 1.21 (inclusive) and 1.22- (exclusive) of 'Minecraft'` | 8,3 | só com `noGui` | regra de versão do jogo |
| Mod de outro loader | Fabric 1.20.1 (jar Forge) | `Found 1 non-fabric mod: - comforts-forge-...jar` (INFO) e o jogo **abre normalmente** | 9,7 | — | não é falha de abertura; a análise estática acusa antes |
| Mod de outro loader | NeoForge 21.1.252 (jar Fabric) | `Skipping jar. File mods\comforts-fabric-...jar is a Fabric mod and cannot be loaded` e a tela de erro | 4,8 | não | regra do catálogo |
| Mod de outro loader | Forge 1.20.1 (jar Fabric) | **nada**: o jar é ignorado em silêncio e o jogo abre | — | — | só a análise estática pega |
| Mod duplicado | Fabric, NeoForge, Forge 1.20.1 | **nada**: carregam uma das cópias em silêncio (NeoForge e Forge registram `Found mod file` das duas) | — | — | só a análise estática pega |
| Mod duplicado | Forge 1.12.2 | `Found a duplicate mod comforts at [...]` + `DuplicateModsFoundException` | 11,4 | não (tela de erro) | regra do catálogo |
| Mixin que quebra ao iniciar | Fabric 1.20.1 | `Mixin apply for mod wardenfalhas failed wardenfalhas.fabric.mixins.json:MixinQuebrado ... InvalidInjectionException: Critical injection failure: @Inject annotation on ... could not find any targets matching ...` | 24,4 | sim (código 1, sem crash report) | `Mixin apply ... failed` + nome do `.mixins.json` (dono do mixin) |
| Mixin que quebra ao iniciar | NeoForge 21.1.252 | a mesma linha `Mixin apply for mod ... failed`, depois `Exception in thread "main"` | 6,2 | sim | idem |
| Mixin que quebra ao iniciar | Forge 1.20.1 | `Mixin apply failed wardenfalhas.mojmap.mixins.json:MixinQuebrado -> net.minecraft.client.Minecraft: ... InvalidInjectionException` e `MixinTransformerError` | 11,3 | sim | idem (no Forge a linha não traz o id do mod; o `.mixins.json` sim) |
| Crash ao abrir | Fabric 1.20.1 / NeoForge 21.1.252 | `---- Minecraft Crash Report ----`, `Description: Initializing game`, a exceção e `#@!@# Game crashed! Crash report saved to: ...` | 22,5 / 24,2 | sim | crash report novo |
| Crash ao entrar no mundo | Fabric 1.20.1, NeoForge 21.1.252, Forge 1.20.1 | **não há crash**: `Couldn't place player in world` + a exceção, e `<perfil> (local:E:<hex>) lost connection: Invalid player data`; o cliente volta para a tela de desconexão | 30,8 / 48,3 / 26,9 | **não** | `Couldn't place player in world` sem `logged in` = falha ao entrar (assinatura = a exceção seguinte) |
| Travado sem fechar | Fabric 1.20.1, NeoForge 21.1.252, Forge 1.20.1 | **nenhum**: a última linha é `Changing simulation distance to 12` (Fabric/NeoForge) ou `Connected to a modded server.` (Forge) e depois silêncio | silêncio a partir de 30,5 / 26,3 / 39,6 | não | `Starting integrated minecraft server` sem `logged in` em N s, e log parado |
| Falta de memória | Fabric 1.20.1 (`-Xmx160M`) | `java.lang.OutOfMemoryError: Java heap space` (54,1 s) e crash report (56,8 s) | 54,1 | sim | `OutOfMemoryError` |
| Falta de memória | NeoForge 21.1.252 com Supplementaries (`-Xmx256M`) | `OutOfMemoryError: Java heap space` (43,7 s), depois `Out of memory` (78,9 s) e o log **para**; o processo continua vivo | 43,7 | não | `OutOfMemoryError` em qualquer thread = falha, encerrar na hora |
| Java errado | Fabric 1.20.1 com Java 8 | `UnsupportedClassVersionError: net/minecraft/client/main/Main has been compiled by a more recent version of the Java Runtime (class file version 61.0)` | 9,6 | só com `noGui` | regra de Java (não é mod: a rodada 1 já falha) |
| Java errado | NeoForge 21.1.252 com Java 17 | `Error occurred during initialization of boot layer` / `InvalidModuleDescriptorException: Unsupported major.minor version 65.0` | 0,4 | sim | idem |
| Java errado | Forge 1.12.2 com Java 21 | `ClassCastException: class jdk.internal.loader.ClassLoaders$AppClassLoader cannot be cast to class java.net.URLClassLoader` em `net.minecraft.launchwrapper.Launch` | 0,2 | sim | idem |

Golden logs em `spikes/s-r5-3/fixtures/falhas/<cenário>.log`. Resultados brutos de todas as
rodadas em `spikes/s-r5-3/resultados/*.jsonl` (caminhos trocados por `<DADOS>`).

Ruído comum que **não** é falha **[verificado]**: `InvalidCredentialsException: Status: 401` em
`/player/attributes` e `Failed to verify authentication` (perfil offline), `Missing sound for event`,
`Invalid registry value type detected for PerfOS counters`, `Failed to add PDH Counter`,
`Mod file ...fmlcore... is missing mods.toml file` (Forge 1.20.1), `Attempting connection with
missing mods [minecraft, mcp, FML, forge] at CLIENT` (Forge 1.7.10/1.12.2), `Incorrect key
client.experimentalForgeLightPipelineEnabled was corrected`, `Reference map ... could not be read`
e `Error loading class: ...` (mixins opcionais de compatibilidade). A assinatura por "primeira
exceção do log" pegava o 401; a assinatura por âncora (§7) resolveu.

### 5.3 Queda nativa intermitente (Fabric 1.19.2)

Na 1ª rodada da matriz, o Fabric 1.19.2 saiu com `0xc0000005` (violação de acesso) aos 16,2 s,
logo depois de `Unrecognized user type:` (criação da janela), sem crash report e sem `hs_err`. Nas
4 rodadas seguintes abriu normalmente **[verificado]**. Causa provável: driver de vídeo ou LWJGL
dessa versão **[inferência]**. Para a busca isso é uma "falha diferente": a rodada repete, não conta
como reprodução.

## 6. Tempo de uma rodada num pack de ~150 mods

Pack: **Create: Complete (by Shalz) 2.0.0**, https://modrinth.com/modpack/create-complete-by-shalz
(versão `LJD7YFQa`, publicada em 2026-09-15), NeoForge 21.1.236, Minecraft 1.21.1, **154 mods**
(354 MB), baixados do CDN do Modrinth sem chave em 20 s **[verificado]**. Inclui Create e 40
complementos, Sodium, Distant Horizons, JEI/EMI, spark, Not Enough Crashes e o Sinytra Connector
com a Forgified Fabric API (mods Fabric dentro do NeoForge). O `.mrpack` não traz configs.

Cada rodada como a busca faria: `mods/` recriada com links físicos (0,1–0,2 s), `config/` apagada,
mundo de teste copiado, Quick Play com `onboardAccessibility:false`, 20 s de estabilidade no mundo
e encerramento **[verificado]**:

| Rodada | Pronto | Entrou no mundo | Rodada inteira |
|---|---|---|---|
| Cache frio (instância nova; inclui instalar o NeoForge 21.1.236, ~35 s) | 55,1 | 74,0 | 130,1 |
| 1 | 51,0 | 69,0 | 90,9 |
| 2 | 51,6 | 69,7 | 91,5 |
| 3 | 50,4 | 68,4 | 90,5 |
| Sem mods (rodada 1 da busca) | 16,4–17,8 | 22,1–23,7 | 43,7–45,3 |

O tempo **não** é proporcional ao número de mods: na busca, prefixos de 77, 116, 135 e 145 mods
entraram no mundo em 67,0, 57,6, 65,1 e 66,7 s, quase o mesmo que o pack inteiro (69 s). O custo
fixo (Java, NeoForge, Create, Connector, Sodium) domina **[verificado]**. A R5A supunha "metade do
pack ≈ metade do tempo" **[inferência da R5A, desmentida aqui]**.

## 7. Busca do culpado de ponta a ponta

Driver `scripts/busca_culpado.py` (o algoritmo da R5A §3.2) sobre o pack da §6 com um culpado
plantado: o `wardenfalhas` em modo `par:vectorwing.farmersdelight.FarmersDelight`, que derruba a
entrada no mundo **só** se o Farmer's Delight também estiver ligado (conflito de 2 mods). Total:
155 mods. Na ordem topológica (bibliotecas primeiro, depois nome), o culpado ficou na posição 153 e
o parceiro na 101. Cada rodada: §6 (links físicos, `config/` apagada, mundo novo, Quick Play,
20 s de estabilidade). Veredito: "passou" = entrou no mundo e ficou estável; "falhou" = sinal da
§5.2 com a mesma assinatura da rodada 0; qualquer outra coisa = inconclusiva.

**Tentativa 1 (interrompida pelo driver, 4 rodadas):** o prefixo de 77 mods falhou com outra
assinatura (`Mod loading has failed`): `Mod continuity requires fabric_api any` /
`Currently, fabric_api is not installed`. O `continuity` declara `fabric_api` no
`neoforge.mods.toml` **sem `type`**, e meu leitor só contava `type = "required"`. No NeoForge, a
ausência de `type` significa obrigatória. Corrigi a regra e liguei a dependência inferida do log
(§14, item 8). É exatamente o "falhou diferente" previsto pela R5A §3.4, visto num pack real.

**Tentativa 2 (completa) [verificado]:**

| Fase | Rodadas | Resultado |
|---|---|---|
| Rodada 0 (pack inteiro) | 1 | falhou: `entrar \| java.lang.IllegalStateException: Warden: conflito de teste com vectorwing.farmersdelight.FarmersDelight (wardenfalhas)` |
| Rodada 1 (sem mods) | 1 | passou |
| Busca binária no prefixo (77, 116, 135, 145, 150, 152, 153) | 7 | último necessário: `wardenfalhas-neoforge-1.0.0.jar` (posição 153) |
| Culpado sozinho | 1 | passou, logo há parceiro |
| Busca do parceiro entre os 152 anteriores (76, 114, 95, 104, 99, 101, 100) | 7 | parceiro: `FarmersDelight-1.21.1-1.3.3.jar` (posição 101) |
| Par sozinho | 1 | falhou igual: conjunto mínimo de 2 |
| Confirmação: só o conjunto / pack sem o culpado | 2 | falhou igual / passou: **confiança alta** |
| **Total** | **20** | **26 min 55 s** (1 614 s de rodadas) |

Tempo por rodada: 88,4 s em média nas 11 que passaram com 77+ mods, 93,0 s nas 5 que falharam
com 77+ mods e 44,3 s nas 4 com 0–2 mods. Preparar a instância custou 0,1–0,2 s por rodada.
Nenhuma rodada foi inconclusiva e nenhuma dependência precisou ser inferida na tentativa 2. O
detector de entrada acusou teclado ou mouse em 17 das 20 rodadas, mas com `onboardAccessibility:false`
não há tela para fechar e os resultados bateram com o esperado.

Desperdícios que a D-12 elimina: as rodadas que falharam ficaram 20 s a mais esperando
"estabilidade" (7 rodadas, ~140 s); e a confirmação "só o conjunto" repetiu a rodada "par
sozinho" (44 s). Sem os dois, a mesma busca levaria ~24 min **[inferência, cálculo]**.

## 8. Estimativa de rodadas revisada

Com W = tempo até "entrou no mundo" com o pack inteiro (69 s aqui) e E = estabilidade (20 s):

- **Rodada** ≈ W + E + 2 s quando passa; ≈ W + 2 s quando falha (encerrando no sinal); ≈ 45 s
  para conjuntos de 0–2 mods. O tamanho do prefixo quase não muda o tempo (§6), porque o custo
  fixo do loader e das bibliotecas grandes domina.
- **Rodadas** para *n* candidatos: 1 culpado = 2 (rodadas 0 e 1) + ⌈log₂ n⌉ + 1 (culpado sozinho)
  + 1 (pack sem ele) = ⌈log₂ n⌉ + 4. Par com o parceiro na posição *p* = ⌈log₂ n⌉ + ⌈log₂ p⌉ + 5
  (o "par sozinho" já é a confirmação). Medido: n = 155, p = 101 → 8 + 7 + 5 = 20 **[verificado]**.

| Pack | W (até o mundo) | 1 culpado | Par |
|---|---|---|---|
| 155 mods, NeoForge 1.21.1 (este) | 69 s | 12 rodadas ≈ **16 min** | 20 rodadas ≈ **27 min** (medido 26 min 55 s com os desperdícios acima) |
| 50 mods (parceiro na posição ~25) | ~40 s **[inferência]** | 10 rodadas ≈ 10 min | 16 rodadas ≈ 16 min |
| 300 mods (parceiro na posição ~150) | ~150 s **[inferência]** | 13 rodadas ≈ 37 min | 22 rodadas ≈ 63 min |

A R5A §3.3 previa, para 200 mods que abrem em 3 min, "12 rodadas × 1,5–2 min ≈ 20–25 min" por
supor que um prefixo custa metade do pack. Corrigindo pelo que vi aqui (rodada ≈ pack inteiro +
20 s), o mesmo caso daria 12 × 3,3 min ≈ **40 min** para um culpado. O diálogo da D-12 deve
calcular a estimativa com o W **medido** na rodada 0 (e o tempo da rodada 1 para os conjuntos
pequenos), não com uma fração fixa. Problema intermitente multiplica pelas repetições (R5A §3.6).

## 9. Validade das medições: cliques humanos

As janelas do jogo abriram na tela do dono enquanto ele usava o computador. O dono contou que às
vezes clicou em "Continue" na tela de boas-vindas de acessibilidade. Como identifiquei e tratei:

- **Indício objetivo:** o jogo grava `onboardAccessibility:false` no `options.txt` quando essa
  tela é fechada. O roteiro apaga o `options.txt` antes de cada rodada sem a opção; terminar a
  rodada com `false` gravado prova que alguém fechou a tela (ou apertou uma tecla com a janela do
  jogo em foco). Na série "monitorada" isso aconteceu em 5 rodadas (Forge 1.21.1, Fabric 1.21.1,
  NeoForge 26.2, Fabric 26.2 e Fabric 26.3, que entraram no mundo 9–120 s depois do menu); nas
  rodadas em que o arquivo terminou sem `false`, o jogo **sempre** ficou parado até o tempo-limite.
- **Primeira série e série "limpa"** (antes de registrar o `options.txt`): rodadas sem a opção que
  entraram no mundo 11–104 s depois do menu (NeoForge 1.21.1 ×4, Fabric 1.21.1 ×2, Forge 26.2 ×2,
  Fabric 26.3), várias com um silêncio de 77–96 s no log antes de seguir. Em duas delas (NeoForge e
  Fabric 1.21.1 da série "limpa") conferi depois que o `options.txt` tinha `false`. Tratei todas
  como contaminadas.
- **Descartadas:** todas as rodadas 1.21.1 e 26.x **sem** `onboardAccessibility:false` que
  entraram no mundo. Os tempos de "mundo" dessas faixas na §3 vêm só de rodadas com a opção. Os
  tempos de "pronto" não dependem da tela (ela aparece depois do menu) e foram mantidos.
- **Também afetado:** `fab-dep-faltando` sem `noGui` (§4.4), em que a janela de erro do Fabric foi
  fechada antes do tempo-limite em duas rodadas.
- **Detector de entrada:** a partir da série "monitorada", cada rodada registra se houve teclado
  ou mouse (`GetLastInputInfo`, `scripts/entrada.py`). Houve entrada em quase todas (o dono estava
  usando a máquina), então o detector não separa um clique no jogo de um uso normal; ele fica como
  aviso. O indício que decide é o `options.txt`.
- Hipótese **[inferência]**: a janela nova do jogo pega o foco do Windows; uma tecla (Enter ou
  espaço) digitada em outro programa nesse momento fecha a tela sem intenção.
- As rodadas do pack (§6) e da busca (§7) usam `onboardAccessibility:false`, então a tela não
  aparece; um clique na janela, no máximo, pausa o jogo (`Saving and pausing game...`), o que não
  muda os marcadores.

## 10. Linux (não verificado; fica para a CI)

Não há mais Linux nesta máquina. Nada desta seção foi executado. A CI Linux (L-05,
`smoke-game.yml`, Xvfb + Mesa) teria de conferir:

- os mesmos marcadores de pronto e de mundo da §3 com Mesa/llvmpipe (o S1 já viu 1.7.10, 1.12.2,
  1.16.5, 1.20.1 Fabric e 1.21.1 NeoForge até o menu em Linux);
- Quick Play com `onboardAccessibility:false` em 1.20.1, 1.21.1 e 26.x, e o arquivo do
  `--quickPlayPath`;
- `--server/--port` com servidor local em 1.7.10, 1.12.2, 1.16.5, 1.18.2 e 1.19.2;
- `-Dfml.queryResult=confirm` no servidor Forge 1.12.2 e `-Dfabric.noGui=true` (que em Linux sem
  servidor gráfico deve se comportar igual; sem `noGui` e sem `DISPLAY`, o Fabric pode cair direto
  **[inferência]**);
- encerramento por SIGTERM/SIGKILL no lugar do Job Object (o S1 já viu 0,2–0,5 s);
- os cenários de falha da §5 usando os golden logs (os textos dos loaders não dependem do sistema
  **[inferência]**; o que muda são caminhos, o Java e os códigos de saída).

## 11. Limpeza e espaço

Tudo ficou em `C:\wt\s-r5-3` e foi **apagado** no fim **[verificado]**. Ocupação medida antes de
apagar: **7,3 GB** de dados (assets 1,4 GB; bibliotecas 978 MB; versões 667 MB; runtimes Java da
Mojang 517 MB; natives 197 MB; 9 servidores dedicados 2,0 GB; 66 instâncias 1,2 GB, incluindo o pack
de 154 mods por links físicos; JDK Temurin 21 329 MB; jars de teste 83 MB; capturas 26 MB) e
**526 MB** da pasta `target`. Nada foi instalado no Windows. Não percebi nenhum bloqueio do Kaspersky (nenhum arquivo sumiu e
nenhuma falha teve essa cara), mas não consultei o registro do antivírus. Ficaram no repositório só
o código, os golden logs (`fixtures/`, ~440 KB) e os resultados brutos (`resultados/`, ~85 KB). A
pasta de dados do S1 (`%LOCALAPPDATA%\warden-spike`, 1,1 GB) não é deste spike e não foi tocada.

## 12. O que ficou sem verificar

- Linux (§10).
- Cliente 1.12.2 com `-Dfml.queryResult=confirm` (só o servidor; o cliente 1.12.2 não abre mundo
  local sem clique).
- Forge 26.3 e NeoForge 26.3 (só havia versões beta do NeoForge e "latest" do Forge para 26.3;
  usei 26.2 para os dois loaders).
- O mundo salvo com mod ausente aberto pela interface normal (só pelo Quick Play).
- Se a tela de avisos do NeoForge (`showLoadWarnings`) também bloqueia o Quick Play: no NeoForge
  21.1.252 o mesmo tipo de aviso não bloqueou.
- Pack grande em Fabric ou Forge (só o NeoForge 1.21.1).
- Problema intermitente de verdade (a única intermitência vista foi a queda nativa da §5.3).
- Encerramento gracioso no Windows (`WM_CLOSE`): só `TerminateJobObject`.

## 13. Achados fora do escopo (para outras tarefas)

- **portablemc consulta a rede a cada abertura** quando a versão do loader Fabric não está fixada
  (`LoaderVersion::Stable`): duas rodadas falharam na *instalação* com `os error 10060` ao consultar
  `meta.fabricmc.net`. Com a versão fixada (`fabric:0.19.5`) não houve consulta **[verificado]**.
  Para a L-02/L-03: o Warden sempre passa a versão exata do loader (o `pack.toml` já tem).
- Java 8 da Mojang no Windows (8u51): o log do servidor 1.7.10 mostrou horário deslocado e o
  `Done` com vírgula decimal; o relógio do log não é confiável nessa faixa, use o do supervisor.
- O Forge 1.21.1 (52.1.0) e o 26.2 não imprimem `Launching target` (mudou o *bootstrap*); o
  NeoForge ainda imprime.
- O Quilt "estável" escolhido pelo portablemc para 1.20.1 foi o 0.24.0, embora o meta do Quilt
  liste betas mais novas.
- Nenhum `.mrpack` dos packs populares de 1.20.1/1.21.1 com ~150 mods tinha arquivos fora do CDN do
  Modrinth (`cdn.modrinth.com`) **[verificado em 7 packs]**.

## 14. Implicações para a tarefa D-12

Itens prontos para o ROADMAP (entregas e critérios da D-12):

1. **Catálogo de marcadores por faixa** (`warden-bisect`, testado com os golden logs de
   `spikes/s-r5-3/fixtures/`):
   - pronto: §3.1 (1.7.10 com o atlas **definitivo**, não `16x16`; 1.8–1.12.2 com
     `successfully loaded` + `textures-atlas` + som; 1.13+ com `blocks.png-atlas` + som, casando só o
     sufixo);
   - mundo: `logged in with entity id` no cliente (1.20+, Quick Play) ou no servidor local (antes de
     1.20); `joined the game` como confirmação; o JSON do `--quickPlayPath` como sinal secundário;
   - parser de log com XML do log4j **e** texto puro na mesma execução.
2. **Sinais de falha têm prioridade sobre o pronto.** `Error during pre-loading phase`,
   `Missing or unsupported mandatory dependencies` e `Cowardly refusing to send event ... broken mod
   state` (NeoForge/Forge modernos) marcam a rodada como falha mesmo com som e atlas vistos. Critério
   novo: golden `neo-dep-faltando.log` e `forge-dep-faltando.log` dão "falhou", nunca "passou".
3. **Lista de sinais de falha da §5.2 no veredito**, com a âncora e a assinatura: `Couldn't place
   player in world` (falha ao entrar, **sem crash e sem o processo fechar**), `Mixin apply ... failed`
   (assinatura = `.mixins.json` + alvo), `OutOfMemoryError` (em qualquer thread), crash report novo,
   `Incompatible mods found!`, `MissingModsException`, `DuplicateModsFoundException`,
   `UnsupportedClassVersionError`/`Unsupported major.minor version`/`ClassCastException ...
   URLClassLoader` (Java errado: problema de ambiente, não de mod).
4. **Assinatura = âncora + primeira exceção depois dela**, com números trocados por `#`. A "primeira
   exceção do log" pega ruído (401 do perfil offline). Lista de ruído conhecido na §5.2.
5. **Encerrar ao ver o sinal**: várias falhas não fecham o jogo (§4.6). O supervisor encerra o Job
   Object na hora e não gasta o tempo de estabilidade numa rodada que já falhou.
6. **Travado**: sem linha nova depois de `Starting integrated minecraft server` (ou de `Connected to
   a modded server`) e sem `logged in` em N s = "travou". Nos 3 loaders a última linha veio 4–12 s
   depois do pronto. Sugestão: N = 3× o intervalo entre `Starting integrated...` e `logged in` da
   rodada de controle, mínimo 60 s (no pack de 154 mods esse intervalo foi de ~15 s).
7. **Preparo da instância de cada rodada**, além dos mods:
   - `options.txt` com `onboardAccessibility:false` (obrigatório em 1.21+; sem isso o Quick Play
     para na tela de boas-vindas);
   - Forge: `config/forge-client.toml` com `showLoadWarnings = false` (a tela de avisos bloqueia o
     Quick Play no 1.20.1);
   - `-Dfabric.noGui=true` em todo Fabric/Quilt; `-Dfml.queryResult=confirm` no Forge 1.12.2 (e
     faixas com `StartupQuery`, 1.8–1.12.2 **[inferência]**);
   - a pasta do mundo existe antes de abrir (mundo inexistente = jogo parado sem log);
   - mundo de teste **gerado sem mods** (servidor vanilla da mesma versão, semente fixa) e copiado a
     cada rodada: assim nenhuma rodada abre um mundo salvo com mods que não estão ligados. Se o
     usuário escolher um mundo dele, NeoForge e Fabric ainda abrem pelo Quick Play com mods ausentes
     (§4.1).
8. **Grafo de dependências** (o que a busca real exigiu, §7):
   - no `neoforge.mods.toml`, dependência **sem `type` é obrigatória**; no `mods.toml` do Forge vale
     `mandatory`;
   - ler `fabric.mod.json` **também** em pack NeoForge (o Sinytra Connector carrega mods Fabric) e
     os ids fornecidos pelos jars embutidos (`META-INF/jarjar`, `META-INF/jars`); `fabric-api` e
     `fabric_api` são o mesmo mod (Forgified Fabric API);
   - **dependência inferida do log** ligada desde o início: `Mod X requires Y` (NeoForge/Forge) e as
     linhas `requires ... of mod '...' (id)` do Fabric viram aresta, a ordem é recalculada e a busca
     recomeça sem repetir as rodadas 0 e 1;
   - versões: a faixa declarada pode aceitar a versão errada (NeoForge comparou `1.21.1-3.6.3` como
     maior que `1.21-3.6.4`); o `NoClassDefFoundError` resultante entra pela inferência.
9. **Estimativa de rodadas e tempo no diálogo** (substitui a tabela da R5A §3.3): ver §8. O tempo
   de cada rodada é quase o do pack inteiro, não metade.
10. **Rodada 0 com repetição** já cobre a queda nativa intermitente (§5.3): "falhou diferente" é
    inconclusiva e repete.
11. **Mod duplicado, mod de outro loader e mod para outra versão** não são casos para a busca:
    Fabric/NeoForge/Forge modernos os aceitam em silêncio ou falham antes do menu com mensagem
    clara; a análise estática do pack (antes de abrir) é que deve pegar.
12. **Mods de ferramenta** (spark, Not Enough Crashes) estavam no pack e não atrapalharam os
    marcadores; a R5A manda desligá-los durante a busca, o que continua valendo (o NEC pode trocar
    o crash por uma tela) **[inferência]**.
13. **Verificação manual da D-12**: rodar com a tela do jogo sem ninguém mexendo (ou conferir o
    `options.txt` e o detector de entrada) para os tempos não serem contaminados (§9).

## 15. Implicações para a tarefa L-05

1. Matriz fechada com as versões que funcionaram aqui (§3), incluindo Forge 1.18.2-40.3.0,
   1.19.2-43.5.0, 1.21.1-52.1.0, 26.2-65.1.0, NeoForge 1.20.1-47.1.106, 21.1.252 e 26.2.0.88,
   Fabric 0.19.5 em 1.16.5–26.3 e Quilt 0.24.0 em 1.20.1. "Versão mais nova" = 26.3 (vanilla e
   Fabric); para Forge e NeoForge, 26.2 até haver versão estável de 26.3.
2. O teste de fumaça usa os mesmos marcadores de pronto da §3.1 e, para "entrou no mundo", Quick
   Play (1.20+) ou servidor local (antes), com `onboardAccessibility:false`, `showLoadWarnings =
   false` (Forge) e `-Dfabric.noGui=true` (Fabric/Quilt).
3. Fixar sempre a versão exata do loader (§13): com `fabric` sem versão, o portablemc consulta o
   meta a cada abertura e a CI fica sujeita à rede.
4. O roteiro manual no Windows (CA da L-05) não pode ter cliques na janela do jogo; registrar o
   `options.txt` final como prova.
5. Tempos de referência no Windows (GPU real) para o relatório da L-05: pronto em 11–37 s, mundo em
   12–48 s, conforme a faixa (§3). A CI Linux deve medir os seus.
6. Os golden logs desta pasta podem ser reaproveitados como fixtures dos testes de marcador da L-05
   (`crates/warden-launcher/tests/matrix/**`).
