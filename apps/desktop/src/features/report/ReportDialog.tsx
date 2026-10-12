/**
 * Diálogo "Relatar problema" (UI-02 e UI-03): categoria, título (obrigatório), descrição,
 * prints, "Incluir os registros do app", o que o Warden coletou sozinho e a pré-visualização
 * exata do que vai para o GitHub.
 *
 * - Com o login do GitHub (Configurações → Chaves e contas): **Enviar** cria a issue pela API,
 *   com os prints, e mostra "Relato enviado (#número)" com **Ver no GitHub**. Numa falha, o
 *   rascunho fica: **Tentar de novo** e **Abrir no GitHub**.
 * - Sem login: o aviso para entrar nas Configurações e **Abrir no GitHub**, que abre a issue
 *   nova já preenchida no navegador (UI-02); se algo foi cortado para o endereço caber, o
 *   relato completo vai para a área de transferência no clique.
 *
 * Toda a montagem e a redação acontecem no Rust (`problem_report_preview` e, no envio,
 * `problem_report_send`, que monta de novo). Os prints ficam no Rust; aqui só o token, o nome
 * e a miniatura.
 */
import { useNavigate, useRouter } from '@tanstack/react-router';
import { ExternalLink, ImagePlus, Send, Trash2 } from 'lucide-react';
import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../components/common/ErrorPanel';
import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../components/ui/dialog';
import { Loader } from '../../components/ui/loader';
import { showToast } from '../../components/ui/toast';
import {
  events,
  type ProblemReportInput,
  type ProblemReportPreview,
  type ProblemReportSent,
  type RefusedImage,
  type ReportCategory,
  type ReportImage,
  type ReportImagesAdded,
} from '../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../lib/ipc/errors';
import { useTauriEvent } from '../../lib/ipc/events';
import { formatBytes } from '../../lib/format';
import { log } from '../../lib/log';
import { Checkbox, SelectField, TextField } from '../settings/components/fields';
import { useGithubLogin } from '../settings/github/api';
import { useDebouncedValue } from '../versioning/api';
import {
  detachImage,
  readAsBase64,
  setReportDraft,
  useAttachImage,
  useChooseImages,
  useOpenReport,
  useReportPreview,
  useSendReport,
  type SendStep,
} from './api';
import { reportButtonElement } from './ReportButton';
import { screenOf } from './screen';
import { closeReportDialog, useReportDialog } from './store';
import './report.css';

/** Ganchos dos testes (E2E e componente). */
const TEST_IDS = {
  open: 'report-open',
  send: 'report-send',
  category: 'report-category',
  title: 'report-title',
  description: 'report-description',
  prints: 'report-prints',
  choose: 'report-choose-images',
  progress: 'report-progress',
  sent: 'report-sent',
  collected: 'report-collected',
  previewTitle: 'report-preview-title',
  previewBody: 'report-preview-body',
} as const;

/** Maior título aceito pelo GitHub. */
export const MAX_TITLE = 256;
/** Maior descrição (o Rust corta o que não couber). */
export const MAX_DESCRIPTION = 8000;
/** Mais prints num relato (o mesmo limite do Rust). */
export const MAX_PRINTS = 5;

/** Âncora da seção "Chaves e contas" em Configurações (`settings/sections.ts`). */
const KEYS_SECTION = 'chaves';

const CATEGORIES: readonly ReportCategory[] = [
  'error',
  'suggestion',
  'visualOrText',
  'slowness',
  'other',
];

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
  category: ReportCategory;
  title: string;
  description: string;
  includeLog: boolean;
}

const EMPTY: Fields = {
  category: 'error',
  title: '',
  description: '',
  includeLog: true,
};

function ReportForm() {
  const { t } = useTranslation('relato');
  const lastError = useReportDialog((state) => state.lastError);
  const router = useRouter();
  const navigate = useNavigate();
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
  const login = useGithubLogin();
  const direct = typeof login.data?.login === 'string';
  const [fields, setFields] = useState<Fields>(EMPTY);
  const [titleTouched, setTitleTouched] = useState(false);
  const [images, setImages] = useState<ReportImage[]>([]);
  const [refused, setRefused] = useState<RefusedImage[]>([]);
  const [announcement, setAnnouncement] = useState('');
  const [step, setStep] = useState<SendStep | null>(null);
  const [sent, setSent] = useState<ProblemReportSent | null>(null);

  // O rascunho de prints vive no Rust enquanto o diálogo está aberto.
  useEffect(() => {
    void setReportDraft(true);
    return () => {
      void setReportDraft(false);
    };
  }, []);

  const addImages = (result: ReportImagesAdded) => {
    setImages((current) => [
      ...current,
      ...result.added.filter((image) => !current.some((known) => known.token === image.token)),
    ]);
    setRefused(result.refused);
    if (result.added.length > 0) {
      setAnnouncement(t('prints.acrescentados', { count: result.added.length }));
    }
  };

  useTauriEvent(events.reportImagesDropped, addImages);
  const attach = useAttachImage();
  const choose = useChooseImages();

  // Ctrl+V com uma imagem na área de transferência, em qualquer ponto do diálogo.
  const onPaste = (event: ClipboardEvent) => {
    if (!direct) return;
    const files = Array.from(event.clipboardData?.files ?? []).filter((file) =>
      file.type.startsWith('image/'),
    );
    if (files.length === 0) return;
    event.preventDefault();
    void (async () => {
      const added: ReportImage[] = [];
      const failed: RefusedImage[] = [];
      for (const file of files) {
        const name = file.name === '' || file.name === 'image.png' ? 'print colado.png' : file.name;
        try {
          added.push(await attach.mutateAsync({ name, data: await readAsBase64(file) }));
        } catch (cause) {
          log.warn('relato: print colado recusado', cause);
          failed.push({ name, error: toAppError(cause) });
        }
      }
      addImages({ added, refused: failed });
    })();
  };
  const pasteRef = useRef(onPaste);
  useEffect(() => {
    pasteRef.current = onPaste;
  });
  useEffect(() => {
    const listener = (event: ClipboardEvent) => {
      pasteRef.current(event);
    };
    window.addEventListener('paste', listener);
    return () => {
      window.removeEventListener('paste', listener);
    };
  }, []);

  const input = useMemo<ProblemReportInput>(
    () => ({
      ...fields,
      attachments: direct ? images.map((image) => image.token) : [],
      screen,
      packId,
      lastError,
    }),
    [fields, direct, images, screen, packId, lastError],
  );
  const settled = useDebouncedValue(input, 300);
  const preview = useReportPreview(settled);
  const openReport = useOpenReport();
  const send = useSendReport(setStep);

  const titleMissing = fields.title.trim() === '';
  // A pré-visualização mostrada é a dos campos atuais (nem atrasada, nem de antes).
  const current =
    JSON.stringify(settled) === JSON.stringify(input) &&
    preview.isSuccess &&
    !preview.isPlaceholderData;
  const busy = openReport.isPending || send.isPending;
  const ready = !titleMissing && current && !busy;

  const set =
    <K extends keyof Fields>(key: K) =>
    (value: Fields[K]) => {
      setFields((previous) => ({ ...previous, [key]: value }));
    };

  const openInBrowser = async () => {
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

  const sendDirect = () => {
    if (!ready) {
      setTitleTouched(true);
      return;
    }
    setStep(null);
    send.mutate(input, {
      onSuccess: (result) => {
        setSent(result);
      },
    });
  };

  if (sent) {
    return <SentView sent={sent} />;
  }

  const removeImage = (image: ReportImage) => {
    void detachImage(image);
    setImages((current) => current.filter((known) => known.token !== image.token));
    setAnnouncement(t('prints.removido', { nome: image.name }));
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
          {direct ? (
            <Button
              variant="primary"
              icon={Send}
              loading={send.isPending}
              disabled={!ready}
              data-testid={TEST_IDS.send}
              onClick={sendDirect}
            >
              {send.isPending ? t('acoes.enviando') : t('acoes.enviar')}
            </Button>
          ) : (
            <Button
              variant="primary"
              icon={ExternalLink}
              loading={openReport.isPending}
              disabled={!ready}
              data-testid={TEST_IDS.open}
              onClick={() => {
                void openInBrowser();
              }}
            >
              {openReport.isPending ? t('acoes.abrindo') : t('acoes.abrir')}
            </Button>
          )}
        </>
      }
    >
      <div className="stack report">
        <Alert kind="warn" compact role="none">
          {t('publico')}
        </Alert>
        {login.isSuccess && !direct ? (
          <Alert
            kind="info"
            compact
            role="none"
            actions={
              <Button
                size="sm"
                variant="link"
                onClick={() => {
                  closeReportDialog();
                  void navigate({ to: '/configuracoes', hash: KEYS_SECTION });
                }}
              >
                {t('semLogin.ir')}
              </Button>
            }
          >
            {t('semLogin.texto')}
          </Alert>
        ) : null}
        <form
          className="stack-3"
          noValidate
          onSubmit={(event) => {
            event.preventDefault();
            if (direct) sendDirect();
            else void openInBrowser();
          }}
        >
          <SelectField
            label={t('campos.categoria')}
            value={fields.category}
            options={CATEGORIES.map((category) => ({
              value: category,
              label: t(`campos.categorias.${category}`),
            }))}
            data-testid={TEST_IDS.category}
            onChange={(event) => {
              set('category')(event.target.value as ReportCategory);
            }}
          />
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
            label={t('campos.descricao')}
            placeholder={t('campos.descricaoPlaceholder')}
            value={fields.description}
            testId={TEST_IDS.description}
            onChange={set('description')}
          />
          {direct ? (
            <Prints
              images={images}
              refused={refused}
              announcement={announcement}
              choosing={choose.isPending || attach.isPending}
              onChoose={() => {
                choose.mutate(undefined, {
                  onSuccess: addImages,
                  onError: (error) => {
                    setRefused([{ name: t('prints.escolher'), error: error.appError }]);
                  },
                });
              }}
              onRemove={removeImage}
            />
          ) : null}
          <Checkbox
            label={t('campos.incluirRegistros')}
            desc={t('campos.incluirRegistrosDica')}
            checked={fields.includeLog}
            onChange={set('includeLog')}
          />
        </form>
        <PreviewBlock preview={preview} includeLog={settled.includeLog} direct={direct} />
        <div aria-live="polite" data-testid={TEST_IDS.progress}>
          {send.isPending ? (
            <p className="row t-sm t-2">
              <Loader />
              <span>{stepText(step, t)}</span>
            </p>
          ) : null}
        </div>
        {send.isError ? (
          <ErrorPanel
            error={send.error}
            title={t('resultado.naoEnviado')}
            onRetry={sendDirect}
            actions={
              <Button
                size="sm"
                icon={ExternalLink}
                loading={openReport.isPending}
                disabled={!preview.data}
                onClick={() => {
                  void openInBrowser();
                }}
              >
                {t('acoes.abrir')}
              </Button>
            }
          />
        ) : null}
        {send.isError ? <p className="t-sm t-2">{t('resultado.rascunhoGuardado')}</p> : null}
        {openReport.isError ? (
          <ErrorPanel error={openReport.error} title={t('resultado.falhou')} compact />
        ) : null}
        {direct ? null : <p className="t-sm t-3">{t('print')}</p>}
      </div>
    </DialogContent>
  );
}

function stepText(step: SendStep | null, t: ReturnType<typeof useTranslation<'relato'>>['t']) {
  if (step === null || step.stage === 'labels') return t('progresso.labels');
  if (step.stage === 'issue') return t('progresso.issue');
  return t('progresso.prints', {
    feito: Math.min(step.done + 1, step.total),
    total: step.total,
  });
}

/** Depois do envio: o número da issue e o link. */
function SentView({ sent }: { sent: ProblemReportSent }) {
  const { t } = useTranslation('relato');
  const openReport = useOpenReport();
  return (
    <DialogContent
      title={t('titulo')}
      description={t('descricao')}
      size="lg"
      returnFocusTo={reportButtonElement}
      footer={
        <>
          <Button
            icon={ExternalLink}
            loading={openReport.isPending}
            onClick={() => {
              openReport.mutate(sent.url);
            }}
          >
            {t('acoes.verNoGithub')}
          </Button>
          <DialogClose asChild>
            <Button variant="primary">{t('acoes.concluir')}</Button>
          </DialogClose>
        </>
      }
    >
      <div className="stack report" data-testid={TEST_IDS.sent}>
        <Alert kind="ok" title={t('resultado.enviado', { numero: sent.number })} role="status">
          {t('resultado.enviadoTexto')}
        </Alert>
        {openReport.isError ? (
          <ErrorPanel error={openReport.error} title={t('resultado.linkFalhou')} compact />
        ) : null}
      </div>
    </DialogContent>
  );
}

interface PrintsProps {
  images: ReportImage[];
  refused: RefusedImage[];
  announcement: string;
  choosing: boolean;
  onChoose: () => void;
  onRemove: (image: ReportImage) => void;
}

/** Os prints: miniaturas com Remover, o aviso de dados pessoais e "Escolher imagens…". */
function Prints({ images, refused, announcement, choosing, onChoose, onRemove }: PrintsProps) {
  const { t } = useTranslation('relato');
  const headingId = useId();
  const hintId = useId();
  return (
    <section
      className="stack-2"
      aria-labelledby={headingId}
      aria-describedby={hintId}
      data-testid={TEST_IDS.prints}
    >
      <h3 className="field__label" id={headingId}>
        {t('prints.titulo')} <span className="opt">{t('campos.opcional')}</span>
      </h3>
      <p className="t-sm t-2" id={hintId}>
        {t('prints.dica')}
      </p>
      {images.length > 0 ? (
        <>
          <Alert kind="warn" compact role="none">
            {t('prints.aviso')}
          </Alert>
          <ul className="report__prints">
            {images.map((image, index) => (
              <li key={image.token} className="report__print">
                <img
                  className="report__thumb"
                  src={image.preview}
                  alt={t('prints.miniatura', { n: index + 1, nome: image.name })}
                />
                <span className="report__print-name t-sm">
                  {image.name}
                  <span className="t-xs t-3">{formatBytes(image.size)}</span>
                </span>
                <Button
                  size="sm"
                  variant="danger-ghost"
                  icon={Trash2}
                  aria-label={t('prints.removerNome', { nome: image.name })}
                  onClick={() => {
                    onRemove(image);
                  }}
                >
                  {t('prints.remover')}
                </Button>
              </li>
            ))}
          </ul>
        </>
      ) : null}
      <div>
        <Button
          size="sm"
          icon={ImagePlus}
          loading={choosing}
          disabled={images.length >= MAX_PRINTS}
          data-testid={TEST_IDS.choose}
          onClick={onChoose}
        >
          {t('prints.escolher')}
        </Button>
      </div>
      <p className="t-sm t-2" aria-live="polite">
        {announcement}
      </p>
      {refused.length > 0 ? (
        <Alert kind="danger" compact role="alert" title={t('prints.recusados')}>
          <ul className="stack-1">
            {refused.map((item, index) => (
              <li key={`${item.name}-${String(index)}`}>
                {appErrorMessage(toAppError(item.error))}
              </li>
            ))}
          </ul>
        </Alert>
      ) : null}
    </section>
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
        rows={4}
        maxLength={MAX_DESCRIPTION}
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
  direct,
}: {
  preview: ReturnType<typeof useReportPreview>;
  includeLog: boolean;
  direct: boolean;
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
  return <PreviewContent data={preview.data} includeLog={includeLog} direct={direct} />;
}

function PreviewContent({
  data,
  includeLog,
  direct,
}: {
  data: ProblemReportPreview;
  includeLog: boolean;
  direct: boolean;
}) {
  const { t } = useTranslation('relato');
  const bodyId = useId();
  const logLines = direct ? data.directLogLines : data.logLines;
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
          {includeLog ? t('coletado.registros', { count: logLines }) : t('coletado.semRegistros')}
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
          {direct ? data.directBody : data.body}
        </pre>
        {!direct && data.cut !== 'none' ? (
          <Alert kind="info" compact role="status">
            {data.cut === 'log' ? t('corte.registro') : t('corte.texto')}
          </Alert>
        ) : null}
      </section>
    </>
  );
}
