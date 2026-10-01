# ADR-0026 — Estrutura de navegação do app

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (aprovação da tarefa D2)

## Contexto

O primeiro protótipo (D1) foi reprovado pela organização: menu que misturava itens do app e de um pack, abas dentro de abas, nomes que não diziam o que havia dentro e itens demais. A tarefa D2 propôs duas estruturas (`docs/design/ESTRUTURA.md`, rascunho em `design/estrutura/index.html`).

## Decisão

- **Alternativa A (seções do pack)**, aprovada pelo dono.
- **Dois níveis que não se misturam.** Nível do app: Meus packs (tela inicial, sem barra lateral), Criar pack, Abrir pack existente e Configurações (com "Sobre o Warden" como última seção). Nível do pack: tudo o que é de um pack aberto.
- **Pack aberto:** cabeçalho fixo (← Meus packs, nome com "Editar informações", Minecraft/loader/versão do pack, avisos passageiros, **Salvar versão · N alterações** e **▶ Testar ▾**) e menu lateral com **6 seções**, cada uma com nome e uma linha de descrição: Mods, Configs, Problemas, ✦ Diagnóstico com IA, Histórico, Exportar.
- **Testar** não é seção: é o botão principal do cabeçalho e abre a tela do teste, cujo resultado inclui "O que mudou durante o teste" e "Por que travou".
- **No máximo um nível de navegação visível**: nenhuma tela tem abas; filtros e modos usam caixas de seleção.
- **Tarefas** é uma gaveta aberta pelo indicador do rodapé, presente em todas as telas.
- As mudanças M1 a M12 da ESTRUTURA §7 foram aprovadas e aplicadas na SPEC.
- O ícone de brilhinho (✦/sparkles) marca o que é IA e só isso.

## Alternativas consideradas

- **Alternativa B (painel único do pack com páginas de detalhe):** mais guiada na primeira visita, mas cada troca de seção custa dois cliques. Descartada pelo dono.
- Estrutura do D1 (barra lateral única com 9 abas do pack): reprovada.

## Consequências

- SPEC §5, T05 e telas relacionadas reescritas; rotas e registro de seções do frontend seguem esta estrutura (ARCHITECTURE §18; ROADMAP F0-06, P1-08).
- Uma sétima seção exige nova decisão do dono.
