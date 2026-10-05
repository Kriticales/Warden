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
};
