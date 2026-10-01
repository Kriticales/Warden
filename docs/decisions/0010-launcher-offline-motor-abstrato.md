# ADR-0010 — Launcher integrado, só perfil offline, com motor abstrato

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (launcher integrado, offline, motor pronto) + decisão técnica (A1) (interface e supervisão)

## Contexto

A ponte com o Prism foi a fonte de bugs mais recorrente nos apps anteriores (R4 §3.2). Construir um launcher do zero é grande (R2 §1–§3). Existem motores prontos: a app-lib do Modrinth App (Rust, GPL-3.0) e o portablemc (Rust, Apache-2.0). O spike S1 está avaliando os dois.

## Decisão

- O Warden tem **launcher integrado** para testar packs, reaproveitando um **motor open source pronto**, escolhido pelo S1.
- O resto do Warden só conhece a interface `LauncherEngine` (ARCHITECTURE §7.1): instalar versão/loader e montar a linha de comando. **O Warden inicia e supervisiona o processo** (logs, codificação, encerramento, classificação), igual para qualquer motor.
- **Só perfil offline**, sem nenhum login Mojang/Microsoft, nunca multiplayer online. Sem contorno de autenticação.
- Postura legal (R2 §4.5): nada da Mojang é redistribuído; tudo é baixado dos servidores oficiais na máquina do usuário; aviso "não oficial" e "você precisa possuir o jogo" na primeira execução e em Sobre.

## Alternativas consideradas

- Prism externo: rejeitado pela experiência anterior.
- Motor próprio do zero: fica como plano B se o S1 reprovar os dois candidatos.
- Login Microsoft para verificar posse: exige app aprovado pela Mojang; o dono decidiu não ter login.

## Consequências

- L-02 implementa o adaptador e registra em ADR próprio o resultado do S1 e qualquer ajuste da interface.
- A matriz de versões (L-05) valida o motor escolhido em todas as faixas garantidas.
