# ADR-0021 — Layout em disco e metadados do Warden

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão técnica (A1)

## Contexto

É preciso separar o que é do pack (versionado e distribuído), o que é do Warden sobre o pack (versionado, não distribuído) e o que é da máquina (instância, caches, registros). A pesquisa deixou em aberto onde guardar metadados próprios (R1 §7 item 11; R3 §6.2).

## Decisão

- **Pack:** pasta escolhida pelo usuário (padrão `Documentos\Warden\<pack>`), repositório git.
- **`.warden/project.toml` dentro do pack:** `PackId` (ULID), `schemaVersion` e avisos do diagnóstico ignorados. Versionado no git e **ignorado pelo packwiz** (linha obrigatória do `.packwizignore`, sempre garantida). `CHANGELOG.md` idem.
- **Configuração do app:** `%APPDATA%\dev.kriticales.warden\` (`settings.json`; `packs.json` com o registro dos packs e as preferências de teste de cada pack neste computador: memória, Java, argumentos JVM).
- **Dados locais:** `%LOCALAPPDATA%\dev.kriticales.warden\` com `instances/`, `shared/`, `cache/`, `logs/` (ARCHITECTURE §13).
- Dados de máquina (preferências de teste, linha de base, sessões, escolhas de opcionais da instância) nunca ficam no pack; assim, ajustar o teste não gera "alteração não salva".

## Alternativas consideradas

- Metadados como comentários nos TOML do packwiz: o `refresh` apaga.
- Tudo fora do pack: o identificador e os avisos ignorados se perderiam ao clonar o pack em outra máquina.

## Consequências

- O `.packwizignore` padrão inclui `/.warden/`, `/CHANGELOG.md` e `/README.md`.
- Pastas copiadas com o mesmo `PackId` são detectadas e recebem id novo.
