# ADR-0017 — Segredos no cofre de credenciais do sistema

- **Status:** substituída por ADR-0025 · **Data:** 2026-10-01 · **Origem:** decisão do dono

## Contexto

O Warden usa chaves da CurseForge e do Gemini e um token do GitHub. Durante o desenvolvimento, a chave da CurseForge do dono está em `.env` na raiz do repositório principal (`CURSEFORGE_API_KEY`, entre aspas simples porque começa com `$2a$`).

## Decisão

- No app, chaves e tokens são **digitados pelo usuário** e guardados no **cofre do sistema** (`keyring`: Gerenciador de Credenciais do Windows; Secret Service no Linux).
- Nunca voltam para a interface (só o estado "configurada"), nunca vão para arquivos de configuração, registros, mensagens de erro, argumentos de processos ou URLs.
- No Rust circulam como `SecretString`.
- O `.env` é **só para desenvolvimento e testes**: lido pelo xtask a partir do repositório principal, sem copiar nem imprimir; em build de debug, o app pode usar variáveis de ambiente se o cofre estiver vazio; em release, não.
- Na CI, a chave fica nos *secrets* do GitHub Actions e só é usada nos testes de rede.

## Alternativas consideradas

- Arquivo de configuração criptografado pelo próprio app: reinventa o cofre do sistema.
- Embutir a chave no binário: contraria a decisão e espalha a chave.

## Consequências

- `gitleaks` na CI e no checklist de revisão.
- Os testes e o app de desenvolvimento nunca tocam no cofre real do dono: usam um cofre de teste em arquivo (só em debug, trait `SecretStore`) e, para as chaves, o recuo por variável de ambiente, tanto no Windows do desenvolvimento quanto no runner Linux, que não tem Secret Service. O cofre real do Windows é testado na CI Windows e no roteiro manual dos marcos ([ADR-0048](0048-desenvolvimento-no-windows.md)).
