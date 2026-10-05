# ADR-0051 — Mods só da CurseForge: aviso ao publicar; chave de aplicativo e fork em segundo plano

- **Status:** aceita · **Data:** 2026-10-04 · **Origem:** decisão do dono (R7, decisão 4 aprovada; chave de aplicativo e fork do packwiz-installer em segundo plano, 04/10/2026)

## Contexto

Os jogadores recebem o pack pelo packwiz-installer-bootstrap ([ADR-0028](0028-publicacao-no-github.md)). O packwiz-installer baixa os mods da CurseForge direto da CDN, **sem chave**, e não recebe commit desde 06/2024. A pesquisa R7 (`docs/research/08-concorrencia-e-mercado.md` §8.3, risco R1) registrou que a CurseForge anunciou exigir chave também nos downloads da CDN (prazo de 16/07/2026), que o Prism e o FTB App mudaram o código para mandar a chave, e que hoje o download sem chave ainda funciona (302 → 200, verificado em 04/10/2026). Se a exigência voltar, os mods que só existem na CurseForge param de baixar para os jogadores, sem aviso. O pacote para servidor pelo link ([ADR-0035](0035-importar-e-pacote-para-servidor.md)) tem o mesmo risco.

## Decisão

- **Aviso ao publicar (v1):** o `publish_plan` (V-03) avisa quando a versão tem mods, resource packs ou shaders que **só existem na CurseForge** (referência `metadata:curseforge` sem equivalente no Modrinth): "N mods só existem na CurseForge e podem deixar de baixar para os jogadores se a CurseForge passar a exigir chave nos downloads." com a lista. Para cada um, quando o mesmo mod existe no Modrinth, **Trocar pelo Modrinth**: pelo mesmo arquivo (mesmo hash, `version_files`) quando houver; senão, pela versão compatível do mesmo projeto, achado pela regra de deduplicação da busca combinada ([ADR-0027](0027-busca-combinada.md): mesmo autor e mesmo nome), mostrada antes de trocar. É **aviso, não bloqueio**: o botão final continua "Publicar mesmo assim". A troca muda o pack e exige salvar e publicar uma versão nova, como a troca já existente para mods bloqueados.
- O aviso fica junto do aviso que já existe para mods com distribuição bloqueada (que continua: esses não baixam nunca, nem hoje).
- **Em segundo plano, fora da v1 e da 1.1 (SPEC §9):** pedir à CurseForge a chave oficial "de aplicativo" (a que vai embutida num programa distribuído) e fazer um fork do packwiz-installer que mande essa chave. Motivo: o risco ainda não aconteceu, o dono usa o Warden só para ele e os amigos, e manter um fork de um projeto Java parado é trabalho contínuo; o aviso, a preferência pelo Modrinth na busca e a instância pronta para o Prism ([ADR-0050](0050-instancia-pronta-para-o-prism.md), em que o Prism baixa com a chave dele) cobrem o caso comum. O orquestrador reabre o assunto se o "Testar como o jogador recebe" (L-07) começar a falhar nos mods da CurseForge.

## Alternativas consideradas

- **Bloquear a publicação** com mods só da CurseForge: impediria publicar packs que funcionam hoje.
- **Embutir os jars da CurseForge no pack:** só vale para mods que permitem; aumenta o repositório e exige confirmação de licença mod a mod.
- **Fazer o fork agora:** ver acima.

## Consequências

- A V-03 consulta o Modrinth por hash (`version_files`, já usado na publicação) e, sem hash igual, pela busca, só para os itens marcados; a parte da CurseForge fica só em memória.
- O mesmo dado ("só existe na CurseForge") serve à futura linha "Download pelos jogadores" da nota de saúde, que a R7 sugeriu; ela não entra agora.
