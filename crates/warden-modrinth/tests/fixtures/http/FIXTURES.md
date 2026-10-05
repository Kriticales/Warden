# Respostas reais da API do Modrinth

Gravadas em 04/10/2026 com `curl`, User-Agent `Kriticales/Warden/0.1.0 (+https://github.com/Kriticales/Warden)`,
direto de `https://api.modrinth.com/v2`. Sem alteração: o conteúdo é o que a API devolveu. Os projetos
são públicos; os textos e imagens pertencem aos autores (licenças nas próprias respostas). Usadas
só nos testes da `warden-modrinth` (`tests/api.rs`).

| Arquivo | Pedido |
|---|---|
| `2026-10-04-search-sodium-fabric-1.21.1.json` | `GET /search?query=sodium&limit=5&facets=[["categories:fabric"],["versions:1.21.1"],["project_type:mod"]]` |
| `2026-10-04-project-sodium.json` | `GET /project/sodium` |
| `2026-10-04-projects-3.json` | `GET /projects?ids=["AANobbMI","P7dR8mSH","gvQqBUqZ"]` (Sodium, Fabric API, Lithium) |
| `2026-10-04-project-sodium-versions-fabric-1.21.1.json` | `GET /project/AANobbMI/version?loaders=["fabric"]&game_versions=["1.21.1"]&include_changelog=false` |
| `2026-10-04-project-embeddium-versions.json` | `GET /project/embeddium/version?loaders=["neoforge"]&game_versions=["1.21.1"]&include_changelog=false` (dependências `incompatible`) |
| `2026-10-04-version-SMxNOGZ6.json` | `GET /version/SMxNOGZ6` (Sodium 0.8.13 para Fabric 1.21.1) |
| `2026-10-04-versions-2.json` | `GET /versions?ids=["SMxNOGZ6","QV48eyCs"]` |
| `2026-10-04-version_files-sha1.json` | `POST /version_files` com `{"hashes":["003c114c…","2172d0e9…","0000…0000"],"algorithm":"sha1"}` (o hash zerado não volta) |
| `2026-10-04-version_files-update-sha1.json` | `POST /version_files/update` com `{"hashes":["2172d0e9…","d01f2112…","0000…0000"],"algorithm":"sha1","loaders":["fabric"],"game_versions":["1.21.1"]}` |
| `2026-10-04-tag-loader.json` | `GET /tag/loader` |
| `2026-10-04-tag-game_version.json` | `GET /tag/game_version` |
| `2026-10-04-tag-category.json` | `GET /tag/category` |

Os facets e as listas vão codificados na URL (com colchetes crus a API responde 400). O projeto
inexistente (`GET /project/warden-nao-existe-xyz`) respondeu 404 com corpo vazio; por isso não há
arquivo para ele.
