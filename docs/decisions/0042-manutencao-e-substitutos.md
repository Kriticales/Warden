# ADR-0042 — Manutenção dos mods e substitutos

- **Status:** aceita · **Data:** 2026-10-02 · **Origem:** decisão do dono (tarefa D5, decisão D28)

## Contexto

Mods somem das plataformas, são arquivados ou param de ser atualizados. Quando o arquivo exato do pack some, os jogadores não conseguem mais baixar o pack. O packwiz não confere nada disso; o Prism Launcher só avisa projeto arquivado no instalador (PR #1979). As plataformas dão os sinais, mas os lotes **omitem itens sem avisar**: o Warden precisa comparar o que pediu com o que voltou.

## Decisão

- **Sinais do Modrinth:** projeto ausente no lote `GET /projects?ids=` ou com 404; `status = archived`; hash ausente em `POST /version_files`. Desde 13/08/2026, "arquivado" também é uma *content disclosure* na busca, filtrável por `disclosure_types!=archived`.
- **Sinais da CurseForge:** ausente em `POST /v1/mods` ou em `POST /v1/mods/files`; ModStatus 7 (Inactive), 8 (Abandoned) e 9 (Deleted); `isAvailable`; FileStatus 5, 6, 7, 8 e 12.
- **Frequência:** junto com a verificação de atualizações ao abrir o pack. Modrinth no máximo uma vez a cada 24 h, com cache. CurseForge refeita a cada execução do Warden e mantida só em memória, porque os termos da API (§3.1) proíbem guardar ou fazer cache dos dados (<https://support.curseforge.com/en/support/solutions/articles/9000207405-curse-forge-3rd-party-api-terms-and-conditions>).
- **Obrigatória antes de publicar**, sem cache. "Arquivo removido" bloqueia a publicação; as outras situações são aviso ou informação.
- **Substitutos:** busca na mesma plataforma do mod, com as mesmas categorias, a versão do Minecraft e o loader do pack, sem arquivados e sem o que já está no pack, cada candidato com os motivos reais da sugestão; antes deles, o substituto conhecido da lista curada `obsolete.toml`. "Trocar por este" adiciona o novo e remove o antigo numa transação só, com ponto de segurança.
- Código em `warden-diagnostics::maintenance`.

## Alternativas consideradas

- Verificar só ao publicar: o usuário descobre o problema tarde, já com a versão pronta.
- Guardar as respostas da CurseForge como as do Modrinth: contraria os termos da API.
- Substitutos por semelhança de texto ou por IA: motivos difíceis de explicar e de testar; categorias e filtros da própria plataforma bastam.
- Buscar substitutos nas duas plataformas: a troca mudaria a fonte do item, e as categorias do Modrinth e da CurseForge não se correspondem.

## Consequências

- Usa os campos que o cache do Modrinth já guarda desde a v1 ([ADR-0039](0039-warden-1-1-profissional.md), gancho 7) e a lista de checagens plugáveis do `publish_plan` (gancho 6).
- SPEC T29; a página mora em Problemas ([ADR-0045](0045-funcoes-1-1-na-estrutura.md)).
