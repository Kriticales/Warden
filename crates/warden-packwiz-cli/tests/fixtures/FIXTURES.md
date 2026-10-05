# Fixtures da `warden-packwiz-cli`

## `saida/`: saída real do packwiz, byte a byte

Gravadas em 2026-10-04 com o sidecar da F0-03 (packwiz `ef87d964f8cbd52b3b13ea42453ef322290e2b9e` + patch 0001, `packwiz-x86_64-pc-windows-msvc.exe`), no Windows 11, com stdout e stderr num arquivo (sem terminal), como o Warden executa. As opções globais foram as do Warden (`--config`, `--cache`, `--pack-file pack.toml` com a pasta do pack como diretório de trabalho); a pasta `C:\wt\p1-02` foi escolhida só para o caminho impresso na primeira linha não conter dados pessoais.

| Arquivo | Comando | Pack |
|---|---|---|
| `refresh.bin` | `refresh` (código 0) | cópia de `crates/warden-packwiz/tests/fixtures/packwiz-output/fabric-1.21.1` com mais 400 arquivos em `config/`, para a barra de progresso do `mpb` redesenhar várias vezes (`ESC[1A ESC[J` antes de cada redesenho) |
| `curseforge-add-sem-chave.bin` | `curseforge add -- https://www.curseforge.com/minecraft/mc-mods/jei/files/5101366` sem `WARDEN_CURSEFORGE_API_KEY` (código 1) | o mesmo |
| `refresh-pack-invalido.bin` | `refresh` (código 1) | `pack.toml` com `pack-format = "packwiz:9.9.9"` |

Cada `<nome>.linhas.txt` é o resultado esperado da decodificação (`output::LineDecoder`): as linhas limpas, uma por linha, escritas à mão a partir da leitura dos bytes com `cat -v`. O teste `ca_4_saida_real_vira_linhas_limpas` compara os dois e confere as porcentagens da barra de progresso (`refresh.bin`: 21, 42, 66, 87, 100, 100).

Os `.bin` não têm `\r` (o packwiz usa `\n` também no Windows), então a normalização de fim de linha do git (`text=auto`) não os altera. Ainda assim, convém acrescentar `crates/warden-packwiz-cli/tests/fixtures/** -text` ao `.gitattributes` da raiz (posse da F0-01), como já existe para as fixtures da `warden-packwiz`.
