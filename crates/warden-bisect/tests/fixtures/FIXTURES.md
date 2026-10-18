# Golden logs da busca do culpado (D-12)

Cópias **sem alteração** de `spikes/s-r5-3/fixtures/` da branch `Kriticales/spike-s-r5-3-marcadores`
(commit `2c867c8`), spike S-R5-3, execuções reais no Windows em 2026-10-04 (relatório:
`docs/spikes/S-R5-3-marcadores-da-busca.md`). Os dados pessoais já vieram trocados pelo spike:
`<DADOS>` (pasta de dados), `<PERFIL>` (pasta do usuário do Windows), `<USUARIO>` e `<UUID>`.

| Pasta | O que é | Usada em |
|---|---|---|
| `matriz/<combinação>-cliente.log` e `-servidor.log` | marcadores de pronto e de mundo das 20 combinações da matriz (§3) | `tests/golden.rs`, CA-T25-05 |
| `falhas/<cenário>.log` | os 23 cenários de falha da §5.2 (`fab-*` Fabric 1.20.1, `neo-*` NeoForge 21.1.252, `forge-*` Forge 1.20.1, `f112-*` Forge 1.12.2) | `tests/golden.rs`, CA-T25-07 |
| `busca/` | rodadas da busca real da §7 (rodada 0, "falhou diferente" por dependência sem `type`, confirmação que passou) | `tests/golden.rs` |
| `mecanismos/` | Quick Play, `--quickPlayPath`, `-Dfabric.noGui`, `-Dfml.queryResult`, o falso crash report do 1.7.10 | `tests/golden.rs` |

O `README.md` original do spike não foi copiado; o conteúdo dele está resumido aqui. Os golden logs
`neo-dep-faltando.log` e `fab-mixin.log` também estão em `crates/warden-launcher/tests/fixtures/logs/`
(testes do leitor de log da L-02); aqui eles testam o veredito da busca.
