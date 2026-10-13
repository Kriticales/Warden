import type { VersioningErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `versioning` (QUALITY §3; crate `warden-versioning`, tarefa V-01). O
 * dono não vê git: as frases falam de versões salvas, histórico e pontos de segurança (QUALITY §8.2).
 */
export const versioning: Record<VersioningErrorCode, string> = {
  INTERNAL:
    'Algo deu errado no histórico de versões do pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  NOT_VERSIONED:
    'A pasta {{path}} ainda não tem histórico de versões. Abra o pack pelo Warden para ele criar o histórico.',
  ALREADY_VERSIONED:
    'A pasta {{path}} já tem um histórico de versões. Abra o pack com Abrir ou importar… para usar o histórico que já existe.',
  REPO_NOT_OWNED:
    'O histórico de versões em {{path}} pertence a outro usuário do Windows, e o Warden não mexe nele por segurança. Copie o pack para uma pasta sua e abra a cópia.',
  REPO_LOCKED:
    'Outro programa está mexendo no histórico de versões do pack agora. Feche esse programa e tente de novo.',
  REPO_BUSY:
    'O histórico de versões do pack está no meio de uma operação feita fora do Warden ({{state}}). Termine ou cancele essa operação no programa em que ela começou e tente de novo.',
  DETACHED_HEAD:
    'O histórico de versões do pack foi deixado fora da linha de versões por outro programa. Volte o pack para a linha de versões nesse programa e tente de novo.',
  CONFLICTS:
    'O histórico de versões do pack tem conflitos deixados por outro programa em {{paths}}. Resolva os conflitos nesse programa e tente de novo.',
  INVALID_VERSION:
    '"{{version}}" não é um número de versão válido. Use três números separados por ponto, como 1.4.0.',
  VERSION_NOT_GREATER:
    'A versão {{version}} não é maior que a última versão salva, {{last}}. Escolha um número maior que {{last}}.',
  VERSION_EXISTS: 'Já existe uma versão {{version}} salva. Escolha outro número.',
  NOTHING_CHANGED: 'Nada mudou desde a última versão salva. Faça uma alteração antes de salvar.',
  VERSION_NOT_FOUND:
    'A versão {{version}} não existe no histórico do pack. Atualize a lista de versões e escolha outra.',
  SAFETY_POINT_NOT_FOUND:
    'Esse ponto de segurança não existe mais no histórico do pack. Atualize a lista e escolha outro.',
  VERSION_PUBLISHED:
    'A versão {{version}} já foi publicada para os jogadores e não pode deixar de ser versão final.',
  FILE_IN_USE:
    'O arquivo {{path}} está aberto em outro programa (o jogo, por exemplo), então o pack não foi alterado. Feche o jogo e tente de novo.',
  ROLLBACK_FAILED:
    'Não foi possível voltar o pack e nem desfazer tudo: {{paths}} ficaram diferentes. O estado de antes está guardado no ponto de segurança {{safetyPoint}}; recupere esse ponto para voltar ao que era.',
  GIT_FAILED:
    'O Warden não conseguiu ler ou gravar o histórico de versões do pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  // Publicar versão (V-03, SPEC T18).
  NOT_FINAL:
    'A versão {{version}} não é versão final. Marque-a como versão final no Histórico para poder publicar.',
  ALREADY_PUBLISHED: 'A versão {{version}} já foi publicada para os jogadores.',
  PUBLISH_NOT_NEWER:
    'A versão {{version}} é menor que a última publicada, {{last}}: os jogadores voltariam para trás. Publique uma versão maior que {{last}}.',
  PUBLISH_SECRET_FOUND:
    'Encontramos algo que parece uma chave em {{path}}, linha {{line}}. Remova antes de publicar.',
  PUBLISH_BLOCKED:
    'A versão tem arquivos que não podem ir para os jogadores. Limpe esses arquivos, salve uma versão nova e publique essa.',
  GITHUB_NOT_LINKED:
    'Este pack ainda não tem repositório no GitHub. Abra Publicar versão no Histórico para criar ou escolher um.',
  GITHUB_OFFLINE:
    'Sem conexão com o GitHub. Confira a internet e tente de novo.',
  GITHUB_TOKEN_REJECTED: 'O GitHub recusou o token. Gere um novo e salve em Configurações.',
  GITHUB_PERMISSION_MISSING:
    'O token do GitHub não tem a permissão {{permission}} (leitura e escrita). Gere um token com ela e salve em Configurações.',
  GITHUB_REPO_NOT_FOUND:
    'O repositório {{repo}} não existe no GitHub ou o token não tem acesso a ele. Confira o nome e o acesso do token em Configurações.',
  GITHUB_REPO_NAME_TAKEN:
    'Já existe um repositório {{repo}} na sua conta. Escolha outro nome ou use Usar um repositório que já existe.',
  GITHUB_INVALID_REPO_NAME:
    '"{{name}}" não serve como nome de repositório. Use letras sem acento, números, ponto, hífen ou sublinhado.',
  REMOTE_DIVERGED:
    'O GitHub ({{repo}}) tem mudanças que não estão aqui: alguém mexeu no repositório por fora do Warden. Substitua o GitHub pelo que está aqui ou cancele.',
  PUSH_REJECTED:
    'O GitHub recusou o envio da versão. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  RELEASE_PENDING:
    'A versão {{version}} foi enviada aos jogadores, mas a página da versão no GitHub (Release) não foi criada. Use Tentar de novo para criar só a Release.',
  GITHUB_FAILED:
    'O GitHub respondeu de um jeito inesperado. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
