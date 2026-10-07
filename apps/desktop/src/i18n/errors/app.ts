import type { AppErrorCode } from '../../lib/ipc/bindings';

/** Frases dos erros do domínio `app`: o que aconteceu + o que fazer (QUALITY §3). */
export const app: Record<AppErrorCode, string> = {
  INTERNAL:
    'Algo deu errado dentro do Warden. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  OPERATION_NOT_FOUND: 'Essa tarefa não existe mais. Abra Tarefas de novo para ver a lista atual.',
  OPERATION_NOT_CANCELLABLE:
    'Essa tarefa está numa etapa que não pode ser interrompida. Espere ela terminar.',
  SETTINGS_INVALID: 'Um valor das configurações foi recusado. Confira o campo e tente de novo.',
  SETTINGS_NEWER_VERSION:
    'As configurações foram salvas por uma versão mais nova do Warden e não podem ser alteradas nesta. Atualize o Warden.',
  GAME_ALREADY_RUNNING:
    'Já existe um jogo em execução (pack {{packName}}). Só dá para testar um pack por vez: feche o jogo ou vá para o teste dele.',
  NO_GAME_RUNNING: 'Não há jogo aberto neste pack. Ele pode ter fechado agora há pouco.',
  TEST_MODE_UNAVAILABLE:
    'Este tipo de teste ainda não está disponível nesta versão do Warden. Use o Testar normal.',
  TEST_INSTANCE_CHANGED:
    'Arquivos do pack mudaram na instância de teste durante o teste anterior e seriam substituídos pela versão do pack: {{files}}. Substitua para testar, ou copie antes o que quiser guardar.',
  TEST_MANUAL_DOWNLOADS:
    'A CurseForge não deixa outros apps baixarem estes mods: {{names}}. Baixe-os pelo site e coloque na pasta Downloads; a ajuda para isso chega numa próxima versão do Warden.',
  TEST_LOADER_UNSUPPORTED:
    'O Warden não testa packs com {{loader}}. Use Forge, NeoForge ou Fabric.',
  TEST_SESSION_NOT_FOUND:
    'Esse teste não está mais gravado. Os testes antigos que fecharam normalmente são apagados depois dos 30 mais novos.',
  TEST_INSTANCE_MISSING:
    'Este pack ainda não tem instância de teste. Clique em Testar para criá-la.',
  TEST_MEMORY_HIGH_JAVA8:
    'O teste vai usar {{memoryMb}} MB com o Java 8. Acima de 8 GB, o Java 8 costuma travar o jogo por segundos nas limpezas de memória. Se acontecer, diminua a memória em Ajustes do teste.',
  TEST_ARTIFACT_INVALID: 'Esse arquivo não faz parte deste teste ou foi apagado.',
};
