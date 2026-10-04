import type { ConfigsErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `configs` (QUALITY §3; crate `warden-configs`, tarefa C-01). Nos
 * casos em que o formulário não serve, o arquivo continua aberto no editor de texto.
 */
export const configs: Record<ConfigsErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao ler ou gravar uma config. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  TOO_LARGE:
    'Esta config tem {{sizeMb}} MB, mais que o limite de {{limitMb}} MB do formulário. Edite o arquivo no editor de texto.',
  NOT_UTF8:
    'Esta config não está gravada em UTF-8, então o Warden não consegue mostrá-la no formulário. Edite o arquivo no editor de texto.',
  PARSE_FAILED:
    'O arquivo não segue o formato {{format}} na linha {{line}}, coluna {{column}}. Corrija esse trecho no editor de texto.',
  KEY_NOT_FOUND:
    'A chave {{key}} não existe mais nesta config; o arquivo pode ter mudado por fora. Abra a config de novo.',
  KEY_NOT_EDITABLE:
    'A chave {{key}} agrupa outras chaves e não tem valor próprio ({{reason}}). Edite as chaves dentro dela.',
  INVALID_VALUE:
    'Esse valor não serve para a chave {{key}}: {{reason}}. Escolha outro valor ou edite o arquivo no editor de texto.',
  DUPLICATE_EDIT:
    'A chave {{key}} foi alterada duas vezes na mesma gravação. Desfaça uma das alterações e salve de novo.',
  EDIT_NOT_CONFIRMED:
    'O Warden não conseguiu confirmar que só o valor pedido mudaria, então não gravou nada. Edite o arquivo no editor de texto e, se puder, copie os detalhes técnicos e registre o problema.',
};
