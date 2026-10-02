# ADR-0041 — Lista de sinais embutida e atualizada com o app

- **Status:** aceita · **Data:** 2026-10-02 · **Origem:** técnica (tarefa D5, parte da decisão D27)

## Contexto

A busca de sinais da [ADR-0040](0040-seguranca-dos-mods.md) precisa de uma lista de padrões de casos conhecidos (código injetado pelo fractureiser, hashes de arquivos maliciosos já identificados). A lista envelhece. A SPEC §9 deixa a atualização remota de dados curados para depois da v1 (P2), e o Warden não tem atualização automática.

## Decisão

- A lista é um arquivo de dados versionado, `crates/warden-security/data/signatures.toml`, embutido no app com `include_str!` e atualizado a cada versão do Warden.
- O formato segue a ideia dos modelos da Concoction (MIT): cada sinal é um padrão com um nível de risco e a origem (o caso de onde veio). A atribuição fica em `THIRD_PARTY.md`.
- A interface mostra a data da lista; com mais de 180 dias, aparece a informação "A lista de sinais tem mais de 6 meses. Atualize o Warden quando puder."
- O mecanismo "vivo" é a conferência de hash com as plataformas: quando o Modrinth ou a CurseForge removem ou marcam um arquivo, o Warden fica sabendo na verificação seguinte, sem depender da lista.
- `cargo xtask check-signatures` valida o arquivo na CI: esquema e cada sinal testado contra jars sintéticos (um que deve disparar e um que não deve).
- A atualização remota de dados curados continua P2 (SPEC §9), inclusive para esta lista.

## Alternativas consideradas

- Baixar a lista de um repositório fixo, com assinatura: protege melhor contra um caso novo, mas é atualização remota e exige infraestrutura de chaves e assinatura. Fica como ponto pendente para o dono (abaixo).
- Usar o banco remoto do jNeedle: depende de um serviço de terceiros sem manutenção garantida.

## Consequências

- Entre duas versões do Warden, um caso novo só é pego se a plataforma remover ou marcar o arquivo; a página diz isso no aviso fixo da T28.
- Atualizar a lista passa a fazer parte do preparo de cada versão do Warden.
- **Ponto pendente para o dono:** lista embutida (recomendação) ou baixada de um repositório fixo com assinatura. Até ele responder, vale a lista embutida.
