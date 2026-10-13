# Logs reais dos scripts

Gerados em 13/10/2026 por um servidor Fabric 1.20.1 (Fabric Loader 0.19.5, Java 21) com os
mods do Modrinth: Fabric API 0.92.12, Architectury 9.2.14, Rhino 2001.2.3-build.10,
KubeJS 2001.6.5-build.26 (linha 6) e CraftTweaker 14.0.60. Os scripts que produziram cada log
estão ao lado, na pasta em que o jogo os lê.

- `kubejs-6-fabric-1.20.1/logs/kubejs/server.log`: `kubejs/server_scripts/receitas.js` com um
  ID errado na linha 16 (`minecraft:copper_ingott`) e uma função inexistente na linha 18.
- `kubejs-6-fabric-1.20.1/logs/kubejs/startup.log`: `kubejs/startup_scripts/itens.js` com uma
  função inexistente na linha 3 (o servidor para com esse erro).
- `crafttweaker-14-fabric-1.20.1/logs/crafttweaker.log`: `scripts/receitas.zs` com um ID errado
  na linha 2 e um colchete faltando no fim.

Os arquivos são os do jogo, sem edição. Usados pelos testes de `tests/reais.rs` (CA-T26-02).
