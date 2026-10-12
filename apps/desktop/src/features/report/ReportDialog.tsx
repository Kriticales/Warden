/**
 * Diálogo "Relatar problema" (UI-02, redesenhado na UI-03): categoria (obrigatória), título
 * (obrigatório), uma descrição só (opcional), "Incluir os registros do app", "Copiar um print
 * desta tela", os três passos do envio e, recolhidos, o que o Warden coleta sozinho e a
 * pré-visualização exata. "Abrir no GitHub" abre a issue nova já preenchida no navegador; nada
 * sai antes desse clique, e quem envia é a pessoa, no GitHub.
 *
 * Toda a montagem e a redação acontecem no Rust (`problem_report_preview`). Com o print
 * marcado, o diálogo some, o Rust copia o print só da janela (`problem_report_capture`) e o
 * diálogo volta antes de abrir o navegador. A área de transferência guarda uma coisa só: se o
 * relato foi cortado para o endereço caber, o texto completo vai para ela quando não há print;
 * com o print, o aviso final ganha o botão "Copiar relato completo", e nenhum dos dois se perde.
 */
import { useRouter } from '@tanstack/react-router';
import {
  ExternalLink,
  Flag,
  Globe,
  Hourglass,
  Lightbulb,
  MessageSquare,
  TriangleAlert,
  Type,
  type LucideIcon,
} from 'lucide-react';
import { useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../components/common/ErrorPanel';
import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../components/ui/dialog';
import { Icon } from '../../components/ui/icon';
import { Loader } from '../../components/ui/loader';
import { showToast, type ToastAction, type ToastInput } from '../../components/ui/toast';
import type {
  ProblemReportInput,
  ProblemReportPreview,
  ReportCategory,
  ReportCut,
} from '../../lib/ipc/bindings';
import { log } from '../../lib/log';
import { Checkbox, TextField } from '../settings/components/fields';
import { useDebouncedValue } from '../versioning/api';
import { copyWindowScreenshot, useOpenReport, useReportPreview } from './api';
import { reportButtonElement } from './ReportButton';
import { screenOf } from './screen';
import { closeReportDialog, useReportDialog } from './store';
import './report.css';

/** Ganchos dos testes (E2E e componente). */
const TEST_IDS = {
  open: 'report-open',
  title: 'report-title',
  description: 'report-description',
  count: 'report-count',
  screenshot: 'report-screenshot',
  steps: 'report-steps',
  collected: 'report-collected',
  previewTitle: 'report-preview-title',
  previewLabels: 'report-preview-labels',
  previewBody: 'report-preview-body',
} as const;

/** Maior título aceito pelo GitHub. */
export const MAX_TITLE = 256;
/** Maior descrição (o Rust corta o que não couber no endereço). */
export const MAX_DESCRIPTION = 4000;

/** As categorias, na ordem da tela, com o ícone de cada uma. */
export const CATEGORIES: readonly { id: ReportCategory; icon: LucideIcon }[] = [
  { id: 'error', icon: TriangleAlert },
  { id: 'visual', icon: Type },
  { id: 'slowness', icon: Hourglass },
  { id: 'suggestion', icon: Lightbulb },
  { id: 'other', icon: MessageSquare },
];

/** Classe do fundo do diálogo enquanto o print é tirado (o diálogo some do print). */
export const CAPTURING_CLASS = 'report-scrim--capturing';

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

/** Espera a tela ser pintada de novo (o diálogo escondido precisa sumir antes do print). */
function nextPaint(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        // O WebView2 e o WebKitGTK compõem a janela depois do quadro: uma folga curta.
        setTimeout(resolve, 120);
      });
    });
  });
}

/** O que aconteceu com o print no envio. */
type Shot = 'none' | 'copied' | 'failed';

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
        search: match.search,
      })),
      t,
    ),
  );
  const [fields, setFields] = useState<Fields>(EMPTY);
  const [includeScreenshot, setIncludeScreenshot] = useState(false);
  const [capturing, setCapturing] = useState(false);
  const [titleTouched, setTitleTouched] = useState(false);
  const openRef = useRef<HTMLButtonElement>(null);
  const descriptionId = useId();

  const input = useMemo<ProblemReportInput>(
    () => ({ ...fields, screen, packId, lastError }),
    [fields, screen, packId, lastError],
  );
  const settled = useDebouncedValue(input, 300);
  const preview = useReportPreview(settled);
  const openReport = useOpenReport();

  const titleMissing = fields.title.trim() === '';
  // A pré-visualização usada é a dos campos atuais (nem atrasada, nem de antes).
  const current =
    JSON.stringify(settled) === JSON.stringify(input) &&
    preview.isSuccess &&
    !preview.isPlaceholderData;
  const busy = capturing || openReport.isPending;
  const ready = !titleMissing && current && !busy;

  const set =
    <K extends keyof Fields>(key: K) =>
    (value: Fields[K]) => {
      setFields((previous) => ({ ...previous, [key]: value }));
    };

  /** Copia o relato completo; `false` se a área de transferência recusou. */
  const copyFullBody = async (fullBody: string): Promise<boolean> => {
    try {
      await navigator.clipboard.writeText(fullBody);
      return true;
    } catch (cause) {
      log.warn('relato: não deu para copiar o relato completo', cause);
      return false;
    }
  };

  /** Botão do aviso que copia o relato completo (quando ele não está na área de transferência). */
  const copyAction = (fullBody: string): ToastAction => ({
    label: t('resultado.copiarRelato'),
    altText: t('resultado.copiarRelatoAlt'),
    onClick: () => {
      void copyFullBody(fullBody).then((copied) => {
        showToast(
          copied
            ? {
                kind: 'ok',
                title: t('resultado.relatoCopiado'),
                text: t('resultado.relatoCopiadoTexto'),
              }
            : { kind: 'warn', title: t('resultado.relatoNaoCopiado') },
        );
      });
    },
  });

  const submit = async () => {
    if (!ready) {
      setTitleTouched(true);
      return;
    }
    const data = preview.data;
    let shot: Shot = 'none';
    if (includeScreenshot) {
      setCapturing(true);
      try {
        await nextPaint();
        shot = (await copyWindowScreenshot()) ? 'copied' : 'failed';
      } finally {
        setCapturing(false);
      }
      // O diálogo volta à tela antes de o navegador abrir.
      await nextPaint();
    }
    // Sem print na área de transferência, o relato completo vai para ela.
    const textCopied =
      data.cut !== 'none' && shot !== 'copied' ? await copyFullBody(data.fullBody) : null;
    openReport.mutate(data.url, {
      onSuccess: () => {
        showToast(resultToast(t, data, shot, textCopied, copyAction(data.fullBody)));
        closeReportDialog();
      },
      onError: () => {
        openRef.current?.focus();
      },
    });
  };

  const category = fields.category;
  return (
    <DialogContent
      title={
        <>
          <Icon icon={Flag} className="report__flag" />
          {t('titulo')}
        </>
      }
      description={t('descricao')}
      size="lg"
      aside={
        <span className="report__shortcut t-xs t-3">
          <span className="sr-only">{t('atalhoRotulo')} </span>
          <kbd className="kbd">{t('atalho')}</kbd>
        </span>
      }
      {...(capturing ? { overlayClassName: CAPTURING_CLASS } : {})}
      // Aberto pelo atalho sem nada focado: o foco volta ao botão "Relatar problema".
      returnFocusTo={reportButtonElement}
      footerSplit
      footer={
        <>
          <p className="report__public t-xs t-3">
            <Icon icon={Globe} size="sm" />
            <span>{t('publico')}</span>
          </p>
          <div className="btn-row">
            <DialogClose asChild>
              <Button variant="ghost">{t('acoes.cancelar')}</Button>
            </DialogClose>
            <Button
              ref={openRef}
              variant="primary"
              icon={ExternalLink}
              loading={busy}
              disabled={!ready}
              data-testid={TEST_IDS.open}
              onClick={() => {
                void submit();
              }}
            >
              {capturing
                ? t('acoes.copiandoPrint')
                : openReport.isPending
                  ? t('acoes.abrindo')
                  : t('acoes.abrir')}
            </Button>
          </div>
        </>
      }
    >
      <div className="stack report">
        <form
          className="stack-3"
          noValidate
          onSubmit={(event) => {
            event.preventDefault();
            void submit();
          }}
        >
          <CategoryPicker value={category} onChange={set('category')} />
          <TextField
            label={t('campos.titulo')}
            required
            aria-required="true"
            maxLength={MAX_TITLE}
            value={fields.title}
            hint={t(`categoria.${category}.dica`)}
            error={titleTouched && titleMissing ? t('campos.tituloObrigatorio') : null}
            data-testid={TEST_IDS.title}
            onBlur={() => {
              setTitleTouched(true);
            }}
            onChange={(event) => {
              set('title')(event.target.value);
            }}
          />
          <div className="field">
            <label className="field__label" htmlFor={descriptionId}>
              {t('campos.descricao')} <span className="opt">{t('campos.opcional')}</span>
            </label>
            <textarea
              id={descriptionId}
              className="textarea report__description"
              rows={5}
              maxLength={MAX_DESCRIPTION}
              value={fields.description}
              placeholder={t(`categoria.${category}.exemplo`)}
              aria-describedby={`${descriptionId}-count`}
              data-testid={TEST_IDS.description}
              onChange={(event) => {
                set('description')(event.target.value);
              }}
            />
            <p
              className="report__count t-xs t-3"
              id={`${descriptionId}-count`}
              data-testid={TEST_IDS.count}
            >
              <span aria-hidden="true">
                {t('campos.contador', { count: fields.description.length, max: MAX_DESCRIPTION })}
              </span>
              <span className="sr-only">
                {t('campos.contadorLeitor', {
                  count: fields.description.length,
                  max: MAX_DESCRIPTION,
                })}
              </span>
            </p>
          </div>
          <div className="stack-2">
            <Checkbox
              label={t('campos.incluirRegistros')}
              desc={t('campos.incluirRegistrosDica')}
              checked={fields.includeLog}
              onChange={set('includeLog')}
            />
            <div data-testid={TEST_IDS.screenshot}>
              <Checkbox
                label={
                  <>
                    {t('campos.print')}{' '}
                    <span className="badge badge--info report__new">{t('campos.novo')}</span>
                  </>
                }
                desc={t('campos.printDica')}
                checked={includeScreenshot}
                onChange={setIncludeScreenshot}
              />
            </div>
          </div>
        </form>
        <Steps />
        {preview.data && preview.data.cut !== 'none' ? (
          <Alert kind="info" compact role="status">
            {cutText(t, preview.data.cut, includeScreenshot)}
          </Alert>
        ) : null}
        <PreviewBlock preview={preview} includeLog={settled.includeLog} />
        {openReport.isError ? (
          <ErrorPanel error={openReport.error} title={t('resultado.falhou')} compact />
        ) : null}
      </div>
    </DialogContent>
  );
}

type Translate = ReturnType<typeof useTranslation<'relato'>>['t'];

/** O aviso do corte, que muda com o print (ele ocupa a área de transferência). */
function cutText(t: Translate, cut: Exclude<ReportCut, 'none'>, withScreenshot: boolean): string {
  if (cut === 'log') return withScreenshot ? t('corte.registroComPrint') : t('corte.registro');
  return withScreenshot ? t('corte.textoComPrint') : t('corte.texto');
}

/**
 * O aviso depois de abrir. O relato completo nunca se perde: ou está na área de transferência,
 * ou o aviso fica na tela com o botão "Copiar relato completo".
 */
export function resultToast(
  t: Translate,
  data: Pick<ProblemReportPreview, 'cut'>,
  shot: Shot,
  textCopied: boolean | null,
  copyAction: ToastAction,
): ToastInput {
  const title = t('resultado.aberto');
  const cut = data.cut !== 'none';
  if (shot === 'copied') {
    return cut
      ? {
          kind: 'ok',
          title,
          text: t('resultado.printCopiadoComCorte'),
          action: copyAction,
          persistent: true,
        }
      : { kind: 'ok', title, text: t('resultado.printCopiado') };
  }
  if (cut && textCopied === false) {
    return {
      kind: 'warn',
      title,
      text: t('resultado.naoCopiou'),
      action: copyAction,
      persistent: true,
    };
  }
  if (shot === 'failed') {
    return {
      kind: 'warn',
      title,
      text: cut ? t('resultado.printFalhouComCorte') : t('resultado.printFalhou'),
    };
  }
  return { kind: 'ok', title, text: cut ? t('resultado.copiado') : t('resultado.abertoTexto') };
}

/** As cinco categorias em blocos: um grupo de rádio (setas trocam, Tab sai). */
function CategoryPicker({
  value,
  onChange,
}: {
  value: ReportCategory;
  onChange: (value: ReportCategory) => void;
}) {
  const { t } = useTranslation('relato');
  const name = useId();
  return (
    <fieldset className="report__categories">
      <legend className="field__label">{t('categoria.legenda')}</legend>
      <div className="report__category-grid">
        {CATEGORIES.map(({ id, icon }) => (
          <label key={id} className="report__category">
            <input
              type="radio"
              className="sr-only"
              name={name}
              value={id}
              required
              checked={value === id}
              aria-labelledby={`${name}-${id}`}
              aria-describedby={`${name}-${id}-desc`}
              onChange={() => {
                onChange(id);
              }}
            />
            <Icon icon={icon} size="lg" />
            <span className="report__category-name" id={`${name}-${id}`}>
              {t(`categoria.${id}.nome`)}
            </span>
            <span className="report__category-desc" id={`${name}-${id}-desc`}>
              {t(`categoria.${id}.desc`)}
            </span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}

/** Os três passos do envio, numerados e à vista. */
function Steps() {
  const { t } = useTranslation('relato');
  const steps = [
    [t('passos.abrir'), t('passos.abrirTexto')],
    [t('passos.colar'), t('passos.colarTexto')],
    [t('passos.enviar'), t('passos.enviarTexto')],
  ] as const;
  return (
    <ol className="report__steps" aria-label={t('passos.titulo')} data-testid={TEST_IDS.steps}>
      {steps.map(([name, text], index) => (
        <li key={name}>
          <span className="report__step-number" aria-hidden="true">
            {index + 1}
          </span>
          <span>
            <strong className="block t-sm">{name}</strong>
            <span className="block t-xs t-2">{text}</span>
          </span>
        </li>
      ))}
    </ol>
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

/** O coletado e a pré-visualização, recolhidos: abre quem quiser conferir. */
function PreviewContent({ data, includeLog }: { data: ProblemReportPreview; includeLog: boolean }) {
  const { t } = useTranslation('relato');
  const bodyId = useId();
  return (
    <div className="stack-2">
      <details className="disclosure">
        <summary>{t('coletado.titulo')}</summary>
        <dl className="kv report__collected" data-testid={TEST_IDS.collected}>
          {data.environment.map((field) => (
            <div key={field.label} className="report__kv">
              <dt>{field.label}</dt>
              <dd>{field.value}</dd>
            </div>
          ))}
          <div className="report__kv">
            <dt>{t('coletado.registrosRotulo')}</dt>
            <dd>
              {includeLog
                ? t('coletado.registros', { count: data.logLines })
                : t('coletado.semRegistros')}
            </dd>
          </div>
        </dl>
      </details>
      <details className="disclosure">
        <summary>{t('previa.titulo')}</summary>
        <div className="panel stack-2 report__preview">
          <p className="t-sm t-2">{t('previa.explicacao')}</p>
          <div className="report__label t-xs t-3">{t('previa.tituloDaIssue')}</div>
          <div className="report__title" data-testid={TEST_IDS.previewTitle}>
            {data.title === '' ? t('previa.semTitulo') : data.title}
          </div>
          <div className="report__label t-xs t-3">{t('previa.rotulos')}</div>
          <ul className="report__labels" data-testid={TEST_IDS.previewLabels}>
            {data.labels.map((label) => (
              <li key={label} className="report__gh-label">
                {label}
              </li>
            ))}
          </ul>
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
        </div>
      </details>
    </div>
  );
}
