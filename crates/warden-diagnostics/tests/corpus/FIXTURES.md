# Corpus de logs reais da análise pós-crash (D-02)

Cada pasta é um caso: os arquivos com os nomes de uma sessão (`output.log`, `latest.log`, `crash-reports/crash.txt`) e o `esperado.toml` com os achados na ordem (regra, padrão, arquivo, linha, parâmetros, itens). O teste `tests/corpus.rs` (CA-T14-02) compara a análise com o esperado e confere que cada evidência aponta uma linha que contém o trecho citado.

Origens:

- **spike S-R5-3** (`docs/spikes/S-R5-3-marcadores-da-busca.md`): golden logs de execuções reais no Windows em 2026-10-04, copiados sem alteração de `spikes/s-r5-3/fixtures/` da branch `Kriticales/spike-s-r5-3-marcadores`. Uma única correção: o spike tinha trocado o usuário do Windows por `<USUARIO>` até dentro da palavra `StatusConsoleListener`; a palavra foi restaurada. Os demais marcadores do spike (`<DADOS>`, `<PERFIL>`, `<UUID>`) ficaram.
- **issues públicas do GitHub**: logs citados em issues, onde o spike e o codex não cobriam (queda nativa no driver de vídeo, jar Fabric no NeoForge), copiados do texto da issue com o link anotado.
- **codex-minecraft** (https://github.com/aternosorg/codex-minecraft, commit `d7fb6a30b8dbe9d9f73c97e4ad7927a45c3173e9`, MIT, Copyright (c) 2019-2025 Aternos GmbH): logs públicos de `test/data/`, que vêm do mclo.gs. Nomes de pessoas nos caminhos e nos jogadores trocados por nomes fictícios (`fulano`, `ciclano`, `beltrano`, `JogadorX`, `JogadorY`); o resto sem alteração. Um log que começa com `---- Minecraft Crash Report ----` vira `crash-reports/crash.txt`.

`.gitattributes` desta pasta marca tudo como `-text`: os bytes (codificação, fim de linha) fazem parte do caso.

Para regravar os esperados depois de mudar o catálogo: `$env:WARDEN_DIAGNOSTICS_ATUALIZAR_CORPUS = '1'; cargo nextest run -p warden-diagnostics --test corpus`, e revisar a diferença no git.

| Caso | Origem | O que mostra |
|---|---|---|
| `spike-f112-dep-faltando` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/f112-dep-faltando.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.12.2, Quark sem AutoRegLib |
| `spike-f112-duplicado` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/f112-duplicado.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.12.2, Comforts duas vezes |
| `spike-f112-java21` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/f112-java21.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.12.2 com Java 21 |
| `spike-fab-dep-faltando` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/fab-dep-faltando.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.20.1, Comforts sem Fabric API |
| `spike-fab-dep-faltando-nogui` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/fab-dep-faltando-nogui.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.20.1, Comforts sem Fabric API, -Dfabric.noGui |
| `spike-fab-dep-versao` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/fab-dep-versao.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.20.1, Fabric API antiga |
| `spike-fab-duplicado` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/fab-duplicado.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.20.1, mod duplicado (aceito em silêncio) |
| `spike-fab-entrar` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/fab-entrar.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.20.1, falha ao entrar no mundo (wardenfalhas) |
| `spike-fab-iniciar` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/fab-iniciar.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.20.1, exceção ao iniciar (wardenfalhas) |
| `spike-fab-java8` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/fab-java8.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.20.1 com Java 8 |
| `spike-fab-memoria` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/fab-memoria.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.20.1 com -Xmx160M |
| `spike-fab-mixin` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/fab-mixin.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.20.1, mixin sem alvo (wardenfalhas) |
| `spike-fab-outra-versao-mc` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/fab-outra-versao-mc.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.20.1 com mods de 1.21 |
| `spike-fab-outro-loader` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/fab-outro-loader.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.20.1 com jar do Forge |
| `spike-fab-travar` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/fab-travar.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.20.1, jogo travado sem fechar |
| `spike-forge-avisos-bloqueiam-quickplay` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/forge-avisos-bloqueiam-quickplay.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.20.1, tela de avisos (sem falha no log) |
| `spike-forge-dep-faltando` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/forge-dep-faltando.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.20.1, Supplementaries sem Moonlight |
| `spike-forge-duplicado` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/forge-duplicado.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.20.1, mod duplicado (aceito em silêncio) |
| `spike-forge-entrar` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/forge-entrar.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.20.1, falha ao entrar no mundo |
| `spike-forge-mixin` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/forge-mixin.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.20.1, mixin sem alvo |
| `spike-forge-outro-loader` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/forge-outro-loader.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.20.1 com jar Fabric (ignorado em silêncio) |
| `spike-forge-travar` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/forge-travar.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.20.1, jogo travado sem fechar |
| `spike-neo-dep-faltando` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/neo-dep-faltando.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 21.1.252, Supplementaries sem Moonlight |
| `spike-neo-dep-versao` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/neo-dep-versao.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 21.1.252, Moonlight antigo |
| `spike-neo-duplicado` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/neo-duplicado.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 21.1.252, mod duplicado (aceito em silêncio) |
| `spike-neo-entrar` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/neo-entrar.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 21.1.252, falha ao entrar no mundo |
| `spike-neo-iniciar` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/neo-iniciar.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 21.1.252, exceção ao iniciar |
| `spike-neo-java17` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/neo-java17.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 21.1.252 com Java 17 |
| `spike-neo-memoria` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/neo-memoria.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 21.1.252 com -Xmx256M |
| `spike-neo-mixin` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/neo-mixin.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 21.1.252, mixin sem alvo |
| `spike-neo-outro-loader` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/neo-outro-loader.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 21.1.252 com jar Fabric (recorte sem a linha do pulo) |
| `spike-neo-travar` | spike S-R5-3, `spikes/s-r5-3/fixtures/falhas/neo-travar.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 21.1.252, jogo travado sem fechar |
| `spike-r0-pack-inteiro-falhou` | spike S-R5-3, `spikes/s-r5-3/fixtures/busca/r0-pack-inteiro-falhou.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 1.21.1, pack de 155 mods, conflito plantado |
| `spike-c2-pack-sem-o-culpado-passou` | spike S-R5-3, `spikes/s-r5-3/fixtures/busca/c2-pack-sem-o-culpado-passou.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 1.21.1, pack sem o culpado (passou) |
| `spike-tentativa1-falhou-diferente-dependencia` | spike S-R5-3, `spikes/s-r5-3/fixtures/busca/tentativa1-falhou-diferente-dependencia.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 1.21.1, Continuity sem Fabric API (Connector) |
| `spike-forge-1.7.10-falso-crash-report-splash` | spike S-R5-3, `spikes/s-r5-3/fixtures/mecanismos/forge-1.7.10-falso-crash-report-splash.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.7.10, crash report do SplashProgress (não é falha) |
| `spike-forge-1.12.2-servidor-pergunta-mundo-sem-mod` | spike S-R5-3, `spikes/s-r5-3/fixtures/mecanismos/forge-1.12.2-servidor-pergunta-mundo-sem-mod.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Servidor Forge 1.12.2 esperando /fml confirm |
| `spike-forge-1.12.2-servidor-queryResult-confirm` | spike S-R5-3, `spikes/s-r5-3/fixtures/mecanismos/forge-1.12.2-servidor-queryResult-confirm.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Servidor Forge 1.12.2 com -Dfml.queryResult=confirm |
| `spike-neoforge-1.21.1-mundo-salvo-com-mod-ausente` | spike S-R5-3, `spikes/s-r5-3/fixtures/mecanismos/neoforge-1.21.1-mundo-salvo-com-mod-ausente.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 1.21.1, mundo salvo com mod ausente |
| `spike-forge-1.7.10-cliente` | spike S-R5-3, `spikes/s-r5-3/fixtures/matriz/forge-1.7.10-cliente.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.7.10, abriu e entrou no mundo |
| `spike-forge-1.12.2-cliente` | spike S-R5-3, `spikes/s-r5-3/fixtures/matriz/forge-1.12.2-cliente.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.12.2, abriu e entrou no mundo |
| `spike-forge-1.16.5-cliente` | spike S-R5-3, `spikes/s-r5-3/fixtures/matriz/forge-1.16.5-cliente.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.16.5, abriu e entrou no mundo |
| `spike-forge-1.20.1-cliente` | spike S-R5-3, `spikes/s-r5-3/fixtures/matriz/forge-1.20.1-cliente.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 1.20.1, abriu e entrou no mundo |
| `spike-forge-26.2-cliente` | spike S-R5-3, `spikes/s-r5-3/fixtures/matriz/forge-26.2-cliente.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Forge 26.2, abriu e entrou no mundo |
| `spike-neoforge-1.20.1-cliente` | spike S-R5-3, `spikes/s-r5-3/fixtures/matriz/neoforge-1.20.1-cliente.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 1.20.1, abriu e entrou no mundo |
| `spike-neoforge-26.2-cliente` | spike S-R5-3, `spikes/s-r5-3/fixtures/matriz/neoforge-26.2-cliente.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | NeoForge 26.2, abriu e entrou no mundo |
| `spike-fabric-1.16.5-cliente` | spike S-R5-3, `spikes/s-r5-3/fixtures/matriz/fabric-1.16.5-cliente.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 1.16.5, abriu e entrou no mundo |
| `spike-fabric-26.3-cliente` | spike S-R5-3, `spikes/s-r5-3/fixtures/matriz/fabric-26.3-cliente.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Fabric 26.3, abriu e entrou no mundo |
| `spike-quilt-1.20.1-cliente` | spike S-R5-3, `spikes/s-r5-3/fixtures/matriz/quilt-1.20.1-cliente.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Quilt 1.20.1, abriu e entrou no mundo |
| `spike-vanilla-26.3-cliente` | spike S-R5-3, `spikes/s-r5-3/fixtures/matriz/vanilla-26.3-cliente.log` (branch `Kriticales/spike-s-r5-3-marcadores`) | Vanilla 26.3, abriu e entrou no mundo |
| `codex-fabric-1-20-1-dependencies-wrong-version` | codex-minecraft `test/data/Vanilla/Fabric/fabric-1-20-1-dependencies-wrong-version.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-1171-dependency` | codex-minecraft `test/data/Vanilla/Fabric/fabric-1171-dependency.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-conflicting-mods` | codex-minecraft `test/data/Vanilla/Fabric/fabric-conflicting-mods.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-crash-report-client` | codex-minecraft `test/data/Vanilla/Fabric/fabric-crash-report-client.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-duplicate-mod` | codex-minecraft `test/data/Vanilla/Fabric/fabric-duplicate-mod.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-entrypoint-error` | codex-minecraft `test/data/Vanilla/Fabric/fabric-entrypoint-error.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-incompatible-minecraft-version` | codex-minecraft `test/data/Vanilla/Fabric/fabric-incompatible-minecraft-version.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-incompatible-mods` | codex-minecraft `test/data/Vanilla/Fabric/fabric-incompatible-mods.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-java11` | codex-minecraft `test/data/Vanilla/Fabric/fabric-java11.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-java8` | codex-minecraft `test/data/Vanilla/Fabric/fabric-java8.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-missing-dependencies-1` | codex-minecraft `test/data/Vanilla/Fabric/fabric-missing-dependencies-1.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-missing-dependencies-2` | codex-minecraft `test/data/Vanilla/Fabric/fabric-missing-dependencies-2.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-missing-dependencies-3` | codex-minecraft `test/data/Vanilla/Fabric/fabric-missing-dependencies-3.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-missing-dependency-unversioned` | codex-minecraft `test/data/Vanilla/Fabric/fabric-missing-dependency-unversioned.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-missing-dependency-versioned` | codex-minecraft `test/data/Vanilla/Fabric/fabric-missing-dependency-versioned.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-mixin-error` | codex-minecraft `test/data/Vanilla/Fabric/fabric-mixin-error.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-version-range-dependency` | codex-minecraft `test/data/Vanilla/Fabric/fabric-version-range-dependency.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-wrong-mc-version` | codex-minecraft `test/data/Vanilla/Fabric/fabric-wrong-mc-version.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-fabric-26-2-client` | codex-minecraft `test/data/Vanilla/Fabric/fabric-26-2-client.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-1-20-1-duplicate-mods` | codex-minecraft `test/data/Vanilla/Forge/forge-1-20-1-duplicate-mods.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-1-7-10-crash-report` | codex-minecraft `test/data/Vanilla/Forge/forge-1-7-10-crash-report.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-confirm` | codex-minecraft `test/data/Vanilla/Forge/forge-confirm.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-crash-report` | codex-minecraft `test/data/Vanilla/Forge/forge-crash-report.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-early-crash` | codex-minecraft `test/data/Vanilla/Forge/forge-early-crash.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-language-provider-version-above` | codex-minecraft `test/data/Vanilla/Forge/forge-language-provider-version-above.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-language-provider-version-between` | codex-minecraft `test/data/Vanilla/Forge/forge-language-provider-version-between.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-loading-stage-error` | codex-minecraft `test/data/Vanilla/Forge/forge-loading-stage-error.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-missing-datapack-mod` | codex-minecraft `test/data/Vanilla/Forge/forge-missing-datapack-mod.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-missing-mods-exception` | codex-minecraft `test/data/Vanilla/Forge/forge-missing-mods-exception.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-missing-mods-exception-2` | codex-minecraft `test/data/Vanilla/Forge/forge-missing-mods-exception-2.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-mod-dependency` | codex-minecraft `test/data/Vanilla/Forge/forge-mod-dependency.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-mod-duplicate` | codex-minecraft `test/data/Vanilla/Forge/forge-mod-duplicate.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-mod-exception` | codex-minecraft `test/data/Vanilla/Forge/forge-mod-exception.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-mod-fatal` | codex-minecraft `test/data/Vanilla/Forge/forge-mod-fatal.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-mod-fatal-1444` | codex-minecraft `test/data/Vanilla/Forge/forge-mod-fatal-1444.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-mod-loading-dependency` | codex-minecraft `test/data/Vanilla/Forge/forge-mod-loading-dependency.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-mod-required` | codex-minecraft `test/data/Vanilla/Forge/forge-mod-required.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-mod-wrong-minecraft-version` | codex-minecraft `test/data/Vanilla/Forge/forge-mod-wrong-minecraft-version.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-multiple-fatal-problems` | codex-minecraft `test/data/Vanilla/Forge/forge-multiple-fatal-problems.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-multiple-modules-export` | codex-minecraft `test/data/Vanilla/Forge/forge-multiple-modules-export.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-ptrlib-dependency` | codex-minecraft `test/data/Vanilla/Forge/forge-ptrlib-dependency.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-ticking-block-entity` | codex-minecraft `test/data/Vanilla/Forge/forge-ticking-block-entity.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-ticking-block-entity-crash-report` | codex-minecraft `test/data/Vanilla/Forge/forge-ticking-block-entity-crash-report.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-ticking-entity-1-18-2` | codex-minecraft `test/data/Vanilla/Forge/forge-ticking-entity-1-18-2.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-world-missing-mod` | codex-minecraft `test/data/Vanilla/Forge/forge-world-missing-mod.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-world-mod-version` | codex-minecraft `test/data/Vanilla/Forge/forge-world-mod-version.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-forge-start-1122` | codex-minecraft `test/data/Vanilla/Forge/forge-start-1122.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-neoforge-1-20-4-client-report` | codex-minecraft `test/data/Vanilla/NeoForge/neoforge-1-20-4-client-report.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-neoforge-1-21-1-server-mod-loading-crash-report` | codex-minecraft `test/data/Vanilla/NeoForge/neoforge-1-21-1-server-mod-loading-crash-report.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-neoforge-1-21-3-client` | codex-minecraft `test/data/Vanilla/NeoForge/neoforge-1-21-3-client.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-neoforge-26-1-client` | codex-minecraft `test/data/Vanilla/NeoForge/neoforge-26-1-client.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-quilt-crash-report-client` | codex-minecraft `test/data/Vanilla/Quilt/quilt-crash-report-client.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-quilt-client-1-19-2` | codex-minecraft `test/data/Vanilla/Quilt/quilt-client-1-19-2.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-vanilla-aquatic-world-on-older-version` | codex-minecraft `test/data/Vanilla/vanilla-aquatic-world-on-older-version.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-vanilla-code-of-conduct` | codex-minecraft `test/data/Vanilla/vanilla-code-of-conduct.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-vanilla-crash-report-1-21-11-pre-release-1` | codex-minecraft `test/data/Vanilla/vanilla-crash-report-1-21-11-pre-release-1.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-vanilla-forge-ticking-entity` | codex-minecraft `test/data/Vanilla/vanilla-forge-ticking-entity.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-vanilla-malformed-encoding` | codex-minecraft `test/data/Vanilla/vanilla-malformed-encoding.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-vanilla-old-player-directory` | codex-minecraft `test/data/Vanilla/vanilla-old-player-directory.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `codex-vanilla-auth-servers-unreachable` | codex-minecraft `test/data/Vanilla/vanilla-auth-servers-unreachable.log` (commit d7fb6a3, MIT) | nomes de pessoas trocados por fictícios |
| `github-hs-err-driver-intel` | issue pública https://github.com/siw201/siw201/issues/1, primeiras 30 linhas do `hs_err_pid` | Queda nativa da JVM no driver de vídeo Intel (`ig9icd64.dll`) |
| `github-neoforge-jar-fabric` | issue pública https://github.com/SparkUniverse/Elementa/issues/160, a linha do log citada | NeoForge 1.21.1 pula um jar feito para Fabric |
