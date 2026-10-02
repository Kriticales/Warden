# ADR-0032 — Servidor local sob demanda e EULA do Minecraft

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (tarefa D4, pesquisa R5B §4.2)

## Contexto

Um servidor dedicado local permite testar o pack como servidor (o caso clássico do mod "de cliente" que derruba o servidor), validar o pacote para servidor, recarregar scripts e mandar comandos pelo stdin, e ajuda a busca do culpado em versões antigas. O motor de launcher escolhido não instala servidor; os instaladores oficiais fazem isso sem interface (verificado em Fabric e NeoForge, R5B §4.2). Rodar um servidor do Minecraft exige aceitar a EULA da Mojang.

## Decisão

- O servidor local é uma **ferramenta opcional e sob demanda**. **Nunca** faz parte do "▶ Testar" normal.
- Usos: **Testar como servidor…** (menu ▾ do Testar), **validar o pacote para servidor** depois de gerar, e **apoio à busca do culpado** quando necessário (o Warden explica por que precisa e pede antes de abrir).
- **Segurança:** escuta só em `127.0.0.1` (`server-ip=127.0.0.1`), porta livre escolhida pelo Warden, `online-mode=false`, RCON desligado; **fecha sozinho** quando o teste acaba (`stop` pelo stdin, encerramento forçado após 60 s); **avisa antes** se a RAM livre não comportar servidor e jogo juntos.
- **EULA:** o aceite é pedido **uma vez**, com o link https://aka.ms/MinecraftEULA, e lembrado nas Configurações (dá para revogar). Sem aceite, o Warden não grava `eula=true` e não abre o servidor.
- Instalação por loader com os instaladores oficiais (Fabric meta, `--installServer` do Forge/NeoForge), numa pasta própria por pack (`instances/<pack-id>/server/`), só com os mods cujo lado não é "Só cliente".
- Faixas na v1: Fabric, NeoForge e Forge 1.17+ primeiro; Forge antigo (1.7.10 a 1.16.5) e vanilla na mesma tarefa, com verificação na matriz; Quilt fora (ADR-0005).

## Alternativas consideradas

- Servidor sempre junto com o teste: dobra o consumo de memória e o tempo de cada teste sem pedido do usuário.
- Aceitar a EULA em nome do usuário: inaceitável; o padrão do mercado é pedir o aceite explícito.
- Escutar em todas as interfaces: expõe o servidor na rede e faz o Windows abrir o aviso do firewall.

## Consequências

- Nova crate `warden-server` (ARCHITECTURE §7.6) e tarefa própria no ROADMAP (L-09). Comandos pelo stdin servem ao spark (perfil de desempenho no servidor) e à recarga de scripts.
- O pacote para servidor (ADR-0035) e a busca do culpado (ADR-0031) usam o servidor sem duplicar a lógica.
