# ADR-0071 — Renumeração: a 1.1 é o lote do bug-01; a Profissional vira 1.2

- **Status:** aceita · **Data:** 2026-10-25 · **Origem:** decisão do dono (25/10/2026), registrada na tarefa REL-03 · **Renomeia a versão de:** [ADR-0039](0039-warden-1-1-profissional.md) a [ADR-0047](0047-desempenho-entre-versoes.md), sem mudar o que elas decidem

## Contexto

A [ADR-0039](0039-warden-1-1-profissional.md) chamou de **Warden 1.1 "Profissional"** a versão seguinte à v1, com as seis funções da D5 (segurança dos mods, manutenção e substitutos, travamento de um jogador, notas e grupos, itens repetidos e desempenho entre versões): fase 7 do ROADMAP, tarefas W-01 a W-12, marco M6, prioridade e etiqueta "1.1" na SPEC (T28 a T33) e "Gancho 1.1" nas tarefas da v1.

Depois da 1.0.0 e do conserto 1.0.1, o dono mandou o relatório de bugs nº 1. O lote que saiu dele é grande e muda o uso do dia a dia: os consertos de funcionamento (loader estrito, o mesmo mod pelas duas fontes, Java sem janela, o app que se comporta como app; ADR-0061 a ADR-0063), as seis telas novas aprovadas em `design/bug-01/` (Sensor Sculk, Problemas com "Desempenho do último teste", Configs em três colunas, Adicionar ao pack com "Lista | Cartões", o diálogo Adicionar itens em árvore e Configurações com menu lateral; ADR-0064, ADR-0067, ADR-0068 e ADR-0070) e as chaves só no cofre ([ADR-0069](0069-chaves-so-no-cofre-do-sistema.md)). Não cabe num número de conserto (1.0.2), e a 1.1 já estava reservada para a Profissional.

## Decisão

- **A versão 1.1 do app (1.1.0) é o lote do relatório de bugs nº 1**, publicada depois da 1.0.1. As notas ficam em `docs/versoes/1.1.0.md`. O ROADMAP §13 registra a 1.1 com o que ela entrega, como os marcos da v1; ela não tem tarefas próprias no plano (foi feita fora dele, pelos lotes B1 e T1 a T5).
- **O Warden "Profissional" passa a ser a versão 1.2**: "Warden 1.2 \"Profissional\"", fase 7, tarefas W-01 a W-12 e marco M6 continuam com os mesmos nomes e números; o que muda é o número da versão. A prioridade e a etiqueta da SPEC passam de "1.1" para "1.2", e "Gancho 1.1" passa a "Gancho 1.2".
- **Documentos vivos atualizados** na REL-03: SPEC, ROADMAP, ARCHITECTURE, QUALITY, `docs/design/` (ESTRUTURA §14, DESIGN-SYSTEM, HANDOFF), `docs/DESEMPENHO.md`, `docs/ORQUESTRADOR.md`, `AGENTS.md`, `PRODUCT.md`, os comentários do código que falam do gancho ou das funções e os textos do protótipo e da galeria do design system (o selo "1.1" vira "1.2"; o identificador interno `v11` fica).
- **Registros que não se editam continuam com o nome antigo:** as ADR-0039 a ADR-0047 (e as outras ADRs que citam a 1.1), as pesquisas em `docs/research/`, as revisões em `docs/revisoes/`, os textos das pré-versões em `docs/pre-versoes/` e as notas já publicadas da 1.0.0 e da 1.0.1. Neles, **"1.1" ou "Warden 1.1 \"Profissional\"" quer dizer a versão 1.2 de hoje.** O índice das ADRs marca a 0039 e a 0045 com "hoje a 1.2".
- Não mudam os números que não são a versão da Profissional: as versões dos documentos (SPEC 1.1, design system 1.1), as seções numeradas (§1.1), as versões do Minecraft, de mods e de bibliotecas, `HTTP/1.1` e a licença SIL OFL 1.1.

## Alternativas consideradas

- **Lançar o lote como 1.0.2:** o número de conserto esconde as telas novas e a mudança das chaves para o cofre, que o dono precisa saber antes de instalar.
- **Manter a Profissional como 1.1 e chamar o lote de 1.0.x:** mesmo problema; e a 1.1 continuaria prometida para funções que ainda não começaram.
- **Editar as ADR-0039 a ADR-0047:** contraria a regra das ADRs (ADR-0001); a renumeração fica registrada aqui.

## Consequências

- Nenhum código ou teste dependia da string "1.1" como prioridade: só mudaram comentários, o teste do `slug` do xtask (o título da fase 7 virou `fase-7--warden-12-profissional`) e as âncoras dos sumários.
- A branch planejada da W-12 passa a ser `test/w-12-revisao-1-2`, e a seção do roteiro de aceite da W-12, "Warden 1.2".
- O xtask de pré-versão continua com os marcos M1 a M6; a 1.1 não tem pré-versão (vai direto como versão final, como a 1.0.1).
