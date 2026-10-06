# Golden logs do spike S-R5-3 (para os testes da D-12 e da L-05)

Recortes curtos de execuções reais no Windows (2026-10-04), gerados por
`scripts/make_fixtures.py` a partir das capturas do protótipo. Cada arquivo tem só os registros que
importam, inteiros (evento XML do log4j ou linha de texto com o stack trace, cortado em até 8–25
linhas), na ordem em que o jogo escreveu, **sem** o prefixo de tempo do supervisor. Os tempos de cada
marcador estão no relatório (`docs/spikes/S-R5-3-marcadores-da-busca.md`) e em `../resultados/`.

Marcadores no lugar de dados pessoais: `<DADOS>` (pasta de dados do spike), `<PERFIL>` (pasta do
usuário do Windows), `<USUARIO>`, `<UUID>` (UUID do jogador offline `WardenTest`). O jogador é o
offline `WardenTest`; nenhuma conta.

Atenção: os logs de Fabric, vanilla e Forge antigo vêm como **XML do log4j** na saída padrão; os de
NeoForge e Forge modernos começam em XML e passam a **texto puro** quando o FML assume. Os testes
devem passar pelo mesmo parser que o Warden usa na saída real.

| Pasta | Conteúdo | Origem |
|---|---|---|
| `matriz/<combinação>-cliente.log` | identificação do loader, marcadores de pronto e de entrada no mundo, por combinação da matriz (§3 do relatório) | rodada válida mais recente de `scripts/matrix.py` |
| `matriz/<combinação>-servidor.log` | servidor dedicado local (antes de 1.20): `Done`, `logged in with entity id`, `joined the game` | idem |
| `mecanismos/` | `--server/--port` ignorado no 1.20.1, `--quickPlayMultiplayer`, Quick Play no NeoForge 1.21.1, mundo salvo com mod ausente, pergunta do FML 1.12.2 com e sem `-Dfml.queryResult=confirm`, dependência faltando no Fabric com e sem `-Dfabric.noGui`, o falso crash report do `SplashProgress` (1.7.10) e o JSON do `--quickPlayPath` (`quickplay-path-*.json`) | comandos do §4 |
| `falhas/<cenário>.log` | um arquivo por cenário de falha da §5.2 (`fab-*` = Fabric 1.20.1, `neo-*` = NeoForge 21.1.252, `forge-*` = Forge 1.20.1-47.4.10, `f112-*` = Forge 1.12.2) | `scripts/scenarios.py` |
| `busca/` | rodadas-chave da busca real (§7): a rodada 0 que falhou, a confirmação sem o culpado que passou e o "falhou diferente" da tentativa 1 | `scripts/busca_culpado.py` |

Rodadas que não viraram golden log: as contaminadas por clique humano (§9 do relatório) e as
refeitas (`forge-entrar` e `forge-entrar-b`, com o jar de teste sem `pack.mcmeta`; a válida é a
`forge-entrar-c`, salva como `falhas/forge-entrar.log`).

Ruído que aparece nos recortes e **não** é falha: `InvalidCredentialsException: Status: 401`
(perfil offline), `Missing sound for event`, avisos de `PerfOS`/`PDH`, `missing mods.toml file` das
bibliotecas do Forge 1.20.1 e `Attempting connection with missing mods [minecraft, mcp, FML, forge]`
(Forge 1.7.10/1.12.2).
