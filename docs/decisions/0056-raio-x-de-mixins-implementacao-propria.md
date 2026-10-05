# ADR-0056 — Raio-x de mixins com implementação própria (`cafebabe`)

- **Status:** aceita · **Data:** 2026-10-04 · **Origem:** decisão técnica (spike S-R5-1; registrada na D7) · **Complementa:** [ADR-0034](0034-analise-estatica-do-pack.md)

## Contexto

A [ADR-0034](0034-analise-estatica-do-pack.md) deixou para o spike S-R5-1 escolher entre usar o intermed (Rust, MIT) como dependência, portá-lo com atribuição ou fazer uma implementação própria com `cafebabe`. O spike (`docs/spikes/S-R5-1-intermed.md`, §5 a §9) comparou os três caminhos em 3 packs reais (Fabric 1.20.1, Forge 1.20.1, NeoForge 1.21.1, 250 a 313 mods): o intermed não desce em jars aninhados (perde de 7 a 20% das classes, inclusive a Fabric API inteira), foi 10 a 50 vezes mais lento e usou 20 a 80 vezes mais memória que o protótipo, achou 0 de 3 conflitos reais e teve 48 de 48 avisos julgados falsos; o protótipo próprio (~1.200 linhas) cumpre a meta da D-10 sem cache (0,3 a 0,8 s com 8 threads).

## Decisão

- O raio-x (`warden-mixin`, D-10) é **implementação própria com `cafebabe`**, a partir do protótipo do spike (`spikes/s-r5-1/harness/src/proto.rs`, branch do spike). O intermed não entra como dependência nem como código portado e não vai para o `THIRD_PARTY.md` (ideias aproveitadas sem copiar código: `@WrapWithCondition` como "pode cancelar a chamada", ordem por prioridade efetiva, alvos inexistentes).
- Entram as regras e os rebaixadores que o spike mediu (opções desligadas por `lithium:options`/`canary:options`/`radium:options` e pelas configs do pack; seletor de `@ModifyVariable`/`@ModifyArg`; "o plugin ou a config cita o outro mod"; "declara dependência" não vale quando o outro é biblioteca; `@Redirect` + `@Inject` no mesmo ponto = baixo), os dados curados iniciais e a marcação "nomes não comparados" para mods Fabric carregados pelo Sinytra Connector em packs Forge/NeoForge.
- Cópias aninhadas repetidas: fica a de versão maior (no empate, a do jar que traz mais aninhados). Cache num **arquivo binário único** em `cache/xray/`.
- **Pendente do dono, sem efeito aqui:** a proposta do spike de mostrar em Problemas (`W_MIXIN_OVERLAP`) só o risco alto (SPEC §10, "Decisões pendentes do dono"). Até ele decidir, vale a regra da ADR-0034 (alto ou médio).

## Alternativas consideradas

Ver o contexto: dependência e porte com atribuição foram descartados pelo spike.

## Consequências

- Sem dependência de um projeto alfa de um autor só, fora do crates.io.
- A D-10 mantém o critério de 3 s sem cache e 300 ms com cache e passa a medir também com disco frio.
