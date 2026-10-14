# Desempenho com packs grandes (A-04)

Medições das metas da [SPEC §8](SPEC.md#8-requisitos-não-funcionais) com um pack de **300 mods e 500 configs** (tarefa A-04 do [ROADMAP](ROADMAP.md)), os gargalos encontrados e o que foi corrigido. As metas P1 da D4 (raio-x de mixins, console agrupado) e as da 1.1 são medidas pelas tarefas donas.

## Como medir

- **Benchmarks** (`criterion`, build de release): `cargo bench -p <crate>` em `warden-packwiz`, `warden-project`, `warden-configs`, `warden-jarmeta`, `warden-diagnostics` e `warden-export` (arquivos `benches/pack_grande.rs`). Cada um imprime, além do relatório do criterion, uma linha `A-04 | medida | mediana | pior | meta | ok` com 10 medidas.
- **E2E de desempenho** (app real, build de debug, como na CI): `apps/desktop/e2e/perf/` (`meus-packs.e2e.ts` e `pack-grande.e2e.ts`). Cada medida roda várias vezes e o teste confere a **mediana** contra a meta com uma folga para o build de debug nas máquinas da CI (1,5 vez a meta; 2 vezes na exportação, que não tem meta própria): as metas são do app instalado na máquina de referência, e a CI roda o debug sem GPU, com tempos que variam muito entre rodadas. O log traz `A-04 E2E | …` com todas as medidas e a situação contra a **meta exata**; os benchmarks conferem a meta exata no build de release. Rodam junto dos outros E2E (`xvfb-run -a pnpm exec wdio run ./wdio.conf.ts`).

O pack de teste é **gerado** (nada é baixado):

- 300 metafiles do Modrinth com IDs inventados e lado variado; no E2E, mais 150 resource packs e 50 shaders, para a lista de Mods ter 500 itens (CA-T06-05). O servidor de fixtures dos E2E responde esses IDs (`e2e/perf/modrinth.ts`), então a primeira abertura enche o cache de metadados como no uso de verdade.
- 300 jars sintéticos com o que os índices de pacotes (D-05) e o pré-teste (D-01) leem: `fabric.mod.json` com dependências entre os mods, config de mixin, arquivo de idioma e classes mínimas em 8 a 32 pacotes; a cada 25 jars, cinco embutidos; a cada 100, um grande (150 pacotes). Em dois tamanhos: mínimos (poucos KB) e com o tamanho dos de verdade (~1 MB em média, 300 MB ao todo: a cada 20 mods, um de 8 MB, três de 2 MB e dezesseis de 400 KB).
- 500 configs reais: o corpus da `warden-configs` (`crates/warden-configs/tests/corpus/`) repetido em lotes.
- `pack.toml` (Fabric 1.20.1) e `index.toml` com os hashes certos; o packwiz aceita o pack.
- Para Meus packs: 50 packs versionados no git como o Warden os deixa (5 grandes e 45 pequenos; metade com a versão 1.0.0 salva e alterações por salvar).

O gerador das crates fica em `crates/warden-packwiz/benches/comum/pack_grande.rs` (incluído pelos benchmarks das seis crates); o dos E2E, em `apps/desktop/e2e/perf/comum.ts`.

## Máquina e data

Medido em **2026-10-14**, no contêiner de desenvolvimento da nuvem: Intel Xeon 2,10 GHz, 4 vCPU, 15 GB de RAM, SSD virtual, Ubuntu 24.04.5, WebKitGTK 2.52.6 sob Xvfb (sem GPU), Rust 1.98.1, Node 24.21. Não é a máquina de referência da SPEC (Windows 11, SSD, 16 GB): os números do Windows vêm do job "Windows · E2E" do Noturno, que roda os mesmos E2E (o log traz as linhas `A-04 E2E`).

## Resultados: benchmarks (release)

Mediana e pior caso de 10 medidas.

| Meta da SPEC §8 | Medida | Mediana | Pior | Meta |
|---|---|---|---|---|
| Abrir um pack de 300 mods (com cache) | Ler `pack.toml`, `index.toml` e os 300 metafiles | 5,8 ms | 6,4 ms | < 1 s |
| Abrir um pack de 300 mods (com cache) | Inventário da lista de Mods, cache do Modrinth em arquivo | 10,0 ms | 13,7 ms | < 1 s |
| CA-T02-01 (Meus packs) | Lista com 50 packs versionados (5 grandes) | 42,8 ms | 45,6 ms | < 1 s |
| Atualizar índice (300 mods + 500 configs) | `packwiz refresh` pelo sidecar, com uma config alterada | 59,5 ms | 63,8 ms | < 5 s |
| Diagnóstico pré-teste (300 mods, cache quente) | Passagem rápida (regras com os dados das APIs) | 0,1 ms | 0,1 ms | < 3 s |
| Diagnóstico pré-teste (300 mods, cache quente) | Passagem completa: ler os 300 jars, regras e índice de pacotes | 148,6 ms | 170,7 ms | < 3 s |
| Índices de pacotes (300 mods) | Sem cache, jars mínimos | 256,8 ms | 269,9 ms | < 3 s |
| Índices de pacotes (300 mods) | Com cache, jars mínimos | 6,7 ms | 8,1 ms | < 300 ms |
| Índices de pacotes (300 mods) | Sem cache, jars do tamanho real (300 MB) | 424,0 ms | 493,5 ms | < 3 s |
| Índices de pacotes (300 mods) | Com cache, jars do tamanho real (lê e calcula o hash de cada jar) | 78,3 ms | 84,8 ms | < 300 ms |
| Busca em todas as configs (500 arquivos) | Índice pronto | 101,8 ms | 125,3 ms | < 2 s |
| Busca em todas as configs (500 arquivos) | Busca "spawn" / "enable" / "render distance" / "max" / "a" | 5,4 a 12,5 ms | 14,7 ms | < 200 ms |
| Exportação (E-01; referência: a meta de "Atualizar índice") | Prévia da seção Exportar | 40,9 ms | 47,5 ms | < 1 s |
| Exportação (E-01) | `.zip`, com o `packwiz refresh` em staging | 328,5 ms | 370,4 ms | < 5 s |
| Exportação (E-01) | Pasta, com o `packwiz refresh` em staging | 941,6 ms | 1.013,0 ms | < 5 s |

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

Duas rodadas do job "Windows · E2E" (5 medidas por linha, 3 na exportação). A primeira, antes de otimizar as bibliotecas em C no build de debug (tabela de gargalos); a segunda, depois. Entre as duas, a mesma exportação foi de 4,0 s para 6,7 s sem nenhuma mudança que a afetasse: a máquina da CI varia muito.

| Medida | 1ª rodada (mediana / pior) | 2ª rodada (mediana / pior) | Meta |
|---|---|---|---|
| CA-T02-01, `packs.e2e.ts` (50 packs simples, sem git) | 532 ms | 629 ms | < 1 s |
| CA-T02-01, 50 packs versionados (5 grandes) | 996 / 1.133 ms | (não rodou: o preparo dos 50 repositórios passou do tempo do gancho, corrigido) | < 1 s |
| Abrir o app até Meus packs (uma medida, de fora) | 2.116 ms | — | < 3 s |
| Abrir o pack de 300 mods (com cache) | 1.027 / 1.032 ms | 1.011 / 1.078 ms | < 1 s |
| CA-T06-05: quadros rolando (parado: 15,6 ms) | mediana 31,2 ms, pior 46,9 ms | mediana 15,7 ms, pior 46,9 ms, 4 de 240 acima de 2,5 vezes | 60 por segundo |
| Problemas: Verificar agora | 401 / 439 ms | 439 / 620 ms | < 3 s |
| Configs: índice pronto (uma medida) | 1.035 ms | 904 ms | < 2 s |
| Configs: cada busca | 110 / 156 ms | 108 / 158 ms | < 200 ms |
| Exportar: `.zip` | 3.987 / 4.084 ms | 6.740 / 8.379 ms | (referência < 5 s) |
| Importar o pack grande (git do zero) | 7.236 ms | 6.292 ms | — |

No build de debug do Windows da CI, abrir o pack grande fica em torno de 1 s (de 985 a 1.078 ms nas 10 medidas) e Meus packs com packs versionados também; no release os mesmos passos levam 10 a 45 ms no backend (benchmarks). O E2E confere essas metas com a folga do debug (1,5 s); a meta exata nessa máquina fica como acompanhamento.

O build de debug é bem mais lento que o de release (a libgit2, o SQLite e o `toml_edit` compilados sem otimização): o mesmo inventário leva ~90 ms no debug e 10 ms no release. As metas valem para o app instalado; os E2E as conferem com folga mesmo no debug.

### CA-T06-05: rolar a lista de 500 itens

O E2E rola a lista de Mods (500 itens, 300 mods e 150 resource packs virtualizados) 60 px por quadro e compara o intervalo entre quadros com o da tela parada.

- **Linux da CI (este contêiner):** parada, 16 ms por quadro; rolando, ~280 ms por quadro (pior 390 ms). O WebKitGTK no Xvfb não tem composição acelerada e pinta tudo na CPU: cada quadro em que a lista virtualizada troca de linha repinta a tela inteira. Medido trocando o CSS em tempo de execução: sem a animação de brilho do botão Testar (`filter: drop-shadow` animado), ~115 ms; sem as sombras (`box-shadow`), ~138 ms; sem as duas, ~64 ms; o trabalho do React em cada quadro fica em ~5 ms (uma linha nova por quadro). A mesma rolagem em Meus packs (sem troca de linhas) fica em 16 ms. No WebView2 (Chromium) da máquina de referência, a animação de `filter` e a rolagem são compostas na GPU. Por isso, no Linux, o teste só confere que a rolagem andou e que a lista acompanhou, e o número vai para o log.
- **Windows da CI (Noturno):** o WebView2 também roda sem GPU, e os quadros vêm em múltiplos do intervalo da tela (15,6 ms). Na primeira rodada a mediana rolando foi de 31,2 ms (30 por segundo); na segunda, 15,7 ms (60 por segundo), com 4 de 240 quadros acima de 2,5 intervalos. O teste confere pelo menos 30 por segundo (mediana de até dois intervalos) e no máximo 1 em 10 quadros acima de 2,5 intervalos.
- **Máquina de referência:** os 60 por segundo da CA valem para ela (Windows 11 com GPU), que a CI não tem: fica para o dono conferir no roteiro de aceite (A-07).

## Gargalos encontrados e correções

| Onde | O que pesava | Correção | Antes | Depois |
|---|---|---|---|---|
| Meus packs (`warden-project::registry`) | Cada linha lia o índice e os 300 metafiles (`read_pack`) e montava o changelog inteiro (lendo e comparando o conteúdo de cada arquivo alterado) só para contar arquivos; com o pack nunca salvo, eram os 800 arquivos. Tudo em sequência e **na thread da interface** (comando síncrono do Tauri). | A linha lê só o `pack.toml` (`warden_packwiz::read_manifest`) e só os nomes dos arquivos alterados (`PackRepo::files_changed_since_last_version`); as linhas são montadas em paralelo; `packs_list` e `pack_get` rodam fora da thread da interface; `pack_get` (página do pack) monta só a linha dele (`Registry::row`) em vez da lista toda. | 463 ms (debug), 128 ms (release) | 65 ms (debug), 43 ms (release) |
| Abrir o pack: inventário (`warden-modrinth`) | Um projeto do cache por vez: 500 tarefas, cada uma pegando a trava da conexão do SQLite, disputando com as outras leituras da abertura do pack (atualizações, higiene, diagnóstico). | `ModrinthClient::projects` lê todos os projetos do cache numa tarefa só (`MetadataCache::projects_by_key`). | 76 a 220 ms (debug, no app) | uma ida ao banco |
| Abrir o pack: leitura (`warden-packwiz`) | `read_pack` lia os metafiles um por um. | Em paralelo a partir de 64 metafiles, na ordem do índice. | 43 ms (debug) | 25 ms (debug) |
| Build de debug (raiz do `Cargo.toml`) | A libgit2, o SQLite e a zlib compiladas sem otimização no perfil `dev`: o git recalcula o hash dos arquivos recém-gravados e o cache de metadados faz centenas de leituras; no Windows da CI, Meus packs e abrir o pack ficavam perto do dobro do Linux. | Só essas três bibliotecas em C com `opt-level = 3` no perfil `dev`; o código Rust continua sem otimização. | inventário 91 ms (debug) | 44 ms (debug) |
| Abrir o pack: lista de Mods (`ItemGroupTable`) | Cada grupo virtualizado se supunha no topo antes de medir onde estava e montava uma tela de linhas (29) que sumiam em seguida: 108 linhas montadas ao abrir, com uma medida de layout forçada sobre todas elas. | O grupo só monta linhas depois da primeira medida (antes da pintura; nada pisca). | ~260 ms de renderização, 108 linhas | ~150 ms, 85 linhas |

Efeito no E2E de "abrir o pack com cache": ~1.000 ms (estourava a meta) → 732 ms de mediana. Em Meus packs com packs versionados, o build de debug gastava ~460 ms só no backend, na thread da interface, além dos ~550 ms da interface: a lista passaria de 1 s.

## CA-T02-01: folga real ou gargalo?

O `packs.e2e.ts` mede a lista com 50 packs vazios e sem git, e leva 500 a 650 ms no Windows da CI (600 ms aqui). Desse tempo, quase nada é o backend: medido dentro do WebView, o JavaScript do app termina de carregar em ~170 ms, as primeiras chamadas ao Rust voltam juntas em ~300 ms (o comando `packs_list` em si leva ~8 ms), o React monta as 50 linhas em ~130 ms e a pintura leva ~100 ms. A folga é real (~400 ms), mas é da interface e da máquina; a vez que passou de 1 s foi lentidão da máquina da CI.

Com packs de verdade (versionados no git, alguns grandes), havia um gargalo no backend: ~460 ms no build de debug, na thread da interface (o comando era síncrono). Corrigido (tabela acima); o E2E novo (`e2e/perf/meus-packs.e2e.ts`) mede esse caso pela mediana de 5 aberturas: 498 ms aqui e 996 ms no Windows da CI (build de debug), onde ainda há pouca folga: o que resta é o git de cada pack no debug e a interface. No release o backend da lista leva 43 ms.

## O que não foi medido aqui

- Busca no Modrinth (< 2 s) e início da página de descoberta: dependem da rede; ficam com as tarefas donas e o teste de rede.
- Amostragem de desempenho durante o teste (< 1% de CPU) e console agrupado (50.000 linhas): L-10 e D-09 (metas P1 da D4 medidas pelas tarefas donas).
- Memória do app (< 500 MB) e robustez a encerramento forçado: fora do pack grande; não medidos pela A-04.
