/**
 * **Nova conversa** no Sensor Sculk (SPEC T14 "Na seção"; protótipo
 * `design/bug-01/sensor-sculk.html`): o que está acontecendo (opções em botões, como rádios), o
 * que analisar ("Começar pelo"), "Conte o que aconteceu", **Como procurar** (Procurar rápido ou
 * a fundo, só no Claude e na OpenAI; ADR-0064) com o provedor e o modelo, e **Começar conversa**,
 * que abre o consentimento. Começar fica indisponível, com a dica, até escolher o sintoma.
 *
 * Os botões "Perguntar ao Sculk" (resultado do teste) e "Conversar com o Sculk" (Travamentos)
 * trazem `sessao` na busca: a sessão vem escolhida em "Começar pelo" e o consentimento abre
 * sozinho.
 */
import { Link } from '@tanstack/react-router';
import { Check, Sparkles } from 'lucide-react';
import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import { Tooltip } from '../../../components/ui/tooltip';
import type {
  AiLocalPick,
  AiPreview,
  AiProvider,
  AiSessionOption,
  AiSource,
  AiSymptom,
  PackId,
  SearchMode,
} from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { aiErrorMessage, modelFor, providerName } from '../../settings/ai/providers';
import { formatWhen } from '../../test/text';
import { useAiModels, useAiPreview, useAiSources, useChooseLog } from './api';
import { PackConsentDialog } from './ConsentDialog';
import { ModeSelect } from './ModeSelect';
import { symptomLabel } from './text';

export const SYMPTOMS: readonly AiSymptom[] = ['crashOnStart', 'crashInWorld', 'slow', 'other'];

/** O valor do `<select>` de "Começar pelo". */
type SourceChoice = `sessao:${string}` | 'ultimo' | 'arquivo' | 'pack';
const LAST_CRASH: SourceChoice = 'ultimo';
const LOCAL_FILE: SourceChoice = 'arquivo';
const PACK_ONLY: SourceChoice = 'pack';

/** Opções em botões que funcionam como rádios (as setas trocam, como num rádio nativo). */
export function ChipRadios<T extends string>({
  options,
  value,
  onChange,
  labelledBy,
  describedBy,
}: {
  options: readonly { value: T; label: string }[];
  value: T | null;
  onChange: (value: T) => void;
  labelledBy: string;
  describedBy?: string;
}) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const values = options.map((option) => option.value);
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
    if (step === undefined) return;
    event.preventDefault();
    const current = value ? values.indexOf(value) : -1;
    const index = (current + step + values.length) % values.length;
    const next = values[index];
    if (next) {
      onChange(next);
      refs.current[index]?.focus();
    }
  };
  return (
    // As setas trocam a opção, como num grupo de rádio nativo.
    // eslint-disable-next-line jsx-a11y/interactive-supports-focus
    <div
      className="chips"
      role="radiogroup"
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      onKeyDown={onKeyDown}
    >
      {options.map((option, index) => (
        <button
          key={option.value}
          ref={(element) => {
            refs.current[index] = element;
          }}
          type="button"
          role="radio"
          className="chip"
          aria-checked={value === option.value}
          tabIndex={(value ? value === option.value : index === 0) ? 0 : -1}
          onClick={() => {
            onChange(option.value);
          }}
        >
          {value === option.value ? <Icon icon={Check} size="sm" /> : null}
          {option.label}
        </button>
      ))}
    </div>
  );
}

/** O sintoma ("O que está acontecendo?"). */
export function SymptomChips(props: {
  value: AiSymptom | null;
  onChange: (symptom: AiSymptom) => void;
  labelledBy: string;
  describedBy: string;
}) {
  return (
    <ChipRadios
      options={SYMPTOMS.map((symptom) => ({ value: symptom, label: symptomLabel(symptom) }))}
      {...props}
    />
  );
}

/** "Como procurar" e o provedor e o modelo das conversas novas. */
export function ModeField({
  provider,
  mode,
  onChange,
}: {
  provider: AiProvider;
  mode: SearchMode;
  onChange: (mode: SearchMode) => void;
}) {
  const { t } = useTranslation('ia');
  const labelId = useId();
  const models = useAiModels(provider, true);
  const gemini = provider === 'gemini';
  const model = modelFor(models.data, gemini ? 'deep' : mode);
  const line = models.isPending
    ? t('modo.consultando')
    : models.isError
      ? aiErrorMessage(toAppError(models.error), provider)
      : model
        ? t('modo.provedor', { provedor: providerName(provider), modelo: model })
        : t('modo.provedorSemModelo', { provedor: providerName(provider) });
  return (
    <div className="fgroup">
      {gemini ? null : (
        <>
          <span className="flabel" id={labelId}>
            {t('modo.rotulo')}
          </span>
          <ModeSelect value={mode} onChange={onChange} labelledBy={labelId} />
          <p className="fhint">{t(`modo.hint.${mode}`)}</p>
        </>
      )}
      <p className="provline">
        <span>
          {line}
          {gemini ? ` ${t('modo.gemini')}` : ''}
        </span>
        <Link to="/configuracoes" className="link">
          {t('lista.rodape.mudar')}
        </Link>
      </p>
    </div>
  );
}

export function NewConversation({
  packId,
  sessionId,
  provider,
  onStarted,
}: {
  packId: PackId;
  sessionId: string | null;
  provider: AiProvider;
  onStarted: (conversationId: string) => void;
}) {
  const { t } = useTranslation('ia');
  const sources = useAiSources(packId);
  const preview = useAiPreview(packId);
  const chooseLog = useChooseLog(packId);
  const ids = useId();
  const [symptom, setSymptom] = useState<AiSymptom | null>(sessionId ? 'crashInWorld' : null);
  const [choice, setChoice] = useState<SourceChoice | null>(null);
  const [pick, setPick] = useState<AiLocalPick | null>(null);
  const [note, setNote] = useState('');
  const [mode, setMode] = useState<SearchMode>('deep');
  const [consent, setConsent] = useState<AiPreview | null>(null);
  const opener = useRef<HTMLButtonElement>(null);
  const autoOpened = useRef(false);

  const data = sources.data;
  const lastCrash = data?.lastCrash ?? null;
  const defaultChoice: SourceChoice = sessionId
    ? `sessao:${sessionId}`
    : lastCrash
      ? 'ultimo'
      : 'pack';
  const current = choice ?? defaultChoice;

  const sourceOf = (value: SourceChoice): AiSource | null => {
    if (value === 'ultimo') return { kind: 'lastCrash' };
    if (value === 'pack') return { kind: 'packOnly' };
    if (value === 'arquivo') return pick ? { kind: 'localFile', token: pick.token } : null;
    return { kind: 'session', sessionId: value.slice('sessao:'.length) };
  };

  const begin = (value: SourceChoice = current, chosen: AiSymptom | null = symptom) => {
    const source = sourceOf(value);
    if (!source || !chosen || preview.isPending) return;
    preview.mutate(
      {
        symptom: chosen,
        source,
        note: note.trim() === '' ? null : note,
        mode: provider === 'gemini' ? null : mode,
      },
      {
        onSuccess: (result) => {
          setConsent(result);
        },
      },
    );
  };

  // Vindo de "Perguntar ao Sculk" ou "Conversar com o Sculk": o consentimento abre sozinho.
  useEffect(() => {
    if (sessionId && data && !autoOpened.current) {
      autoOpened.current = true;
      begin(`sessao:${sessionId}`, symptom ?? 'crashInWorld');
    }
  });

  const onSelect = (value: SourceChoice) => {
    if (value !== 'arquivo') {
      setChoice(value);
      return;
    }
    chooseLog.mutate(undefined, {
      onSuccess: (picked) => {
        if (picked) {
          setPick(picked);
          setChoice('arquivo');
        }
      },
      onError: (error) => {
        showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
      },
    });
  };

  const optionLabel = (option: AiSessionOption, last: boolean): string => {
    const quando = formatWhen(option.startedAtMs);
    return last
      ? t('nova.fonte.ultimo', { quando })
      : t('nova.fonte.sessao', { quando, resultado: t(`resultado.${option.outcome}`) });
  };
  const sessionOptions = (data?.sessions ?? []).filter(
    (option) => option.sessionId !== lastCrash?.sessionId || sessionId === option.sessionId,
  );
  const ready = symptom !== null && (current !== 'arquivo' || pick !== null);

  const start = (
    <Button
      ref={opener}
      variant="primary"
      icon={Sparkles}
      loading={preview.isPending}
      aria-disabled={!ready}
      onClick={() => {
        if (ready) begin();
      }}
    >
      {preview.isPending ? t('nova.preparando') : t('nova.comecar')}
    </Button>
  );

  return (
    <form
      className="newconv"
      noValidate
      aria-labelledby={`${ids}-titulo`}
      onSubmit={(event) => {
        event.preventDefault();
      }}
    >
      <h3 id={`${ids}-titulo`}>{t('nova.titulo')}</h3>
      <p className="newconv__sub">{t('nova.sub')}</p>
      <div className="fgroup">
        <span className="flabel" id={`${ids}-sintoma`}>
          {t('nova.sintoma')}
        </span>
        <SymptomChips
          value={symptom}
          onChange={setSymptom}
          labelledBy={`${ids}-sintoma`}
          describedBy={`${ids}-sintoma-dica`}
        />
        <p className="fhint" id={`${ids}-sintoma-dica`}>
          {symptom ? t(`sintoma.${symptom}.desc`) : t('nova.sintomaDica')}
        </p>
      </div>
      <div className="fgroup">
        <label className="flabel" htmlFor={`${ids}-fonte`}>
          {t('nova.fonte.rotulo')}
        </label>
        <select
          id={`${ids}-fonte`}
          className="select"
          value={current}
          disabled={sources.isPending || chooseLog.isPending}
          onChange={(event) => {
            onSelect(event.target.value as SourceChoice);
          }}
        >
          {lastCrash ? <option value={LAST_CRASH}>{optionLabel(lastCrash, true)}</option> : null}
          {sessionOptions.map((option) => (
            <option key={option.sessionId} value={`sessao:${option.sessionId}`}>
              {optionLabel(option, false)}
            </option>
          ))}
          <option value={LOCAL_FILE}>
            {pick
              ? t('nova.fonte.arquivoEscolhido', { arquivo: pick.fileName })
              : t('nova.fonte.arquivo')}
          </option>
          <option value={PACK_ONLY}>{t('nova.fonte.pack')}</option>
        </select>
        <p className="fhint">
          {pick && current === 'arquivo'
            ? `${t('nova.fonte.arquivoDica', { tamanho: formatBytes(pick.bytes) })} `
            : ''}
          {t('nova.fonte.dica')}
        </p>
      </div>
      <div className="fgroup">
        <label className="flabel" htmlFor={`${ids}-nota`}>
          {t('nova.nota')} <span className="opt">{t('nova.opcional')}</span>
        </label>
        <textarea
          id={`${ids}-nota`}
          className="textarea"
          maxLength={2000}
          value={note}
          spellCheck={false}
          placeholder={t('nova.notaExemplo')}
          onChange={(event) => {
            setNote(event.target.value);
          }}
        />
      </div>
      <ModeField provider={provider} mode={mode} onChange={setMode} />
      {preview.isError ? (
        <Alert kind="danger" compact title={t('nova.erro')}>
          {aiErrorMessage(toAppError(preview.error), provider)}
        </Alert>
      ) : null}
      <div className="newconv__go">
        {ready ? start : <Tooltip content={t('nova.comecarDica')}>{start}</Tooltip>}
        <p className="fhint">{t('nova.antes')}</p>
      </div>
      {consent ? (
        <PackConsentDialog
          packId={packId}
          preview={consent}
          returnFocusTo={() => opener.current}
          onClose={() => {
            setConsent(null);
          }}
          onStarted={(id) => {
            setConsent(null);
            onStarted(id);
          }}
        />
      ) : null}
    </form>
  );
}
