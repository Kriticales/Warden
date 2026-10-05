# Respostas reais das fontes do catálogo

Gravadas em 05/10/2026 com `curl`, User-Agent `Kriticales/Warden/0.1.0 (+https://github.com/Kriticales/Warden)`,
direto das fontes oficiais. Sem alteração: o conteúdo é o que cada servidor devolveu (nenhum tem
`\r`). São dados públicos de versões; usados só nos testes da `warden-catalog` (`tests/fontes.rs`).

| Arquivo | Pedido |
|---|---|
| `2026-10-05-mojang-version_manifest_v2.json` | `GET https://piston-meta.mojang.com/mc/game/version_manifest_v2.json` (917 versões; `latest.release` = `26.3`) |
| `2026-10-05-mojang-1.7.10.json` | `GET https://piston-meta.mojang.com/v1/packages/ed5d8789ed29872ea2ef1c348302b0c55e3f3468/1.7.10.json` (o `sha1` do arquivo é o do caminho) |
| `2026-10-05-fabric-versions-game.json` | `GET https://meta.fabricmc.net/v2/versions/game` |
| `2026-10-05-fabric-versions-loader.json` | `GET https://meta.fabricmc.net/v2/versions/loader` |
| `2026-10-05-forge-maven-metadata.xml` | `GET https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml` (5058 versões) |
| `2026-10-05-forge-promotions_slim.json` | `GET https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json` |
| `2026-10-05-neoforge-versions-neoforge.json` | `GET https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge` (1778 versões) |
| `2026-10-05-neoforge-versions-forge.json` | `GET https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/forge` (NeoForge da 1.20.1) |

Conferido na mesma data, para as regras do catálogo:

- `forge-1.7.10-10.13.4.1614-1.7.10-installer.jar` existe no Maven do Forge (200): o sufixo
  `-1.7.10` fica no artefato e sai da versão do `pack.toml`;
- `net/neoforged/forge/47.1.82/forge-47.1.82-installer.jar` não existe (404), embora o `47.1.82`
  apareça na lista da API: fica fora do catálogo;
- `net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar` existe (200).
