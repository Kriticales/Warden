# portablemc (cópia emendada do Warden)

O motor do launcher do Warden é o crate [`portablemc`](https://github.com/theorzr/portablemc) 5.0.5 (licença Apache-2.0, em [`LICENSE`](LICENSE)), escolhido no spike S1 ([ADR-0057](../../docs/decisions/0057-motor-do-launcher-portablemc.md)). Esta pasta é uma cópia do crate publicado no crates.io, com patches pequenos e versionados do Warden, no mesmo modelo da cópia do packwiz ([ADR-0049](../../docs/decisions/0049-copia-fixa-do-packwiz-e-patches.md); [`third_party/packwiz/README.md`](../packwiz/README.md)). O `Cargo.toml` da raiz do repositório troca o crate do crates.io por esta pasta (`[patch.crates-io]`), e a pasta fica fora do workspace (`exclude`), sem os lints e testes do Warden. Decisão: [ADR-0062](../../docs/decisions/0062-emenda-do-portablemc-java-sem-janela.md).

| Arquivo | O que é |
|---|---|
| `Cargo.toml`, `Cargo.toml.orig`, `README.md`, `src/`, `tests/`, `.cargo_vcs_info.json` | O crate como saiu do crates.io, com os patches já aplicados. O `Cargo.lock` do pacote foi deixado de fora (o do Warden vale). |
| [`patches/`](patches/) | Os patches do Warden, em ordem de nome, no formato do `git apply` (caminhos relativos a esta pasta). |
| [`ORIGINAL.sha256`](ORIGINAL.sha256) | SHA-256 de cada arquivo do crate original, antes dos patches. |
| [`LICENSE`](LICENSE) | Licença Apache-2.0 do portablemc, do repositório de origem no commit do pacote (o `.crate` publicado não traz o arquivo). |

## Versão de origem

- Crate `portablemc` 5.0.5 do crates.io, checksum do `.crate` `bd8558231180f57fefd5c1bc13561df68bae7d67080f43b52b2fa6f57f4fcd0f` (o que estava no `Cargo.lock` antes da cópia).
- Commit de origem no repositório `theorzr/portablemc`: `99fdead1eb974cdf29a5d16fb2c1554a8c159a9e` (pasta `portablemc/`, segundo o `.cargo_vcs_info.json`).

O teste `portablemc::tests::copia_e_o_crate_original_mais_os_patches` do xtask (roda no `cargo xtask check`) desfaz os patches numa pasta temporária e exige que cada arquivo volte ao SHA-256 de `ORIGINAL.sha256`, sem arquivo a mais ou a menos. Mudar qualquer coisa no crate sem um patch faz o teste falhar.

## Patches

### `0001-java-sem-janela-no-windows.patch`

O Warden é um app de janela, sem console. No Windows, todo programa de console aberto por ele sem a opção `CREATE_NO_WINDOW` ganha um console novo, com janela. O portablemc roda o `java.exe` que o Warden entrega (`JvmPolicy::Static`) em dois lugares sem essa opção: o `java -version` de `find_jvm_versions` (`src/base/mod.rs`), a cada instalação, ou seja, a cada teste do pack; e cada "processor" do instalador do Forge e do NeoForge (`try_install`, `src/forge/mod.rs`), na primeira instalação. Cada um piscava uma janela preta de terminal.

O patch só acrescenta `CREATE_NO_WINDOW` (`0x08000000`) a esses dois comandos, no Windows. O `Game::command` (`src/base/mod.rs`) não muda: ele só monta o comando do jogo, que o Warden não usa (o Warden monta e abre o processo do jogo em `warden_launcher::process`, já com a opção).

O teste `crates/warden-launcher/tests/java_sem_janela.rs` roda a instalação de uma versão mínima, sem rede, a partir de um processo sem console (como o Warden), com um Java falso (`crates/warden-launcher/src/bin/java_fake.rs`) que grava o `GetConsoleWindow()` dele. No Windows exige `0` (sem janela); sem o patch, o Java falso ganha um console com janela. Os processors do Forge não têm teste automático (exigem o instalador real do Forge e a rede); a mudança neles é a mesma linha.

Correção a propor ao autor do portablemc (o Warden não abriu a issue): usar `CREATE_NO_WINDOW` nesses comandos no Windows.

## Como atualizar o portablemc

Uma versão nova do portablemc passa pela matriz L-05 (ADR-0057). Para trocar a cópia: baixar o crate novo (`cargo fetch` com a versão nova no `Cargo.toml`, sem o `[patch]`), copiar os arquivos acima para esta pasta, regravar `ORIGINAL.sha256` a partir do crate baixado (`sha256sum` dos arquivos, caminhos relativos), aplicar cada patch com `git apply` (refazendo o que não aplicar) e atualizar esta página e o `THIRD_PARTY.md`. Se o autor corrigir o problema de um patch, o patch sai.
