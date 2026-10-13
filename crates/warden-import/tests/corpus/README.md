# Corpus da importação (P1-19)

Arquivos de entrada dos testes de `warden-import` (CA-T24-01 a CA-T24-04). Cada um tem a origem
anotada abaixo; nada aqui tem licença que impeça o uso. Jars e arquivos grandes **não** ficam no
repositório: os testes montam o `.mrpack`, o zip e a instância numa pasta temporária, com bytes
gerados na hora, a partir destes arquivos.

| Arquivo | Origem | Licença |
|---|---|---|
| `create-plus/modrinth.index.json` | Recorte do `modrinth.index.json` real do modpack **Create+** 6.0.0 Alpha D, de Sshmoob, baixado do Modrinth em 2026-10-13 (`https://modrinth.com/modpack/create_plus`, versão `Tg9n3zcA`). Ficaram 9 dos 144 itens de `files[]`, sem nenhuma alteração: um de cada `env` que aparece no arquivo (inclusive o `"unknown"` do Status Effect Bars e o jar de `mods/.connector/temp/` que o próprio modpack lista em `files[]`) e mais dois comuns. O resto do arquivo (`formatVersion`, `game`, `versionId`, `name`, `summary`, `dependencies`) é o original. | LGPL-3.0-only (licença do projeto no Modrinth). Os itens são só metadados públicos (caminho, hashes, link do CDN) de mods de terceiros, que continuam com as licenças deles. |
| `create-plus/xmcl.json` | O `xmcl.json` (arquivo do X Minecraft Launcher) que vem na raiz do mesmo `.mrpack`, sem alteração. | LGPL-3.0-only (parte do modpack). |
| `create-plus/overrides.txt` | Lista, escrita para os testes, de caminhos de `overrides/` tirados do mesmo `.mrpack` (`.mixin.out/`, `mods/.connector/temp/`, `*.toml.bak`, `cache/`, `emi.json`, configs, um jar solto). O teste cria cada arquivo com bytes gerados. | Escrito para o Warden. |
| `prism-instancia/**` | Instância do Prism Launcher escrita para os testes, no formato que o Prism Launcher 11 grava (`instance.cfg`, `mmc-pack.json` e os metadados packwiz em `mods/.index/`, com os campos `x-prismlauncher-*`). Os IDs e hashes do Sodium são os da versão `SMxNOGZ6` do Modrinth, já usados nas fixtures de `warden-modrinth`. | Escrito para o Warden. |
| `curseforge-zip/manifest.json` | Manifesto de zip da CurseForge escrito para os testes, no formato que o app da CurseForge exporta (`manifestType: "minecraftModpack"`). Os projetos e arquivos são os da CurseForge simulada dos testes. | Escrito para o Warden. |
