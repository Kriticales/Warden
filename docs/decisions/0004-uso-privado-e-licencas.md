# ADR-0004 — Uso privado e política de licenças de terceiros

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono

## Contexto

A pesquisa (R2 §9 item 1) deixou em aberto a licença do Warden, que decide se código GPL (Prism, app-lib do Modrinth, HMCL) pode ser incorporado.

## Decisão

- O Warden é de **uso pessoal e privado**, em repositório privado, e **nunca será distribuído**. Não recebe licença pública ("todos os direitos reservados").
- Código e bibliotecas de terceiros sob GPL, LGPL, MIT, Apache etc. **podem ser usados e estudados**. Licenças não comerciais/proibitivas para uso pessoal também não bloqueiam, mas devem ser evitadas quando houver alternativa equivalente.
- **Toda origem é registrada:** dependências pelo `cargo deny`/lockfiles; código copiado ou portado com comentário de origem (URL, licença, commit) no topo do arquivo e entrada em `THIRD_PARTY.md`.
- Avisos de terceiros aparecem na tela Sobre (A-02).

## Alternativas consideradas

- Licença MIT/proprietária com proibição de GPL: restringiria a escolha do motor do launcher (S1) sem benefício, já que nada é distribuído.

## Consequências

- O spike S1 pode escolher a app-lib do Modrinth (GPL-3.0) se for a melhor opção técnica.
- Se um dia o dono quiser distribuir o Warden, será preciso revisar `THIRD_PARTY.md` e as dependências GPL antes. Esse é o motivo do registro rigoroso.
