# Corpus adversário da assinatura de travamento (D-06, 4ª revisão)

Cada pasta é um caso: um crash report real (`crash-reports/crash.txt`) ou um `latest.log` real, copiado de um paste público do mclo.gs (os `Crash Report`/`Client Log` que aparecem em issues de mods). `grupos.toml` diz o grupo esperado de cada caso e, nos casos que provam uma regra, as partes da assinatura. Os testes estão em `src/signature.rs` (`corpus_adversario_*`): mesmo grupo ⇔ mesmo hash, e cada caso continua com a mesma assinatura depois de trocar o ruído (números de linha, endereços, ids de sessão do Mixin, contadores, UUIDs, horários).

Anonimização: o mclo.gs já troca o usuário dos caminhos por `********`; aqui os caminhos ficaram com `fulano` e os nomes de jogadores (`ServerPlayer['…']`) com `JogadorX`. O resto está sem alteração. `.gitattributes` marca os logs como `-text`.

O loader e a versão do Minecraft do contexto vêm do nome da pasta (`fabric-1.21.1-…`), como o Warden passa os do pack.

| Caso | Origem (mclo.gs) | O que mostra |
|---|---|---|
| `fabric-1.20.1-autotag-lambda` | `E1fTP11` | Fabric 1.20.1 (Kilt); NPE numa lambda de mod, `md6558fa$autotag$lambda$…` e `localvar$…` na pilha |
| `fabric-1.20.1-configsync-desconectar-a` | `nnUXzcc` | Fabric 1.20.1; NPE no Forge Config API Port ao desconectar |
| `fabric-1.20.1-configsync-desconectar-b` | `S8MxoMu` | O mesmo travamento em outra execução (ids de mixin, linhas e launcher diferentes) |
| `fabric-1.20.1-mixin-vcc` | `naecDuI` | `MixinApplyError` do `vcc.mixins.json` |
| `fabric-1.20.1-registros-mouseclicked` | `W36vXwW` | `IllegalStateException` ao carregar registros, embrulho do Kilt e `mixinextras$bridge` |
| `fabric-1.21.1-bootstrap-injection` | `nhQqej4` | `InjectionError` no Bootstrap sem linha `MixinApplyError` (config só na mensagem) |
| `fabric-1.21.1-entrypoint-beachparty` | `LbWLdZK` | `Could not execute entrypoint` com `ExceptionInInitializerError` encadeado |
| `fabric-1.21.1-entrypoint-cerulean-suppressed` | `bObvwhU` | Entrypoint com cinco `Suppressed:` antes do `Caused by:` |
| `fabric-1.21.1-entrypoint-cobblefurnies` | `u9IiClt` | Entrypoint com `IllegalArgumentException` numa propriedade de bloco |
| `fabric-1.21.1-log-mixin-flywheel` | `DtBbqUB` | `latest.log` do Modrinth App: `Mixin apply for mod flywheel failed` |
| `fabric-1.21.1-mixin-cobblemon-ui-tweaks` | `0hazLdL` | `MixinApplyError` (`@Shadow` ausente) do `cobblemon-ui-tweaks.mixins.json` |
| `fabric-1.21.1-owo-network` | `CWugXjS` | Exceção própria de um mod (`NetworkException`) ao iniciar |
| `fabric-1.21.1-registros-mouseclicked` | `HRKVVrS` | `wrapOperation$…$fabric-registry-sync-v0$…$36` (hífen e contador) e o launcher do Modrinth embaixo |
| `fabric-1.21.1-registrysync-desconectar` | `F5M7Hik` | `handler$ddo000$fabric-registry-sync-v0$disconnectAfter` abaixo de guava e do jogo |
| `fabric-1.21.11-mixin-sodium` | `uFAHdSj` | `MixinApplyError` do `sodium-common.mixins.json` |
| `fabric-26.1.1-mixin-flashback-iris` | `hcjwwde` | `MixinApplyError` do `flashback.mixins.json` (alvo do Iris) |
| `fabric-26.2-servidor-log-mixin-arcade-replay` | `PJFyeom` | `latest.log` de servidor: `Mixin apply for mod arcade-replay failed` (id com hífen) |
| `forge-1.12.2-abstractmethod-ae2` | `ZbJ74a0` | Forge 1.12.2; `AbstractMethodError` num mod, servidor integrado |
| `forge-1.12.2-jei-rendering-ingredient` | `tjYM599` | Forge 1.12.2 com coremods; queda no jogo chamada pelo JEI |
| `forge-1.12.2-memoria` | `8A7nD1B` | Forge 1.12.2; `OutOfMemoryError` com o OptiFine no topo |
| `forge-1.16.5-init-essential-portalmod` | `l8bkR0j` | Forge 1.16.5 (Java 8); `ExceptionInInitializerError` → NPE num `<clinit>` de mod |
| `forge-1.20.1-overlay-classcast-sem-pilha` | `AsYDlWZ` | Forge 1.20.1 (Connector); `ClassCastException: null` sem pilha, frame no `-- Head --` |
| `forge-1.20.1-overlay-emf-init` | `UgTxdwe` | Forge 1.20.1; `md23f78d$lambda$…` na pilha, `Suspected Mods` com o alvo do mixin |
| `forge-1.20.1-servidor-concurrent` | `rUBynPf` | `ConcurrentModificationException` com guava no topo |
| `forge-1.20.1-servidor-config` | `Q1vWuE7` | Config de servidor corrompida (`config.failed_loading`) |
| `forge-1.20.1-servidor-fastutil` | `LbdgIHL` | `fastutil` com jar anotado no topo; `wrapOperation$…$managedBlock` embrulhando o laço |
| `forge-1.20.1-servidor-powergrid` | `pVxo0fh` | NPE num mod pelo Architectury, servidor integrado |
| `forge-1.20.1-ticking-block-entity-epicfight` | `mwTeG9p` | `Ticking block entity` com `ClassCastException` entre dois mods |
| `forge-1.7.10-ticking-block-entity-thaumic` | `9hcOqf4` | Forge 1.7.10; `Ticking block entity` com `Suspected Mods` na própria linha |
| `neoforge-1.21.1-config-allthecompatibility` | `JYDF1Ld` | NeoForge 1.21.1; a mesma exceção e queda do caso do MineColonies, em outro mod |
| `neoforge-1.21.1-config-minecolonies` | `bakYcQh` | NeoForge 1.21.1; `Cannot get config value before config is loaded` |
| `neoforge-1.21.1-create-colisao` | `sZPYziX` | NPE no Create (módulos `TRANSFORMER/create@…`) |
| `neoforge-1.21.1-levelrenderer-indice` | `yrCy44t` | Só frames do Java e do jogo (`LevelRenderer.renderLevel`) |
| `neoforge-1.21.1-moonlight-rendertarget` | `wuSq2pr` | `handler$zda000$qdaa$sssaDraw` (handler com o mod no nome) |
| `neoforge-1.21.1-simulated-mola` | `HUffuEa` | NPE num mod com jar de nome longo |
| `neoforge-1.21.1-simulated-tecla` | `GJcAzlg` | NPE no mesmo mod, em outro ponto |
| `neoforge-1.21.1-voxy-sem-sodium` | `VPu1vA7` | `NoClassDefFoundError` → `ClassNotFoundException` com `... 36 more` |
| `neoforge-1.21.1-zume-indice` | `vYTqUGR` | Mod ofuscado (`zume.Q.d`) e ForgeWrapper/Prism embaixo do `main` |
