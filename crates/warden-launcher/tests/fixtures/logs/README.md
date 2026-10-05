# Golden logs do leitor de saída

Cópias sem alteração de `spikes/s-r5-3/fixtures/falhas/` (branch `Kriticales/spike-s-r5-3-marcadores`,
spike S-R5-3, Windows, 2026-10-04), com `<DADOS>`, `<PERFIL>`, `<USUARIO>` e `<UUID>` no lugar de dados
pessoais:

- `fab-mixin.log`: Fabric 1.20.1, eventos XML do log4j com `log4j:Throwable` de várias linhas;
- `neo-dep-faltando.log`: NeoForge 21.1.252, começa em XML e passa a texto puro quando o FML assume.

Usados por `tests/leitor_de_log.rs`. Os golden logs da matriz inteira ficam para a L-05
(`tests/matrix/**`).
