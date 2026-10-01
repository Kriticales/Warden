# ADR-0027 — Busca de mods combinada (Modrinth + CurseForge)

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (aprovação da tarefa D2) · **Substitui em parte:** ADR-0008 (busca por fonte separada e estado vazio da aba CurseForge)

## Contexto

A ADR-0008 previa buscas separadas por fonte (abas Modrinth e CurseForge na tela Adicionar). A estrutura aprovada tirou as abas, e o dono decidiu juntar as duas fontes num resultado só.

## Decisão

- Uma busca consulta **Modrinth e CurseForge ao mesmo tempo** e mostra **um resultado só**, com a fonte marcada em cada item ("Modrinth", "CurseForge" ou "Modrinth e CurseForge").
- **Sem duplicatas:** o mesmo mod nas duas fontes vira um item. Critério na lista: mesmo autor e mesmo nome ou slug normalizados. Na pré-visualização, o Warden confere o arquivo da versão escolhida pelo SHA-1 nas duas fontes e avisa se forem diferentes. O item unificado tem o seletor **Fonte** (padrão Modrinth, que informa o lado e tem cache local).
- **Ordem:** relevância intercalada pela posição em cada fonte, sem favorecer nenhuma; as outras ordens usam os valores de cada fonte.
- **Sem chave da CurseForge** (ou chave recusada): só Modrinth, com aviso e caminho para Configurações; nenhuma requisição à CurseForge. Uma fonte com erro não impede a outra.
- O resto da ADR-0008 continua: link colado (agora no mesmo campo da busca), arquivos locais, termos da CurseForge (nada persistido) e downloads manuais.

## Alternativas consideradas

- Fontes separadas (ADR-0008): obriga o usuário a saber onde cada mod está e repetir a busca.
- Deduplicar só por hash em toda a lista: exigiria uma requisição de versões por resultado no Modrinth a cada página; o hash fica para a pré-visualização.

## Consequências

- O motor de busca combinada fica em `warden-project` (ARCHITECTURE §17), com fontes plugáveis; P1-09 cria o motor com o Modrinth, P1-10 acrescenta a CurseForge (ROADMAP).
- Novos critérios CA-T08-08 e CA-T08-09; CA-T08-06 passa a exigir "só Modrinth com aviso".
