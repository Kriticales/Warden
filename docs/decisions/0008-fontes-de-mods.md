# ADR-0008 — Fontes de mods

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono

## Contexto

Os apps anteriores só buscavam no Modrinth; CurseForge só por link (R4). A pesquisa deixou em aberto a chave da CurseForge (R1 §7 item 4; R3 §7 item 4).

## Decisão

- **Modrinth:** busca completa, detalhes, versões, dependências e atualizações pela API v2 (sem chave), com cache local de metadados.
- **CurseForge:** busca completa com a **chave pessoal do dono**, digitada no app e guardada no cofre; mais a opção de **colar link**, que passa pelo packwiz (ADR-0006).
- **Arquivos locais e links diretos** (`https://`), com identificação por hash no Modrinth e na CurseForge para oferecer a referência em vez do arquivo.
- Termos da CurseForge: nenhum dado da API é persistido além da sessão; `downloadUrl` nunca é gravada; mods com distribuição bloqueada usam o fluxo de download manual (SPEC T20).

## Alternativas consideradas

- Só Modrinth: excluiria boa parte dos mods de 1.7.10/1.12.2, que estão só na CurseForge.
- CurseForge só por link: experiência pior e sem detecção antecipada de bloqueios.

## Consequências

- Sem chave, a aba CurseForge mostra um estado vazio explicando como configurar.
- Detalhes de mods da CurseForge exigem internet.
- Deduplicação entre fontes é responsabilidade do Warden (o packwiz não faz).
