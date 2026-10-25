# ADR-0069 — Chaves só no cofre do sistema; sai o modo `.env`

- **Status:** aceita · **Data:** 2026-10-25 · **Origem:** decisão do dono (relatório de bugs nº 1, P12; esboço aprovado `design/bug-01/configuracoes.html`, versão 2 · menu lateral) · **Substitui:** [ADR-0025](0025-segredos-cofre-ou-env.md)

## Contexto

A ADR-0025 deixava a pessoa escolher onde guardar as chaves: o cofre do sistema (padrão) ou um arquivo `.env` na pasta de configuração do Warden, com um aviso de que o `.env` é texto legível por qualquer programa, backup ou sincronização. No relatório de bugs nº 1 (P12), ao redesenhar Configurações, o dono decidiu que as chaves ficam **sempre** no cofre: a escolha confundia (duas opções para algo que a pessoa leiga não precisa decidir) e o `.env` deixava as chaves expostas. O esboço aprovado tira o "Onde guardar" e diz, no rodapé do menu e no alto de Chaves e contas, "Chaves no Cofre do Windows".

## Decisão

- **As chaves (CurseForge, GitHub, Gemini, OpenAI e Claude) ficam sempre no cofre do sistema:** o Gerenciador de Credenciais no Windows; o Secret Service no Linux (só a CI roda lá). Saem o modo `.env` (`EnvFileStore`), a troca de modo (`secrets_backend_get`, `secrets_backend_set`), o campo `secretsBackend` do `settings.json`, a escolha "Onde guardar as chaves" em Configurações e nas boas-vindas e o diálogo de confirmação do `.env`.
- **Migração obrigatória, ao abrir o app:** se existir o `.env` antigo da pasta de configuração, cada chave dele é gravada no cofre e **relida para conferir**; o arquivo só é apagado depois de **todas** conferidas, e então o `secretsBackend` sai do `settings.json`.
  - Com o `settings.json` ainda dizendo `"secretsBackend": "envfile"`, o valor do `.env` vale (substitui o que houver no cofre, que era uma sobra). Sem isso (o cofre já era o lugar em uso e o `.env` era uma sobra), o `.env` só completa as chaves que faltam no cofre.
  - Se algo falhar no meio (cofre fora do ar, arquivo ilegível, falha ao conferir), **nada se perde**: o `.env` fica intacto e a próxima abertura tenta de novo. Configurações → Chaves e contas mostra o aviso com **Tentar de novo** (`secrets_migration_retry`), e um aviso aparece ao abrir o app.
  - Se só apagar o `.env` falhar, as chaves já estão no cofre; o aviso diz que o arquivo sobrou e o Warden tenta apagar de novo na próxima abertura.
  - Deu certo: o aviso "Suas chaves agora ficam no Cofre do Windows" aparece uma vez, com quantas chaves foram.
- **O cofre de teste em arquivo continua** para desenvolvimento, testes e E2E (`TestFileStore`, só em build de debug, por `WARDEN_SECRET_BACKEND=file:<pasta>` ou pela pasta de desenvolvimento; ADR-0048 e ADR-0053). Ele faz o papel do cofre do sistema e nunca existe no app instalado.
- **A tela mostra o final da chave** (`••••xxxx`, os últimos 4 caracteres, só para chaves com pelo menos 16 caracteres) e o resultado do último **Testar** (funcionando ou recusada, com a hora), guardado em `key-checks.json` na pasta de configuração, sem nada da chave. Só a resposta do serviço conta ("o serviço recusou a chave"); sem internet, a situação gravada não muda. Gravar ou remover a chave apaga o resultado; salvar uma chave testa na hora.
- O resto da ADR-0025 e da ADR-0017 continua: chaves digitadas pelo usuário, a chave inteira nunca aparece de novo, nunca em registros, mensagens de erro, argumentos de processos ou URLs; `SecretString` no Rust; o recuo por variável de ambiente só em build de debug.

## Alternativas consideradas

- Manter o `.env` como opção avançada: contraria a decisão do dono e mantém o risco para quem não entende a diferença.
- Migrar sem conferir e apagar o `.env` na hora: rápido, mas uma falha do cofre perderia as chaves.
- Deixar o `.env` lá depois da migração: as chaves continuariam expostas em texto.

## Consequências

- `warden-secrets` fica com um armazenamento só (o cofre) e um leitor do `.env` antigo usado só pela migração (`legacy_env.rs`). Os códigos de erro só crescem: `BACKEND_SWITCH_FAILED` fica sem uso e entra `ENV_MIGRATION_FAILED`.
- SPEC: T01 (as chaves vão para o cofre, sem escolha), T21 (Chaves e contas no cofre; sai o CA-T21-03; o CA-T21-07 passa a cobrir a migração do `.env`) e D12.
- Testes: a migração com o cofre de teste (cópia, conferência, falha no meio sem perder nada, arquivo ilegível, sobra do arquivo), a varredura de segredos (a chave nunca aparece em `settings.json`, `key-checks.json` nem nos registros) e um E2E que abre o app no antigo modo `.env`.
