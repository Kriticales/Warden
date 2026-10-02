# ADR-0044 — Notas e grupos em `.warden/mods.toml`

- **Status:** aceita · **Data:** 2026-10-02 · **Origem:** técnica (tarefa D5, decisão D30)

## Contexto

O dono quer anotar por que cada mod está no pack e juntar mods em grupos seus. O lugar óbvio seria o próprio metafile (`.pw.toml`). Verificação no packwiz, commit `ef87d964f8cbd52b3b13ea42453ef322290e2b9e`:

- O metafile é lido para uma struct fixa com BurntSushi/toml v1.5.0 (`core/mod.go`, linhas 15 a 27; `LoadMod` descarta o `MetaData`) e regravado inteiro (`Write`, linhas 89 a 119).
- Teste prático: `refresh` não regrava metafiles (campos extras sobrevivem), mas `pin`, `unpin`, `update`, `rehash` e `add` apagam campos e tabelas desconhecidos.
- Chaves dentro de `[update.modrinth]` sobrevivem a `pin`, `update` e `rehash`, mas somem no `add`. Uma tabela `[update.x-warden]` quebra o packwiz ("Update plugin x-warden not found!").
- `pack.toml` e `index.toml` perdem campos extras no `refresh`.
- O packwiz-installer (4koma) ignora campos desconhecidos.

## Decisão

- Notas e grupos ficam num arquivo do Warden, **`.warden/mods.toml`**, que já está fora do índice pelo bloco obrigatório `/.warden/` do `.packwizignore`.
- Cada entrada é ligada ao item pelo caminho do metafile e pelo projeto (Modrinth ou CurseForge), para sobreviver a atualizações, trocas de versão e renomeações.
- O arquivo é versionado com o pack (entra em "Voltar para esta versão"), mas não vai para os jogadores nem para o GitHub.
- As notas só entram no resumo da versão quando o usuário marca "Incluir as notas dos mods no resumo".

## Alternativas consideradas

- Campos extras no `.pw.toml`: mesmo que o Warden não use `pin` nem `update` do packwiz (ARCHITECTURE §6.1), qualquer uso manual do packwiz apagaria as notas.
- Chaves dentro de `[update.modrinth]`: somem no `add`, não servem para itens da CurseForge nem para arquivos locais, e misturam dados do Warden com os do plugin de atualização.
- Tabela própria `[update.x-warden]`: quebra o packwiz.
- Guardar nos dados locais do Warden: as notas são do pack, não do computador, e se perderiam ao trocar de máquina.

## Consequências

- Usa os ganchos da v1: leitor de `.warden/` que preserva o que não conhece (nome `.warden/mods.toml` reservado) e chave estável de item com agrupamento genérico na lista de Mods ([ADR-0039](0039-warden-1-1-profissional.md), gancho 4).
- Remover um mod remove a nota dele na mesma transação; os grupos ficam.
- SPEC T31.
