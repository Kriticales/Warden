# ADR-0046 — Itens repetidos: detecção estática e peso na saúde

- **Status:** aceita · **Data:** 2026-10-02 · **Origem:** técnica + decisão do dono (tarefa D5, decisão D31)

## Contexto

Vários mods adicionam o mesmo material (quatro lingotes de cobre que não empilham) e geram o próprio minério. Não trava nada, mas incomoda quem joga, e minério em dobro afeta todos os mundos e é difícil de desfazer. Dá para descobrir lendo os jars, sem abrir o jogo, mas as convenções mudam por loader e versão:

- **Tags:** `data/forge/tags/items/<tipo>/<mat>.json` (Forge 1.13 a 1.20.x); `data/c/tags/items/<mat>_<tipo>s.json` (Fabric, convenção antiga); `data/c/tags/item/<tipo>/<mat>.json` (Fabric 1.20.5+ e NeoForge 1.21; pasta no singular desde a 24w19a).
- **Geração de minério:** `worldgen/configured_feature` mais `forge/biome_modifier` ou `neoforge/biome_modifier`; no Fabric, a ligação ao bioma é feita em código.
- **1.12.2 e 1.7.10:** o dicionário de minérios é registrado em código; só há heurística (arquivos de idioma com nomes do dicionário, JSON de geração da CoFH).
- Exemplos verificados em jars: Mekanism 1.21.1 com `data/c/tags/item/ingots/tin.json`; Tech Reborn 1.20.1 com `configured_feature/lead_ore.json` e sem `biome_modifier`.

Unificadores por faixa: **AlmostUnified** (All Rights Reserved; Forge e Fabric 1.18.2 a 1.20.1, NeoForge 1.19.2 a 1.20.1, NeoForge e Fabric 1.21.1), que unifica receitas, esconde os repetidos no JEI e no REI e, no 1.21.1, o loot; `world_gen_unification` só no NeoForge 1.21.1; não unifica líquidos nem receitas que não são JSON. **UniDict** (Forge 1.7.10 a 1.12.2, MPL-2.0, parado desde 2021). Forge 1.16.5 não tem unificador mantido. JAOPCA e Emendatus Enigmatica geram materiais, não unificam.

## Decisão

- Detecção estática na passagem completa do diagnóstico, com o cache por hash: leitura em `warden-jarmeta::materials` e análise em `warden-diagnostics::duplicates`. Nas versões antigas e no minério do Fabric, o resultado é marcado como "provável".
- Materiais e sinônimos em `materials.toml`; a solução por versão e loader em `unifiers.toml`. O Warden só sugere o unificador e o adiciona pelo fluxo normal, por referência.
- **Peso na saúde:** categoria "Itens repetidos", −1 por material com minério gerado por 2 ou mais mods sem unificador que resolva a geração, teto de −3; repetição só de itens vale 0.
- É **conselho**, nunca erro ou aviso: não bloqueia o teste nem a publicação.

## Alternativas consideradas

- Tratar como aviso: quase todo pack grande tem materiais repetidos e nada está quebrado; o aviso viraria ruído.
- Não pesar na saúde: esconderia o único efeito duradouro, o minério em dobro nos mundos.
- Detectar com o jogo aberto (lendo as tags carregadas): mais exato, mas exige um teste e um jeito de ler o estado interno do jogo, que o Warden não tem.

## Consequências

- A nota de saúde ([ADR-0034](0034-analise-estatica-do-pack.md)) ganha a categoria, com o peso em `health-score.toml`.
- O que é feito só em código ou por scripts (KubeJS, CraftTweaker) fica de fora, e a página diz isso.
- SPEC T32.
