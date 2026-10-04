# S-R5-1 — intermed como base do raio-x de mixins (spike)

> Tarefa S-R5-1 do projeto Warden (onda 0). Data: 2026-10-04. Branch: `Kriticales/spike-s-r5-1-intermed`.
> Código descartável em `spikes/s-r5-1/` (ver o README de lá para reproduzir); fixtures para a D-10 em
> `spikes/s-r5-1/fixtures/`.
>
> Convenção: **[verificado]** = executei e vi o resultado; **[código]** = li no código-fonte do projeto
> citado; **[inferência]** = conclusão minha, não testada.

## 1. Resumo e recomendação

**Recomendação: implementação própria com `cafebabe`, partindo do protótipo da R5A (refeito e validado
neste spike) e acrescentando os refinamentos da §6. O intermed não entra como dependência nem é portado;
fica só como referência de ideias, sem copiar código.**

Os números que decidem (3 packs reais, 250 a 313 mods cada, mesmos jars para as duas ferramentas):

| | Protótipo R5A (refeito) | intermed `7d90f2e` |
|---|---|---|
| Tempo, sem cache | **0,3–0,8 s** (8 threads); 1,7–2,6 s (1 thread) | 6,0–12,4 s (biblioteca); 3,0–6,1 s só a fase de mixins dentro do `doctor` |
| Tempo, com cache | **0,16–0,36 s** | 4,5–9,8 s (o cache quase não ajuda) |
| Pico de memória | **20–62 MB** | 1,3–2,1 GB (biblioteca); 1,6–2,8 GB (`doctor`) |
| Desce em jars aninhados | sim | **não** (0 classes vindas de aninhados) |
| Avisos mostrados ao usuário | 74 (modo refinado), dos quais **2 altos, ambos conflitos reais** | 53, **nenhum conflito real** |
| Conflitos reais encontrados (de 3 conhecidos) | **3** | **0** |

- O intermed compila e roda no Windows **[verificado]**, mas: não desce em jars aninhados (perde a Fabric
  API inteira, o Porting Lib e a Forgified Fabric API, 7 a 20 % das classes de mixin dos packs); a função
  pública de varredura não recebe o loader e lê configs de mixin de outros loaders; trata dois
  `@WrapOperation` no mesmo ponto como "redirects-same-call", o que gera a maioria dos avisos falsos; é
  lento e pesado demais para a meta da D-10 (300 mods em menos de 3 s sem cache e 300 ms com cache) **[verificado]**.
- O projeto é alfa: 25 commits, 1 autor, 5 estrelas, nenhuma issue ou PR em 4 meses, fora do crates.io,
  CI só em Ubuntu, e 1 dos 249 testes do crate de mixins falha no Windows **[verificado]**.
- O protótipo da R5A, refeito com o mesmo desenho, bate com os números publicados na R5A (mesmos 2 altos e
  5 médios no conjunto Fabric dela) e já cumpre a meta de tempo **[verificado]**. As regras da §5.5 acertam
  nos **altos que sobram depois dos rebaixadores** (2 de 2 reais), mas os **médios** são ruído: 72 avisos
  médios para 1 conflito real (§5). A D-10 deve mostrar só o alto como aviso em Problemas (§9).

## 2. Ambiente e método

| Item | Valor |
|---|---|
| Máquina | Windows 11 Pro 10.0.26300, 16 threads lógicas, Kaspersky ativo (sem interferência observada) |
| Toolchain | Rust 1.98.1 `x86_64-pc-windows-msvc`; Python 3.14 para os scripts de apoio |
| intermed | `https://github.com/jarettr/intermed`, commit `7d90f2e557a2eb6fe954fad51d8454da913c5252` (2026-09-30, `0.2.0-alpha`) |
| Protótipo | `spikes/s-r5-1/harness` (`cafebabe` 0.9.0, `zip` 8.6.0 só com `deflate`, `serde_json`, `toml` 0.9) |
| Pastas `target` | `C:\wt\s-r5-1` (intermed) e `C:\wt\s-r5-1h` (harness), caminhos curtos (QUALITY §13.3) |

Método:

1. Clonei o intermed, compilei `intermed-mixin-intel` e a CLI em release (3 min 12 s) e rodei os testes do crate.
2. Baixei 3 modpacks públicos do Modrinth pela API (sem chave), com sha1 conferido (§4).
3. Refiz o protótipo da R5A (o original ficou no WSL, que não existe mais) a partir da descrição da R5A §5.3–§5.5
   e o validei no mesmo conjunto Fabric da R5A (§3.3).
4. Rodei, nos mesmos jars: o protótipo (regras da R5A como estão escritas e um modo "refinado"), o intermed como
   biblioteca (`scan_mods_dir`) e o intermed completo (`intermed doctor --mixin-risk`, que é o que ele mostra ao usuário).
5. Julguei à mão cada caso de risco alto e cada aviso do intermed, lendo o bytecode, os descritores e as configs
   dos próprios jars (§5). Não abri o Minecraft (a tarefa dispensa); por isso "real" e "falso" são
   **[inferência]** apoiada em evidência dos jars, salvo quando indicado.

Comandos principais (PowerShell 7; `$sp` é a pasta de trabalho no scratchpad):

```powershell
$env:CARGO_TARGET_DIR = 'C:\wt\s-r5-1'; cargo build --release -p intermed-mixin-intel -p intermed-cli   # no clone do intermed
C:\wt\s-r5-1\release\intermed.exe doctor "$sp\packs\<pack>\mods" --mixin-risk --exit-zero --quiet `
  --pack-manifest "<.mrpack>" --cache-dir "$sp\intermed-cache" --json "$sp\out\doctor-<pack>-frio.json" --profile "<...>.profile.json"
$env:CARGO_TARGET_DIR = 'C:\wt\s-r5-1h'; cargo build --release --manifest-path spikes\s-r5-1\harness\Cargo.toml
C:\wt\s-r5-1h\release\s-r5-1-harness.exe proto <loader> "$sp\packs\<pack>\mods" "$sp\out\proto-<pack>.json" --threads 8 --config-dir "$sp\packs\<pack>\config"
C:\wt\s-r5-1h\release\s-r5-1-harness.exe intermed "$sp\packs\<pack>\mods" "$sp\out\intermed-<pack>.json"
python spikes\s-r5-1\tools\comparar.py "$sp\out"
```

Todas as medições de tempo foram feitas com os jars já no cache de arquivos do Windows (lidos antes pelo
download e pelas rodadas anteriores); o tempo com disco frio não foi medido. Memória = pico do *working set*
(`GetProcessMemoryInfo` no harness; leitura periódica de `PeakWorkingSet64` para a CLI do intermed).

## 3. O intermed

### 3.1 Licença e maturidade

| Item | Valor |
|---|---|
| Licença | MIT (`LICENSE`, "Copyright (c) 2026 jarettr"; `license = "MIT"` em todo o workspace) **[verificado]** |
| Idade | primeiro commit em 2026-06-11; repositório público desde 2026-06-23 **[verificado]** |
| Histórico | 25 commits, quase todos "Prepare 0.1.x-alpha release" ou "Implement ... phases 0-5"; 16 do autor `jarettr` e o resto de "InterMed Builder"; 11 releases `0.1.0-alpha` a `0.2.0-alpha` **[verificado: `git log`, API do GitHub]** |
| Comunidade | 5 estrelas, 0 forks, 0 issues e 0 PRs (abertas ou fechadas), 1 contribuidor **[verificado: API do GitHub]** |
| Tamanho | ~150 mil linhas de Rust em 23 crates; `intermed-mixin-intel` sozinho tem 28.948 linhas **[verificado]** |
| Publicação | não está no crates.io (só dependência git) **[verificado]** |
| CI | só `ubuntu-latest` (`.github/workflows/ci.yml`) **[código]** |
| Testes | `cargo test --release -p intermed-mixin-intel`: 248 passam, **1 falha no Windows** (`rejected_official_jar_preserves_observed_namespace_without_coverage` monta um nome de arquivo com o nome da thread, que tem `::`; erro `InvalidFilename`, os 123) **[verificado]** |
| Autoavaliação | `docs/PROJECT_STATUS.md`: "alpha", formatos "may still change before 1.0"; mede 101 packs, "Maximum per-run peak RSS 3.09 GiB" **[código]** |

O volume de código em poucos commits grandes, por um autor só, indica geração em massa (provavelmente com IA)
**[inferência]**: é muito código para alguém manter, sem histórico de revisão nem usuários que relatem problemas.

### 3.2 API pública e encaixe como biblioteca

- Entrada pública da varredura: `scan_mods_dir(dir)` e `scan_mods_dir_with_cache(dir, Option<&JarCache>)` →
  `MixinScan` (30 campos: configs, classes, sites, composições, arestas de conflito, clusters, recursos,
  segurança...). O módulo `scan` é privado; a versão que recebe o loader
  (`scan_mods_dir_filtered_with_target_environment`) **não é exportada** **[código: `lib.rs`, `scan.rs`]**.
- Sem o loader, `discover_mixin_configs` junta as configs de todos os descritores do jar **[código: `scan.rs` 948–1030]**.
  Resultado nos packs: no Fabric, leu as configs Forge do PacketFixer (99 classes), do BadOptimizations (24) e do
  Collective (5), que o Fabric nunca carrega **[verificado]**. O caminho certo (com loader) só existe dentro do
  pipeline `doctor` (`Collector` + armazém de fatos do `intermed-doctor-core`).
- Dependências: o crate de mixins puxa 66 crates (inclui `chrono`, `tempfile`, `rayon`, `zip` 2.4 e `toml` 0.8,
  versões diferentes das que o Warden usa: `zip` 8 e `toml` 0.9) **[verificado: `cargo tree`]**.
- Compila como biblioteca no Windows, ligada ao harness por dependência git fixada, sem avisos **[verificado]**.

### 3.3 Cobertura

- **Não desce em jars aninhados.** Não há código que abra `META-INF/jars/` ou `META-INF/jarjar/` no crate
  de mixins **[código: busca em `intermed-mixin-intel/src`]**, e nas 3 varreduras 0 das classes vieram de jar
  aninhado **[verificado]**. No conjunto Fabric da R5A ele achou 596 classes contra 895 do protótipo: faltam as
  299 classes da Fabric API, todas em módulos aninhados **[verificado]**. A R5A §5.7 já tinha mostrado que no
  Sodium NeoForge 26.3 todos os mixins estão no jar aninhado.
- Classes de mixin achadas por um e não pelo outro (nomes únicos):

| Pack | Protótipo | intermed | Só no protótipo | Só no intermed |
|---|---|---|---|---|
| Fabric 1.20.1 | 5.418 | 4.442 | 1.108 (aninhados: Porting Lib do Farmer's Delight, CoroUtil, Graves/Polymer, Fabric API...) | 132 (configs de outro loader) |
| Forge 1.20.1 | 3.852 | 3.744 | 284 (Forgified Fabric API aninhada, Connector Extras...) | 176 (configs de outro loader; `continuity`) |
| NeoForge 1.21.1 | 2.790 | 2.561 | 325 (Forgified Fabric API, Connector...) | 96 (configs de outro loader) |

## 4. Os packs

Modpacks públicos e conhecidos do Modrinth, da mesma família (Better MC) nos três loaders, todos com os mods de
otimização que mais disputam mixins (Sodium/Xenon, Iris/Oculus, Lithium/Radium, ModernFix, FerriteCore) e,
no Forge e no NeoForge, mods Fabric via Sinytra Connector. Baixados com `tools/baixar_pack.py` em 2026-10-04,
sha1 de cada arquivo conferido; a lista dos jars com URL e sha1 está em `fixtures/jars-<pack>.json`.

| Pack | Versão | Loader | Jars em `mods/` | `.mrpack` (sha1) |
|---|---|---|---|---|
| [Better MC [FABRIC] - BMC2](https://modrinth.com/modpack/better-mc-fabric-bmc2/version/M5BnAIQy) | v40 | Fabric Loader 0.19.3, MC 1.20.1 | 313 | `326da0b2…c269` |
| [Better MC [FORGE] - BMC4](https://modrinth.com/modpack/better-mc-forge-bmc4/version/bdRIjRgs) | v59 | Forge 47.4.20, MC 1.20.1 | 309 | `5f7db7a9…bce4` |
| [Better MC [NEOFORGE] BMC5](https://modrinth.com/modpack/better-mc-neoforge-bmc5/version/Mo8ro6Ra) | v31 | NeoForge 21.1.176, MC 1.21.1 | 250 | `6c588ff5…feb9c` |

Validação do protótipo refeito: o conjunto Fabric da R5A (Sodium 0.5.13, Lithium 0.11.4, Iris 1.7.6, Fabric API
0.92.12, ModernFix 5.25.2, baixados do Modrinth com sha1 conferido) deu 63 jars aninhados, 80 configs,
895 classes, 1.402 alterações, 150 `@Overwrite`, 52 `@ModifyVariable`, 38 `@ModifyArg`, 15 `@WrapOperation`,
209 `@Accessor` e **2 altos + 5 médios**, contra 63, 80, 899, 1.410, 150, 52, 38, 15, 209 e 2 + 5 na R5A §5.3
**[verificado]**. A diferença de 4 classes (0,4 %) não muda nenhum resultado.

## 5. Resultado nos 3 packs

### 5.1 Volume

| Pack | Jars de topo / aninhados | Configs | Classes de mixin | Alterações | Classes do jogo alteradas por 2+ mods | Métodos alterados por 2+ mods |
|---|---|---|---|---|---|---|
| Fabric 1.20.1 | 313 / 594 | 834 | 6.984 | 9.263 | 690 | 927 |
| Forge 1.20.1 | 309 / 316 | 566 | 4.119 | 5.586 | 521 | 497 |
| NeoForge 1.21.1 | 250 / 191 | 467 | 3.189 | 4.166 | 400 | 355 |

(Contagens do protótipo, com cópias repetidas de aninhados; no cruzamento cada modid conta uma vez. Erros de
leitura de classe: 0. Avisos de config: 3 no Forge e 3 no NeoForge, todos configs declaradas num jar e
guardadas em outro, como `citadel.mixins.json` no manifesto do Alex's Mobs, e uma config com JSON inválido no
Carry On **[verificado]**.)

### 5.2 Classificação pelas regras da R5A §5.5

Categorias dos avisos (`W_MIXIN_OVERLAP` = alto ou médio depois dos rebaixadores, como a ARCHITECTURE §9.6 define hoje):

| Categoria | R5A como escrita (Fabric / Forge / NeoForge) | Refinado (§6) |
|---|---|---|
| A. Dois substitutos no mesmo ponto (`@Overwrite`×2, `@Redirect`/`@ModifyConstant`×2) | 13 / 9 / 9 | 12 / 9 / 9 |
| B. `@Overwrite` + injeção no corpo | 0 / 0 / 1 | 0 / 0 / 1 |
| C. `@Redirect` + `@Inject` no mesmo `@At` | 10 / 4 / 1 | 9 / 2 / 1 |
| D. `@Redirect` + injetor do MixinExtras no mesmo `@At` | 10 / 0 / 2 | 10 / 0 / 2 |
| E. `@ModifyVariable`/`@ModifyArg(s)` no mesmo `@At` | 23 / 6 / 2 | 14 / 3 / 2 |
| **Avisos** | **56 / 19 / 15 = 90** | **45 / 14 / 15 = 74** |
| ...dos quais altos | 1 / 1 / 0 | 1 / 1 / 0 |

### 5.3 Os 31 casos de risco alto, um a um

Cada linha conferida nos jars (dump das alterações, textos do plugin, descritores e configs do pack).
Detalhes e o nível esperado em `fixtures/veredictos.json`.

| Mods | Ponto | Vezes | Veredicto | Evidência |
|---|---|---|---|---|
| sodium-extra × sodiumextras | `SodiumGameOptionPages#general`, 2× `@Redirect` `OptionGroup$Builder.add` | 1 (Fabric) | **real** | prioridade 1000 nos dois, nenhum cita o outro; o Mixin pula um (`@Redirect conflict. Skipping`, **[código: `RedirectInjector`]**) e as opções de um dos mods somem **[inferência]** |
| debugify × fabric-item-api-v1 (Fabric API) | `class_636#method_2922`, 2× `@Redirect` | 1 (Fabric) | **real**, impacto pequeno | correção MC-176559 do Debugify contra a API `allowContinuingBlockBreaking` da Fabric API; um é pulado **[inferência]** |
| badoptimizations × citadel | `LevelRenderer#m_202423_`, 2× `@Redirect` (700 e 1000) | 1 (Forge) | **real**, só desempenho | o de prioridade maior vence; a otimização do BadOptimizations some **[código: `RedirectInjector`; inferência no efeito]** |
| lithium/radium × redirected | redstone e pistões, 2× `@Redirect` `Direction.values()` | 4+4+4 | falso na prática | os dois trocam `values()` por vetor em cache; o plugin do Redirected só cita `nicerportals` **[verificado]**; um é pulado, mesmo efeito |
| modernfix × redirected | `BlockStateBase$Cache#<init>`, 2× `@Redirect` `values()` | 1+1+1 | falso na prática | mesmo efeito |
| stfu × charm | `class_634#method_44075`, 2× `@Redirect` | 1 (Fabric) | falso na prática | as duas opções desligam o mesmo aviso |
| modernfix × noisium | `MaterialRuleList#calculate`, 2× `@Overwrite` (100 e 1000) | 1 (NeoForge) | falso na prática | mesma otimização; o ModernFix escolheu prioridade 100 para perder |
| lithium/radium × modernfix | `Biome#getTemperature`, 2× `@Overwrite` | 1+1+1 | falso | o plugin/config do ModernFix cita `lithium`/`radium` (R5A §5.4) **[verificado: textos]** |
| ferritecore × lithium/radium | `PalettedContainer` acquire/release, 2× `@Overwrite` | 2+2+2 | falso | config `ferritecore.threaddetec.mixin.json` com plugin que só liga com `useSmallThreadingDetector = true`; as configs dos 3 packs deixam `false` **[verificado: configs e textos]** |
| iris × sodium | `ModelPart#render`, 2× `@Overwrite` | 1 (Fabric) | falso | pacote `compat/sodium` ligado por `isModLoaded` (R5A §5.4) |
| krypton × lithium | `class_3898#method_18713`, 2× `@Overwrite` | 1 (Fabric) | falso | o `fabric.mod.json` do Krypton desliga `lithium:options` `mixin.world.player_chunk_tick` **[verificado]** |

Resumo: **3 reais em 31** (10 %), 17 falsos na prática e 11 falsos. Depois dos rebaixadores da R5A ficam
**2 altos, os dois reais** (sodium-extra × sodiumextras e badoptimizations × citadel); o terceiro real
(debugify × Fabric API) cai para médio pelo rebaixador "declara dependência", pelo motivo errado (o Debugify
depende da Fabric API em geral, não para conviver com essa correção).

### 5.4 Os médios (categorias B a E)

- **C (`@Redirect` + `@Inject`), 12 no modo refinado: falsos prováveis.** O Mixin aplica em fases para cada
  classe-alvo: primeiro junta todos os mixins (`MAIN`, inclusive os `@Overwrite`), depois **procura os pontos de
  injeção de todos** (`INJECT_PREPARE`) e só então aplica (`INJECT_APPLY`) **[código: `MixinApplicatorStandard`
  do SpongePowered/Mixin, `4053421`]**. O `@Inject` acha a chamada antes de o `@Redirect` trocá-la, e os dois
  entram **[inferência a partir do código]**.
- **D (`@Redirect` + MixinExtras), 12: dúvida.** A documentação do MixinExtras garante o encadeamento de
  `@WrapOperation` e `@ModifyExpressionValue` entre si, mas não diz o que acontece com um `@Redirect` rival
  **[verificado: wiki do MixinExtras]**; depende da ordem.
- **E (`@ModifyVariable`/`@ModifyArg(s)`), 19 no refinado: dúvida, quase sempre inofensivo.** Os dois alteram o
  mesmo valor em cadeia (ex.: Zeta × Blueprint na colocação de blocos de estruturas; Easy Anvils × Easy Magic ×
  Visual Workbench, do mesmo autor). Sem a refinação do seletor eram 31: 12 mexiam em variáveis ou argumentos
  diferentes.
- **B, 1:** Sodium substitui `MultiPartBakedModel#getQuads` e o FerriteCore troca um `Map.get` dentro dele (o mesmo
  caso da R5A §5.3). O pack é usado por muita gente e abre, então o ponto existe ou o `require` é 0
  **[inferência]**; dúvida.

Totais dos avisos (modo refinado, 74): **3 reais, 39 falsos (27 da categoria A rebaixados para médio + 12 da C) e
32 em dúvida**. Pelas regras como escritas (90): 3 reais, 43 falsos, 44 em dúvida.

### 5.5 O que o intermed mostra

`intermed doctor --mixin-risk` gera milhares de notas e, como aviso (`warn`), 13 (Fabric), 13 (Forge) e 27
(NeoForge) achados de mixin **[verificado]**. Julgados com o dump dos injetores de cada ponto:

| Pack | Avisos | Reais | Falsos | Dúvida | Principais causas dos falsos |
|---|---|---|---|---|---|
| Fabric | 13 | 0 | 11 | 2 | 6 pares de `@WrapOperation`/`@WrapWithCondition` encadeados (FancyMenu × Konkrete × Aether, Fast Item Frames × Fast Paintings...); 4 pares que o protótipo também rebaixa (Iris × Sodium, Lithium × ModernFix, Krypton × Lithium, FerriteCore × Lithium) |
| Forge | 13 | 0 | 12 | 1 | `@WrapOperation` encadeados (Farmer's Delight × Smarter Farmers, Galosphere × Quark, Particle Effects × Subtle Effects); 2 "pode não aplicar" falsos; "unique-member-conflict" entre mods do mesmo autor ou addon (EMF × ETF, AeroBlender × TerraBlender) |
| NeoForge | 27 | 0 | 25 | 2 | 10 pares de mods de estrutura (MES, MNS, MVS, Repurposed Structures, YUNG's API) com `@WrapOperation` encadeado em `Codec.intRange`; 5 "pode não aplicar" falsos |

- A causa principal está no código: o analisador agrupa `Redirect` **e** `WrapOperation` na mesma lista de
  "redirects" e emite `RedirectsSameCall` para qualquer par de mods nela **[código: `analyzer.rs` 706–745]**,
  ao contrário do que diz o próprio `composition.rs` ("two `@WrapOperation`s ... are a legal chain").
- Os "Mixin may not apply" (`remap = false` com nome *intermediary* no Forge/NeoForge) são falsos: os mods listam
  os dois nomes no mesmo seletor (ex.: `lambda$static$1` e `method_28886`; `m_289842_`, `method_29015` e
  `lambda$levelSummaryReader$5` no World Play Time) e um deles resolve **[verificado: dump]**.
- **Nenhum dos 3 conflitos reais aparece** nos avisos do intermed: o debugify × Fabric API está num jar aninhado
  que ele não lê; sodium-extra × sodiumextras e badoptimizations × citadel não viram aviso **[verificado]**.

### 5.6 Comparação direta

| | Protótipo, regras R5A | Protótipo refinado | intermed `doctor` |
|---|---|---|---|
| Avisos | 90 | 74 | 53 |
| Altos | 2 (2 reais) | 2 (2 reais) | sem distinção (todos `warn`) |
| Reais entre os avisos (de 3 conhecidos) | 3 | 3 | 0 |
| Falsos entre os decididos | 43 de 46 (93 %) | 39 de 42 (93 %) | 48 de 48 (100 %) |
| Falsos entre os **altos** | 0 de 2 | 0 de 2 | — |

"Conhecidos" = a união do que as duas ferramentas acharam; nenhuma das duas garante achar tudo (nomes não
comparáveis entre mods Fabric via Connector e mods Forge, §6).

## 6. Refinamentos que o spike mostrou necessários

Implementados no modo "refinado" do protótipo e medidos (§5.2):

1. **Opções desligadas por outros mods e pelo pack.** XaeroPlus, FerriteCore, Krypton e Supplementaries
   desligam opções do Lithium/Canary no próprio descritor (`"lithium:options": {"mixin.chunk.palette": false}` no
   `fabric.mod.json`; `["lithium:options"]` no `mods.toml`) **[verificado]**; os packs trazem
   `config/lithium.properties`, `modernfix-mixins.properties` e `sodium-mixins.properties`. O nome da opção sai do
   pacote do mixin (`.../mixin/world/player_chunk_tick/X` → `mixin.world.player_chunk_tick`). Removeu 13 classes de
   mixin no Fabric, 1 no Forge e 1 no NeoForge, inclusive o Krypton × Lithium.
2. **Seletor do `@ModifyVariable`/`@ModifyArg`.** O ponto passa a incluir `ordinal`/`index`/`name`/`argsOnly` e o
   tipo do parâmetro (`@ModifyArgs` = todos). Tirou 12 médios falsos.
3. **Rebaixador "o plugin ou a config do mod cita o outro mod"** (textos `CONSTANT_String` do plugin e das classes
   `*Config*`/`*Plugin*`/`*Compat*` do jar). Pegou ModernFix × Lithium/Radium e Radium × FerriteCore.
4. **Cópia aninhada repetida:** fica a de versão maior; no empate, a do jar que traz mais aninhados (o agregador,
   como a Fabric API). Sem isso, módulos da Fabric API embutidos no CoroUtil apareciam como "coroutil".

Medidos mas não implementados (dados ou regras para a D-10):

5. **Configs próprias de cada mod de otimização** que ligam/desligam mixins com nomes que não seguem o pacote
   (FerriteCore: `useSmallThreadingDetector` → `ferritecore.threaddetec.mixin.json`). Resolveria os 6 falsos do
   FerriteCore. Exige dado curado por mod.
6. **Lista curada** (`mixin-known-compatible.toml`) com os falsos "na prática": Lithium/Radium/Canary × Redirected,
   ModernFix × Redirected, ModernFix × Noisium, STFU × Charm.
7. **"Declara dependência" não serve para bibliotecas** (Fabric API, Architectury, Forgified Fabric API...): rebaixou
   um conflito real. Restringir a `recommends`/`suggests` ou a dependência que não seja biblioteca.
8. **Mods Fabric via Sinytra Connector** (Forge e NeoForge) usam nomes *intermediary*, e os mods nativos usam SRG ou
   Mojang: o raio-x não compara os dois sem tabelas de nomes (camada D da R5A, P2). Hoje é falso negativo
   silencioso; a interface deve dizer "não comparado".

## 7. Desempenho

| Medida (16 threads lógicas; jars no cache do Windows) | Fabric (313) | Forge (309) | NeoForge (250) |
|---|---|---|---|
| Protótipo, 1 thread, sem cache | 2.639 ms · 43 MB | 1.862 ms · 40 MB | 1.693 ms · 38 MB |
| Protótipo, 8 threads, sem cache | **826 ms** · 62 MB | **491 ms** · 56 MB | **315 ms** · 51 MB |
| Protótipo, 8 threads, cache por jar (JSON) | **358 ms** · 33 MB | **193 ms** · 25 MB | **162 ms** · 20 MB |
| Cruzamento (dentro dos totais acima) | 26–30 ms | 15–24 ms | 11–20 ms |
| Tamanho do cache do protótipo | 8,8 MB | 6,3 MB | 4,9 MB |
| intermed biblioteca, sem cache | 8.335 ms · 1.444 MB | 12.442 ms · 2.125 MB | 5.987 ms · 1.280 MB |
| intermed biblioteca, cache frio / morno | 6.877 / 6.269 ms | 9.753 / 9.760 ms | 4.701 / 4.517 ms |
| intermed `mixin-map` (CLI) | 13,2 s · 1.234 MB | 12,0 s · 1.917 MB | 7,4 s · 1.103 MB |
| intermed `doctor --mixin-risk`, frio / morno | 21,5 / 15,2 s · 2,0 GB | 26,8 / 18,5 s · 2,8 GB | 31,4 / 10,4 s · 1,7 GB |
| ...só a fase de mixins no `doctor`, frio / morno | 4.834 / 4.347 ms | 6.093 / 4.702 ms | 2.990 / 2.779 ms |

Todos **[verificado]**. O cache em disco do `doctor` chegou a 501 MB para os 3 packs. O protótipo já cumpre a
meta da D-10 sem cache (< 3 s) com folga; com cache fica entre 160 e 360 ms, perto dos 300 ms: um cache num
arquivo único e binário (em vez de um JSON por jar) deve bastar **[inferência]**.

## 8. Recomendação final e justificativa

**Implementação própria com `cafebabe`** (opção 3), a partir do protótipo refeito, que já está validado contra a
R5A e cumpre a meta de tempo.

- **Dependência (opção 1): não.** Não desce em jars aninhados (perda de 7 a 20 % das classes, inclusive a Fabric API
  inteira); a API pública ignora o loader; 10 a 50 vezes mais lento e 20 a 80 vezes mais memória que o protótipo,
  fora da meta da D-10 mesmo com cache; 0 de 3 conflitos reais e 48 de 48 avisos decididos falsos; alfa de um autor
  só, fora do crates.io, CI só em Linux, teste quebrado no Windows; traz 66 crates e versões de `zip`/`toml`
  diferentes das do Warden.
- **Porte com atribuição (opção 2): não.** O que o Warden precisa (configs, refmap, anotações, aninhados, regras da
  §5.5) é pequeno e já existe no protótipo (cerca de 1.200 linhas). As partes grandes do intermed (fluxo de dados,
  efeitos de handler, clusters, arestas por tipo) são justamente as que geram os falsos e o custo; portar só os
  pedaços bons não compensa e herdaria um modelo de dados enorme.
- **O que vale como ideia** (sem copiar código, então sem atribuição obrigatória; se algum trecho for copiado, entra
  no `THIRD_PARTY.md`): tratar `@WrapWithCondition` como "pode cancelar a chamada"; ordenar os participantes de
  cada ponto por prioridade efetiva; conferir alvos que não existem (com o cuidado dos seletores com vários nomes).

## 9. Implicações para a tarefa D-10

Itens para o orquestrador aplicar na D-10 do ROADMAP (e, onde indicado, na ARCHITECTURE §9.6 e na R5A):

1. **Entregas:** "implementação própria com `cafebabe`, a partir do protótipo do S-R5-1 (`spikes/s-r5-1/harness/src/proto.rs`,
   na branch do spike)"; tirar as alternativas "intermed como dependência" e "porte com atribuição". O intermed não
   entra no `THIRD_PARTY.md`.
2. **`W_MIXIN_OVERLAP` só para risco alto depois dos rebaixadores.** Os médios ficam no raio-x do mod ("Ver todas as
   alterações") e na ferramenta da IA, sem aviso em Problemas e sem tirar ponto da nota de saúde. Nos 3 packs isso
   dá 2 avisos (os 2 reais) em vez de 74. Ajustar a ARCHITECTURE §9.6 ("alto ou médio" → "alto") e a linha "Mixins"
   da tabela da nota de saúde (SPEC T14 / R5A §6.2).
3. **Regras e rebaixadores (acrescentar aos da R5A §5.5):** opções desligadas por `lithium:options`/`canary:options`/
   `radium:options` nos descritores e pelas configs do pack (`config/<mod>.properties`; a mixin some do cruzamento);
   seletor de `@ModifyVariable`/`@ModifyArg` no ponto; rebaixador "o plugin/config cita o outro mod"; "declara
   dependência" não vale quando o outro é biblioteca; `@Redirect` + `@Inject` no mesmo ponto passa a baixo.
4. **Dados curados iniciais:** `mixin-known-compatible.toml` com Lithium/Radium/Canary × Redirected, ModernFix ×
   Redirected, ModernFix × Noisium e STFU × Charm; um arquivo de "opções que ligam mixins" para FerriteCore
   (`useSmallThreadingDetector` → `threaddetec`) e afins, com o mesmo teste de esquema dos outros dados.
5. **Leitura:** só as configs do loader do pack (o `fabric.mod.json` vale no Forge/NeoForge apenas para jar sem
   descritor do loader, isto é, mod Fabric via Connector); cópias aninhadas repetidas: versão maior, empate para o
   agregador; config declarada e ausente no jar é ignorada sem erro (fica em outro jar).
6. **Connector:** marcar no raio-x os mods Fabric em pack Forge/NeoForge como "nomes não comparados com mods nativos"
   até existir a camada D.
7. **Desempenho:** leitura paralela por jar (8 threads já dão 0,3–0,8 s para 250–313 mods) e cache num arquivo único
   binário em `cache/xray/` (o JSON por jar deu 160–360 ms). Critério de aceite mantido; medir também com disco frio.
8. **Corpus de teste:** `spikes/s-r5-1/fixtures/` (configs e descritores reais com origem, `resultado-<pack>.json` com
   os números do protótipo e `veredictos.json` com os 31 casos altos julgados; a origem está em `ORIGEM.json`, que a
   D-10 converte no `FIXTURES.md` pedido pela QUALITY §4). Os jars não são versionados: o teste com o corpus real
   baixa os jars pela lista `jars-<pack>.json` (URL e sha1) e fica marcado `#[ignore = "rede"]`, rodando com
   `cargo xtask test-network` (QUALITY §4); os testes comuns usam jars sintéticos e as configs
   reais das fixtures. Os casos que a D-10 deve reproduzir:
   sodium-extra × sodiumextras e badoptimizations × citadel = alto; Krypton × Lithium = some; Lithium × ModernFix,
   Iris × Sodium = baixo.

## 10. O que ficou sem verificar

- Nenhum conflito foi confirmado abrindo o jogo; "real" e "falso" são julgamentos sobre os jars (§2).
- O efeito de `@Redirect` + injetor do MixinExtras no mesmo ponto (categoria D) não foi resolvido.
- Tempo com disco frio (jars fora do cache do Windows) não foi medido.
- Forge 1.12.2/1.7.10 (MixinBooter/UniMixins, configs registradas em código) ficou fora: a tarefa pedia só os 3 packs.
- O código de opção do Radium/Canary para `lithium.properties` (o pack Forge traz `config/lithium.properties` com o
  Radium instalado) não foi conferido; o protótipo só aplica o arquivo à família de mesmo nome.

## 11. Achados fora do escopo

- `THIRD_PARTY.md` citado no AGENTS.md e no prompt da tarefa ainda não existe na `main`.
- A R5A §5.3 cita o protótipo em `~/.local/share/warden-r5/` (WSL), que não existe mais; o protótipo refeito fica em
  `spikes/s-r5-1/harness` (branch do spike).
- Os 3 packs trazem Sinytra Connector; a busca do culpado (D-12) e o console agrupado (D-08) vão encontrar mods Fabric
  em packs Forge/NeoForge com nomes *intermediary* nos stack traces.
