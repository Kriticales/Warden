# packwiz (sidecar do Warden)

O Warden lê e escreve os arquivos do pack em Rust e usa o binário do [packwiz](https://github.com/packwiz/packwiz) (licença MIT, em [`LICENSE`](LICENSE)) para refresh, validação e exportação ([ADR-0006](../../docs/decisions/0006-integracao-hibrida-packwiz.md), [ADR-0007](../../docs/decisions/0007-sidecar-packwiz.md), [ARCHITECTURE §6.3](../../docs/ARCHITECTURE.md#63-sidecar-do-packwiz-adr-0007)). O packwiz não publica versões, então o Warden compila o próprio binário a partir de um commit fixado, com patches pequenos e versionados aqui.

| Arquivo | O que é |
|---|---|
| [`COMMIT`](COMMIT) | Hash completo do commit do packwiz usado no build. |
| [`patches/`](patches/) | Patches aplicados em ordem de nome, com `git apply`. |
| [`LICENSE`](LICENSE) | Licença MIT do packwiz (vai nos avisos de terceiros do instalador, A-02). |

## Commit escolhido

`ef87d964f8cbd52b3b13ea42453ef322290e2b9e` (2026-09-06, "Proper err msg when installing from repository with no releases", #411), o mesmo analisado na pesquisa R3.

Conferido em 2026-10-04: é a ponta da `main` do packwiz, a única branch do repositório, e não há tags nem releases. Não existe commit mais novo para avaliar. Os pull requests abertos que interessariam ao Warden não foram integrados pelo autor e ficam de fora (o Warden não aplica código que não foi revisado lá):

- #391, filtro de loaders irrelevantes na comparação de versões;
- #381, limite de requisições ao Modrinth no `update` (o Warden checa atualizações em Rust, pela API, com limite próprio; R3 §1.13);
- #394 e #401, pasta de saída nas exportações (o Warden escolhe o caminho do arquivo e move depois).

Em relação a versões mais antigas, este commit traz o que o Warden precisa: os dados novos de ambiente (cliente/servidor) do Modrinth (#407), NeoForge 26.1 em diante (#386), a busca de versões de loaders refeita (#373) e downloads sempre pelo cache (#362).

## Patches

### `0001-chave-curseforge-em-tempo-de-execucao.patch`

Em `curseforge/request.go`, a chave da API da CurseForge passa a vir da variável de ambiente `WARDEN_CURSEFORGE_API_KEY`, lida a cada requisição. Sem ela (ou só com espaços), toda requisição à CurseForge falha **antes de sair da máquina**, e o comando imprime um erro que começa por:

```
WARDEN_CURSEFORGE_API_KEY ausente: a chave da CurseForge não foi informada.
```

Nada usa mais a chave embutida do packwiz: a função que decodificava `cfApiKeyDefault` e a variável trocável por `-ldflags -X` foram removidas, sem recuo para a chave do autor do packwiz, que pede a projetos derivados que usem a própria chave. A constante em si fica no código, sem uso, de propósito: o Go não grava constantes não usadas no executável, e assim o texto da chave não entra no repositório do Warden (o patch tem só uma linha de contexto e cabeçalhos de hunk sem o nome da função, para não citá-la). O build confere a ausência em cada executável (a chave original, em base64 e decodificada, não pode aparecer nos bytes), e o teste `ca2_chave_embutida_do_packwiz_nao_existe_nos_executaveis` repete a conferência. Com uma chave inválida, a CurseForge responde 403 e o comando falha (`rede_ca2_chave_invalida_e_recusada_sem_recuo_para_a_embutida`).

Quem chama o packwiz (P1-02) define `WARDEN_CURSEFORGE_API_KEY` só nos comandos que falam com a CurseForge e reconhece a falta da chave pelo começo da mensagem acima (constante `MISSING_KEY_MESSAGE` do `xtask/src/packwiz.rs`).

## Build

```powershell
cargo xtask build-packwiz          # pula se o commit e os patches não mudaram
cargo xtask build-packwiz --force  # compila de novo mesmo assim
```

`cargo xtask setup` já chama o `build-packwiz`. Requer o Go 1.24 ou mais novo e git; a primeira vez precisa de rede (baixa só o commit fixado, num clone raso, e os módulos Go do `go.sum`).

O comando:

1. clona o commit em `<target>/packwiz/src` (cache por worktree, apagado pelo `cargo clean`);
2. compila um executável **sem patch** do sistema atual em `<target>/packwiz/referencia/`, só para os testes compararem o comportamento e para provar que a busca pela chave funciona (nele a chave existe);
3. aplica os patches e compila, com `CGO_ENABLED=0 GOARCH=amd64 GOTOOLCHAIN=local go build -trimpath -buildvcs=false -ldflags="-s -w"`:
   - `apps/desktop/src-tauri/binaries/packwiz-x86_64-pc-windows-msvc.exe` (`GOOS=windows`, PE32+ de console);
   - `apps/desktop/src-tauri/binaries/packwiz-x86_64-unknown-linux-gnu` (`GOOS=linux`, ELF de 64 bits estático), por compilação cruzada do Go, sem WSL;
4. confere cada executável (cabeçalho, mensagem do patch presente, chave ausente) e só então grava `binaries/packwiz.commit` (o commit, para a tela Sobre) e `binaries/packwiz.build.toml` (o registro que permite pular o próximo build).

Os nomes `packwiz-<triplo>[.exe]` são os que o Tauri procura para `bundle.externalBin: ["binaries/packwiz"]`; no instalador, o binário vai para junto do executável do Warden sem o sufixo. Nada da pasta `binaries/` é versionado.

O build pula quando o registro tem o mesmo commit, os mesmos patches (SHA-256) e a mesma receita, e os executáveis gravados têm os SHA-256 registrados. Qualquer diferença (inclusive um executável apagado ou alterado) refaz o build.

Medido nesta máquina (Windows 11, Go 1.26.5) em 2026-10-04:

| Situação | Tempo |
|---|---|
| Primeiro build, sem cache do Go nem módulos baixados | 49 s |
| Segundo build (nada mudou, só a conferência dos SHA-256) | 1 s |

Com o mesmo Go, o build é reproduzível: a compilação a frio e a compilação com cache deram executáveis idênticos byte a byte.

## Cuidado ao chamar o packwiz

Com `--pack-file` absoluto (ARCHITECTURE §6.3), passe também `--meta-folder-base` com a pasta absoluta do pack. Sem isso, todo `add` (CurseForge, Modrinth, URL) falha com `Rel: can't make mods\<nome>.pw.toml relative to <pasta do pack>` **depois** de gravar o metadado da primeira dependência, deixando um `.pw.toml` fora do índice. O defeito é do packwiz original, não do patch: o teste `rede_pack_file_absoluto_exige_meta_folder_base` mostra o mesmo comportamento nos dois executáveis, com e sem patch.

## Atualizar o commit

1. Veja os commits novos em <https://github.com/packwiz/packwiz/commits/main> e leia cada um (o que muda no formato, nos comandos e nas APIs).
2. Troque o hash em [`COMMIT`](COMMIT) (sempre o hash completo, 40 dígitos).
3. Rode `cargo xtask build-packwiz`. Se um patch não se aplicar, refaça-o sobre o commit novo (próxima seção).
4. Rode `cargo xtask check` e `cargo xtask test-network`, e a suíte de integração com o packwiz das crates que o usam (ADR-0007: atualizar o commit é uma tarefa própria).
5. Atualize a linha do packwiz no [`THIRD_PARTY.md`](../../THIRD_PARTY.md) e esta página (commit escolhido e motivo).

## Criar ou refazer um patch

```powershell
cargo xtask build-packwiz --force   # deixa o clone do cache com os patches atuais aplicados
cd target\packwiz\src
git add -A                          # marca o estado atual como base do patch novo
# edite o código; rode `go vet ./...` e `gofmt -l .`
git diff > ..\..\..\third_party\packwiz\patches\0002-<assunto>.patch   # só as edições novas
```

Gere os diffs com `git diff -U1` se o contexto padrão citar a constante da chave, e confira que o patch não contém o texto dela. Cada patch começa com um parágrafo em português explicando o porquê (o `git apply` ignora o texto antes do primeiro `diff --git`) e muda o mínimo possível. Para refazer um patch que não se aplica mais, aplique à mão os anteriores no clone limpo, marque com `git add -A`, refaça a mudança e gere o diff do mesmo jeito. O `build-packwiz` seguinte descarta as mudanças locais do clone e recompila sozinho, porque os patches mudaram.
