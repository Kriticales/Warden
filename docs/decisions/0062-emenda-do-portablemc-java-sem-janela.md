# ADR-0062 — Emenda do portablemc: Java sem janela no Windows

- **Status:** aceita · **Data:** 2026-10-19 · **Origem:** decisão técnica do orquestrador (tarefa B1-C, problema 6 do dono) · **Complementa:** [ADR-0057](0057-motor-do-launcher-portablemc.md) e [ADR-0049](0049-copia-fixa-do-packwiz-e-patches.md)

## Contexto

O dono viu janelas de terminal aparecerem ao testar o pack e ao abrir o Minecraft, e quer que tudo aconteça por baixo dos panos. O Warden já abre os próprios processos de console com `CREATE_NO_WINDOW` (o jogo em `warden_launcher::process`, o Java do teste de Java, o instalador do jogador). As janelas vinham do motor do launcher: o `portablemc` 5.0.5 roda o `java.exe` que o Warden entrega (`JvmPolicy::Static`, `crates/warden-launcher/src/engine/pmc.rs`) sem essa opção em dois lugares:

- `src/base/mod.rs`, `find_jvm_versions`: o `java -version`, em toda instalação (a cada teste);
- `src/forge/mod.rs`, `try_install`: cada "processor" do instalador do Forge e do NeoForge, na primeira instalação (uma janela por processor).

O motor não tem opção para mudar isso.

## Decisão

- O Warden usa uma **cópia do `portablemc` 5.0.5 do crates.io em `third_party/portablemc/`**, trocada pelo `[patch.crates-io]` do `Cargo.toml` da raiz e fora do workspace (`exclude`).
- A cópia segue o modelo da cópia do packwiz (ADR-0049): toda mudança é um patch mínimo em `third_party/portablemc/patches/NNNN-<assunto>.patch`, com o porquê em português e um teste que falha sem ele; `third_party/portablemc/WARDEN.md` descreve a origem e os patches.
- `ORIGINAL.sha256` registra os arquivos do crate original; um teste do xtask desfaz os patches e confere cada hash, para nada mudar na cópia fora de um patch.
- O primeiro patch (`0001-java-sem-janela-no-windows.patch`) acrescenta `CREATE_NO_WINDOW` aos dois comandos acima, só no Windows. Nada mais muda.
- A correção é proposta ao autor do portablemc por uma issue no repositório dele (a abrir pelo dono). Quando ela sair numa versão nova, a atualização (que passa pela matriz L-05) remove o patch.

## Alternativas consideradas

- **Entregar o `javaw.exe` ao motor:** o `javaw` não tem console, então não abre janela, mas mostra uma caixa de erro do Java e espera um clique quando a JVM não sobe (memória demais, opção inválida). O Warden ficaria preso na instalação sem saber por quê. Recusado pelo orquestrador.
- **Fork do portablemc no GitHub:** mais um projeto a manter para uma mudança de duas linhas. Fica para quando houver muitos patches (ADR-0057 já prevê o fork se o projeto parar).
- **Esperar a correção do autor:** o problema continuaria até uma versão nova, que ainda precisaria passar pela matriz.

## Consequências

- O `Cargo.lock` registra o `portablemc` sem fonte do crates.io (é uma dependência por caminho); o `cargo deny` confere a licença Apache-2.0 declarada no `Cargo.toml` da cópia.
- Atualizar o portablemc passa a incluir trocar a cópia, regravar `ORIGINAL.sha256` e reaplicar os patches (`third_party/portablemc/WARDEN.md`).
- O teste `crates/warden-launcher/tests/java_sem_janela.rs` (com o Java falso `src/bin/java_fake.rs`) cobre o `java -version`; os processors do Forge, que exigem o instalador real e a rede, ficam sem teste automático.
