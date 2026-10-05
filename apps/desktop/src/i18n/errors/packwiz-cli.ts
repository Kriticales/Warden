import type { PackwizCliErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `packwizCli` (QUALITY §3): execução do packwiz que vem junto com o
 * Warden (refresh, adição pela CurseForge, exportações). Dona: P1-02. Cancelamento, tempo esgotado
 * e falha de disco usam as frases do domínio `core`.
 */
export const packwizCli: Record<PackwizCliErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao executar o packwiz. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  BINARY_NOT_FOUND:
    'O packwiz que vem junto com o Warden não foi encontrado em {{path}}. Reinstale o Warden; se o antivírus apagou o arquivo, libere-o e reinstale.',
  SPAWN_FAILED:
    'O Windows não deixou o Warden iniciar o packwiz ({{path}}). Se o antivírus bloqueou o arquivo, libere-o e tente de novo.',
  PACK_FILE_MISSING:
    'A pasta {{path}} não tem o arquivo pack.toml, então não é um pack do packwiz. Confira a pasta escolhida.',
  COMMAND_FAILED:
    'O packwiz não conseguiu concluir o comando {{command}} (código {{exitCode}}). A mensagem dele está nos detalhes técnicos.',
  POSTCONDITION_FAILED:
    'O packwiz disse que concluiu o comando {{command}}, mas os arquivos do pack não ficaram como deveriam. Nada foi aproveitado; os detalhes técnicos mostram o que não conferiu.',
  CURSEFORGE_KEY_MISSING:
    'Para usar a CurseForge, informe a sua chave da CurseForge em Configurações.',
  MANUAL_DOWNLOADS_REQUIRED:
    'A CurseForge não deixa apps de terceiros baixarem {{count}} arquivo(s): {{files}}. Baixe cada um pela página indicada nos detalhes técnicos e tente de novo.',
  DOWNLOAD_FAILED:
    'Não foi possível baixar {{files}}, então o resultado ficaria incompleto e foi descartado. Confira a conexão e tente de novo.',
  LINK_IN_PACK:
    'O pack tem um atalho de pasta ou arquivo (link simbólico ou junção) em {{path}}, que o Warden não segue. Troque o atalho pelos arquivos de verdade.',
};
