# R7 — Resumo para apresentação (slides em texto)

> Base: `docs/research/08-concorrencia-e-mercado.md` (04/10/2026). Cada slide tem no máximo 5 itens curtos, a sugestão de visual e, quando há gráfico, os dados brutos com a fonte.

## Slide 1 — Quem mais ajuda a montar modpack?

- Pesquisamos 30 ferramentas: launchers, ferramentas de linha de comando e de diagnóstico.
- Lemos cerca de 50 conversas do Reddit e mais de 400 comentários.
- Contamos os modpacks do Modrinth e da CurseForge.
- Resposta curta: ninguém faz o ciclo inteiro. O Warden faz.

**Visual:** título grande + ícone de lupa sobre um bloco de Minecraft.

## Slide 2 — O ciclo do criador, e quem cobre cada parte

- Criar: todos os launchers fazem.
- Testar: os launchers abrem o jogo, mas misturam pack e teste.
- Diagnosticar: só ferramentas soltas, que não conhecem o pack.
- Versionar e publicar: só o packwiz e o Pakku, sem interface.
- Warden: as cinco partes no mesmo lugar.

**Visual:** linha com 5 etapas (Criar → Testar → Diagnosticar → Versionar → Publicar); abaixo, uma faixa por tipo de ferramenta cobrindo só as etapas que ela faz; a faixa do Warden cobre todas.

**Dados brutos:**

| Tipo de ferramenta | Criar | Testar | Diagnosticar | Versionar | Publicar |
|---|---|---|---|---|---|
| Launchers (Prism, CurseForge, Modrinth…) | sim | parcial | parcial | não | parcial |
| packwiz / Pakku | sim | não | não | sim | sim |
| Diagnóstico (Crash Assistant, MCDoctor…) | não | não | sim | não | não |
| Warden | sim | sim | sim | sim | sim |

Fonte: matriz da §6 do relatório.

## Slide 3 — Dor nº 1: achar o mod que trava

- "O log não me diz nada" é a frase mais comum.
- A solução de hoje: tirar metade dos mods e testar, várias vezes, à mão.
- Dependências fazem a busca dar errado.
- Ninguém maduro automatiza isso. O Warden faz (Encontrar o mod culpado).

**Visual:** número grande "775 votos" (ferramenta caseira para a busca) + ícone de lupa com um mod em vermelho.

**Dados brutos (votos no Reddit e no GitHub):**

| Evidência | Votos | Fonte |
|---|---|---|
| Modpack Debug Kit (ferramenta caseira de busca) | 775 | r/feedthebeast, 03/11/2025 |
| Pedido de busca no Prism (aberto desde 2023) | 17 | GitHub Prism #1924 |
| Tentativa do Prism | fechada sem entrar | PR #5855, 22/09/2026 |

## Slide 4 — Dor nº 2: mods que não conversam

- Quatro mods, quatro lingotes de cobre diferentes.
- O meme sobre isso teve 5.610 votos.
- A resposta mais votada: usar um unificador (AlmostUnified).
- O Warden 1.1 avisa e sugere o unificador certo para a versão.

**Visual:** número grande "5.610 votos" + ícone de quatro lingotes iguais.

## Slide 5 — O que os criadores mais pedem nos launchers

- Agrupar mods com etiquetas: 66 votos (o pedido nº 1 do Prism).
- Facilitar o pacote para servidor: 37 votos no Prism, 11 no Modrinth.
- Mods que sobram ao remover outro: pedido no packwiz e no Modrinth.
- Achar o mod culpado: 17 votos no Prism.
- Os quatro já estão no plano do Warden.

**Visual:** gráfico de barras horizontais com marca "no Warden" em cada barra.

**Dados brutos:**

| Pedido | Votos | Fonte | No Warden |
|---|---|---|---|
| Grupos/etiquetas de mods | 66 | Prism #1055 | 1.1 (Notas e grupos) |
| Pastas de mods | 42 | Prism #154 | 1.1 |
| Facilitar servidor | 37 | Prism #864 | v1 (Pacote para servidor) |
| Busca do culpado | 17 | Prism #1924 | v1 |
| Pacote para servidor | 11 | Modrinth #535 | v1 |
| Remover dependências que sobraram | 8 | packwiz #97 | v1 |

## Slide 6 — O que incomoda nos launchers

- Anúncio ocupando um terço da tela (CurseForge): 415 votos.
- Overwolf gastando 100 GB de dados num mês: 1.771 votos.
- App do Modrinth "piorando a cada atualização".
- O Prism ganha por ser leve e "só funcionar".
- O Warden: sem anúncio, sem coleta de dados, rápido.

**Visual:** comparação lado a lado: "Eles" (ícones de anúncio, lentidão, coleta) × "Warden" (ícones de cadeado, raio, casa).

## Slide 7 — Quantos modpacks existem

- Modrinth: 18.649 modpacks.
- CurseForge: mais de 104.741 modpacks.
- Modrinth: 161.507 projetos e 69.696 autores.
- A CurseForge tem mais de 5 vezes mais packs.

**Visual:** dois números grandes lado a lado, com o logotipo de cada loja em texto (sem marca registrada).

**Dados brutos:**

| Loja | Modpacks | Fonte |
|---|---|---|
| Modrinth | 18.649 | API do Modrinth, 04/10/2026 |
| CurseForge | ≥ 104.741 (piso) | API da CurseForge, soma por versão, 04/10/2026 |

## Slide 8 — O Modrinth cresce rápido

- 2023: 1.786 packs novos.
- 2024: 4.823.
- 2025: 6.607.
- 2026: 5.154 até outubro (ritmo parecido com 2025).

**Visual:** gráfico de barras por ano.

**Dados brutos (modpacks que existem hoje, pelo ano de criação):**

| Ano | Modpacks |
|---|---|
| 2022 | 279 |
| 2023 | 1.786 |
| 2024 | 4.823 |
| 2025 | 6.607 |
| 2026 (até 04/10) | 5.154 |

Fonte: API do Modrinth, busca com filtro `created_timestamp`, 04/10/2026.

## Slide 9 — Qual loader e qual versão

- No Modrinth, 71% dos packs são Fabric.
- Na CurseForge, o NeoForge virou o padrão da 1.21.1.
- 1.20.1 ainda é a versão mais usada nas duas lojas.
- 1.7.10 e 1.12.2 vivem na CurseForge.

**Visual:** gráfico de barras agrupadas por versão (Forge, Fabric, NeoForge), só CurseForge; e uma pizza simples do Modrinth por loader.

**Dados brutos — CurseForge por versão (Forge / Fabric / NeoForge):**

| Versão | Forge | Fabric | NeoForge |
|---|---|---|---|
| 1.12.2 | ≥ 10.000 | 22 | 11 |
| 1.20.1 | ≥ 10.000 | 4.616 | 288 |
| 1.21.1 | 715 | 2.942 | ≥ 10.000 |
| 1.21.11 | 96 | 1.018 | 300 |

**Dados brutos — Modrinth por loader:** Fabric 13.322 · Forge 3.825 · NeoForge 2.503 · Quilt 686 (um pack pode ter mais de um).

Fonte: APIs da CurseForge e do Modrinth, 04/10/2026. "≥ 10.000" é o teto da API.

## Slide 10 — packwiz: pouca gente, gente certa

- Dos 200 packs mais baixados do Modrinth, 27 mostram o código no GitHub.
- Desses, 10 usam packwiz e 3 usam Pakku.
- O pack nº 1 em downloads (Fabulously Optimized) usa packwiz.
- Quem trata pack como código escolhe packwiz.

**Visual:** funil: 200 → 27 → 13.

**Dados brutos:** 200 packs do topo · 30 com repositório · 27 no GitHub · 10 packwiz · 3 Pakku. Fonte: API do Modrinth + GitHub, 04/10/2026.

## Slide 11 — Onde o Warden é único

- Separa o pack do teste e mostra o que mudou no jogo.
- Acha sozinho o mod culpado, respeitando dependências.
- Guarda o histórico de travamentos por causa.
- Testa como o jogador recebe e testa o servidor.
- Versões, changelog e publicação sem mostrar Git.

**Visual:** lista com ícone de estrela em cada item.

## Slide 12 — Onde o Warden empata

- Java automático.
- Dependências ao adicionar.
- Importar e exportar nos formatos das lojas.
- Atualizar vários mods de uma vez.
- Servidor local (o XMCL também tem).

**Visual:** lista com ícone de "=" em cada item.

## Slide 13 — Onde o Warden fica atrás

- O jogador precisa de 4 passos para instalar o pack.
- Não gera a lista de mods para os jogadores.
- Não diz se dá para mudar de versão do Minecraft.
- Não roda em macOS (de propósito).

**Visual:** lista com ícone de seta para baixo; os três primeiros com a etiqueta "dá para resolver".

## Slide 14 — Um concorrente novo em IA

- O XMCL ligou um assistente de IA para todos em agosto de 2026.
- Ele lê e edita configs, remove mods e escreve o changelog.
- Pede confirmação, mas não mostra a prova de cada afirmação.
- O Warden: cada afirmação aponta para o trecho do log; nada muda sem Aplicar.

**Visual:** comparação lado a lado "XMCL" × "Warden" com 3 linhas (prova, diferença antes de aplicar, chave do usuário).

**Dados brutos:**

| | XMCL | Warden |
|---|---|---|
| Mostra a prova de cada afirmação | não | sim |
| Mostra a diferença exata antes de mudar | não (só confirma) | sim |
| De quem é a IA | serviço próprio (assinatura) ou do usuário | chave do usuário (Gemini) |

Fonte: código do XMCL (`composables/agent`), 04/10/2026; SPEC T14.

## Slide 15 — Riscos

- A CurseForge quer exigir chave em todo download.
- A ferramenta dos seus jogadores não manda chave.
- Hoje ainda funciona; pode parar sem aviso.
- O Modrinth foi comprado em 2026; a comunidade desconfia.
- Malware em mods continua aparecendo.

**Visual:** três placas de alerta (CurseForge, Modrinth, malware) com o nível de risco em palavra (alto, médio, alto).

## Slide 16 — 5 decisões para você

- 1. Instância pronta para os jogadores (arrastar e jogar)? Recomendo: sim, na v1.
- 2. Lista de mods para os jogadores? Recomendo: sim, na 1.1.
- 3. "Dá para mudar de versão?" na manutenção? Recomendo: sim, na 1.1.
- 4. Avisar sobre mods só da CurseForge antes de publicar? Recomendo: sim, na v1.
- 5. Publicar direto nas lojas? Recomendo: continuar fora.

**Visual:** cinco cartões, cada um com a pergunta, a recomendação e dois botões desenhados ("Sim" / "Não").
