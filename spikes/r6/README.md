# Spike R6: segurança dos mods

Código descartável da pesquisa `docs/research/07-seguranca-dos-mods.md`. Nada aqui entra no app.

- `probe/`: gera amostras **seguras** (EICAR montado em tempo de execução e jars sintéticos com o *formato* do estágio 0 do fractureiser, sem função real), chama a AMSI do Windows e roda o protótipo da busca de sinais com `cafebabe`. Comandos no topo de `probe/src/main.rs`.
- `yarax/`: mede o custo do YARA-X e aplica `yarax/regras.yar` aos jars e às amostras (em memória).
- `hashlookup/consulta.py`: consulta por hash sem chave (YARAify, CIRCL hashlookup, Team Cymru MHR por DNS). `contar-eventos-kaspersky.py` conta os eventos exportados com `avp.com REPORT AMSI /RA:<arquivo>`.
- `fixtures/iocs-publicos.txt`: hashes públicos (IOCs) usados nas consultas; nenhum arquivo malicioso.

Compilar fora do worktree com caminho curto: `CARGO_TARGET_DIR=C:\wt\r6\target cargo build --release` (em `probe/`).
**Atenção:** `gen` grava EICAR e jars que o antivírus apaga ou bloqueia; o comando `amsi-mem` faz o antivírus mostrar avisos. Apague as pastas geradas ao terminar.
