# ADR-0025 — Segredos: cofre do sistema por padrão, arquivo .env opcional

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (comentário na página da estrutura, tarefa D2) · **Substitui:** ADR-0017

## Contexto

A ADR-0017 guardava as chaves (CurseForge, Gemini) e o token do GitHub só no cofre de credenciais do sistema. Revendo a estrutura do app, o dono pediu que as chaves pudessem ficar num arquivo `.env` nos arquivos do aplicativo. A pergunta foi levada a ele com a diferença entre as duas opções: o cofre protege as chaves com o usuário do Windows; um `.env` é texto legível por qualquer programa, backup ou sincronização da pasta. Decisão do dono: **os dois**.

## Decisão

- **Padrão:** cofre do sistema (`keyring`: Gerenciador de Credenciais do Windows; Secret Service no Linux), como na ADR-0017.
- **Opção:** em Configurações → Chaves e contas, "Arquivo .env na pasta de dados do Warden": `%APPDATA%\dev.kriticales.warden\.env` (Linux: `~/.config/dev.kriticales.warden/.env`), com as variáveis `CURSEFORGE_API_KEY`, `GEMINI_API_KEY` e `GITHUB_TOKEN`. Fica fora de qualquer pack e de qualquer repositório; nunca vai para o GitHub.
- Escolher o `.env` exige confirmação com o aviso de que o arquivo é texto legível por outros programas. Enquanto ele estiver em uso, o aviso fica fixo na seção, com "Voltar para o cofre".
- Trocar de modo **move** as chaves (grava no destino, confere, apaga da origem); uma falha no meio mantém a cópia de origem. Só um modo fica ativo; ele é gravado em `settings.json` (`secretsBackend: "keyring" | "envfile"`), que nunca contém valores.
- O resto da ADR-0017 continua: chaves digitadas pelo usuário, nunca exibidas de novo, nunca em registros, mensagens de erro, argumentos de processos ou URLs; `SecretString` no Rust; o `.env` do **repositório** do Warden continua sendo só para desenvolvimento e testes (lido pelo xtask, sem copiar nem imprimir); em build de debug, recuo por variável de ambiente quando o armazenamento escolhido não tem a chave.

## Alternativas consideradas

- Só cofre (ADR-0017): mais seguro, mas não atende ao pedido do dono.
- Só `.env`: perde a proteção do cofre para todos os casos.
- `.env` criptografado pelo próprio app: reinventa o cofre e não é o que o dono pediu.

## Consequências

- `warden-secrets` ganha uma segunda implementação de `SecretStore` (arquivo `.env`, escrita atômica) e a migração entre modos (ARCHITECTURE §14).
- Testes de varredura passam a cobrir os dois modos: a chave nunca aparece em `settings.json`, registros ou pastas de packs; no modo `.env`, aparece só no `.env` da pasta de dados (SPEC CA-T21-03).
