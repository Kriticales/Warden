import type { PackwizErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `packwiz` (QUALITY §3): leitura e escrita de `pack.toml`,
 * `index.toml` (o índice) e dos metafiles `.pw.toml`. Dona: P1-01.
 */
export const packwiz: Record<PackwizErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao ler ou gravar os arquivos do packwiz do pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  INVALID_TOML:
    'O arquivo {{file}} do pack está com o texto quebrado e não pôde ser lido. Corrija o trecho indicado nos detalhes técnicos ou volte a uma versão salva no Histórico.',
  INVALID_FIELD_TYPE:
    'No arquivo {{file}}, o campo {{key}} tem um valor do tipo errado. Corrija o campo ou volte a uma versão salva no Histórico.',
  INVALID_FIELD_VALUE:
    'No arquivo {{file}}, o campo {{key}} tem um valor que o packwiz não aceita. Corrija o campo ou volte a uma versão salva no Histórico.',
  UNSAFE_PATH:
    'O caminho {{path}} aponta para fora da pasta do pack ou usa um nome que o Windows não aceita, e foi recusado. Remova esse item do índice ou renomeie o arquivo.',
  INVALID_URL:
    'O link {{url}} não pôde ser usado. Confira se ele começa com http:// ou https:// e termina no nome do arquivo.',
  UNKNOWN_HASH_FORMAT:
    'O formato de hash {{format}} não é conhecido pelo packwiz. Use sha1, sha256, sha512, md5 ou murmur2, ou adicione o item de novo.',
};
