/**
 * Comandos do relato: a pré-visualização (montada e redigida no Rust), a abertura no navegador
 * do endereço que a pré-visualização mostrou (UI-02) e, com o login do GitHub, o envio direto e
 * os prints do rascunho (UI-03).
 */
import { keepPreviousData, useQuery } from '@tanstack/react-query';

import {
  commands,
  type ProblemReportInput,
  type ReportImage,
  type ReportImagesAdded,
} from '../../lib/ipc/bindings';
import { operationChannel } from '../../lib/ipc/channel';
import { useCommandMutation } from '../../lib/ipc/query';
import { unwrap } from '../../lib/ipc/result';
import { githubKeys } from '../settings/github/api';

export const reportKeys = {
  preview: (input: ProblemReportInput) => ['app', 'problemReport', input] as const,
};

/**
 * A pré-visualização. Lê os registros de novo a cada mudança do texto (com o atraso de quem
 * chama) e nunca fica em cache depois de fechar o diálogo.
 */
export function useReportPreview(input: ProblemReportInput) {
  return useQuery({
    queryKey: reportKeys.preview(input),
    queryFn: async () => unwrap(await commands.problemReportPreview(input)),
    placeholderData: keepPreviousData,
    staleTime: Number.POSITIVE_INFINITY,
    gcTime: 0,
  });
}

/** Abre no navegador padrão o endereço da pré-visualização ou a issue criada. */
export function useOpenReport() {
  return useCommandMutation((url: string) => commands.problemReportOpen(url));
}

/** Etapa do envio, para o texto de progresso. */
export type SendStep =
  { stage: 'labels' } | { stage: 'prints'; done: number; total: number } | { stage: 'issue' };

/**
 * Envia o relato direto pela API do GitHub. `onStep` recebe cada etapa do canal da operação.
 * Um login vencido no envio muda a situação do login: ela é lida de novo ao terminar.
 */
export function useSendReport(onStep: (step: SendStep) => void) {
  return useCommandMutation(
    (input: ProblemReportInput) => {
      let stage = '';
      const channel = operationChannel((event) => {
        if (event.type === 'stage') {
          stage = event.stage;
          if (stage === 'labels' || stage === 'issue') onStep({ stage });
        } else if (event.type === 'progress' && stage === 'prints') {
          onStep({ stage: 'prints', done: event.current, total: event.total ?? 0 });
        }
      });
      return commands.problemReportSend(input, channel);
    },
    { invalidates: [githubKeys.login] },
  );
}

/** Abre ou fecha o rascunho de prints (os dois esvaziam). */
export async function setReportDraft(open: boolean): Promise<void> {
  await commands.problemReportDraft(open);
}

/** Acrescenta um print colado. */
export function useAttachImage() {
  return useCommandMutation(({ name, data }: { name: string; data: string }) =>
    commands.problemReportAttach(name, data),
  );
}

/** "Escolher imagens…". */
export function useChooseImages() {
  return useCommandMutation(() => commands.problemReportChooseImages());
}

/** "Remover" um print. */
export async function detachImage(image: ReportImage): Promise<void> {
  await commands.problemReportDetach(image.token);
}

/** Lê um arquivo colado como base64 (sem o prefixo `data:`). */
export function readAsBase64(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = typeof reader.result === 'string' ? reader.result : '';
      resolve(result.slice(result.indexOf(',') + 1));
    };
    reader.onerror = () => {
      reject(reader.error ?? new Error('arquivo não lido'));
    };
    reader.readAsDataURL(file);
  });
}

export type { ReportImagesAdded };
