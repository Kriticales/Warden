/**
 * O detalhe da chave escolhida em Chaves e contas (esboço `design/bug-01/configuracoes.html`):
 * situação, final `••••xxxx`, a posição no Sensor Sculk e os modelos (nas chaves de IA), as
 * ações (Testar / Substituir / Remover) e o passo a passo "Como criar". Sem chave, ou ao
 * substituir, o campo de senha com **Salvar**; a chave inteira nunca aparece.
 */
import { Link } from '@tanstack/react-router';
import { Plus } from 'lucide-react';
import { useId, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type {
  AiProvider,
  AiSettingsView,
  KeyOverview,
  SecretKind,
} from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useAiSettings } from '../../diagnostics/ai/api';
import { useModelsOf } from '../ai/api';
import { aiErrorMessage, providerName } from '../ai/providers';
import { useRemoveSecret, useSetSecret } from '../api';
import { useVaultName } from '../layout/vault';
import { SCULK_PAGE } from '../pages';
import { keyState, providerOf, providerRole, successorAfterRemoval, type KeyState } from './model';
import { HowTo, KeyMask, KeyStateTag } from './parts';
import type { KeyTest } from './useKeyTests';
import { useKeyWords, useWhen } from './words';

export interface KeyDetailProps {
  kind: SecretKind;
  keyInfo: KeyOverview | undefined;
  test: KeyTest | undefined;
  /** Abre já no campo de substituir (o "Substituir a chave" do Sensor Sculk). */
  startEditing?: boolean;
  onTest: (kind: SecretKind) => void;
  /** A chave mudou (gravada ou removida): esquece o teste desta visita. */
  onChanged: (kind: SecretKind) => void;
}

export function KeyDetail({
  kind,
  keyInfo,
  test,
  startEditing = false,
  onTest,
  onChanged,
}: KeyDetailProps) {
  const { t } = useTranslation('configuracoes');
  const words = useKeyWords(kind);
  const provider = providerOf(kind);
  const state = keyState(keyInfo, test?.status === 'testing');
  const [editing, setEditing] = useState(startEditing && state !== 'none');
  const showEditor = state === 'none' || editing;

  return (
    <section className="keydet" aria-labelledby={`chave-${kind}`}>
      <div>
        <div className="keydet__head">
          <h2 className="keydet__title" id={`chave-${kind}`}>
            {words.name}
          </h2>
          <KeyStateTag kind={kind} state={state} />
        </div>
        <p className="keydet__lead">{t(`chaves.usos.${kind}`)}</p>
      </div>
      {showEditor ? (
        <>
          <KeyEditor
            kind={kind}
            replacing={state !== 'none'}
            onCancel={() => {
              setEditing(false);
            }}
            onSaved={() => {
              setEditing(false);
              onChanged(kind);
              onTest(kind);
            }}
          />
          <HowTo
            kind={kind}
            title={
              words.token
                ? t('chaves.comoCriarToken', { de: words.de })
                : t('chaves.comoCriarChave', { de: words.de })
            }
          />
        </>
      ) : (
        <>
          <KeyFacts kind={kind} keyInfo={keyInfo} state={state} provider={provider} />
          <KeyActions
            kind={kind}
            state={state}
            onTest={() => {
              onTest(kind);
            }}
            onReplace={() => {
              setEditing(true);
            }}
            onRemoved={() => {
              onChanged(kind);
            }}
          />
          <TestNote kind={kind} test={test} />
          <details className="keydet__more" open={state === 'refused' ? true : undefined}>
            <summary className="btn btn--link">
              {words.token ? t('chaves.comoCriarNovo') : t('chaves.comoCriarNova')}
            </summary>
            <HowTo kind={kind} />
          </details>
        </>
      )}
    </section>
  );
}

function KeyFacts({
  kind,
  keyInfo,
  state,
  provider,
}: {
  kind: SecretKind;
  keyInfo: KeyOverview | undefined;
  state: KeyState;
  provider: AiProvider | null;
}) {
  const { t } = useTranslation('configuracoes');
  const words = useKeyWords(kind);
  const when = useWhen()(keyInfo?.check?.at);

  let situation;
  if (state === 'testing') situation = t('chaves.situacaoTestando');
  else if (state === 'ok') situation = t('chaves.situacaoOk', { quando: when ?? '' });
  else if (state === 'refused')
    situation = t('chaves.situacaoRecusada', {
      erro: t(`chaves.recusas.${kind}`),
      quando: when ?? '',
    });
  else situation = t('chaves.situacaoNaoTestada');

  return (
    <dl className="kv">
      <dt>{t('chaves.situacao')}</dt>
      <dd className={state === 'refused' ? 'is-danger' : undefined}>{situation}</dd>
      <dt>{words.token ? t('chaves.rotuloToken') : t('chaves.rotuloChave')}</dt>
      <dd>
        <KeyMask kindToken={words.token} keyInfo={keyInfo} />
        <span className="kv__sub">
          {words.token ? t('chaves.finalDicaToken') : t('chaves.finalDica')}
        </span>
      </dd>
      {provider ? (
        <SculkFacts provider={provider} refused={state === 'refused'} />
      ) : (
        <>
          <dt>{words.token ? t('chaves.semEle') : t('chaves.semEla')}</dt>
          <dd>{t(`chaves.sem.${kind as 'curseforge' | 'github'}`)}</dd>
        </>
      )}
    </dl>
  );
}

/** A posição no Sensor Sculk e os modelos de cada modo (só para consulta). */
function SculkFacts({ provider, refused }: { provider: AiProvider; refused: boolean }) {
  const { t } = useTranslation('configuracoes');
  const settings = useAiSettings();
  const view = settings.data;
  const usable = !refused && (view ? providerRole(view, provider) !== 'noKey' : false);
  const models = useModelsOf(provider, usable);
  if (!view) return null;
  const position = view.order.indexOf(provider) + 1;
  const role = providerRole(view, provider);
  const suffix =
    role === 'use'
      ? t('chaves.posicaoUsado')
      : role === 'refused'
        ? t('chaves.posicaoPulado')
        : role === 'reserve'
          ? t('chaves.posicaoReserva')
          : '';

  let modelsText;
  if (refused) {
    modelsText = <span className="t-3">{t('chaves.modelosQuandoVoltar')}</span>;
  } else if (!models.data) {
    modelsText = (
      <span className="t-3">
        {models.isError
          ? aiErrorMessage(toAppError(models.error), provider)
          : t('chaves.modelosCarregando')}
      </span>
    );
  } else if (provider === 'gemini') {
    modelsText = (
      <>
        {t('chaves.modelosGemini', { modelo: models.data.economical ?? '' })}
        {models.data.lighter ? (
          <span className="kv__sub">
            <Trans
              t={t}
              i18nKey="chaves.modelosGeminiLeve"
              values={{ modelo: models.data.lighter }}
              components={{ m: <span className="nw" /> }}
            />
          </span>
        ) : null}
      </>
    );
  } else {
    modelsText = (
      <Trans
        t={t}
        i18nKey="chaves.modelosRapido"
        values={{ rapido: models.data.economical ?? '', fundo: models.data.careful ?? '' }}
        components={{ m: <span className="nw t-mono" /> }}
      />
    );
  }

  return (
    <>
      <dt>{t('chaves.noSculk')}</dt>
      <dd>
        {t('chaves.posicao', { pos: position })}
        {suffix} ·{' '}
        <Link to="/configuracoes/$pagina" params={{ pagina: SCULK_PAGE }} className="btn btn--link">
          {t('chaves.mudarOrdem')}
        </Link>
      </dd>
      <dt>{t('chaves.modelos')}</dt>
      <dd>{modelsText}</dd>
    </>
  );
}

function KeyActions({
  kind,
  state,
  onTest,
  onReplace,
  onRemoved,
}: {
  kind: SecretKind;
  state: KeyState;
  onTest: () => void;
  onReplace: () => void;
  onRemoved: () => void;
}) {
  const { t } = useTranslation('configuracoes');
  const [confirmRemove, setConfirmRemove] = useState(false);
  const busy = state === 'testing';
  const words = useKeyWords(kind);
  const test = (
    <Button key="test" size="sm" loading={busy} disabled={busy} onClick={onTest}>
      {busy
        ? t('chaves.testando')
        : state === 'refused'
          ? t('chaves.testarDeNovo')
          : t('chaves.testar')}
    </Button>
  );
  const replace = (
    <Button
      key="replace"
      size="sm"
      variant={state === 'refused' ? 'primary' : 'secondary'}
      disabled={busy}
      onClick={onReplace}
    >
      {t('chaves.substituir')}
    </Button>
  );
  return (
    <div className="keydet__actions">
      {state === 'refused' ? [replace, test] : [test, replace]}
      <Button
        size="sm"
        variant="danger-ghost"
        disabled={busy}
        onClick={() => {
          setConfirmRemove(true);
        }}
      >
        {t('chaves.remover')}
      </Button>
      <RemoveDialog
        kind={kind}
        open={confirmRemove}
        onOpenChange={setConfirmRemove}
        onRemoved={onRemoved}
        tokenWord={words.token}
      />
    </div>
  );
}

function RemoveDialog({
  kind,
  open,
  onOpenChange,
  onRemoved,
  tokenWord,
}: {
  kind: SecretKind;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onRemoved: () => void;
  tokenWord: boolean;
}) {
  const { t } = useTranslation('configuracoes');
  const vault = useVaultName();
  const words = useKeyWords(kind);
  const removeSecret = useRemoveSecret();
  const settings = useAiSettings();
  const provider = providerOf(kind);
  return (
    <ConfirmDialog
      open={open}
      onOpenChange={onOpenChange}
      title={
        tokenWord
          ? t('chaves.removerTituloToken', { de: words.de })
          : t('chaves.removerTituloChave', { de: words.de })
      }
      description={
        tokenWord
          ? t('chaves.removerTextoToken', { cofre: vault })
          : t('chaves.removerTexto', { cofre: vault })
      }
      confirmLabel={
        tokenWord ? t('chaves.removerConfirmarToken') : t('chaves.removerConfirmarChave')
      }
      confirmingLabel={t('chaves.removendo')}
      onConfirm={async () => {
        await removeSecret.mutateAsync(kind);
        onRemoved();
        showToast({ kind: 'ok', title: t('chaves.removida', { nome: words.name }) });
      }}
    >
      {provider && settings.data ? <Successor view={settings.data} provider={provider} /> : null}
    </ConfirmDialog>
  );
}

/** Remover a chave do provedor em uso avisa quem assume as conversas novas (ADR-0070). */
function Successor({ view, provider }: { view: AiSettingsView; provider: AiProvider }) {
  const { t } = useTranslation('configuracoes');
  const next = successorAfterRemoval(view, provider);
  if (next === undefined) return null;
  return (
    <Alert kind="ai" role="none">
      {next ? (
        <Trans
          t={t}
          i18nKey="chaves.removerAssume"
          values={{ provedor: providerName(next) }}
          components={{ b: <strong /> }}
        />
      ) : (
        t('chaves.removerNinguem')
      )}
    </Alert>
  );
}

/** "Esta versão não sabe testar" ou "não deu para testar agora" (sem mudar a situação). */
function TestNote({ kind, test }: { kind: SecretKind; test: KeyTest | undefined }) {
  const { t } = useTranslation('configuracoes');
  const provider = providerOf(kind);
  if (test?.status === 'notTestable') {
    return (
      <p className="t-sm t-2" role="status">
        {t('chaves.naoTestavel')}
      </p>
    );
  }
  if (test?.status === 'failed') {
    const message = provider ? aiErrorMessage(test.error, provider) : appErrorMessage(test.error);
    return (
      <Alert kind="warn" compact role="status">
        {t('chaves.naoTestou', { erro: message })}
      </Alert>
    );
  }
  return null;
}

/** O campo de senha com **Salvar** (e **Cancelar**, ao substituir). */
function KeyEditor({
  kind,
  replacing,
  onCancel,
  onSaved,
}: {
  kind: SecretKind;
  replacing: boolean;
  onCancel: () => void;
  onSaved: () => void;
}) {
  const { t } = useTranslation('configuracoes');
  const id = useId();
  const words = useKeyWords(kind);
  const vault = useVaultName();
  const [value, setValue] = useState('');
  const setSecret = useSetSecret();
  const label = replacing
    ? words.token
      ? t('chaves.rotuloCampoNovoToken', { de: words.de })
      : t('chaves.rotuloCampoNovoChave', { de: words.de })
    : words.token
      ? t('chaves.rotuloCampoToken', { de: words.de })
      : t('chaves.rotuloCampoChave', { de: words.de });

  const save = async () => {
    if (value.trim() === '' || setSecret.isPending) return;
    try {
      // `mutateAsync`, não `mutate` com `onSuccess`: o campo some assim que a chave aparece no
      // cofre (a visão das chaves é relida antes de a gravação terminar), e o `onSuccess` do
      // `mutate` de um componente que saiu da tela nunca roda; o teste da chave não rodaria.
      await setSecret.mutateAsync({ kind, value });
    } catch {
      // O erro fica em `setSecret.error`, mostrado abaixo do campo.
      return;
    }
    setValue('');
    showToast({ kind: 'ok', title: t('chaves.salva', { nome: words.name, cofre: vault }) });
    onSaved();
  };

  return (
    <div className="keydet__box stack-2">
      <label className="field__label" htmlFor={id}>
        {label}
      </label>
      <div className="field__row">
        <input
          id={id}
          className="input"
          type="password"
          autoComplete="off"
          spellCheck={false}
          // Ao abrir pelo "Substituir", o campo já recebe o foco.
          // eslint-disable-next-line jsx-a11y/no-autofocus
          autoFocus={replacing}
          placeholder={words.token ? t('chaves.campoToken') : t('chaves.campo')}
          value={value}
          aria-describedby={`${id}-dica`}
          onChange={(event) => {
            setValue(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              void save();
            }
          }}
        />
        <Button
          variant="primary"
          {...(replacing ? {} : { icon: Plus })}
          disabled={value.trim() === ''}
          loading={setSecret.isPending}
          onClick={() => {
            void save();
          }}
        >
          {setSecret.isPending ? t('chaves.salvando') : t('chaves.salvar')}
        </Button>
        {replacing ? (
          <Button
            variant="ghost"
            onClick={() => {
              setSecret.reset();
              onCancel();
            }}
          >
            {t('chaves.cancelar')}
          </Button>
        ) : null}
      </div>
      <p className="field__hint" id={`${id}-dica`}>
        {replacing
          ? `${words.token ? t('chaves.atualContinuaToken') : t('chaves.atualContinua')} `
          : ''}
        {t('chaves.vaiParaCofre', { cofre: vault })}
      </p>
      {setSecret.isError ? <ErrorPanel compact error={setSecret.error} /> : null}
    </div>
  );
}
