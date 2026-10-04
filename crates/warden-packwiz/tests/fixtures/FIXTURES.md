# Fixtures da `warden-packwiz`

Geradas por `cargo xtask fixtures-packwiz` (código em `xtask/src/fixtures_packwiz.rs`, com a
lista exata de comandos e IDs). Não edite à mão: rode o comando de novo. O `.gitattributes` da
raiz marca esta pasta com `-text`, para os bytes (e os hashes do `index.toml`) não mudarem.

## Origem

- **packwiz** `packwiz/packwiz` (MIT), commit `ef87d964f8cbd52b3b13ea42453ef322290e2b9e`, compilado pela F0-03 com o patch
  `0001-chave-curseforge-em-tempo-de-execucao.patch` (chave da CurseForge do `.env` do dono,
  passada por variável de ambiente; nenhuma chave fica gravada aqui).
- Chamado sempre de dentro da pasta do pack com `--pack-file pack.toml` relativo,
  `--config` vazio e `--cache` temporário.
- Os mods são escolhidos por ID de projeto e de versão (Modrinth) ou de projeto e de arquivo
  (CurseForge). Nenhum jar é versionado: os metafiles só têm nome, link, hash e IDs.

## Pastas

- `packwiz-output/<pack>/`: o pack inteiro como o packwiz deixou (`pack.toml`, `index.toml`,
  metafiles e os arquivos comuns usados). Cobre Fabric, Forge (1.7.10, 1.12.2, 1.20.1),
  NeoForge (1.20.1 e 1.21.1), Quilt, Modrinth (mod, resource pack, shader), CurseForge,
  link direto, `pin`, `[option]`, `preserve`, `[options]`, `[export.curseforge]`,
  `description`, `no-internal-hashes` e índice com `hash-format` diferente de `sha256`.
- `packwiz-output/dados-api.json`: dados das APIs usados em cada metafile. Do Modrinth, o
  necessário para montar o metafile (inclusive `environment`, que decide o lado); da
  CurseForge, só nome, slug, IDs, nome do arquivo e `sha1` (os termos da CurseForge não
  permitem guardar as respostas; esses campos já estão no próprio `.pw.toml`).
- `ignore-100/`: 100 caminhos (`caminhos.txt`), o `.packwizignore` usado
  (`packwizignore.txt`: o padrão do Warden mais linhas do usuário) e os arquivos que o
  `packwiz refresh` pôs no índice: `indexados-pack-file-relativo.txt` (chamado com
  `--pack-file pack.toml`) e `indexados-pack-file-absoluto.txt` (chamado com o caminho
  absoluto do `pack.toml`).
- `murmur2/vetores.txt`: tamanho, dados em hexadecimal, murmur2 da CurseForge e murmur2 puro
  (semente 1), calculados pela biblioteca Go que o packwiz usa
  (`github.com/aviddiviner/go-murmur` v0.0.0-20150519214947-b9740d71e571, MIT) com o programa
  `murmur2/gerar_vetores.go.txt`.

## Achado: `--pack-file` absoluto quebra os padrões ancorados

O packwiz compara o `.packwizignore` com o caminho que o percurso da pasta produz. Com
`--pack-file pack.toml` (relativo), esse caminho é relativo à pasta do pack e `/logs/` funciona.
Com `--pack-file C:\...\pack.toml` (absoluto), o caminho é absoluto e todo padrão ancorado
(começando por `/`) deixa de casar: compare os dois arquivos `indexados-*` de `ignore-100/`.
O Warden deve chamar o packwiz com o `pack.toml` relativo e a pasta do pack como pasta atual.
