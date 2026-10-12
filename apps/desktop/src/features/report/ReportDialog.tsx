/**
 * Diálogo "Relatar problema" (UI-02): título (obrigatório), o que você fez, o que aconteceu e
 * o que você esperava, "Incluir os registros do app", o que o Warden coletou sozinho e a
 * pré-visualização exata do que vai para o GitHub. "Abrir no GitHub" abre a issue nova já
 * preenchida no navegador; nada sai antes desse clique, e quem envia é a pessoa, no GitHub.
 *
 * Toda a montagem e a redação acontecem no Rust (`problem_report_preview`). Se algo foi
 * cortado para o endereço caber, o relato completo vai para a área de transferência no clique.
 */
import { useRouter } from '@tanstack/react-router';
import { ExternalLink } from 'lucide-react';
import { useId, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../components/common/ErrorPanel';
import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../components/ui/dialog';
import { Loader } from '../../components/ui/loader';
import { showToast } from '../../components/ui/toast';
import type { ProblemReportInput, ProblemReportPreview } from '../../lib/ipc/bindings';
import { log } from '../../lib/log';
import { Checkbox, TextField } from '../settings/components/fields';
import { useDebouncedValue } from '../versioning/api';
import { useOpenReport, useReportPreview } from './api';
import { reportButtonElement } from './ReportButton';
import { screenOf } from './screen';
import { closeReportDialog, useReportDialog } from './store';
import './report.css';

/** Ganchos dos testes (E2E e componente). */
const TEST_IDS = {
  open: 'report-open',
  title: 'report-title',
  did: 'report-did',
  happened: 'report-happened',
  expected: 'report-expected',
  collected: 'report-collected',
  previewTitle: 'report-preview-title',
  previewBody: 'report-preview-body',
} as const;

/** Maior título aceito pelo GitHub. */
export const MAX_TITLE = 256;
/** Maior texto de cada campo (o Rust corta o que não couber no endereço). */
export const MAX_FIELD = 4000;

/** O diálogo, ligado ao estado global (botões e atalho). */
export function ReportDialog() {
  const open = useReportDialog((state) => state.open);
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) closeReportDialog();
      }}
    >
      {open ? <ReportForm /> : null}
    </Dialog>
  );
}

interface Fields {
  title: string;
  whatYouDid: string;
  whatHappened: string;
  whatYouExpected: string;
  includeLog: boolean;
}

const EMPTY: Fields = {
  title: '',
  whatYouDid: '',
  whatHappened: '',
  whatYouExpected: '',
  includeLog: true,
};

function ReportForm() {
  const { t } = useTranslation('relato');
  const lastError = useReportDialog((state) => state.lastError);
  const router = useRouter();
  // A tela em que o diálogo abriu (ela não muda com o diálogo aberto).
  const [{ screen, packId }] = useState(() =>
    screenOf(
      router.state.matches.map((match) => ({
        routeId: match.routeId,
        params: match.params,
      })),
      t,
    ),
  );
  const [fields, setFields] = useState<Fields>(EMPTY);
  const [titleTouched, setTitleTouched] = useState(false);

  const input = useMemo<ProblemReportInput>(
    () => ({ ...fields, screen, packId, lastError }),
    [fields, screen, packId, lastError],
  );
  const settled = useDebouncedValue(input, 300);
  const preview = useReportPreview(settled);
  const openReport = useOpenReport();

  const titleMissing = fields.title.trim() === '';
  // A pré-visualização mostrada é a dos campos atuais (nem atrasada, nem de antes).
  const current =
    JSON.stringify(settled) === JSON.stringify(input) &&
    preview.isSuccess &&
    !preview.isPlaceholderData;
  const ready = !titleMissing && current && !openReport.isPending;

  const set =
    <K extends keyof Fields>(key: K) =>
    (value: Fields[K]) => {
      setFields((previous) => ({ ...previous, [key]: value }));
    };

  const submit = async () => {
    if (!ready) {
      setTitleTouched(true);
      return;
    }
    const data = preview.data;
    let copied: boolean | null = null;
    if (data.cut !== 'none') {
      try {
        await navigator.clipboard.writeText(data.fullBody);
        copied = true;
      } catch (cause) {
        log.warn('relato: não deu para copiar o relato completo', cause);
        copied = false;
      }
    }
    openReport.mutate(data.url, {
      onSuccess: () => {
        showToast({
          kind: 'ok',
          title: t('resultado.aberto'),
          text:
            copied === true
              ? t('resultado.copiado')
              : copied === false
                ? t('resultado.naoCopiou')
                : t('resultado.abertoTexto'),
        });
        closeReportDialog();
      },
    });
  };

  return (
    <DialogContent
      title={t('titulo')}
      description={t('descricao')}
      size="lg"
      // Aberto pelo atalho sem nada focado: o foco volta ao botão "Relatar problema".
      returnFocusTo={reportButtonElement}
      footer={
        <>
          <DialogClose asChild>
            <Button variant="ghost">{t('acoes.cancelar')}</Button>
          </DialogClose>
          <Button
            variant="primary"
            icon={ExternalLink}
            loading={openReport.isPending}
            disabled={!ready}
            data-testid={TEST_IDS.open}
            onClick={() => {
              void submit();
            }}
          >
            {openReport.isPending ? t('acoes.abrindo') : t('acoes.abrir')}
          </Button>
        </>
      }
    >
      <div className="stack report">
        <Alert kind="warn" compact role="none">
          {t('publico')}
        </Alert>
        <form
          className="stack-3"
          noValidate
          onSubmit={(event) => {
            event.preventDefault();
            void submit();
          }}
        >
          <TextField
            label={t('campos.titulo')}
            required
            aria-required="true"
            maxLength={MAX_TITLE}
            value={fields.title}
            hint={t('campos.tituloDica')}
            error={titleTouched && titleMissing ? t('campos.tituloObrigatorio') : null}
            data-testid={TEST_IDS.title}
            onBlur={() => {
              setTitleTouched(true);
            }}
            onChange={(event) => {
              set('title')(event.target.value);
            }}
          />
          <TextArea
            label={t('campos.oQueFez')}
            placeholder={t('campos.oQueFezPlaceholder')}
            value={fields.whatYouDid}
            testId={TEST_IDS.did}
            onChange={set('whatYouDid')}
          />
          <TextArea
            label={t('campos.oQueAconteceu')}
            placeholder={t('campos.oQueAconteceuPlaceholder')}
            value={fields.whatHappened}
            testId={TEST_IDS.happened}
            onChange={set('whatHappened')}
          />
          <TextArea
            label={t('campos.oQueEsperava')}
            placeholder={t('campos.oQueEsperavaPlaceholder')}
            value={fields.whatYouExpected}
            testId={TEST_IDS.expected}
            onChange={set('whatYouExpected')}
          />
          <Checkbox
            label={t('campos.incluirRegistros')}
            desc={t('campos.incluirRegistrosDica')}
            checked={fields.includeLog}
            onChange={set('includeLog')}
          />
        </form>
        <PreviewBlock preview={preview} includeLog={settled.includeLog} />
        {openReport.isError ? (
          <ErrorPanel error={openReport.error} title={t('resultado.falhou')} compact />
        ) : null}
        <p className="t-sm t-3">{t('print')}</p>
      </div>
    </DialogContent>
  );
}

interface TextAreaProps {
  label: string;
  placeholder: string;
  value: string;
  testId: string;
  onChange: (value: string) => void;
}

/** Campo de texto opcional de várias linhas. */
function TextArea({ label, placeholder, value, testId, onChange }: TextAreaProps) {
  const { t } = useTranslation('relato');
  const id = useId();
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>
        {label} <span className="opt">{t('campos.opcional')}</span>
      </label>
      <textarea
        id={id}
        className="textarea report__textarea"
        rows={2}
        maxLength={MAX_FIELD}
        value={value}
        placeholder={placeholder}
        data-testid={testId}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
    </div>
  );
}

function PreviewBlock({
  preview,
  includeLog,
}: {
  preview: ReturnType<typeof useReportPreview>;
  includeLog: boolean;
}) {
  const { t } = useTranslation('relato');
  if (preview.isError && !preview.data) {
    return <ErrorPanel error={preview.error} title={t('previa.erro')} compact />;
  }
  if (!preview.data) {
    return (
      <p className="row t-sm t-2" role="status">
        <Loader />
        <span>{t('previa.montando')}</span>
      </p>
    );
  }
  return <PreviewContent data={preview.data} includeLog={includeLog} />;
}

function PreviewContent({ data, includeLog }: { data: ProblemReportPreview; includeLog: boolean }) {
  const { t } = useTranslation('relato');
  const bodyId = useId();
  return (
    <>
      <section className="stack-2" aria-labelledby={`${bodyId}-coletado`}>
        <h3 className="t-caps t-3" id={`${bodyId}-coletado`}>
          {t('coletado.titulo')}
        </h3>
        <dl className="kv" data-testid={TEST_IDS.collected}>
          {data.environment.map((field) => (
            <div key={field.label} className="report__kv">
              <dt>{field.label}</dt>
              <dd>{field.value}</dd>
            </div>
          ))}
        </dl>
        <p className="t-sm t-2">
          {includeLog
            ? t('coletado.registros', { count: data.logLines })
            : t('coletado.semRegistros')}
        </p>
      </section>
      <section className="stack-2" aria-labelledby={`${bodyId}-previa`}>
        <h3 className="t-caps t-3" id={`${bodyId}-previa`}>
          {t('previa.titulo')}
        </h3>
        <p className="t-sm t-2">{t('previa.explicacao')}</p>
        <div className="report__label t-xs t-3">{t('previa.tituloDaIssue')}</div>
        <div className="code report__title" data-testid={TEST_IDS.previewTitle}>
          {data.title === '' ? t('previa.semTitulo') : data.title}
        </div>
        <div className="report__label t-xs t-3" id={bodyId}>
          {t('previa.corpo')}
        </div>
        <pre
          className="code code--scroll report__body"
          // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- região rolável: o teclado precisa chegar nela
          tabIndex={0}
          aria-labelledby={bodyId}
          data-testid={TEST_IDS.previewBody}
        >
          {data.body}
        </pre>
        {data.cut !== 'none' ? (
          <Alert kind="info" compact role="status">
            {data.cut === 'log' ? t('corte.registro') : t('corte.texto')}
          </Alert>
        ) : null}
      </section>
    </>
  );
}
