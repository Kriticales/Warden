# ADR-0052 — Cofre do sistema com `keyring-core` 1.0 e persistência Local

- **Status:** aceita · **Data:** 2026-10-04 · **Origem:** decisão técnica (tarefa F0-05; registrada na D7)

## Contexto

A [ADR-0025](0025-segredos-cofre-ou-env.md) e a [ADR-0024](0024-bibliotecas-rust.md) citavam o crate `keyring` para o cofre do sistema. Na F0-05, o `keyring` 3 já tinha sido substituído pelo próprio autor por uma família nova: `keyring-core` 1.0 (a interface) e um crate por cofre (`windows-native-keyring-store` para o Gerenciador de Credenciais do Windows e `zbus-secret-service-keyring-store` para o Secret Service do Linux). No Windows, a credencial pode ser gravada com persistência `Enterprise` (padrão do Windows, que acompanha o perfil móvel entre computadores do domínio) ou `Local` (fica só neste computador).

## Decisão

- `warden-secrets` usa `keyring-core` 1.0 com `windows-native-keyring-store` (Windows) e `zbus-secret-service-keyring-store` (Linux, com `rt-tokio-crypto-rust`), no lugar do `keyring` 3.
- No Windows, as credenciais são gravadas com **persistência `Local`**: as chaves do Warden ficam só neste computador.
- Serviço e contas continuam os da ARCHITECTURE §14 (`dev.kriticales.warden`; `curseforge-api-key`, `gemini-api-key`, `github-token`).

## Alternativas consideradas

- **`keyring` 3:** sem evolução; o autor aponta para a família nova.
- **Persistência padrão (`Enterprise`):** levaria as chaves para outros computadores do mesmo perfil de domínio, o que o dono não pediu.
- **Chamar a API do Windows direto:** mais código `unsafe` sem ganho.

## Consequências

- Três dependências diretas no lugar de uma, registradas no `THIRD_PARTY.md`.
- A troca de modo (cofre ↔ `.env`) e o cofre de teste em arquivo não mudam.
