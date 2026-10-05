# R7 — Concorrência e mercado das ferramentas de criação de modpacks

> Tarefa R7 (onda 0). Data da coleta: 04/10/2026. Autor: agente R7, sob o orquestrador.
> Base: `docs/SPEC.md` 1.4 (v1 e 1.1), pesquisas R1 a R6 (`docs/research/01` a `06`), `docs/design/ESTRUTURA.md` e decisões D1 a D33.
> Anexo para slides: `docs/research/08-anexos/apresentacao.md`.

**Marcação das afirmações**

| Marca | Significado |
|---|---|
| **[verificado]** | Executei e vi o resultado (consulta a API, teste de download, contagem, leitura no navegador). |
| **[código]** | Li o código-fonte do projeto. |
| **[fonte]** | Está escrito numa fonte primária (site oficial, documentação, changelog, issue, post do autor). |
| **[fonte secundária]** | Site de terceiros que resume dados; usar com cautela. |
| **[inferência]** | Conclusão minha a partir das evidências. |

## Sumário

0. [Resumo executivo](#0-resumo-executivo)
1. [Como a pesquisa foi feita](#1-como-a-pesquisa-foi-feita)
2. [Panorama: os seis tipos de concorrente](#2-panorama-os-seis-tipos-de-concorrente)
3. [Fichas dos concorrentes](#3-fichas-dos-concorrentes)
4. [A voz dos criadores: dores e pedidos](#4-a-voz-dos-criadores-dores-e-pedidos)
5. [Mercado em números](#5-mercado-em-números)
6. [Matriz de comparação](#6-matriz-de-comparação)
7. [Onde o Warden fica atrás, empata e é único](#7-onde-o-warden-fica-atrás-empata-e-é-único)
8. [Implicações para o Warden](#8-implicações-para-o-warden)
9. [Achados fora do escopo (para o orquestrador)](#9-achados-fora-do-escopo-para-o-orquestrador)
10. [Fontes](#10-fontes)

---

## 0. Resumo executivo

1. **Ninguém junta o ciclo inteiro do criador.** Os launchers (Prism, app do Modrinth, app da CurseForge, ATLauncher, XMCL, FTB App) são feitos para **jogar**; criam e exportam packs como efeito colateral. As ferramentas declarativas (packwiz, Pakku) versionam bem, mas não testam nem diagnosticam. As ferramentas de diagnóstico (Crash Assistant, mclo.gs, MCDoctor.ai) não sabem nada do pack. O Warden é o único plano que liga **criar → testar → diagnosticar → versionar → publicar** num lugar só **[inferência, a partir das fichas da §3 e da matriz da §6]**.
2. **A maior dor de quem monta pack é achar o mod que trava.** Aparece em todas as fontes: conversas do r/feedthebeast com milhares de votos, ferramentas caseiras para automatizar a "busca binária" (Modpack Debug Kit, 775 votos), o pedido de bisseção no Prism aberto desde 2023 e o PR do Prism fechado sem merge em 22/09/2026 **[verificado]**. Nenhuma ferramenta madura faz a busca **automática e respeitando dependências**; a busca do culpado do Warden (T25) continua sem concorrente maduro.
3. **A segunda dor é a integração entre mods** (minérios e itens repetidos, receitas em conflito). O meme sobre minérios repetidos teve 5.610 votos e 171 comentários **[verificado]**. A 1.1 do Warden (T32) ataca isso; o resto do mercado não.
4. **Os pedidos mais votados nos launchers já estão no plano do Warden:** agrupar mods com etiquetas (Prism #1055, 66 votos, o pedido mais votado do Prism), facilitar o pacote para servidor (Prism #864, Modrinth #535, XMCL #1068), bisseção (Prism #1924), remover dependências que sobraram (packwiz #97, Modrinth #3143) **[verificado]**.
5. **Concorrência nova em IA:** o **XMCL** ligou para todos os usuários, em 23/08/2026, um assistente de IA com ferramentas (lê e edita configs, remove mods, diagnostica mods e escreve o changelog da versão), com confirmação antes de agir e um serviço próprio por assinatura ou o provedor do usuário **[código]**. É o concorrente mais próximo da seção ✦ Diagnóstico com IA. A diferença do Warden continua sendo a **evidência obrigatória** e o **Aplicar** com diferença exata.
6. **Mercado:** o Modrinth tem **18.649 modpacks** listados e **161.507 projetos** (04/10/2026) **[verificado]**; a CurseForge tem **mais de 104.741 modpacks** (piso, somando por versão) **[verificado]**. No Modrinth, 71% dos modpacks são Fabric; na CurseForge, o **NeoForge virou o padrão da 1.21.1** (mais de 10.000 packs contra 715 de Forge) **[verificado]**. A versão mais usada ainda é a **1.20.1** nas duas lojas.
7. **packwiz é nicho, mas é o nicho certo:** entre os 200 modpacks mais baixados do Modrinth, só 27 publicam o repositório no GitHub; destes, pelo menos 10 usam packwiz e 3 usam Pakku **[verificado]**. Há cerca de 2.520 `pack.toml` públicos no GitHub **[verificado, contagem aproximada da busca]**. Entre os packs do topo que publicam o repositório, metade usa packwiz ou Pakku **[verificado]**.
8. **Risco urgente para o modelo de distribuição do Warden:** a CurseForge anunciou que, a partir de 16/07/2026, o CDN de arquivos exigiria chave de API **[fonte]**; usuários do Prism tiveram erro em 19/07 **[fonte]**. O **packwiz-installer**, que os jogadores do Warden usam, não manda chave ao baixar e está sem commit desde 06/2024 **[código]**. Hoje (04/10) o download sem chave ainda funciona **[verificado]**, mas pode parar a qualquer momento. Recomendação: preferir o Modrinth e avisar sobre mods só da CurseForge antes de publicar (§8.4, decisão 4).
9. **Os launchers incomodam:** anúncios e peso da CurseForge (Overwolf usando 100 GB de dados em um mês, 1.771 votos; um terço da tela com anúncio, 415 votos), regressões e tom condescendente do app do Modrinth, e o Prism como escolha quase unânime ("It Just Works") **[verificado no Reddit]**. O Warden, sem anúncio, sem telemetria e local, está do lado certo.
10. **Lacunas que valem a pena** (§8.1): exportar uma **instância pronta para o Prism** (o jogador importa e pronto, sem os 4 passos do bootstrap), uma **lista de mods** para os jogadores, e a **prontidão para migrar** de versão do Minecraft. As três cabem na estrutura aprovada sem abas.

## 1. Como a pesquisa foi feita

| O quê | Como | Marca |
|---|---|---|
| Estatísticas dos repositórios (estrelas, releases, último commit, licença) | `gh api repos/...` e `.../releases`, 04/10/2026 | [verificado] |
| Funções dos apps | Notas de versão (GitHub, blog da CurseForge, notícias do Modrinth), páginas de suporte e leitura de código-fonte em pontos específicos | [fonte] e [código] |
| Números do Modrinth | `GET /v2/statistics` e `GET /v2/search` com facetas e filtros de data | [verificado] |
| Números da CurseForge | `GET /v1/mods/search` com a chave do `.env` (nunca impressa), somando por versão do Minecraft e loader, porque a API corta a contagem em 10.000 | [verificado] |
| Uso de packwiz entre os packs mais baixados | Os 200 modpacks mais baixados do Modrinth → `source_url` → árvore do repositório no GitHub (`pack.toml` ou `pakku.json`) | [verificado] |
| Voz dos criadores | (a) Reddit: arquivo público Arctic Shift para títulos e, a pedido do dono durante a tarefa, o **navegador embutido do Orca** (sessão do dono logada, só leitura: nenhuma postagem, voto ou mensagem) com a busca JSON do próprio Reddit; cerca de 50 conversas e mais de 400 comentários lidos. (b) Issues do GitHub ordenadas por votos (packwiz, Prism, Modrinth, Pakku, XMCL, ATLauncher, PW-GUI). (c) Portal de ideias da CurseForge | [verificado] |
| Teste do CDN da CurseForge | `curl` e Python contra `edge.forgecdn.net` e `mediafilez.forgecdn.net`, com e sem chave, com e sem `Range` | [verificado] |

**Limites honestos:**
- **Não baixei versões portáteis** dos apps para olhar telas. A análise das funções vem do código, das notas de versão e da documentação; onde não confirmei, a célula da matriz diz "?" ou a ficha diz [inferência]. Não há capturas de tela.
- A busca do Reddit é ruidosa (ordena por votos, não por assunto); por isso a frequência das dores (§4) cruza três fontes diferentes em vez de contar posts.
- O Modrinth só lista projetos aprovados e visíveis; a contagem por ano de criação é "dos que existem hoje", não dos que já existiram.
- Não criei contas, não fiz login (o login no Reddit foi do dono, no navegador do Orca) e não instalei nada.

## 2. Panorama: os seis tipos de concorrente

| Tipo | Exemplos | O que fazem pelo criador | O que não fazem |
|---|---|---|---|
| **Launcher-loja** (dono da plataforma) | App da CurseForge, app do Modrinth, FTB App | Criar perfil, adicionar mods da própria loja, exportar no formato da loja, publicar na loja | Misturar lojas; versionar; diagnosticar de verdade |
| **Launcher independente** | Prism, ATLauncher, XMCL, GDLauncher, HMCL, Scope | Instâncias, duas lojas, Java automático, exportar em vários formatos | Separar projeto de teste; versionar; publicar com atualização |
| **Ferramenta declarativa** (texto + Git) | packwiz, Pakku, ferium, unsup | Pack como texto versionado, atualização automática dos jogadores, CI | Testar, diagnosticar, interface |
| **Interface para packwiz** | PW-GUI, Pinecone Pack Manager, Packwiz Studio, packwiz-web, os protótipos do dono | Comandos do packwiz com botões | Testar, diagnosticar; quase todos são projetos de uma pessoa, com poucas estrelas |
| **Ferramenta de servidor** | ServerPackCreator, mrpack-install, imagem Docker do itzg, painéis (Pterodactyl), Modrinth Hosting | Montar e instalar o pack no servidor | Criar o pack |
| **Diagnóstico** | Crash Assistant, Not Enough Crashes, mclo.gs, MCDoctor.ai, Modpack Debug Kit, emendator, Modpack Inspector Tool | Explicar um travamento; achar o culpado (manual ou semi); mostrar mixins | Saber o que é o pack, o que mudou, qual versão |

**Tendências de 2025–2026** **[fonte, inferência na síntese]**:
- **As lojas vão para o "jogar com amigos"**: instâncias compartilhadas do Modrinth (24/07/2026), projetos de servidor (03/2026), Modrinth Hosting dentro do app (04/2026); código de perfil por 7 dias e projetos não listados na CurseForge (04/2026). É o mesmo problema que o Warden resolve com GitHub + bootstrap, só que dentro de cada loja.
- **Consolidação**: o Modrinth foi comprado pela Spark Universe (dona do mod Essential) em 02/2026; o GDLauncher fez parceria com a CurseForge e virou "código público, mas não open source".
- **Enxurrada de IA**: o Modrinth passou de 2.500 para mais de 5.000 envios por semana feitos com IA e criou regras de declaração obrigatória (vigentes em 27/09/2026); a comunidade hostiliza ferramentas "vibecodadas" (CraftPacker, MCED) **[fonte]**.
- **Malware continua**: casos de mods falsos fora das lojas (2025–2026), um "modpack" que roubou senhas (r/CurseForge, 08/2026) e uma biblioteca com 1 milhão de downloads que vasculhava o disco do jogador sem documentar (2.367 votos, 08/2026) **[verificado no Reddit]**.

## 3. Fichas dos concorrentes

Formato: o que faz em cada etapa (criar, editar, testar, diagnosticar, publicar); plataformas e versões; licença e modelo de negócio; maturidade; fortes; fracos. Números de 04/10/2026.

### 3.1 App da CurseForge (Overwolf)

| Item | Achado |
|---|---|
| Criar e editar | "Create" com nome, versão e loader; mods só da CurseForge; aba de conteúdo **[fonte: suporte CF]**. Desde 06/2026, **Java por modpack** que viaja na exportação **[fonte: newsletter 26/06/2026]**; desde 07/2026, registro das mudanças de conteúdo e das tentativas de abrir o jogo na pasta de logs da instância **[fonte: notas 1.314]**. |
| Testar | Abre pelo launcher oficial ou próprio; slider de memória sem campo de texto (reclamação com 212 votos) **[verificado no Reddit]**. |
| Diagnosticar | Nada além do registro de mudanças **[inferência por ausência nas notas]**. |
| Publicar | "Share Profile": zip ou código válido por 7 dias **[fonte]**; publicação no site com revisão; pacote para servidor **manual**, e o próprio blog recomenda o ServerPackCreator **[fonte: blog 22/05/2025]**. |
| Plataformas | Windows, macOS, Linux; versão "standalone" sem Overwolf. |
| Licença e negócio | Fechado; anúncios no app e assinatura para tirá-los (≈ US$ 2,50/mês) **[verificado no Reddit]**; programa de recompensas 70/30 para autores **[fonte secundária]**. |
| Maturidade | Muito alta; o maior volume de packs do mercado. |
| Fortes | Catálogo, alcance, Java por pack, prévia do conteúdo antes de instalar (06/2026). |
| Fracos | Só uma loja; anúncios ocupando até um terço da tela (415 votos); Overwolf pesado (100 GB de dados num mês, 1.771 votos); pedidos básicos de criador sem resposta (copiar lista de mods, 17 votos; "fork" de modpack, 20 votos) **[verificado]**. |

### 3.2 App do Modrinth (Modrinth, agora da Spark Universe)

| Item | Achado |
|---|---|
| Criar e editar | Instância com conteúdo do Modrinth; em 03/2026 a aba de conteúdo virou tabela com ações em lote, dependências em vários níveis, modal de atualização com changelog e troca de versão do jogo **[fonte: notícia 17/03/2026]**. |
| Testar | Abre o jogo; Java Azul Zulu automático (R2 §2.4). |
| Diagnosticar | Console do servidor com detecção de travamento no Modrinth Hosting **[fonte]**; no cliente, nada específico **[inferência]**. |
| Publicar | Exporta `.mrpack` escolhendo arquivos, com o lado de cada mod tirado do metadado do Modrinth **[código: `export_mrpack.rs`]**; **instâncias compartilhadas** (o dono empurra mudanças, os amigos recebem ao abrir; sem revisão) **[fonte: 24/07/2026]**; publicar no site passa pela revisão (mais de 3.000 modpacks na fila em 06/2026; "3 semanas esperando" em 08/2026) **[fonte, verificado no Reddit]**. Pacote para servidor: pedido aberto (#535) **[verificado]**. |
| Plataformas | Windows, macOS, Linux (megaissue de Linux com 101 votos). |
| Licença e negócio | Código aberto (monorepo `modrinth/code`, 2.411 estrelas); anúncios com 75% para criadores; Modrinth+ e Modrinth Hosting pagos. |
| Maturidade | Alta; release a cada poucos dias (v0.21.6, 27/09/2026) **[verificado]**. |
| Fortes | Instâncias compartilhadas, conteúdo em lote, formato `.mrpack` aberto, environment novo dos mods. |
| Fracos | Só Modrinth; comunidade reclama de regressões e de avisos condescendentes ("parece que me trata como quem não sabe nada"); medo de piora depois da compra pela Spark (975 votos, 364 comentários) **[verificado no Reddit]**. |

### 3.3 Prism Launcher

| Item | Achado |
|---|---|
| Criar e editar | Instâncias com Modrinth, CurseForge, FTB, Technic e ATLauncher; rastreio de dependências na página de mods e aviso ao desligar mod exigido por outro (11.0, 09/04/2026) **[fonte]**; destaque de recursos incompatíveis com a versão (11.0) **[fonte]**. |
| Testar | Java automático (9.0); aviso de RAM insuficiente ao abrir (11.0); console colorido e leitura de eventos XML do log4j (10.0) **[fonte]**. |
| Diagnosticar | Envio de log ao mclo.gs; sem bisseção: o PR #5855 foi fechado sem merge em 22/09/2026 e a issue #1924 segue aberta **[verificado]**. Aviso de mods não confiáveis embutidos em modpacks importados (11.1, 03/09/2026) **[fonte]**. |
| Publicar | Exporta `.mrpack` e zip da CurseForge com `packignore` (10.0) e **lista de mods** (`ExportToModList`) **[código]**; não publica nem atualiza jogadores (o pedido de packwiz embutido tem 50 votos, #591) **[verificado]**. |
| Plataformas | Windows, macOS, Linux (inclusive ARM, Flatpak, Steam Deck). |
| Licença e negócio | GPL-3.0; doações; sem anúncios. 10.586 estrelas, 759 issues abertas, release 11.1.1 em 28/09/2026 **[verificado]**. |
| Fortes | A escolha da comunidade ("It Just Works", leve, logs fáceis); todas as lojas; formatos de exportação. |
| Fracos | Edita a instância ao vivo (projeto = teste); sem versionamento, sem diagnóstico de verdade, sem grupos de mods (pedido mais votado, 66) **[verificado]**. |

### 3.4 ATLauncher

| Item | Achado |
|---|---|
| O que faz | Instâncias, packs próprios da plataforma ATLauncher, CurseForge, Modrinth, FTB, Technic; exporta em **CurseForge, Modrinth, os dois ao mesmo tempo e MultiMC** **[código: `InstanceExportFormat`]**; converte pack da CurseForge em Modrinth usando os arquivos equivalentes **[fonte]**; reconhece poucos erros conhecidos (falta de memória, Java errado) **[código: `MinecraftError`]**. |
| Licença e números | GPL-3.0; 852 estrelas; v3.4.41.3 em 19/09/2026 **[verificado]**. |
| Fortes | Exportação dupla; preferido por alguns para packs pesados. |
| Fracos | Pedido mais votado é trocar a versão do Minecraft de uma instância (#479) **[verificado]**; sem diagnóstico, sem versionamento. |

### 3.5 X Minecraft Launcher (XMCL)

| Item | Achado |
|---|---|
| O que faz | Instâncias, Modrinth e CurseForge, importação e exportação dos dois formatos, **links simbólicos e físicos** para não duplicar mods no disco, multiplayer P2P **[fonte: README]**; **abre servidor local** da instância (`instanceServerLaunch`) **[código]**; visualizadores de progresso, conquistas e quests dos mundos (0.71.0, 03/10/2026) **[fonte]**. |
| **IA** | Assistente com ferramentas: listar, ler e editar configs da instância, remover mods, editar a instância, diagnóstico de compatibilidade de mods (`modDiagnosis`) e um agente que escreve **versão e changelog do modpack** (`modpackChangelogAgent`); pede confirmação antes de ações (`confirm.ts`); provedor próprio (`ai.xmcl.app`, modelo `xmcl-agent`, com assinatura) ou endpoint do usuário; ligado para todos em 23/08/2026 **[código]**. |
| Licença e números | MIT; 1.632 estrelas; release a cada 1–2 semanas (v0.71.0) **[verificado]**. |
| Fortes | O mais rápido em funções novas; IA com ferramentas; servidor local. |
| Fracos | IA sem evidência citada nem diferença exata antes de aplicar **[inferência pelo código lido]**; foco em jogador; interface com "glassmorphism" e cara de IA (notas da 0.71.0) **[fonte]**. |

### 3.6 GDLauncher (Carbon)

| Item | Achado |
|---|---|
| O que faz | Rust + SolidJS; CurseForge e Modrinth; Java automático; formato próprio `.gdlpack` que mistura as duas lojas e compartilhamento por nuvem com código **[fonte: README; resposta do fundador no Reddit]**. |
| Licença e números | O antigo (GPL-3.0) foi arquivado; o Carbon é "código público, não open source" (licença NOASSERTION), alfa distribuída pelo Discord, parceria com a CurseForge **[verificado]**. 241 estrelas. |
| Fracos | Comunidade acusa "enshittification" (53 votos no comentário) **[verificado no Reddit]**; formato próprio não funciona em outros launchers. |

### 3.7 FTB App (Feed The Beast)

| Item | Achado |
|---|---|
| O que faz | Packs da FTB e da CurseForge; seleção múltipla de packs (1.29.0); botão "Play again" depois de travar; exporta instância (com logs, que viraram obrigatórios na exportação) **[fonte: notas 1.29.x]**; ajustou-se à chave do CDN da CurseForge (1.30.0, 26/06/2026) **[fonte]**. |
| Licença e números | LGPL-2.1 (frontend); 128 estrelas; v1.30.0 **[verificado]**. |
| Fracos | "Adware" segundo a comunidade (42 votos no comentário) **[verificado no Reddit]**; feito para jogar os packs da FTB. |

### 3.8 Technic Launcher e Technic Solder

| Item | Achado |
|---|---|
| O que faz | O Solder é uma API PHP (Laravel) que guarda cada mod separado, para o launcher baixar só o que mudou; integra com a Technic Platform **[fonte: README]**. Voltou a ter atividade forte em 2026: v1.1 a v1.5 entre 04 e 09/2026, com Java por build e packs privados **[verificado]**. |
| Licença e números | Solder MIT (181 estrelas); LauncherV3 GPL-3.0, release 25/09/2026 **[verificado]**. |
| Leitura | Plataforma antiga, ainda usada por packs clássicos; o Solder é, na prática, o "GitHub + bootstrap" da Technic, com servidor próprio **[inferência]**. |

### 3.9 HMCL

Launcher muito popular na China (10.178 estrelas, GPL-3.0); analisador com ~60 regras de crash (R2 §6.4); prerreleases semanais **[verificado]**. Fora do público do Warden; vale como referência de regras (só leitura, licença).

### 3.10 Scope Launcher (novo, 07/2026)

Electron + React, MIT, 12 estrelas, v2.0.4. Promete "corretor de crash com 1 clique", "bisseção automática", inspeção de manifestos dos jars, "flight recorder" de desempenho com exportação CSV e migração de 8 launchers **[fonte: README]**. Não verifiquei se a bisseção funciona; é projeto de uma pessoa, recém-criado **[inferência]**. Mostra para onde o mercado vai: as mesmas ideias do Warden em versão simples.

### 3.11 packwiz (CLI)

| Item | Achado |
|---|---|
| O que faz | Pack em TOML, git-friendly, Modrinth e CurseForge, exportação, `packwiz-installer` para atualizar jogadores e servidores (R3). |
| Números | MIT; 1.017 estrelas; **sem releases versionadas** (issue #403 pede) e último push em 06/09/2026; 163 issues abertas **[verificado]**. |
| Pedidos mais votados | Compatibilidade com o Sinytra Connector (20), lado para arquivos internos (13), usar CurseForge e Modrinth para o mesmo mod (13), remover dependências que sobraram (8), pack com vários loaders (8), publicar direto nas lojas (7), gerar instância do MultiMC (7), lista de mods (5), variantes do pack (5) **[verificado]**. |
| packwiz-installer | Último commit 27/06/2024; baixa os arquivos da CurseForge **sem mandar chave** (só a chamada à API usa a chave embutida) **[código: `CurseForgeSourcer.kt`]**. Ver §8.3, risco 1. |

### 3.12 Pakku

Kotlin, EUPL-1.2, 164 estrelas, v1.5.0 (18/07/2026). "Gerenciador de pacotes de modpack" inspirado em npm e Cargo: `add` com dependências, `rm` com checagem, `update` em lote, `diff` entre versões, gerador de changelog, `export` para CurseForge e Modrinth **ao mesmo tempo**, `remote` para instalar a partir de um Git **[fonte: README]**. Pedidos abertos: RAM recomendada do pack, scripts, **migrar para versão nova do Minecraft**, **comando `bisect`**, **publicar** **[verificado]**. Usado por 3 dos 200 packs mais baixados do Modrinth **[verificado]**. É o "packwiz moderno"; sem interface.

### 3.13 ferium

Rust, MPL-2.0, 1.423 estrelas; CLI para **jogador** baixar e atualizar mods de Modrinth, CurseForge e GitHub Releases; último release em 09/2024 **[verificado]**. Não cria packs distribuíveis.

### 3.14 mrpack-install

Go, MIT, 328 estrelas; instala servidor + `.mrpack` com uma linha, inclusive opcionais por expressão regular; ainda em beta (v0.21.0-beta, 06/2025) **[verificado]**.

### 3.15 unsup

Atualizador de diretório em Java (agente), que lê `pack.toml` do packwiz e acrescenta **variantes** ("flavors") e ambientes; saiu do GitHub para `git.sleeping.town`; o próprio autor diz que o "Creator GUI está parado porque o packwiz virou o padrão" **[fonte]**. Testado de b1.7.3 a 1.21.5.

### 3.16 Interfaces para packwiz (2025–2026)

| Projeto | Estado | O que tem | Observação |
|---|---|---|---|
| **PW-GUI** (AmberIsFrozen) | Java, MIT, 25 estrelas, criado 01/2025, push 07/2026 | Criar, importar da CurseForge, exportar `.mrpack`/zip, editar arquivos, `preserve`, opcional, fixar, lado, download manual da CurseForge, prévia do `.gitignore`/`.packwizignore` **[fonte]** | A GUI de packwiz mais ativa; pede Java 17 |
| **Pinecone Pack Manager** | TypeScript/Electron, 0 estrelas, 07/2026 | packwiz embutido, Modrinth e CurseForge por link, **deploy por SFTP** para o servidor, commit/push automático **[fonte]** | Mostra o pedido de "mandar para o servidor" |
| **Packwiz Studio** | Go, GPL-3.0, 08/2026 | Só Modrinth | Recém-criado |
| **packwiz-web** | Go, MIT, ativo (push 04/10/2026) | Serviço web para gerir e hospedar packs packwiz | Para grupos |
| **modpack-helper**, **PackVulcan**, **packwiz-gui** (2022) | Parados | — | Cemitério de GUIs |

**Leitura [inferência]:** a demanda por "packwiz com interface" é real e recorrente (onze projetos achados pela busca), mas ninguém passou de projeto de fim de semana. O Warden entra nesse espaço com muito mais escopo.

### 3.17 Ferramentas de servidor

| Ferramenta | Achado |
|---|---|
| **ServerPackCreator** (Griefed) | LGPL-2.1, 714 estrelas, ativo (03/10/2026); gera pacote para servidor de qualquer pack Forge, Fabric, Quilt, LegacyFabric e NeoForge; CLI, GUI e web; aviso em letras grandes: "**TESTE SEUS PACOTES PARA SERVIDOR ANTES DE PUBLICAR**" **[fonte]**. É o que a CurseForge e criadores recomendam (comentário no r/feedthebeast) **[verificado]**. |
| **itzg/docker-minecraft-server** | Apache-2.0, 14.377 estrelas; instala versão, loader e **modpacks** (inclusive packwiz por `PACKWIZ_URL`) na inicialização **[fonte]**; é a base do emendator. |
| **Pterodactyl** | 9.287 estrelas; painel de hospedagem; o MCDoctor tem extensão para ele (R5A §1.1). |
| **Modrinth Hosting** | Servidor pago dentro do app (04/2026), com projetos de servidor que instalam o pack do jogador em um clique (03/2026) **[fonte]**. |

### 3.18 Diagnóstico

| Ferramenta | Achado |
|---|---|
| **Crash Assistant** | 19,7 milhões de downloads no Modrinth; janela pós-crash; compara a lista de mods com a do pack; licença com não concorrência (R5A §1.2) **[verificado]**. Já é mod inicial do Warden (D16). |
| **Not Enough Crashes** | MIT; 15,4 milhões de downloads; ativo (28/09/2026) **[verificado]**. |
| **mclo.gs / codex-minecraft** | MIT; v2.3.3 (06/2026), agora "para Minecraft e Hytale" **[verificado]**; base das regras do Warden (ADR-0014). |
| **MCDoctor.ai** | IA na nuvem, paga; o mod tem só 2.123 downloads no Modrinth **[verificado]**; ficha em R5A §1.1. |
| **Outros analisadores com IA** | MC Crash Reader, "Minecraft Log Reader", GPTs e o analisador do dedicatedminecraft.host **[fonte]**: todos leem um log colado, sem saber do pack. |
| **Modpack Debug Kit** (Python) | "Busca binária" com instantâneos da pasta `mods`, foco nos mods novos e lista de dependências (do Modrinth); 775 votos e um comentário "seria incrível integrar ao Prism" **[verificado no Reddit, 03/11/2025 e 19/06/2026]**. Não abre o jogo sozinho **[inferência pelo post]**. |
| **emendator** | Tauri/React + FastAPI, GPL-3.0, alfa; análise estática, servidor sem tela em Docker, bisseção automática, gera config do AlmostUnified e datapack de receitas; só Fabric e alvo 1.21.1 **[fonte: README]**. |
| **Modpack Inspector Tool** | Grafo de dependências, bibliotecas que sobraram e **análise de mixins** com sobreposição entre mods; 727 votos; "bastante preciso num pack Forge 1.20.1" **[verificado no Reddit, 22/03/2026]**. Concorre diretamente com o raio-x (T07) e o grafo (T06), como ferramenta solta. |
| **Modpack visualization tool** (ModpackGraph) | Grafo de dependências a partir dos manifestos; 300 votos; pedido de "análise de mixins" nos comentários **[verificado no Reddit]**. |
| **CraftPacker** | Busca em lote, dependências, compatibilidade pelo arquivo de mixin e pela API, exportação de pacote para servidor; recebido com hostilidade por falhas de segurança (sem HTTPS de verdade, sem o user-agent exigido pelo Modrinth) e texto feito por IA **[verificado no Reddit, 08/05/2026]**. |
| **mrpack-updater** | Site que tenta migrar um `.mrpack` para outra versão do Minecraft e avisa quais mods faltam **[verificado no Reddit, 07/11/2025]**. |
| **MCED** | Editor de configs com formulário para qualquer launcher (MIT, 5 estrelas) **[verificado]**. |
| **Modrinth Extras** | Extensão de navegador com explorador de dependências em vários níveis (132 votos); pedido nos comentários: "quero ver quem depende deste mod" **[verificado no Reddit]**. |

## 4. A voz dos criadores: dores e pedidos

Frequência: **Alta** = aparece nas três fontes (Reddit, issues, portais de ideias) com centenas ou milhares de votos; **Média** = duas fontes ou dezenas de votos; **Baixa** = casos isolados. O que o Warden já planeja está na última coluna.

| # | Dor ou pedido | Freq. | Evidências (resumo) | No Warden |
|---|---|---|---|---|
| V1 | **Achar o mod que trava** quando o log não ajuda; a "busca binária" manual é lenta, quebra por dependências e "às vezes o problema é uma dupla de mods" | Alta | "Is the Binary Search just bad?" (0 votos, mas a lista de problemas é a do Warden); "How do YOU binary search?" (dependências fazem "virar um inferno"); "Man making a modpack can be stressful" ("um chefe específico trava, sozinho o mod funciona"); Modpack Debug Kit (775); Prism #1924 (17) e PR fechado; Pakku #63 | **Sim**: T25 busca do culpado com dependências e pares (P1) |
| V2 | **Logs ilegíveis**; pedir ajuda à IA aparece como saída aceita, mas com medo de alucinação | Alta | "pass it to Gemini or something" (39) e "LLMs alucinam, só use se souber conferir" (5) no mesmo fio | **Sim**: T14 com regras + IA com evidência conferida |
| V3 | **Minérios e itens repetidos**, receitas em conflito, integração entre mods | Alta | Meme do criador de pack (5.610 votos, 171 comentários; resposta mais votada recomenda AlmostUnified + biblioteca de materiais); "Advice for newbie modpack creator"; "slop modpack" (882) | **Sim (1.1)**: T32; conflitos de receita **não** |
| V4 | **Pacote para servidor** é manual e confuso | Alta | "How can I make a server pack for my own modpack?" (65); blog da CurseForge manda montar à mão; Modrinth #535 (11), Prism #864 (37), XMCL #1068; ServerPackCreator existe por isso | **Sim**: T19 (P1) com teste no servidor local |
| V5 | **Organizar a lista de mods** em grupos/etiquetas/pastas | Alta | Prism #1055 (66, o mais votado do Prism), #154 (42), #880 (29); Modrinth #4234 | **Sim (1.1)**: T31 |
| V6 | **Dependências que sobraram** depois de remover um mod; saber "quem depende disto" | Média | packwiz #97 (8), Modrinth #3143 (8); comentário no Modrinth Extras; CurseForge passou a mostrar "dependentes" (04/2026, 574 votos) | **Sim**: T06/T07 (P1) |
| V7 | **Distribuir para amigos** com atualização automática, sem esperar revisão | Alta | Instâncias compartilhadas do Modrinth (420 votos; "chega de mandar mrpack por e-mail"); "hybrid curseforge+modrinth modpack?"; "Packwiz on a server, how do you do it?"; CurseForge "Better share modpack" (9) e "share with friends" (25) | **Sim**: T18 (GitHub + bootstrap); **mas** o jogador precisa de 4 passos (lacuna L1) |
| V8 | **Revisão demorada e regras de permissão** nas lojas | Média | Modrinth: mais de 3.000 modpacks na fila (06/2026); "3 semanas esperando" (08/2026) | Não se aplica (Warden publica no GitHub) |
| V9 | **Licença e permissão de mods** | Média | "300 horas curando um modpack que nunca vai poder ser lançado" (r/Minecraft, 685 votos); Modrinth exige prova de permissão antes de enviar (06/2026) | **Parcial**: aviso de licença ao importar e ao embutir (T19, T24) |
| V10 | **Launchers pesados, com anúncio e coleta** | Alta | Overwolf 100 GB/mês (1.771); um terço da tela com anúncio (415); pop-up no meio do jogo pedindo o rosto (399); FTB "adware" | **Sim, por princípio**: sem anúncio, sem telemetria, local |
| V11 | **Launcher que "só funciona"**: leve, rápido, logs fáceis | Alta | Prism quase unânime em "What is YOUR favorite launcher" (81), "What is the best Mod pack launcher" (35) e "An actual good launcher?" (76) | Meta de desempenho na SPEC §8 |
| V12 | **App que trata o usuário como leigo** demais, ou muda a interface para pior | Média | "Is it only me or the modrinth app is getting worse" (47): atualizar 40 de 55 mods exige clicar um por um; avisos condescendentes | Cuidado de UX: avisos curtos, seleção em lote (T06, T10) |
| V13 | **Memória e Java por pack**, com valor exato | Média | Slider da CurseForge (212); Modrinth #4512 e #4664; "add a java argument permanently for a modpack"; Pakku #138 | **Parcial**: perfis do teste locais (T11); jogadores não recebem (lacuna L1 e L4) |
| V14 | **Atualizar mods quebra** o pack ou o mundo; migrar de versão do Minecraft | Média | "I opened my 2-year-old 400+ mod modpack…" (3.066); ATLauncher #479 (8); Pakku #78; Modrinth #4905; mrpack-updater | **Parcial**: pontos de segurança, changelog; migração é P2 (lacuna L3) |
| V15 | **Malware e mods suspeitos** | Alta | fractureiser (2023), PSA "architecturyapi.com" (388, 05/2026), "compromise" de modpack (08/2026), biblioteca que vasculha o disco (2.367, 08/2026) | **Sim (1.1)**: T28 |
| V16 | **Desconfiança de ferramentas e textos feitos por IA** | Média | CraftPacker e MCED ("vejo descrição de ChatGPT, fecho"); pack com texto "de IA" é cobrado nos comentários; Modrinth cria regras | Coerente com o pedido do dono ("nada com cara de IA") |
| V17 | **Lista de mods** para mostrar aos jogadores | Média | packwiz #13 (5); CurseForge "Copy list of mods" (17); Modrinth #1865 (6) | **Não** (lacuna L2) |
| V18 | **Configs**: editar sem abrir TOML/JSON na mão; padrões que não sobrescrevem o jogador | Média | MCED; Prism #380 "Default Options File" (15) e #1077 (17) | **Sim**: T12 formulário, `preserve` do `options.txt` |
| V19 | **Variantes** do mesmo pack (criativo e sobrevivência, vários loaders) | Baixa | packwiz #163 (5), #369 (8); unsup "flavors" | Fora (P2, §9 da SPEC) |
| V20 | **Publicar direto nas lojas** pela ferramenta | Baixa | packwiz #28 (7), #72 (3); Modrinth #469 (3); Pakku #53; CurseForge "Create a Modpack in App" (16) | Fora (§9) |

**Citações curtas que resumem** (traduzidas; links na §10):
- "Tenho um respeito enorme por quem faz modpack… às vezes não conecta, às vezes trava, e entrar no buraco para consertar os bugs está me tirando o sono." (34 anos, 10 de TI; "Making a Modpack is tough…", 177 votos)
- "Não curta montar? Então não monte: coloque 2 a 4 mods, teste, e quando travar isole um por um." (conselho mais votado a um iniciante, r/feedthebeast, 03/2026)
- "Eu fiz isso porque meu pack de Create travou e a mensagem de erro não me dizia nada." (autor do Modpack Debug Kit)
- "Servidor é só tirar o que não precisa… cada versão é diferente, então ache outro pack da mesma versão e copie o que eles mudaram; depois rode para ver se funciona." (resposta mais votada, 47)

## 5. Mercado em números

### 5.1 Tamanho

| Indicador | Valor | Data | Marca |
|---|---|---|---|
| Projetos no Modrinth | **161.507** (mods 76.723; resource packs 36.146; plugins 18.869; **modpacks 18.649**; datapacks 15.085; shaders 966) | 04/10/2026 | [verificado: `/v2/statistics` e busca] |
| Autores no Modrinth | 69.696 | 04/10/2026 | [verificado] |
| Versões publicadas no Modrinth | 1.452.994 | 04/10/2026 | [verificado] |
| Modpacks na CurseForge | **≥ 104.741** (piso: soma por versão; cinco combinações versão+loader batem no teto de 10.000 da API) | 04/10/2026 | [verificado] |
| Downloads de mods de Minecraft na CurseForge | "perto de 100 bilhões" | 12/01/2026 | [fonte: post no r/feedthebeast, 2.003 votos] |
| Overwolf (dona da CurseForge) | 178 mil criadores, 113 milhões de usuários mensais, US$ 300 milhões pagos a criadores em 2025 (todos os jogos e apps) | 09/06/2026 | [fonte secundária: games.gg] |
| Modrinth: envios por dia | mais de 600 projetos/dia; mais de 3.000 modpacks esperando revisão | 23/06/2026 | [fonte] |
| Modrinth: envios feitos com IA | de 2.500 para mais de 5.000 por semana | 13/08/2026 | [fonte] |
| `pack.toml` do packwiz públicos no GitHub | ≈ 2.520 | 04/10/2026 | [verificado, contagem aproximada da busca de código] |

### 5.2 Crescimento do Modrinth (modpacks que existem hoje, pelo ano de criação)

| Ano | Modpacks criados | Marca |
|---|---|---|
| 2022 | 279 | [verificado] |
| 2023 | 1.786 | [verificado] |
| 2024 | 4.823 | [verificado] |
| 2025 | 6.607 | [verificado] |
| 2026 (até 04/10) | 5.154 (ritmo de ≈ 6.800 no ano) | [verificado; projeção = inferência] |

O Modrinth diz ter **dobrado o número de projetos em 2025** e crescido de 6 para 17 pessoas **[fonte: post de 31/12/2025]**. O crescimento de modpacks desacelerou em 2026 (ritmo parecido com 2025) **[inferência]**.

### 5.3 Loaders e versões

**Modrinth, modpacks por loader** (um pack pode ter mais de um): Fabric 13.322 (71%), Forge 3.825 (21%), NeoForge 2.503 (13%), Quilt 686 (4%) **[verificado]**.
**Modrinth, 200 modpacks mais baixados:** Fabric 138, Forge 66, NeoForge 31, Quilt 12 **[verificado]**.

**Modpacks por versão do Minecraft (as que importam):**

| Versão | Modrinth | CurseForge (total; Forge / Fabric / NeoForge) |
|---|---|---|
| 1.7.10 | 66 | 5.260 (5.258 / 5 / 3) |
| 1.12.2 | 194 | ≥ 10.000 (≥ 10.000 / 22 / 11) |
| 1.16.5 | 296 | ≥ 10.000 (≥ 10.000 / 504 / 5) |
| 1.18.2 | 437 | 9.551 (8.650 / 1.221 / 3) |
| 1.19.2 | 1.068 | ≥ 10.000 (≥ 10.000 / 2.388 / 11) |
| **1.20.1** | **7.329** | ≥ 10.000 (**≥ 10.000** / 4.616 / 288) |
| **1.21.1** | **5.013** | ≥ 10.000 (715 / 2.942 / **≥ 10.000**) |
| 1.21.11 | 2.881 | 1.359 (96 / 1.018 / 300) |
| 26.x (26.1 a 26.3) | 2.187 (Fabric 2.148) | 2.001 (157 / 1.471 / 466; soma por versão, pode repetir um pack) |

Todos [verificado], 04/10/2026. **Leituras [inferência]:** (1) 1.20.1 e 1.21.1 concentram os packs nas duas lojas; (2) na CurseForge, o NeoForge tomou a 1.21.1 do Forge; (3) nas versões 26.x o Fabric domina (98% no Modrinth, 74% na CurseForge); (4) **1.7.10 e 1.12.2 vivem na CurseForge**: quem quiser essas versões depende da chave da CurseForge (o Warden já trata isso na T03).

### 5.4 Git e packwiz entre os criadores

| Recorte | Resultado | Marca |
|---|---|---|
| 200 modpacks mais baixados do Modrinth que informam repositório | 30 (27 no GitHub) | [verificado] |
| Desses, usam packwiz | ≥ 10 (Fabulously Optimized, Sodium Plus, Skyblocker, OneClient, Additive, Adrenaline, uku's pvp, Minehattan, Gensokyo QoL, Technical Electrical) | [verificado] |
| Usam Pakku | 3 (SkyBlock Enhanced, Thunder, Society: Sunlit Valley) | [verificado] |
| Licença dos 200 | 114 "All Rights Reserved", 49 MIT | [verificado] |

**Leitura [inferência]:** packwiz é minoria (≈ 5% do topo), mas é quase todo o mundo "pack como código". Os packs que usam são os mais técnicos e mais mantidos (Fabulously Optimized é o nº 1 em downloads). O Warden mira exatamente esse jeito de trabalhar, com interface para quem não quer CLI.

### 5.5 Como os criadores ganham dinheiro

| Programa | Como funciona | Marca |
|---|---|---|
| **Modrinth Rewards** | 75% da receita de anúncios vai para criadores, por visualização de página e download no app; downloads como dependência de modpack contam igual; pagamento 60 dias depois do mês; mais de **US$ 1,6 milhão pagos em 2025**; recorde de US$ 227 mil de receita e US$ 170 mil pagos em 06/2025; em 09/2024 o total acumulado era US$ 160.868 | [fonte] |
| **CurseForge Author Rewards** | Pontos por download, num fundo mensal tirado da receita do mês anterior; 70% para autores; ≈ US$ 0,05 por ponto | [fonte: FAQ da CF; fonte secundária para o valor do ponto] |
| **Servidores** | Projetos de servidor do Modrinth não recebem pagamento, mas o modpack ligado recebe | [fonte] |
| **Patrocínio de hospedagem** | Muitos mods e packs têm faixa da BisectHosting e afins na página (conversa "Why do so many mods have a bisect hosting segment?") | [verificado no Reddit] |
| **Patreon e encomendas** | Criadores de packs grandes recebem por Patreon; há quem encomende pack ("Commission a modpack creator?") | [verificado no Reddit] |

Para o Warden (uso pessoal), o dinheiro só importa para entender um efeito: **as lojas ganham quando o download passa por elas**, por isso a CurseForge bloqueia apps de terceiros para alguns mods e agora quer chave até no CDN (§8.3) **[inferência]**.

## 6. Matriz de comparação

Legenda: **S** sim · **P** parcial · **N** não · **—** não se aplica · **?** não verificado.
Colunas: **W1** Warden v1 · **W1.1** Warden 1.1 · **CF** app da CurseForge · **MR** app do Modrinth · **Pr** Prism · **AT** ATLauncher · **XM** XMCL · **FTB** FTB App · **pw** packwiz · **Pk** Pakku · **PW** PW-GUI · **SPC** ServerPackCreator · **CA** Crash Assistant · **MD** MCDoctor.ai.
Base de cada linha na última coluna (fichas da §3; telas da SPEC para o Warden).

### 6.1 Criar

| Função | W1 | W1.1 | CF | MR | Pr | AT | XM | FTB | pw | Pk | PW | SPC | CA | MD | Base |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Criar pack escolhendo versão e loader | S | S | S | S | S | S | S | S | S | S | S | — | — | — | T03; fichas |
| 1.7.10 até a mais nova, com Java certo | S | S | S | S | S | S | ? | P | P | ? | P | — | — | — | T03, T11; R3 §1.5.1 (packwiz init falha em Forge antigo) |
| Busca nas duas lojas num resultado só, sem duplicar | S | S | N | N | P | P | P | N | N | P | P | — | — | — | T08; Prism e AT têm lojas separadas [inferência] |
| Mods das duas lojas no mesmo pack | S | S | N | N | S | P | P | P | S | S | S | — | — | — | ADR-0027; fichas |
| Dependências resolvidas ao adicionar | S | S | S | S | S | ? | ? | ? | S | S | S | — | — | — | T09; notas MR 03/2026; Prism 11.0 |
| Importar pack de outros apps | S | S | S | S | S | S | S | P | P | S | P | — | — | — | T24; fichas; packwiz #155 |
| Ver os mods de outro modpack e trazer para o seu | S | S | P | P | N | N | N | N | N | N | N | — | — | — | T08 (P1); CF e MR só prévia |
| Mods iniciais e kits de desempenho | S | S | N | N | N | N | N | N | N | N | N | — | — | — | T03 |

### 6.2 Editar

| Função | W1 | W1.1 | CF | MR | Pr | AT | XM | FTB | pw | Pk | PW | SPC | CA | MD | Base |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Lado, fixar versão e opcionais | S | S | N | P | P | ? | ? | N | S | P | S | — | — | — | T06, T11; README PW-GUI |
| Notas e grupos de mods | N | S | N | N | N | N | P | N | N | N | N | — | — | — | T31; Prism #1055; XMCL tem grupos [inferência pela issue #1739] |
| Grafo de dependências e "quem depende" | S | S | P | N | P | N | N | N | N | N | N | — | — | — | T06; CF mostra dependentes no site (04/2026) |
| Remover dependências que sobraram | S | S | N | N | P | N | ? | N | N | P | N | — | — | — | T06 (P1); packwiz #97; MR #3143 |
| Configs com formulário, padrão e faixa | S | S | N | N | N | N | P | N | N | N | P | — | — | — | T12; IA do XMCL edita configs |
| Buscar em todas as configs | S | S | N | N | N | N | N | N | N | N | N | — | — | — | T12 |
| Editor de scripts KubeJS/CraftTweaker | S | S | N | N | N | N | N | N | N | N | N | — | — | — | T26 |
| Atualizar em lote com changelog | S | S | P | S | S | P | ? | ? | P | S | P | — | — | — | T10; fichas |

### 6.3 Testar

| Função | W1 | W1.1 | CF | MR | Pr | AT | XM | FTB | pw | Pk | PW | SPC | CA | MD | Base |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Abrir o jogo com um clique | S | S | S | S | S | S | S | S | N | N | N | — | — | — | T13 |
| Projeto separado do teste, com "o que mudou durante o teste" | S | S | N | N | N | N | N | N | — | P | N | — | — | — | T15; Pakku `sync` |
| Perfis do teste e entrar direto no mundo | S | S | P | P | S | P | ? | P | — | — | — | — | — | — | T11; Prism 10.0 (Quick Play) |
| Servidor local sob demanda | S | S | N | P | N | N | S | N | — | — | — | P | — | — | T27; XMCL `instanceServerLaunch`; MR só hospedagem paga |
| Testar "como o jogador recebe" | S | S | N | N | N | N | N | N | — | — | — | — | — | — | T13 |
| Desempenho por teste e entre versões | P | S | N | N | P | N | N | N | — | — | — | — | — | — | T13, T33; Prism mostra RAM ao abrir |

### 6.4 Diagnosticar

| Função | W1 | W1.1 | CF | MR | Pr | AT | XM | FTB | pw | Pk | PW | SPC | CA | MD | Base |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Verificar o pack antes de abrir | S | S | N | P | P | P | P | P | N | P | N | — | — | — | T14; Prism 11.0; XMCL `modDiagnosis` |
| Explicar o travamento com a evidência | S | S | N | P | P | P | P | N | — | — | — | — | S | S | T14; fichas |
| Histórico de travamentos agrupado pela causa | S | S | N | N | N | N | N | N | — | — | — | — | N | N | T14 |
| Busca automática do mod culpado, com dependências | S | S | N | N | N | N | N | N | N | N | N | — | N | N | T25; Prism PR fechado |
| Raio-x de mixins | S | S | N | N | N | N | N | N | N | N | N | — | N | N | T07 |
| IA que consulta o pack e propõe mudanças | S | S | N | N | N | N | S | N | N | N | N | — | N | P | T14; XMCL agente; MCDoctor só log |
| Nota de saúde do pack | S | S | N | N | N | N | N | N | N | N | N | — | N | N | T14 |
| Itens e minérios repetidos | N | S | N | N | N | N | N | N | N | N | N | — | N | N | T32 |
| Segurança: hash oficial e sinais de malware | N | S | P | P | P | N | N | N | N | N | N | — | N | N | T28; Prism 11.1 avisa mods embutidos; lojas revisam no servidor |
| Mods removidos ou abandonados, com substitutos | N | S | N | N | N | N | N | N | N | N | N | — | N | N | T29 |
| Travamento de um jogador com a versão do pack | N | S | N | N | N | N | N | N | — | — | — | — | P | P | T30; CA compara lista de mods |

### 6.5 Versionar e publicar

| Função | W1 | W1.1 | CF | MR | Pr | AT | XM | FTB | pw | Pk | PW | SPC | CA | MD | Base |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Versões salvas com changelog automático | S | S | P | N | N | N | P | N | N | P | N | — | — | — | T16, T17; CF registro de conteúdo; XMCL agente de changelog; Pakku `diff` |
| Histórico com voltar versão, sem mostrar Git | S | S | N | N | N | N | N | N | P | P | N | — | — | — | T17; packwiz/Pakku usam Git manual |
| Jogadores recebem atualização sozinhos | S | S | S | S | N | ? | ? | N | S | P | P | — | — | — | T18; lojas com revisão; instâncias compartilhadas MR |
| Exportar `.mrpack` e zip da CurseForge | S | S | P | P | S | S | S | P | S | S | S | — | — | — | T19 |
| Pacote para servidor testado | S | S | N | N | N | ? | N | N | P | ? | N | S | — | — | T19, T27; SPC não testa sozinho |
| Lista de mods para os jogadores | N | N | P | N | S | ? | ? | N | N | ? | N | — | — | — | Prism `ExportToModList`; CF `modlist.html` |
| Instância pronta para importar em outro launcher | N | N | N | N | — | S | ? | N | N | N | N | — | — | — | ATLauncher exporta MultiMC; packwiz #76 |
| Publicar direto nas lojas | N | N | N | N | N | N | N | N | N | N | N | — | — | — | Fora (SPEC §9) |
| Sem anúncios e sem telemetria | S | S | N | P | S | ? | P | N | S | S | S | S | P | N | SPEC §7.8; XMCL tem módulo de telemetria; CA tem envio opcional (desligado pelo Warden) |

## 7. Onde o Warden fica atrás, empata e é único

### 7.1 Único (ninguém maduro faz) [inferência a partir da matriz]

1. **Projeto separado do teste**, com "o que mudou durante o teste" por valor (T15).
2. **Busca automática do mod culpado** respeitando dependências e achando pares (T25). Só ferramentas amadoras ou alfa chegam perto (Modpack Debug Kit, emendator, Scope).
3. **Histórico de travamentos** agrupado pela causa e ligado às versões do pack (T14), e, na 1.1, **travamento de um jogador com a versão identificada** (T30).
4. **Testar como o jogador recebe** e **pacote para servidor testado** no próprio computador (T13, T19, T27).
5. **Pack como Git sem mostrar Git**: versões, changelog automático, voltar versão e publicação com atualização dos jogadores pelo link (T16–T18).
6. **Busca combinada sem duplicatas** nas duas lojas, com pack híbrido (T08).
7. **IA com evidência conferida e Aplicar** com diferença exata (T14). O XMCL tem IA com ferramentas, mas sem a regra da evidência [inferência pelo código].
8. **Saúde do pack**, segurança por hash oficial, manutenção com substitutos e itens repetidos (T14, T28, T29, T32).
9. **Busca em todas as configs** e editor de scripts com IDs dos jars (T12, T26).

### 7.2 Empata

Java automático, dependências ao adicionar, importar e exportar `.mrpack`/zip da CurseForge, atualizar em lote com changelog, servidor local (XMCL), raio-x de mixins e grafo (empata com ferramentas soltas, como o Modpack Inspector Tool, mas integrado ao diagnóstico).

### 7.3 Fica atrás

| Onde | Quem faz | Peso para o dono [inferência] |
|---|---|---|
| **O jogador instala em 4 passos** (criar instância, baixar bootstrap, colar comando, abrir) | Lojas: um clique; instâncias compartilhadas do Modrinth: um link | Alto: é o momento em que um amigo desiste |
| **Lista de mods** para jogadores | Prism, CurseForge | Médio |
| **Migrar de versão do Minecraft** | Ninguém bem; mrpack-updater e pedidos em ATLauncher, Pakku, Modrinth | Médio |
| **macOS, Linux empacotado, outros idiomas** | Todos os launchers | Baixo (uso pessoal no Windows) |
| **Publicar nas lojas** | Só manual em todos | Baixo (fora por decisão) |
| **Variantes e vários loaders no mesmo pack** | unsup (variantes) | Baixo |
| **Conflitos de receita** | emendator (alfa) | Baixo a médio |
| **Economia de disco com links** | XMCL, Prism (pedido #38) | Baixo: uma instância por pack |

## 8. Implicações para o Warden

### 8.1 Lacunas (o que os outros fazem e o Warden não planeja)

| # | Lacuna | Quem faz / quem pede | Recomendação | Onde encaixa (sem abas) |
|---|---|---|---|---|
| **L1** | **Instância pronta para o Prism/MultiMC**: um `.zip` que o jogador arrasta para o Prism e já vem com versão, loader, bootstrap e o comando antes de iniciar configurados (e a memória recomendada) | ATLauncher exporta MultiMC; packwiz #76 (7); a dor V7 | **Entrar na v1 (P1)**, se o dono aprovar (decisão 1). Custo baixo: `instance.cfg` + `mmc-pack.json` + bootstrap fixado, que o Warden já guarda para o T13 e o T19 | **Exportar** (T19), novo formato "Instância para Prism ou MultiMC (.zip)"; e no **Publicar versão** (T18), ao lado de "Como os jogadores instalam", o botão **Baixar instância pronta** |
| **L2** | **Lista de mods para os jogadores** (Markdown/HTML com nome, versão, link e lado) | Prism, CurseForge; packwiz #13, ideia CF (17), Modrinth #1865 | **1.1**. Pequena; publicar um `MODS.md` junto do pack no GitHub e oferecer em Exportar | **Exportar**, formato "Lista de mods"; caixa **Incluir a lista de mods** no **Publicar versão** |
| **L3** | **Prontidão para migrar**: "112 de 128 mods já têm versão para 1.21.1" e a lista do que falta | mrpack-updater; ATLauncher #479, Pakku #78, Modrinth #4905 | **1.1**, como extensão da T29, que já consulta "sem versão para o Minecraft mais novo" por mod; o assistente de migração continua P2 | Página **Manutenção dos mods** (Problemas), bloco "Mudar para outra versão do Minecraft", com seletor de versão (caixa de seleção) |
| **L4** | **Memória e Java recomendados chegam ao jogador** | CurseForge (Java por pack, 06/2026); Solder (Java por build) | **v1, dentro de L1** (a instância pronta leva a memória sugerida do T13); sem L1, uma linha no passo a passo do T18 | T18 e L1 |
| **L5** | **Conflitos de receita** (duas receitas iguais com saídas diferentes) | emendator (alfa) | **Versão futura**: exige ler receitas de cada versão do jogo e de scripts; T32 já cobre o caso mais comum (itens repetidos) | Futuro: Problemas → Verificações do pack |
| **L6** | **Instância compartilhada estilo Modrinth** (link de convite, sem GitHub) | Modrinth (07/2026), GDLauncher | **Não entrar**: o Warden já distribui pelo GitHub (D14); duplicaria o modelo e criaria serviço | — |
| **L7** | **Deploy por SFTP / hospedagem** | Pinecone, painéis | **Não entrar**: uso pessoal; o pacote para servidor (T19) basta | — |
| **L8** | **Publicar direto no Modrinth/CurseForge** | Pedidos em packwiz, Pakku, Modrinth | **Continua fora** (SPEC §9); a exportação já gera o arquivo certo | — |
| **L9** | **Variantes do pack e vários loaders** | unsup; packwiz #163, #369 | **Continua P2**; demanda baixa | — |
| **L10** | **"Quem depende deste mod" na descoberta** (antes de adicionar) | Modrinth Extras; CurseForge (04/2026) | **Já coberto em parte** (T07 "Usado por", só dentro do pack); fora do pack, versão futura | — |

### 8.2 Diferenciais a proteger (não podem ficar fracos)

1. **Busca do culpado (T25).** É a dor nº 1 (V1) e o mercado ainda não tem nada maduro; Scope, emendator e o Modpack Debug Kit mostram que vão aparecer cópias simples. Proteger: dependências sempre ligadas, pares, "travou diferente" não conta, pausar/cancelar, e explicação em linguagem simples. Não deixar cair para P2.
2. **Evidência na IA (T14).** O XMCL já tem IA com ferramentas e confirmação; a diferença do Warden é que **toda afirmação aponta para o trecho** e **nada muda sem a diferença exata**. A comunidade desconfia de IA (V2, V16): a evidência é o que torna a IA aceitável.
3. **Separar projeto e teste (T15).** Todos os launchers editam a instância ao vivo; esse é o motivo pelo qual "atualizar quebrou tudo" e "não sei o que mudei" aparecem tanto (V14).
4. **Pack limpo e testado antes de ir para os jogadores (T04, T18, T19).** Os packs famosos saem com lixo (R1 §1.B); o ServerPackCreator grita "teste antes". O Warden testa.
5. **Sem anúncio, sem telemetria, local.** É o principal motivo de a comunidade preferir o Prism (V10, V11).
6. **Desempenho da interface** (SPEC §8): "leve e rápido" é o elogio mais repetido ao Prism; um app Tauri lento perderia o argumento.
7. **Texto e visual sem cara de IA**: a comunidade cobra isso em ferramentas e em páginas de pack (V16); o Deep Dark com fontes pixel está alinhado.

### 8.3 Riscos

| # | Risco | Probabilidade / impacto [inferência] | Evidência | Mitigação |
|---|---|---|---|---|
| **R1** | **CDN da CurseForge passar a exigir chave**: o `packwiz-installer` dos jogadores não manda chave e está parado desde 2024; os mods da CurseForge deixariam de baixar para os jogadores, sem aviso | Média / **alto** | Anúncio com prazo de 16/07/2026 [fonte]; erros "A valid api-key is required" no Prism em 19/07 [fonte]; FTB App e Prism mudaram o código [fonte]; hoje o download sem chave funciona (302 → 200) [verificado] | Busca combinada já prefere o Modrinth; **avisar no Publicar** sobre mods só da CurseForge (decisão 4); "Testar como o jogador recebe" vai mostrar a falha quando acontecer; acompanhar o anúncio; plano B: o Warden embutir mods da CurseForge permitidos no pacote ou um installer com chave (decisão futura) |
| **R2** | **packwiz e packwiz-installer com manutenção fraca** | Média / médio | packwiz sem releases (#403); installer sem commit desde 06/2024 [verificado] | Commit fixado (ADR do sidecar); testes de conformidade; o Warden só depende do packwiz para refresh, validação e exportação (híbrido) |
| **R3** | **Modrinth mudar regras ou API depois da compra pela Spark** | Baixa a média / médio | Compra fechada em 02/2026; promessa de seguir aberto [fonte]; desconfiança forte da comunidade [verificado] | Duas fontes (ADR-0027); cache local; nada no pack depende do app do Modrinth |
| **R4** | **API do Modrinth trocar campos** (o `environment` novo substitui `client_side`/`server_side`) | Alta / baixo | O campo `environment` já aparece na API v2 do projeto e da versão [verificado]; a ARCHITECTURE §6.2 já usa `environment` [verificado no repositório] | Manter o mapeamento e os dois campos na leitura até o v2 tirar os antigos |
| **R5** | **Concorrente lançar algo parecido** (Prism com bisseção, XMCL com IA mais forte, ferramenta nova em Tauri) | Média / baixo | PR do Prism fechado; XMCL rápido; Scope e emendator [verificado] | Uso pessoal: não há disputa de mercado; ler os concorrentes como fonte de ideias a cada marco |
| **R6** | **Mais malware e comportamentos escondidos em mods** | Alta / alto para os jogadores | Casos de 2025–2026 (§2) | T28 na 1.1; considerar na lista de sinais padrões de "vasculhar o disco" e "detectar launcher" (achado da §2) |
| **R7** | **Pedidos de range do CDN da CurseForge falharem** (afeta a leitura por partes do manifesto na T08) | Alta / baixo | `edge.forgecdn.net` responde 404 a pedidos com `Range`; `mediafilez.forgecdn.net` responde 206 [verificado] | Seguir o redirecionamento sem `Range` e só então pedir as partes (§9) |
| **R8** | **Prazo de revisão e regras de IA do Modrinth** se o dono um dia publicar lá | Baixa / baixo | Fila de 3.000 packs; declaração obrigatória de IA [fonte] | Fora do escopo; a IA do Warden não gera conteúdo do pack |

### 8.4 Decisões para o dono (no máximo 5)

1. **Instância pronta para os jogadores.** Hoje o jogador precisa de 4 passos no Prism para receber o pack. O Warden pode gerar um arquivo que o jogador só arrasta para o Prism, já com tudo configurado (inclusive a memória certa).
   **Recomendação: sim, na v1** (em Exportar e no Publicar versão). É pequeno, e é onde um amigo leigo desiste.
2. **Lista de mods para os jogadores.** Um arquivo com o nome, a versão e o link de cada mod, publicado junto do pack no GitHub e disponível em Exportar.
   **Recomendação: sim, na 1.1.**
3. **"Dá para mudar de versão do Minecraft?"** Na página de manutenção, escolher uma versão e ver quantos mods já existem para ela e quais faltam (sem fazer a mudança sozinho).
   **Recomendação: sim, na 1.1.** O assistente que muda a versão continua para depois.
4. **Mods só da CurseForge podem parar de baixar para os jogadores.** A CurseForge anunciou que vai exigir chave nos downloads, e a ferramenta que seus jogadores usam não manda chave. Hoje ainda funciona. O Warden pode avisar antes de publicar ("N mods só existem na CurseForge e podem deixar de baixar para os jogadores") e sugerir a troca pelo Modrinth quando houver.
   **Recomendação: sim, na v1**, como aviso (não bloqueio) no Publicar versão e na linha "Download pelos jogadores" da saúde do pack.
5. **Publicar direto no Modrinth e na CurseForge continua fora?** Vários criadores pedem, mas o Warden publica no GitHub e já gera os arquivos das lojas.
   **Recomendação: continuar fora.**

## 9. Achados fora do escopo (para o orquestrador)

1. **CDN da CurseForge e pedidos parciais [verificado, 04/10/2026]:** `edge.forgecdn.net/files/...` responde **302 → `mediafilez.forgecdn.net`** sem `Range`, mas **404** quando o pedido tem `Range` (inclusive com `-L`); `mediafilez.forgecdn.net` aceita `Range` (206). A T08 (CA-T08-15/16) lê o manifesto da CurseForge por partes: o cliente precisa resolver o redirecionamento sem `Range` e pedir as partes na URL final. Vale registrar na ARCHITECTURE §17 ou na tarefa P1-17.
2. **Chave no CDN da CurseForge [fonte + verificado]:** o Warden baixa jars da CurseForge para a instância (T13) e para o pacote com mods (T19). Mandar o cabeçalho `x-api-key` também no download, como o Prism 11.0.3 e o FTB App 1.30.0 passaram a fazer, para não quebrar quando a exigência voltar. Sugestão para a tarefa do cliente da CurseForge.
3. **packwiz-installer sem chave [código]:** afeta jogadores e o pacote para servidor pelo link (T19, ADR-0035). Monitorar; ver decisão 4.
4. **Modrinth `environment` [verificado]:** a API v2 já devolve `environment` (ex.: `["client_only"]` no projeto, `"client_only"` na versão), e o packwiz passou a usar o dado novo (issue #407, fechada em 02/09/2026). A ARCHITECTURE §6.2 já está alinhada; confirmar no teste de contrato que o Warden lê o campo da **versão**, não só do projeto.
5. **Modrinth exige user-agent identificável** (lembrado na conversa do CraftPacker) [fonte]: o cliente do Warden deve mandar `User-Agent` no formato pedido pela documentação do Modrinth; conferir se está na ARCHITECTURE.
6. **Sinais de segurança (T28):** dois comportamentos novos vistos em 2026 que poderiam virar "pontos de atenção": mod que procura arquivos de outro launcher no disco e responde ao servidor, e mod que aplica "penalidades escondidas" (Gabou's Libs, 08/2026) [verificado no Reddit]. Decisão de conteúdo da lista de sinais (ADR-0041).

## 10. Fontes

### 10.1 Repositórios e código (consultados em 04/10/2026)

- Prism Launcher: https://github.com/PrismLauncher/PrismLauncher (releases 9.0, 10.0.0, 11.0.0, 11.1.0, 11.1.1; issues #1055, #591, #864, #1924, #154, #880, #175, #380, #5804, #5671; PR #5855; `launcher/ui/dialogs/ExportToModListDialog.cpp`)
- App do Modrinth: https://github.com/modrinth/code (`packages/app-lib/src/api/instance/export_mrpack.rs`; issues #535, #3143, #4234, #4512, #4664, #4905, #469, #1865, #809, #3057)
- ATLauncher: https://github.com/ATLauncher/ATLauncher (`InstanceExportFormat.java`, `MinecraftError.java`; issue #479)
- XMCL: https://github.com/Voxelum/x-minecraft-launcher (`xmcl-keystone-ui/src/composables/agent/*`, `instanceServerLaunch.ts`, `xmcl-runtime-api/src/services/AgentService.ts`; release v0.71.0; issues #1068, #1739)
- GDLauncher: https://github.com/gorilla-devs/GDLauncher (arquivado), https://github.com/gorilla-devs/GDLauncher-Carbon
- FTB App: https://github.com/FTBTeam/FTB-App (releases 1.29.x e 1.30.0)
- Technic: https://github.com/TechnicPack/TechnicSolder, https://github.com/TechnicPack/LauncherV3
- HMCL: https://github.com/HMCL-dev/HMCL
- packwiz: https://github.com/packwiz/packwiz (issues #403, #407, #97, #369, #28, #76, #13, #163, #251, #82, #161); packwiz-installer: https://github.com/packwiz/packwiz-installer (`CurseForgeSourcer.kt`)
- Pakku: https://github.com/juraj-hrivnak/Pakku (issues #138, #63, #78, #53)
- ferium: https://github.com/gorilla-devs/ferium · mrpack-install: https://github.com/nothub/mrpack-install · unsup: https://git.sleeping.town/unascribed/unsup e https://modrinth.com/mod/unsup
- PW-GUI: https://github.com/AmberIsFrozen/PW-GUI · Pinecone: https://github.com/Blade-Punisher/pinecone-pack-manager · Packwiz Studio: https://github.com/PalisadeMC/Packwiz-Studio · packwiz-web: https://github.com/leocov-dev/packwiz-web · MCED: https://github.com/MinecraftEvolve/MCED
- ServerPackCreator: https://github.com/Griefed/ServerPackCreator · itzg: https://github.com/itzg/docker-minecraft-server · Pterodactyl: https://github.com/pterodactyl/panel
- Scope Launcher: https://github.com/lonestill/scope-launcher · emendator: https://github.com/RemiAsselin42/emendator · mod-bisect-tool: https://github.com/Qendolin/mod-bisect-tool
- mclo.gs: https://github.com/aternosorg/mclogs · codex-minecraft: https://github.com/aternosorg/codex-minecraft · Crash Assistant: https://github.com/KostromDan/Crash-Assistant · Not Enough Crashes: https://github.com/natanfudge/Not-Enough-Crashes

### 10.2 Documentação, blogs e notícias

- Modrinth: https://modrinth.com/news (feed RSS), em especial https://modrinth.com/news/article/joining-spark-universe/, /modpack-permissions/, /shared-instances/, /content-management-overhaul/, /ai-policy-and-disclosures/, /introducing-server-projects/, /modrinth-hosting-in-app/, /new-environments/, /becoming-sustainable/; https://modrinth.com/legal/cmp-info; https://support.modrinth.com/en/articles/8802250-modpacks-on-modrinth; https://support.modrinth.com/en/articles/8797522-sharing-modpacks; post de fim de ano: https://x.com/modrinth/status/2006599452102996410
- CurseForge: https://blog.curseforge.com/introducing-api-key-authentication-for-curseforge-file-downloads/; https://blog.curseforge.com/curseforge-june-newsletter-your-modpack-just-got-smarter/; https://blog.curseforge.com/app-release-notes-1-314/; https://blog.curseforge.com/app-release-notes-1-317/; https://blog.curseforge.com/server-packs-tutorial/; https://support.curseforge.com/support/solutions/articles/9000196904-creating-a-custom-profile-modpack; https://support.curseforge.com/support/solutions/articles/9000197912-sharing-modpacks-custom-profiles; https://support.curseforge.com/support/solutions/articles/9000197902-reward-program-faq
- Ideias da CurseForge: https://curseforge-ideas.overwolf.com/ideas/CF-I-1043 (fork, 20), /CF-I-1639 (lista de mods, 17), /CF-I-1023 (criar no app, 16), /CF-I-2355 (25), /CF-I-6382 (9), /CF-I-6895 (IA, 11)
- Overwolf: https://games.gg/news/overwolf-creator-payouts-hit-300-million/ [fonte secundária]; https://www.gemlist.io/programs/curseforge-mod-creator [fonte secundária]
- Analisadores com IA: https://mcdoctor.ai/, https://www.mccrashreader.com/, https://dedicatedminecraft.host/tools/log-analyzer

### 10.3 APIs usadas para os números

- `https://api.modrinth.com/v2/statistics`; `https://api.modrinth.com/v2/search` (facetas `project_type`, `categories`, `versions`; filtro `created_timestamp`; `index=downloads`); `https://api.modrinth.com/v2/projects`; `https://api.modrinth.com/v2/project/{slug}`
- `https://api.curseforge.com/v1/mods/search` (`gameId=432`, `classId=4471`, `gameVersion`, `modLoaderType`), `https://api.curseforge.com/v1/minecraft/version`, `https://api.curseforge.com/v1/mods/{id}/files`
- Busca de código do GitHub: `"pack-format = \"packwiz:1.1.0\"" filename:pack.toml`

### 10.4 Reddit (lido em 04/10/2026; votos e comentários no momento da leitura)

- Making a Modpack is tough… (177/37): https://reddit.com/r/feedthebeast/comments/1eu7iji/making_a_modpack_is_tough/
- Man making a modpack can be stressful (87/20): https://reddit.com/r/feedthebeast/comments/1w9k3rr/man_making_a_modpack_can_be_stressful/
- As a modpack developer, how could I fix this problem (5.610/171): https://reddit.com/r/feedthebeast/comments/1kwq81q/as_a_modpack_developer_how_could_i_fix_this/
- I made a slop modpack… (882/120): https://reddit.com/r/feedthebeast/comments/1u41bcb/i_made_a_slop_modpack_thats_based_on_everything/
- My Modpack Debug Kit is now public! (775/120): https://reddit.com/r/feedthebeast/comments/1on0i6i/my_modpack_debug_kit_is_now_public/ e a revisão (38/20): https://reddit.com/r/feedthebeast/comments/1uag1cq/my_modpack_debug_kit_just_got_a_major_overhaul/
- How do YOU binary search? (4/7): https://reddit.com/r/feedthebeast/comments/1tcuw7k/how_do_you_binary_search/
- Is the "Binary Search" just bad for troubleshooting?: https://reddit.com/r/feedthebeast/comments/1hntxyr/is_the_binary_search_just_bad_for_troubleshooting/
- Question: How can I make a server pack for my own modpack? (65/10): https://reddit.com/r/feedthebeast/comments/1sgfnuw/question_how_can_i_make_a_server_pack_for_my_own/
- I built a tool for making modpacks faster (CraftPacker, 13/15): https://reddit.com/r/feedthebeast/comments/1t7r9p9/i_built_a_tool_for_making_modpacks_faster_bulk/
- Modpack Inspector Tool (727/40): https://reddit.com/r/feedthebeast/comments/1s0i5u7/modpack_inspector_tool/
- Modpack visualization tool (299/46): https://reddit.com/r/feedthebeast/comments/1pb5d8z/modpack_visualization_tool/
- Tool for updating Modpacks to Major MC Versions (101/7): https://reddit.com/r/feedthebeast/comments/1or96cb/tool_for_updating_modpacks_to_major_mc_versions/
- I made a GUI tool for editing Minecraft mod configs (MCED, 36/24): https://reddit.com/r/feedthebeast/comments/1qiegl2/i_made_a_gui_tool_for_editing_minecraft_mod/
- Dependency explorer for Modrinth projects (132/12): https://reddit.com/r/feedthebeast/comments/1uhuhn5/dependency_explorer_for_modrinth_projects/
- Advice for newbie modpack creator: https://reddit.com/r/feedthebeast/comments/1q1euwd/advice_for_newbie_modpack_creator/
- First-time modpack creator tips?: https://reddit.com/r/feedthebeast/comments/1rt6wr5/firsttime_modpack_creator_tips_and_how_to_fix_an/
- Packwiz on a server, how do you all do it?: https://reddit.com/r/feedthebeast/comments/1ia3xi7/packwiz_on_a_server_how_do_you_all_do_it/
- Converting modpacks to packwiz: https://reddit.com/r/feedthebeast/comments/1nwzbtf/converting_modpacks_to_packwiz/
- hybrid curseforge+modrinth modpack?: https://reddit.com/r/feedthebeast/comments/1vszd71/hybrid_curseforgemodrinth_modpack/
- Introducing Shared Instances on Modrinth (420/54): https://reddit.com/r/feedthebeast/comments/1v7ca4g/introducing_shared_instances_on_modrinth/
- Is it only me or the modrinth app is getting worse (47/12): https://reddit.com/r/feedthebeast/comments/1sjexgo/is_it_only_me_or_the_modrinth_app_is_getting/
- modrinth acquired by creators of essential mod (975/364): https://reddit.com/r/feedthebeast/comments/1u6j4av/modrinth_acquired_by_creators_of_essential_mod/
- What is YOUR favorite launcher (and why)? (20/64): https://reddit.com/r/feedthebeast/comments/1ppv52e/what_is_your_favorite_launcher_and_why/
- An actual good Minecraft Launcher? (2/80): https://reddit.com/r/feedthebeast/comments/1olm37o/an_actual_good_minecraft_launcher/
- What is the best Mod pack launcher? (12/30): https://reddit.com/r/feedthebeast/comments/1sjuift/what_is_the_best_mod_pack_launcher/
- just switched over to prismlauncher (759/102): https://reddit.com/r/feedthebeast/comments/1ow1tcw/just_switched_over_to_prismlauncher/
- Overwolf using 100 GB of data in a month (1.771/188): https://reddit.com/r/feedthebeast/comments/1k20m48/overwolf_using_100_gb_of_data_in_a_month/
- Help with logging in CurseForge from Linux/Zorin (39/33): https://reddit.com/r/feedthebeast/comments/1v17mdi/help_with_logging_in_curseforge_from_linuxzorin/
- I opened my 2-year-old, 400+ mod modpack… (3.066/147): https://reddit.com/r/feedthebeast/comments/1nxvlgk/i_opened_my_2yearold_400_mod_modpack_and_suddenly/
- This 1M+ downloads mod scans your hard drive… (2.367/511): https://reddit.com/r/feedthebeast/comments/1w2ojoc/this_1m_downloads_mod_scans_your_hard_drive_for_a/
- PSA: Don't download "architectury" from architecturyapi.com (388/65): https://reddit.com/r/feedthebeast/comments/1t7chhr/psa_dont_download_architectury_from_this_site/
- Minecraft is crazy close to hit 100B mod downloads on Curseforge (2.003/96): https://reddit.com/r/feedthebeast/comments/1qasksn/
- Project dependents are now visible on the CurseForge website (574/34): https://reddit.com/r/feedthebeast/comments/1se02pe/
- I spent about 300 hours curating a modpack only to realise it can never be released (685/70): https://reddit.com/r/Minecraft/comments/1t65pmn/i_spent_about_300_hours_curating_a_modpack_only/
- r/CurseForge: 1/3 of the Launcher space for ads (415/142): https://reddit.com/r/CurseForge/comments/1rovzv5/; Curseforge, what are you up to? (399/84): https://reddit.com/r/CurseForge/comments/1w5pzet/; UI devs… slider de memória (212/56): https://reddit.com/r/CurseForge/comments/1pwwzlr/; another compromise with Curseforge modpacks (93/108): https://reddit.com/r/CurseForge/comments/1vftbpo/; Monthly Subscription (79/27): https://reddit.com/r/CurseForge/comments/1rivgbo/
- r/Modrinth: modpack 3 semanas em revisão: https://reddit.com/r/Modrinth/comments/1vunwz7/; argumento Java permanente: https://reddit.com/r/Modrinth/comments/1waa66c/
- r/PrismLauncher: Prism launcher redesign UI 3.0 (218/153): https://reddit.com/r/PrismLauncher/comments/1qmsohs/
