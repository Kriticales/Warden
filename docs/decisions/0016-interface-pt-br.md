# ADR-0016 — Interface só em português do Brasil

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (pt-BR) + decisão técnica (A1) (mecanismo)

## Contexto

O manager anterior misturou português europeu com pt-BR e manteve três idiomas, com custo alto (R4 §2.4).

## Decisão

- Interface inteira em **pt-BR**, um único idioma.
- Textos num catálogo i18next (`src/i18n/pt-BR/<área>.ts`), com chaves tipadas; ESLint proíbe texto solto em JSX.
- Glossário e regras de escrita em QUALITY §8; frases de erro por código (ADR-0018).

## Alternativas consideradas

- Textos direto no JSX: impossível revisar consistência e glossário.
- Vários idiomas: sem demanda.

## Consequências

- Manter i18next com um idioma custa pouco e deixa a porta aberta, mas nenhum segundo idioma é planejado.
