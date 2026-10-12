# Respostas reais da API do Modrinth (testes de "Adicionar", P1-09)

Gravadas em 07/10/2026 com `curl`, User-Agent `Kriticales/Warden/0.1.0 (+https://github.com/Kriticales/Warden)`,
direto de `https://api.modrinth.com/v2`. Sem alteração: o conteúdo é o que a API devolveu. Os projetos
são públicos; os textos e imagens pertencem aos autores (licenças nas próprias respostas). Usadas
só nos testes da `warden-project` (`tests/add.rs`). As respostas de 04/10/2026 da
`warden-modrinth` (`crates/warden-modrinth/tests/fixtures/http/`) também são usadas.

| Arquivo | Pedido |
|---|---|
| `2026-10-07-projects-add.json` | `GET /projects?ids=["mOgUt4GM","EsAfCjCV","P7dR8mSH","eXts2L7r","AANobbMI","YL57xq9U","sk9rgfiA","4ZqxOvjD","S1tndFDa"]` (Mod Menu, AppleSkin, Fabric API, Text Placeholder API, Sodium, Iris, Embeddium, Rubidium, TexTrue's Embeddium Options) |
| `2026-10-07-versions-modmenu-fabric-1.21.1.json` | `GET /project/modmenu/version?loaders=["fabric"]&game_versions=["1.21.1"]&include_changelog=false` (exige Fabric API e Text Placeholder API) |
| `2026-10-07-versions-appleskin-fabric-1.21.1.json` | `GET /project/appleskin/version?…` (mesmos filtros; exige Fabric API) |
| `2026-10-07-versions-iris-fabric-1.21.1.json` | `GET /project/iris/version?…` (mesmos filtros; a beta exige a versão `s7adptIg` do Sodium) |
| `2026-10-07-versions-fabric-api-fabric-1.21.1.json` | `GET /project/P7dR8mSH/version?…` (mesmos filtros) |
| `2026-10-07-versions-placeholder-api-fabric-1.21.1.json` | `GET /project/eXts2L7r/version?…` (mesmos filtros) |
| `2026-10-07-versions-sodium-neoforge-1.21.1.json` | `GET /project/AANobbMI/version?loaders=["neoforge"]&game_versions=["1.21.1"]&include_changelog=false` |
| `2026-10-07-versions-ids-s7adptIg.json` | `GET /versions?ids=["s7adptIg"]` (Sodium 0.8.12-beta.1 para Fabric) |

Os parâmetros vão codificados na URL (com colchetes crus a API responde 400).

**Incompatibilidades reais (CA-T09-02):** em 07/10/2026 as versões do Embeddium para NeoForge 1.21.1
declaram `incompatible` só com o Rubidium (`4ZqxOvjD`) e com o TexTrue's Embeddium Options
(`S1tndFDa`); nenhuma versão do Sodium para NeoForge 1.21.1 declara o Embeddium. O teste com dados
reais usa o Rubidium; o caso "pack com Sodium" do critério é coberto com a mesma resposta do Embeddium
acrescida de uma dependência `incompatible` com o Sodium, marcada no teste como simulada.

## Mods iniciais (P1-18, `tests/initial_mods.rs`)

Gravadas em 07/10/2026, do mesmo jeito (sem alteração, `include_changelog=false`):

| Arquivo | Pedido |
|---|---|
| `2026-10-07-projects-initial.json` | `GET /projects?ids=["l6YH9Als","ix1qq8Ux"]` (spark e Crash Assistant) |
| `2026-10-07-versions-spark-fabric-1.21.1.json` | `GET /project/l6YH9Als/version?loaders=["fabric"]&game_versions=["1.21.1"]` (a versão do Fabric **não declara** a Fabric API como dependência, por isso `initial-mods.toml` a lista em `with`) |
| `2026-10-07-versions-crash-assistant-fabric-1.21.1.json` | `GET /project/ix1qq8Ux/version?…` (mesmos filtros) |
| `2026-10-07-versions-crash-assistant-forge-1.12.2.json` | `GET /project/ix1qq8Ux/version?loaders=["forge"]&game_versions=["1.12.2"]` |

A CurseForge não tem respostas gravadas aqui: os itens dela (spark 1.10.19 e 1.6.3) só aparecem
desabilitados na oferta enquanto a fonte não estiver ligada (CA-T03-08).
