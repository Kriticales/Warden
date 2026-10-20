# ADR-0061 — Loader estrito ao adicionar, atualizar e trocar versão

- **Status:** aceita · **Data:** 2026-10-19 · **Origem:** regra do dono (bug-01, problemas 14 e 5) · **Complementa:** [ADR-0006](0006-integracao-hibrida-packwiz.md) e a ARCHITECTURE §6.2

## Contexto

Um pack NeoForge 1.21.1 recebeu `xaerominimap-forge-1.21.1-26.6.0.jar` e `xaeroworldmap-forge-1.21.1-1.47.0.jar`, builds de **Forge**. O Warden copiava o `GetCompatibleLoaders` do packwiz (`PackManifest::compatible_loaders`), que devolve `[neoforge, forge]` para qualquer pack NeoForge, em qualquer versão do Minecraft. Essa lista filtrava tudo (Modrinth, CurseForge, atualizações, "Trocar de versão", a marca "compatível" da busca e a descoberta de modpacks), e nenhuma fonte preferia o loader do pack: as candidatas eram ordenadas só pela data. Um arquivo Forge mais novo que o NeoForge entrava sem aviso. Só o NeoForge da 1.20.1 ainda carrega mods do Forge (ARCHITECTURE §6.2); da 1.20.2 em diante, não.

O diagnóstico tinha o mesmo furo: a regra `E_LOADER` pela API rebaixava para **aviso** um mod marcado só `forge` num NeoForge 1.20.2+, e nenhum `E_LOADER` trazia correção, então o card só oferecia "Ignorar".

## Decisão

Regra do dono: ao **adicionar**, **atualizar** ou **trocar a versão** de um mod, ele tem de ser para a versão do Minecraft do pack **e** para o loader do pack; a versão do loader não importa; entra a **última** versão que atende às duas condições.

- **Loaders estritos** (`PackManifest::strict_loaders`, na ordem de preferência):
  - NeoForge → `[neoforge]`; no Minecraft exatamente `1.20.1`, `[neoforge, forge]`;
  - Forge → `[forge]`;
  - Fabric → `[fabric]`;
  - Quilt → `[quilt, fabric]` (o Quilt carrega mods do Fabric).
- `PackTarget` (adicionar, busca, "Trocar de versão", descoberta) e as atualizações usam essa lista. `compatible_loaders` fica só para paridade com o packwiz (o que o binário aceitaria) e está documentado assim.
- **Ordem das candidatas:** pela posição do loader na lista do pack e, dentro do mesmo loader, a mais nova primeiro, nas duas fontes e nas atualizações. Um arquivo com as tags `forge` e `neoforge` conta como `neoforge`. Na atualização pelo Modrinth com mais de um loader (NeoForge 1.20.1), uma consulta por loader na ordem do pack; o seguinte só vale para os arquivos sem nenhuma versão do anterior.
- **Sem versão para o pack:** o mod vai para "Sem versão compatível". Nunca entra o de outro loader.
- **Diagnóstico:** `E_LOADER` pela API é **erro** também para um mod só `forge` num NeoForge 1.20.2+ (a regra do jar já era erro). Os dois `E_LOADER` (API e jar) trazem as correções **Trocar pela versão certa** (`SuggestedFix::ReplaceVersion`, só para itens do Modrinth e da CurseForge) e **Remover**. "Trocar pela versão certa" abre o "Trocar de versão" de sempre, que já começa na versão mais nova para o Minecraft e o loader do pack (a instalada não está na lista); confirmar grava o `.pw.toml` novo no lugar do errado numa transação só. Os dois `E_LOADER` do mesmo item viram um achado só, com as evidências das duas regras.

## Alternativas consideradas

- **Manter `compatible_loaders` e só ordenar pelo loader:** recusada. Sem versão NeoForge, o Forge continuaria entrando num pack 1.21.1, onde não carrega.
- **Uma variante `UpdateItem` mais `RemoveItem` em dois passos:** desnecessária. "Trocar de versão" já troca o `.pw.toml` do mesmo projeto numa transação, com o `packwiz refresh`.
- **Aceitar Forge em NeoForge até a 1.20.x:** recusada. O NeoForge da 1.20.2 em diante tem o próprio `neoforge.mods.toml` e não lê o `mods.toml` do Forge.

## Consequências

- Packs NeoForge 1.21+ deixam de receber builds de Forge ao adicionar, atualizar ou trocar de versão; a busca deixa de marcar como compatível um mod que só tem Forge.
- Um pack que já tem um mod de Forge num NeoForge 1.20.2+ passa a ver um **erro** em Problemas, com as duas correções.
