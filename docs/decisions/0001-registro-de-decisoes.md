# ADR-0001 — Registro de decisões de arquitetura

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão técnica (A1)

## Contexto

O Warden é construído por vários agentes em paralelo, supervisionados por um orquestrador, para um dono que não programa. Os projetos anteriores (R4) perderam o porquê de várias escolhas (o packwiz-gui chegou ao "v7" sem histórico) e repetiram erros.

## Decisão

Toda decisão que afeta mais de uma tarefa, muda um contrato ou escolhe uma tecnologia é registrada como ADR curto em `docs/decisions/NNNN-titulo.md`, com: contexto, decisão, alternativas consideradas e consequências. Decisões do dono são marcadas como "decisão do dono" e só mudam com nova decisão dele. ADRs não são editados depois de aceitos, exceto para mudar o status para "substituída por ADR-XXXX".

## Alternativas consideradas

- Registrar decisões só nos documentos grandes (SPEC/ARCHITECTURE): o porquê se perde nas revisões.
- Wiki externa: fica fora do repositório e das revisões.

## Consequências

- Agentes leem os ADRs antes de propor mudanças; propor algo que contradiz um ADR exige um ADR novo.
- O índice fica em `docs/decisions/README.md`.
