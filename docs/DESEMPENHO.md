# Desempenho com packs grandes (A-04)

Medições das metas da [SPEC §8](SPEC.md#8-requisitos-não-funcionais) com um pack de **300 mods e 500 configs** gerado e com um **pack real** de 405 mods (tarefa A-04 do [ROADMAP](ROADMAP.md)), os gargalos encontrados e o que foi corrigido. As metas P1 da D4 (raio-x de mixins, console agrupado) e as da 1.1 são medidas pelas tarefas donas.

## Como medir

- **Benchmarks** (`criterion`, build de release): `cargo bench -p <crate>` em `warden-packwiz`, `warden-project`, `warden-configs`, `warden-jarmeta`, `warden-diagnostics` e `warden-export` (arquivos `benches/pack_grande.rs`). Cada um imprime, além do relatório do criterion, uma linha `A-04 | medida | mediana | pior | meta | ok` com 10 medidas.
- **Pack real** (jars, metafiles e configs de um `.mrpack` público do Modrinth, baixado só para a medida): veja "Medir com um pack real" abaixo.
- **Windows em release**: o job "Windows · desempenho (release)" do Noturno (`.github/workflows/nightly.yml`, também pelo `workflow_dispatch`) roda os mesmos benchmarks e os E2E de `e2e/perf/` com o app otimizado (`cargo tauri build --no-bundle` com `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`, que liga só os ganchos dos E2E; `WARDEN_E2E_PROFILE=release` no `wdio.conf.ts`). Ali o E2E confere a **meta exata** das metas da SPEC, sem a folga do debug (a exportação, que não tem meta própria, continua com o dobro da referência); as linhas `A-04 | …` e `A-04 E2E (release) | …` saem no log e no artefato `desempenho-windows-<commit>`.
- **E2E de desempenho** (app real, build de debug, como na CI): `apps/desktop/e2e/perf/` (`meus-packs.e2e.ts` e `pack-grande.e2e.ts`). Cada medida roda várias vezes e o teste confere a **mediana** contra a meta com uma folga para o build de debug nas máquinas da CI (1,5 vez a meta; 2 vezes na exportação, que não tem meta própria): as metas são do app instalado na máquina de referência, e a CI roda o debug sem GPU, com tempos que variam muito entre rodadas. O log traz `A-04 E2E | …` com todas as medidas e a situação contra a **meta exata**; os benchmarks conferem a meta exata no build de release. Rodam junto dos outros E2E (`xvfb-run -a pnpm exec wdio run ./wdio.conf.ts`).

O pack de teste é **gerado** (nada é baixado):

- 300 metafiles do Modrinth com IDs inventados e lado variado; no E2E, mais 150 resource packs e 50 shaders, para a lista de Mods ter 500 itens (CA-T06-05). O servidor de fixtures dos E2E responde esses IDs (`e2e/perf/modrinth.ts`), então a primeira abertura enche o cache de metadados como no uso de verdade.
- 300 jars sintéticos com o que os índices de pacotes (D-05) e o pré-teste (D-01) leem: `fabric.mod.json` com dependências entre os mods, config de mixin, arquivo de idioma e classes. De dois tipos:
  - **mínimos** (poucos KB): classes em 8 a 32 pacotes; a cada 25 jars, cinco embutidos; a cada 100, um grande (150 pacotes);
  - **calibrados pelos packs reais** (usados nas metas): cada jar segue uma de 20 faixas (`FAIXAS` em `pack_grande.rs`) com o número de entradas, classes, pacotes, embutidos e o tamanho medidos nos jars de três packs reais (tabela abaixo). No todo, ~730 entradas, 24 pacotes, 1 embutido e ~2,6 MB por jar em média, ~780 MB nos 300 mods; o resto das entradas são recursos (modelos JSON) e um arquivo de dados sem compressão completa o tamanho. A versão anterior (100 a 300 entradas e ~1 MB por jar) não representava um pack real: com ela o índice com cache parecia levar 78 ms, e com o Prominence II levava ~450 ms.
- 500 configs reais: o corpus da `warden-configs` (`crates/warden-configs/tests/corpus/`) repetido em lotes.
- `pack.toml` (Fabric 1.20.1) e `index.toml` com os hashes certos; o packwiz aceita o pack.
- Para Meus packs: 50 packs versionados no git como o Warden os deixa (5 grandes e 45 pequenos; metade com a versão 1.0.0 salva e alterações por salvar).

O gerador das crates fica em `crates/warden-packwiz/benches/comum/pack_grande.rs` (incluído pelos benchmarks das seis crates); o dos E2E, em `apps/desktop/e2e/perf/comum.ts`. Os E2E não abrem jars (nenhuma tela medida lê o conteúdo deles), então só os benchmarks usam os jars.

### Calibração dos jars

Medida em 17/10/2026 nos jars dos mods de três packs grandes do Modrinth: **Prominence II 4.1.4** (Fabric 1.20.1, 405 mods), **Better MC Forge v59** (Forge 1.20.1, 309 mods) e **Better MC Fabric v40** (Fabric 1.20.1, 313 mods), 895 jars únicos. Os jars foram ordenados pelo tamanho e divididos em 20 faixas; cada faixa do gerador tem a **média** dos jars dela (a média, e não a mediana, para o total bater: a última faixa tem os mods de dezenas de MB, até 294 MB).

| Pack | Mods | Total | Tamanho por jar (mediana / p90 / p99) | Entradas por jar (média / mediana / p90 / p99 / maior) | Pacotes por jar (média / mediana) |
|---|---|---|---|---|---|
| Prominence II 4.1.4 | 405 | 1,26 GB | 373 KB / 4,7 MB / 51 MB | 776 / 139 / 1.413 / 9.498 / 38.630 | 24 / 13 |
| Better MC Forge v59 | 309 | 946 MB | 407 KB / 3,6 MB / 53 MB | 813 / 139 / 1.718 / 8.562 / 38.628 | 25 / 11 |
| Better MC Fabric v40 | 313 | 590 MB | 367 KB / 2,6 MB / 23 MB | 665 / 118 / 1.195 / 6.887 / 38.630 | 22 / 10 |

O preparador do pack real (abaixo) imprime as mesmas 20 faixas para o pack baixado (linhas `A-04 | faixa …`, no formato das `FAIXAS`), para refazer a calibração com outros packs.

### Medir com um pack real

O pack real nunca entra no repositório nem nos testes da CI: é baixado só para a medida, numa pasta fora do repositório, e apagado depois (um pack de 400 mods passa de 1 GB; com o `target` de release, o disco do contêiner da nuvem enche).

1. Escolha um `.mrpack` público grande do Modrinth (300+ mods) e copie o link do arquivo da versão (`https://cdn.modrinth.com/data/<projeto>/versions/<versão>/<arquivo>.mrpack`) ou baixe o arquivo.
2. Prepare a pasta (baixa os jars dos mods do índice, conferindo o SHA-1, e copia as configs de `overrides/config/`):
   ```sh
   WARDEN_A04_MRPACK=<link ou arquivo .mrpack> WARDEN_A04_PACK_REAL=<pasta fora do repositório> \
     cargo bench -p warden-jarmeta --bench preparar_pack_real
   ```
   No PowerShell: `$env:WARDEN_A04_MRPACK = "<link>"; $env:WARDEN_A04_PACK_REAL = "<pasta>"` e o mesmo `cargo bench`.
3. Meça com a mesma `WARDEN_A04_PACK_REAL` (os benchmarks montam um pack packwiz com os metafiles do Modrinth, as configs e o loader do `.mrpack`, e leem os jars na pasta preparada):
   ```sh
   WARDEN_A04_PACK_REAL=<pasta> cargo bench -p warden-jarmeta -p warden-diagnostics -p warden-export --bench pack_grande
   ```
   Saem as linhas `A-04 | … pack real <nome>, <n> mods | …`: índice de pacotes sem e com cache, pré-teste completo e prévia e `.zip` da exportação.
4. Apague a pasta. No contêiner da nuvem, use `CARGO_INCREMENTAL=0` e apague `target/debug/incremental` antes de compilar em release.

## Máquina e data

Benchmarks medidos em **2026-10-17** e E2E do Linux em 2026-10-14, no contêiner de desenvolvimento da nuvem: Intel Xeon 2,10 GHz, 4 vCPU, 15 GB de RAM, SSD virtual, Ubuntu 24.04.5, WebKitGTK 2.52.6 sob Xvfb (sem GPU), Rust 1.98.1, Node 24. Não é a máquina de referência da SPEC (Windows 11, SSD, 16 GB): os números do Windows vêm dos jobs "Windows · E2E" (debug) e "Windows · desempenho (release)" do Noturno.

## Resultados: benchmarks (release)

Mediana e pior caso de 10 medidas, no Linux. "Pack real": Prominence II 4.1.4 (405 mods, 1,26 GB de jars, 1.304 configs com 451 MB, dos quais 420 MB são regiões `.mca` de mundos guardadas em `config/`).

| Meta da SPEC §8 | Medida | Mediana | Pior | Meta |
|---|---|---|---|---|
| Abrir um pack de 300 mods (com cache) | Ler `pack.toml`, `index.toml` e os 300 metafiles | 5,7 ms | 14,2 ms | < 1 s |
| Abrir um pack de 300 mods (com cache) | Inventário da lista de Mods, cache do Modrinth em arquivo | 12,7 ms | 15,8 ms | < 1 s |
| CA-T02-01 (Meus packs) | Lista com 50 packs versionados (5 grandes) | 42,3 ms | 53,0 ms | < 1 s |
| Atualizar índice (300 mods + 500 configs) | `packwiz refresh` pelo sidecar, com uma config alterada | 61,6 ms | 71,5 ms | < 5 s |
| Diagnóstico pré-teste (300 mods, cache quente) | Passagem rápida (regras com os dados das APIs) | 0,1 ms | 0,2 ms | < 3 s |
| Diagnóstico pré-teste (300 mods, cache quente) | Passagem completa: ler os 300 jars calibrados, regras e índice de pacotes | 169,6 ms | 176,1 ms | < 3 s |
| Diagnóstico pré-teste (cache quente) | Passagem completa com o **pack real** (405 jars) | 625,6 ms | 673,4 ms | < 3 s |
| Índices de pacotes (300 mods) | Sem cache, jars mínimos | 389,9 ms | 444,3 ms | < 3 s |
| Índices de pacotes (300 mods) | Com cache, jars mínimos | 6,4 ms | 7,9 ms | < 300 ms |
| Índices de pacotes (300 mods) | Sem cache, jars calibrados (~780 MB) | 750,7 ms | 781,1 ms | < 3 s |
| Índices de pacotes (300 mods) | Com cache, jars calibrados | 10,6 ms | 11,9 ms | < 300 ms |
| Índices de pacotes | Sem cache, **pack real** (405 jars, 1,26 GB) | 1.387 ms | 1.438 ms | < 3 s |
| Índices de pacotes | Com cache, **pack real** | 17,9 ms | 18,8 ms | < 300 ms |
| Busca em todas as configs (500 arquivos) | Índice pronto | 111,8 ms | 132,1 ms | < 2 s |
| Busca em todas as configs (500 arquivos) | Busca "spawn" / "enable" / "render distance" / "max" / "a" | 6,0 a 12,6 ms | 13,4 ms | < 200 ms |
| Exportação (E-01; referência: a meta de "Atualizar índice") | Prévia da seção Exportar | 44,7 ms | 58,9 ms | < 1 s |
| Exportação (E-01) | `.zip`, com o `packwiz refresh` em staging | 320,8 ms | 354,1 ms | < 5 s |
| Exportação (E-01) | Pasta, com o `packwiz refresh` em staging | 519,9 ms | 720,2 ms | < 5 s |
| Exportação (E-01) | Prévia do **pack real** (405 mods + 1.304 configs) | 185,8 ms | 255,9 ms | < 1 s |
| Exportação (E-01) | `.zip` do **pack real**, com o `packwiz refresh` em staging | 4.718 ms | 4.907 ms | < 5 s |

Com o pack real, antes das correções desta rodada (mesma máquina, release): índice de pacotes com cache **453 ms** (pior 466 ms; acima da meta de 300 ms) e sem cache 1.738 ms; pré-teste completo 1.114 ms; `.zip` da exportação **12.968 ms** (pior 13.769 ms). O `.zip` do pack real fica perto da referência de 5 s porque a exportação passa várias vezes pelos 451 MB de `config/` (cópia para o staging, dois `packwiz refresh` que calculam o hash de tudo, a conferência de que a saída é estável e o zip): o tempo cresce com os bytes do pack (~10 ms por MB nesta máquina), não com o número de arquivos. Um pack sem mundos dentro de `config/` (o caso comum: as configs do Prominence fora os `.mca` somam ~30 MB) fica na faixa do pack gerado.

## Resultados: E2E (app real, build de debug)

Dez rodadas seguidas de `meus-packs.e2e.ts` e `pack-grande.e2e.ts`, sem falha. "Mediana" e "pior" juntam todas as medidas das dez rodadas (o teste confere a mediana de cada rodada).

| Meta | Medida no app | Medidas | Mediana | Pior | Meta |
|---|---|---|---|---|---|
| CA-T02-01 | Do início da página às 50 linhas de Meus packs (packs versionados, 5 grandes) | 50 | 498 ms | 730 ms | < 1 s |
| Abrir o app até a Lista de packs | Do pedido de sessão ao WebDriver às 50 linhas (inclui criar a sessão; só no log) | 10 | 1.500 ms | 1.609 ms | < 3 s |
| Abrir um pack de 300 mods (com cache) | De clicar no nome em Meus packs à lista de Mods na tela | 50 | 732 ms | 1.133 ms | < 1 s |
| Diagnóstico pré-teste | "Verificar agora" em Problemas (comando `diagnostics_run`, passagem rápida) | 50 | 237 ms | 335 ms | < 3 s |
| Busca em todas as configs | Índice pronto, de abrir a seção Configs até o índice dizer "pronto" | 10 | 639 ms | 791 ms | < 2 s |
| Busca em todas as configs | Cada busca pelo comando `config_search` (5 textos, 3 vezes cada) | 150 | 96 ms | 175 ms | < 200 ms |
| Exportação (E-01) | `.zip` do pack grande, de "Exportar…" ao diálogo do resultado | 30 | 1.718 ms | 2.315 ms | < 5 s |
| (sem meta) | Importar o pack grande pelo diálogo Abrir pack (git criado do zero, 1.300 arquivos) | 10 | 2.383 ms | 2.735 ms | — |

### Windows da CI (Noturno, WebView2 sem GPU, build de debug)

Quatro rodadas do job "Windows · E2E" (5 medidas por linha, 3 na exportação). Na segunda e na terceira as bibliotecas em C (libgit2, SQLite, zlib) estavam compiladas com otimização no build de debug; como isso não mudou os números do Windows além da variação da máquina, a mudança foi desfeita. A quarta é a do código final (todos os 26 E2E passaram). Entre as duas, a mesma exportação foi de 4,0 s para 6,7 s sem nenhuma mudança que a afetasse: a máquina da CI varia muito.

| Medida | 1ª rodada (mediana / pior) | 2ª rodada (mediana / pior) | 3ª rodada (mediana / pior) | 4ª rodada, final (mediana / pior) | Meta |
|---|---|---|---|
| CA-T02-01, `packs.e2e.ts` (50 packs simples, sem git) | 532 ms | 629 ms | 551 ms | 565 ms | < 1 s |
| CA-T02-01, 50 packs versionados (5 grandes) | 996 / 1.133 ms | (não rodou: o preparo dos 50 repositórios passou do tempo do gancho, corrigido) | 943 / 1.107 ms | 975 / 1.184 ms | < 1 s |
| Abrir o app até Meus packs (uma medida, de fora) | 2.116 ms | — | 2.002 ms | 2.042 ms | < 3 s |
| Abrir o pack de 300 mods (com cache) | 1.027 / 1.032 ms | 1.011 / 1.078 ms | 1.005 / 1.045 ms | 1.034 / 1.068 ms | < 1 s |
| CA-T06-05: quadros rolando (parado: 15,6 ms) | mediana 31,2 ms, pior 46,9 ms | mediana 15,7 ms, pior 46,9 ms, 4 de 240 acima de 2,5 vezes | mediana 31,2 ms, pior 46,9 ms, 8 de 240 acima de 2,5 vezes | mediana 31,2 ms, pior 46,9 ms, 8 de 240 acima de 2,5 vezes | 60 por segundo |
| Problemas: Verificar agora | 401 / 439 ms | 439 / 620 ms | 425 / 455 ms | 383 / 429 ms | < 3 s |
| Configs: índice pronto (uma medida) | 1.035 ms | 904 ms | 1.188 ms | 961 ms | < 2 s |
| Configs: cada busca | 110 / 156 ms | 108 / 158 ms | 118 / 185 ms | 111 / 160 ms | < 200 ms |
| Exportar: `.zip` | 3.987 / 4.084 ms | 6.740 / 8.379 ms | 4.465 / 4.981 ms | 4.476 / 4.543 ms | (referência < 5 s) |
| Importar o pack grande (git do zero) | 7.236 ms | 6.292 ms | 6.496 ms | 6.401 ms | — |

No build de debug do Windows da CI, abrir o pack grande fica em torno de 1 s (de 985 a 1.078 ms nas 20 medidas) e Meus packs com packs versionados também; no release os mesmos passos levam 10 a 45 ms no backend (benchmarks). O E2E confere essas metas com a folga do debug (1,5 s); a meta exata no Windows é conferida em release pelo job "Windows · desempenho (release)" (seção abaixo).

O build de debug é bem mais lento que o de release (a libgit2, o SQLite e o `toml_edit` compilados sem otimização): o mesmo inventário leva ~90 ms no debug e 10 ms no release. As metas valem para o app instalado; os E2E as conferem com folga mesmo no debug.

### Windows da CI em release (Noturno, job "Windows · desempenho (release)")

Os benchmarks e os E2E de `e2e/perf/` com o app otimizado, em três rodadas de 17/10/2026 (as duas primeiras na tabela, a 3ª logo abaixo) (WebView2 sem GPU, como no debug). A 1ª rodada é do commit com o cache por identidade e o zip sem recompressão; a 2ª, depois da gravação direta do cache (abaixo). A máquina da 2ª rodada foi mais lenta em tudo (a mesma prévia da exportação: 210 → 425 ms, sem mudança de código que a afetasse).

| Medida | 1ª rodada (mediana / pior) | 2ª rodada (mediana / pior) | Meta |
|---|---|---|---|
| **E2E:** abrir o app até Meus packs (uma medida, de fora) | 1.588 ms | 2.116 ms | < 3 s |
| **E2E:** CA-T02-01, 50 packs versionados (5 grandes) | 538 / 802 ms | 988 / 1.164 ms | < 1 s |
| **E2E:** abrir o pack de 300 mods (com cache) | **455 / 477 ms** | **613 / 626 ms** | < 1 s |
| **E2E:** CA-T06-05, quadros rolando (parado: 15,6 ms) | 15,7 ms, pior 31,4 ms, 0 de 240 acima de 2,5 vezes | 31,2 ms, pior 62,5 ms, 9 de 240 | 60 por segundo |
| **E2E:** Problemas, Verificar agora | 152 / 589 ms | 165 / 198 ms | < 3 s |
| **E2E:** Configs, índice pronto (uma medida) / cada busca | 291 ms / 14 (pior 22) ms | 421 ms / 22 (pior 32) ms | < 2 s / < 200 ms |
| **E2E:** Exportar `.zip` | 2.640 / 12.485 ms | 5.840 / 9.104 ms | (referência < 5 s) |
| Benchmark: ler os 300 metafiles / inventário com cache | 9,4 / 15,6 ms | 13,7 / 24,6 ms | < 1 s |
| Benchmark: Meus packs, 50 packs versionados | 283 ms | 534 ms | < 1 s |
| Benchmark: `packwiz refresh` | 180 ms | 321 ms | < 5 s |
| Benchmark: pré-teste completo (300 jars calibrados) | 420 ms | 562 ms | < 3 s |
| Benchmark: índice de pacotes sem cache, jars calibrados | **5.580 ms** | **670 ms** | < 3 s |
| Benchmark: índice de pacotes sem cache, jars mínimos | **9.990 ms** | 405 ms (pior 3.822 ms) | < 3 s |
| Benchmark: índice de pacotes com cache, calibrados / mínimos | 16,3 / 14,5 ms | 24,7 / 21,6 ms | < 300 ms |
| Benchmark: busca em todas as configs (índice / cada busca) | 113 ms / 5,6 a 13,6 ms | 164 ms / 6,3 a 14,9 ms | < 2 s / < 200 ms |
| Benchmark: exportar prévia / `.zip` / pasta | 210 / 1.510 / 2.605 ms | 425 / 2.366 (pior 8.595) / 2.777 ms | < 1 s / < 5 s |

- **3ª rodada (código final, commit da folga da exportação):** job verde; abrir o pack 329 ms (pior 353), Meus packs 442 ms (pior 800), Problemas 562 ms, cada busca 14 ms, exportar `.zip` 2.498 ms (pior 8.610); benchmarks: índice de pacotes sem cache 688 ms (calibrados) e 326 ms (mínimos, pior 611), com cache 13,9 e 10,5 ms, pré-teste completo 415 ms.
- **Abrir o pack grande** fica em 329 a 626 ms em release no Windows da CI (no debug, ~1.025 ms): a meta de < 1 s fecha com folga, e o E2E de release a confere sem folga. **Meus packs** também fecha, mas com pouca sobra na máquina mais lenta (988 ms de mediana); o backend da lista leva 283 a 534 ms ali (43 ms no Linux) e o resto é a interface no WebView2 sem GPU.
- **Índice de pacotes sem cache no Windows:** a 1ª rodada mostrou 5,6 s e 10 s, acima da meta. A causa era a gravação do cache: cada arquivo era gravado com `fsync` e um temporário renomeado, e a renomeação espera o antivírus soltar o arquivo novo (até 200 ms de novas tentativas por arquivo), duas vezes por jar com a identidade. O cache passou a gravar direto (tudo que ele lê é validado; um arquivo pela metade é refeito), e a 2ª rodada ficou em 670 ms e 405 ms. O pior caso isolado de 3,8 s dos jars mínimos foi a primeira passagem do processo, com 600 arquivos novos de uma vez; a mediana de 10 fica bem abaixo da meta.
- **Exportar `.zip` no Windows da CI** varia muito entre medidas (2,6 s e 12,5 s na mesma rodada; 3,3 a 9,1 s na outra): a exportação copia o pack para o staging e roda o `packwiz refresh` duas vezes, e o antivírus da máquina da CI examina cada arquivo novo. Não há meta própria na SPEC; o E2E confere a mediana com o dobro da referência de "Atualizar índice" (10 s) também em release.

### CA-T06-05: rolar a lista de 500 itens

O E2E rola a lista de Mods (500 itens, 300 mods e 150 resource packs virtualizados) 60 px por quadro e compara o intervalo entre quadros com o da tela parada.

- **Linux da CI (este contêiner):** parada, 16 ms por quadro; rolando, ~280 ms por quadro (pior 390 ms). O WebKitGTK no Xvfb não tem composição acelerada e pinta tudo na CPU: cada quadro em que a lista virtualizada troca de linha repinta a tela inteira. Medido trocando o CSS em tempo de execução: sem a animação de brilho do botão Testar (`filter: drop-shadow` animado), ~115 ms; sem as sombras (`box-shadow`), ~138 ms; sem as duas, ~64 ms; o trabalho do React em cada quadro fica em ~5 ms (uma linha nova por quadro). A mesma rolagem em Meus packs (sem troca de linhas) fica em 16 ms. No WebView2 (Chromium) da máquina de referência, a animação de `filter` e a rolagem são compostas na GPU. Por isso, no Linux, o teste só confere que a rolagem andou e que a lista acompanhou, e o número vai para o log.
- **Windows da CI (Noturno):** o WebView2 também roda sem GPU, e os quadros vêm em múltiplos do intervalo da tela (15,6 ms). Na primeira e na terceira rodada a mediana rolando foi de 31,2 ms (30 por segundo); na segunda, 15,7 ms (60 por segundo); no máximo 8 de 240 quadros acima de 2,5 intervalos. O teste confere pelo menos 30 por segundo (mediana de até dois intervalos) e no máximo 1 em 10 quadros acima de 2,5 intervalos.
- **Máquina de referência:** os 60 por segundo da CA valem para ela (Windows 11 com GPU), que a CI não tem: fica para o dono conferir no roteiro de aceite (A-07).

## Gargalos encontrados e correções

| Onde | O que pesava | Correção | Antes | Depois |
|---|---|---|---|---|
| Meus packs (`warden-project::registry`) | Cada linha lia o índice e os 300 metafiles (`read_pack`) e montava o changelog inteiro (lendo e comparando o conteúdo de cada arquivo alterado) só para contar arquivos; com o pack nunca salvo, eram os 800 arquivos. Tudo em sequência e **na thread da interface** (comando síncrono do Tauri). | A linha lê só o `pack.toml` (`warden_packwiz::read_manifest`) e só os nomes dos arquivos alterados (`PackRepo::files_changed_since_last_version`); as linhas são montadas em paralelo; `packs_list` e `pack_get` rodam fora da thread da interface; `pack_get` (página do pack) monta só a linha dele (`Registry::row`) em vez da lista toda. | 463 ms (debug), 128 ms (release) | 65 ms (debug), 43 ms (release) |
| Abrir o pack: inventário (`warden-modrinth`) | Um projeto do cache por vez: 500 tarefas, cada uma pegando a trava da conexão do SQLite, disputando com as outras leituras da abertura do pack (atualizações, higiene, diagnóstico). | `ModrinthClient::projects` lê todos os projetos do cache numa tarefa só (`MetadataCache::projects_by_key`). | 76 a 220 ms (debug, no app) | uma ida ao banco |
| Abrir o pack: leitura (`warden-packwiz`) | `read_pack` lia os metafiles um por um. | Em paralelo a partir de 64 metafiles, na ordem do índice. | 43 ms (debug) | 25 ms (debug) |
| Abrir o pack: lista de Mods (`ItemGroupTable`) | Cada grupo virtualizado se supunha no topo antes de medir onde estava e montava uma tela de linhas (29) que sumiam em seguida: 108 linhas montadas ao abrir, com uma medida de layout forçada sobre todas elas. | O grupo só monta linhas depois da primeira medida (antes da pintura; nada pisca). | ~260 ms de renderização, 108 linhas | ~150 ms, 85 linhas |
| Índices de pacotes com cache (`warden-jarmeta::index`) | Cada passagem lia cada jar inteiro para calcular o SHA-256 que é a chave do cache: 1,26 GB no Prominence II. | O cache guarda o hash de cada arquivo já visto pela **identidade** dele (tamanho, data de modificação e diretório central do zip, que traz o nome, o tamanho e o CRC-32 de cada entrada), em `cache/jarindex/arquivos/`; a segunda passagem lê só o fim de cada jar. Qualquer mudança no arquivo muda a identidade e o jar é lido de novo; sem identidade (zip64, arquivo que não termina num zip), vale o hash como antes. | 453 ms (pack real, 405 mods) | 18 ms |
| Índices de pacotes sem cache | Cada jar era lido duas vezes (uma para o hash, outra para o índice) e o hash calculado duas vezes; os jars eram divididos em blocos fixos por leitor, e o bloco com um mod de 294 MB segurava os outros. | Uma leitura e um hash por jar; cada leitor pega o próximo jar de uma fila. | 1.738 ms (pack real) | 1.387 ms |
| Índices de pacotes sem cache no Windows (`warden-jarmeta::index`) | Cada arquivo do cache (o índice e a identidade de cada jar) era gravado de forma atômica: `fsync` e um temporário renomeado, e a renomeação esperava o antivírus soltar o arquivo novo. | O cache grava direto: tudo que ele lê é validado, e um arquivo pela metade (queda no meio) não é JSON nem hash válido e é refeito. | 5.580 ms (Windows, release, 300 jars calibrados) | 670 ms |
| Exportar `.zip` (`warden-export::native`) | Tudo entrava com deflate, inclusive o que já vem comprimido: 420 MB de regiões `.mca` em `config/` do pack real, que o deflate diminui 11% em ~8 s. | Regiões de mundo, zips, jars, imagens e áudio entram guardados no zip; o resto, com deflate. | 12.968 ms (pack real) | 4.718 ms |

Efeito no E2E de "abrir o pack com cache": ~1.000 ms (estourava a meta) → 732 ms de mediana. Em Meus packs com packs versionados, o build de debug gastava ~460 ms só no backend, na thread da interface, além dos ~550 ms da interface: a lista passaria de 1 s.

## CA-T02-01: folga real ou gargalo?

O `packs.e2e.ts` mede a lista com 50 packs vazios e sem git, e leva 500 a 650 ms no Windows da CI (600 ms aqui). Desse tempo, quase nada é o backend: medido dentro do WebView, o JavaScript do app termina de carregar em ~170 ms, as primeiras chamadas ao Rust voltam juntas em ~300 ms (o comando `packs_list` em si leva ~8 ms), o React monta as 50 linhas em ~130 ms e a pintura leva ~100 ms. A folga é real (~400 ms), mas é da interface e da máquina; a vez que passou de 1 s foi lentidão da máquina da CI.

Com packs de verdade (versionados no git, alguns grandes), havia um gargalo no backend: ~460 ms no build de debug, na thread da interface (o comando era síncrono). Corrigido (tabela acima); o E2E novo (`e2e/perf/meus-packs.e2e.ts`) mede esse caso pela mediana de 5 aberturas: 498 ms aqui e 996 ms no Windows da CI (build de debug), onde ainda há pouca folga: o que resta é o git de cada pack no debug e a interface. No release o backend da lista leva 43 ms.

## O que não foi medido aqui

- Busca no Modrinth (< 2 s) e início da página de descoberta: dependem da rede; ficam com as tarefas donas e o teste de rede.
- Amostragem de desempenho durante o teste (< 1% de CPU) e console agrupado (50.000 linhas): L-10 e D-09 (metas P1 da D4 medidas pelas tarefas donas).
- Memória do app (< 500 MB) e robustez a encerramento forçado: fora do pack grande; não medidos pela A-04.
