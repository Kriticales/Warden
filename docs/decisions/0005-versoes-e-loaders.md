# ADR-0005 — Versões do Minecraft e loaders suportados

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono

## Contexto

A pesquisa levantou o custo de "qualquer versão" (R2 §9 itens 4 e 5; R3 §7 itens 2, 3 e 10): Fabric oficial só existe a partir da 1.14; NeoForge a partir da 1.20.1; o packwiz não conhece Legacy Fabric; versões ≤ 1.6.4 usam assets e argumentos muito antigos.

## Decisão

- **Suporte garantido de 1.7.10 até a versão mais nova** desde a primeira versão do app. Anteriores a 1.7.10: "melhor esforço" (aparecem com etiqueta, sem testes automáticos).
- Loaders: **Forge** (todas as versões, incluindo 1.7.10 e 1.12.2), **NeoForge** (1.20.1+), **Fabric** (1.14+), e "nenhum" (pack vanilla de resource packs/shaders).
- **Quilt** fora por enquanto. **Legacy Fabric** pode vir depois. Snapshots do Minecraft fora da v1.

## Alternativas consideradas

- Suportar só versões modernas: excluiria 1.7.10/1.12.2, que o dono exige.
- Legacy Fabric já na v1: exige suporte fora do packwiz (o formato não o representa).

## Consequências

- Matriz de testes de lançamento obrigatória (ROADMAP L-05), incluindo Forge legado com Java 8.
- O catálogo trata o sufixo de versão do Forge antigo e o Warden escreve o `pack.toml` sozinho (bug do `packwiz init`, R3 §1.5.1).
- Packs Quilt/LiteLoader abertos no Warden são recusados com mensagem clara.
