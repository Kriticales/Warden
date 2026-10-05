# ADR-0057 — Motor do launcher: `portablemc` 5.0.5, com processo e linha de comando do Warden

- **Status:** aceita · **Data:** 2026-10-05 · **Origem:** decisão técnica (spike S1; tarefa L-02) · **Complementa:** [ADR-0010](0010-launcher-offline-motor-abstrato.md)

## Contexto

A [ADR-0010](0010-launcher-offline-motor-abstrato.md) deixou o motor do launcher atrás da interface `LauncherEngine` e a escolha para o spike S1. O S1 (`docs/spikes/S1-motor-do-launcher.md`) comparou três caminhos:

- **`portablemc`** (crate Rust, Apache-2.0): instalou e abriu, em perfil offline, Fabric 1.20.1, Forge 1.7.10, 1.12.2 e 1.16.5 e NeoForge 1.21.1 no Linux, e Fabric 1.20.1 e Forge 1.12.2 no Windows; devolve a linha de comando (`Game`) em vez de abrir o jogo; usa as fontes primárias (Mojang, meta do Fabric, Maven do Forge/NeoForge); todas as pastas são configuráveis.
- **app-lib do Modrinth (theseus)**: só abriu com credencial falsa injetada no banco interno, trocou em silêncio a versão do Forge pedida e reteve a saída do NeoForge até o fim do processo. Descartado.
- **Motor próprio**: ~5 a 7 semanas mais manutenção contínua. Fica como plano B, partindo de um fork do `portablemc`.

O S-R5-3 (`docs/spikes/S-R5-3-marcadores-da-busca.md` §3 e §13) usou o mesmo adaptador em 20 combinações no Windows, de Forge 1.7.10 a Fabric 26.3, e mostrou que sem a versão exata do loader o motor consulta o meta do loader a cada abertura (duas rodadas falharam por isso).

## Decisão

- **Motor:** `portablemc = "=5.0.5"` (versão exata; cada atualização passa pela matriz L-05), adaptado em `crates/warden-launcher/src/engine/pmc.rs` (`PortableMcEngine`).
- **Interface final** (ARCHITECTURE §7.1, sem mudança de forma): `install(spec, java, progress, cancel) -> InstalledGame` (assíncrona) e `command(game, opts) -> LaunchCommand`. O `command` tem implementação padrão no trait e é **do Warden** (`warden_launcher::command::build`), igual para qualquer motor. O processo também é do Warden (`warden_launcher::process`): não foi preciso o método `spawn` que a ARCHITECTURE previa para motores que só sabem abrir o jogo sozinhos.
- **Versão exata do loader sempre**: `LoaderSpec` não tem "estável" nem "mais nova" (`LoaderSpec::validate` recusa apelidos); o adaptador passa `fabric::LoaderVersion::Name` e `forge::Version::Name`. O Forge do `pack.toml` (`10.13.4.1614`) vira a coordenada Maven (`1.7.10-10.13.4.1614-1.7.10`) pela pasta já instalada ou, na primeira instalação, pelo catálogo da P1-05 (`CatalogForgeResolver`); o NeoForge é convertido localmente, conferindo a versão do Minecraft que ele implica.
- **API síncrona do motor**: ele cria um runtime tokio próprio, então a instalação roda numa thread dedicada e o progresso volta por canal. O único ponto de cancelamento do motor é antes do lote de downloads; cancelar responde na hora com `core.CANCELLED` e a thread termina o lote em segundo plano, segurando a trava de instalação (a próxima espera).
- **Correções automáticas do motor desligadas**: `fix_broken_authlib`, proxy legado, `merge sort` legado, Quick Play e resolução legados. Nenhum contorno de autenticação (ARCHITECTURE §7.2); Quick Play e `--server/--port` são montados pelo Warden.
- **Jogador e pasta de jogo provisórios na instalação** (`WardenPlayer`, `shared/engine-gamedir`): o motor resolve `${auth_*}` e `${game_directory}` na instalação; a linha de comando troca os valores pelas opções (`--username`, `--uuid`, `--accessToken 0`, `--userType legacy` até 1.21.8, `--clientId` fixo do Warden, `--xuid 0`, `--userProperties {}`; `--offlineDeveloperMode` a partir da 1.21.9). Assim uma instalação serve a qualquer instância e jogador.
- **Configuração do log4j**: o motor sempre passa o XML da Mojang, e ao regravar o JSON do Forge **descarta** o `"logging": {}` com que o Forge anula o herdado (verificado na L-02 nos JSON do Forge 1.12.2 e 1.16.5). A regra fica no Warden, pelo loader e pela versão (`engine/logging.rs`): NeoForge sem o XML da Mojang; Forge com a configuração própria quando o jogo é 1.19+ ou a versão está na lista verificada (1.12.2 ≥ 14.23.5.2860, 1.16.5 ≥ 36.2.34, 1.18.2 ≥ 40.3.0); os demais (vanilla, Fabric, Quilt, Forge 1.7.10 e Forge antigos) com o XML corrigido da Mojang; até a 1.18 também `-Dlog4j2.formatMsgNoLookups=true`.
- **`@argfile`** acima de 30 000 caracteres com Java 9+, gravado na página de código ANSI do Windows (o lançador do Java lê o arquivo nela; em UTF-8 o `ç` chegava como `Ã§`, verificado com o Temurin 17). Java 8 no Windows acima de 32 767 caracteres: erro `launcher.COMMAND_TOO_LONG`.
- **Processo**: Windows com Job Object `KILL_ON_JOB_CLOSE` (parar encerra o job; o Warden morrer fecha o handle e o sistema mata o jogo e os filhos); Linux com grupo de processos, `SIGTERM` e `SIGKILL` depois de 3 s.

## Alternativas consideradas

- theseus e motor próprio: ver o contexto.
- Decidir o log4j pelo `"logging"` do JSON instalado (a regra do formato): impossível com o portablemc, que perde a chave vazia; corrigir no motor exigiria fork.
- Fixar a versão do portablemc por intervalo (`^5.0`): recusado; uma versão nova pode mudar a linha de comando sem a matriz perceber.

## Consequências

- Os golden tests (`crates/warden-launcher/tests/golden.rs`) comparam a linha de comando das 20 combinações da matriz, a partir dos planos gravados de instalações reais (`tests/fixtures/matriz/`, regerados por `tests/rede_matriz.rs`). Atualizar o portablemc = regerar e revisar esses arquivos.
- O portablemc guarda o manifesto da Mojang num cache HTTP próprio fora da pasta de dados do Warden (`%LOCALAPPDATA%\portablemc-cache` no Windows, `~/.cache/portablemc-cache` no Linux), sem opção para mudar. É só um JSON público, mas foge do isolamento da [ADR-0053](0053-isolamento-do-app-de-desenvolvimento.md); registrado como limitação conhecida.
- No Linux, se o Warden morrer de forma abrupta (sem passar por "Parar jogo"), o grupo do jogo não é encerrado: não há equivalente ao Job Object sem `unsafe` (`PR_SET_PDEATHSIG`). Fechar o Warden normalmente encerra o jogo (CA-T13-07 vale para o Windows, a plataforma principal).
- Risco do mantenedor único do portablemc: fork mantido pelo Warden se o projeto parar (Apache-2.0 permite).
