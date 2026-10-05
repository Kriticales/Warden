/**
 * Chaves e contas (T21; ADR-0025): CurseForge, Gemini e GitHub.
 *
 * - Sem chave: campo de senha, **Salvar** e "Como criar" (passo a passo com o link).
 * - Com chave: só "Configurada", com **Testar**, **Substituir** e **Remover**. O valor nunca
 *   volta para a interface (não existe comando para ler uma chave).
 * - "Onde guardar as chaves": cofre do Windows (padrão) ou arquivo `.env`. Ir para o `.env`
 *   pede confirmação; enquanto ele estiver em uso, o aviso fica fixo com **Voltar para o
 *   cofre**. O backend move as chaves e apaga a cópia anterior.
 */
import { ExternalLink, ShieldCheck } from 'lucide-react';
import { useId, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { BackendKind, SecretKind, SecretTestResult } from '../../../lib/ipc/bindings';
import { openExternal } from '../../../lib/external';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import {
  SECRET_KINDS,
  useRemoveSecret,
  useSecretsStatus,
  useSetSecret,
  useSetSecretsBackend,
  useTestSecret,
} from '../api';
import { keysSectionSlots } from '../slots';
import { Choice, StatusText, TextField } from './fields';
import { SettingsPanel } from './SettingsPanel';

/** Nome do grupo de rádio do modo das chaves. */
const BACKEND_GROUP = 'secrets-backend';

/** Onde criar cada chave. */
export const KEY_PAGES: Record<SecretKind, string> = {
  curseforge: 'https://console.curseforge.com/#/api-keys',
  gemini: 'https://aistudio.google.com/apikey',
  github: 'https://github.com/settings/personal-access-tokens/new',
};

export function KeysSection() {
  const { t } = useTranslation('configuracoes');
  const status = useSecretsStatus();

  let body;
  if (status.isPending) {
    body = <LoadingState inline label={t('chaves.carregando')} />;
  } else if (status.isError) {
    body = (
      <ErrorPanel
        compact
        error={status.error}
        onRetry={() => {
          void status.refetch();
        }}
      />
    );
  } else {
    body = (
      <>
        <div>
          {SECRET_KINDS.map((kind) => (
            <KeyRow key={kind} kind={kind} configured={status.data[kind]} />
          ))}
        </div>
        {keysSectionSlots.map((Slot, index) => (
          // Encaixes fixos, registrados no carregamento do módulo.
          <Slot key={index} />
        ))}
        <SecretsBackendChoice backend={status.data.backend} />
      </>
    );
  }

  return (
    <SettingsPanel title={t('chaves.titulo')}>
      <p className="t-sm t-2">{t('chaves.texto')}</p>
      {body}
    </SettingsPanel>
  );
}

type TestOutcome = { kind: 'result'; result: SecretTestResult } | { kind: 'error'; error: unknown };

function KeyRow({ kind, configured }: { kind: SecretKind; configured: boolean }) {
  const { t } = useTranslation('configuracoes');
  const stepsId = useId();
  const name = t(`chaves.nomes.${kind}`);
  const [replacing, setReplacing] = useState(false);
  const [value, setValue] = useState('');
  const [showSteps, setShowSteps] = useState(false);
  const [outcome, setOutcome] = useState<TestOutcome | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const setSecret = useSetSecret();
  const removeSecret = useRemoveSecret();
  const testSecret = useTestSecret();
  const editing = !configured || replacing;

  const save = () => {
    if (value.trim() === '' || setSecret.isPending) {
      return;
    }
    setSecret.mutate(
      { kind, value },
      {
        onSuccess: () => {
          setValue('');
          setReplacing(false);
          setOutcome(null);
          showToast({ kind: 'ok', title: t('chaves.salva', { nome: name }) });
        },
      },
    );
  };

  const test = () => {
    setOutcome(null);
    testSecret.mutate(kind, {
      onSuccess: (result) => {
        setOutcome({ kind: 'result', result });
      },
      onError: (error) => {
        setOutcome({ kind: 'error', error });
      },
    });
  };

  let state = null;
  if (!editing) {
    state =
      outcome?.kind === 'result' && outcome.result.status === 'valid' ? (
        <StatusText kind="ok">{t('chaves.funcionando')}</StatusText>
      ) : (
        <StatusText kind="ok">{t('chaves.configurada')}</StatusText>
      );
  }

  return (
    <div className="keyrow" data-testid={`chave-${kind}`}>
      <span className="t-strong">{name}</span>
      {editing ? (
        <TextField
          bare
          type="password"
          value={value}
          placeholder={kind === 'github' ? t('chaves.campoToken') : t('chaves.campo')}
          aria-label={t(`chaves.rotulos.${kind}`)}
          onChange={(event) => {
            setValue(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              save();
            }
          }}
        />
      ) : (
        <span className="row">{state}</span>
      )}
      <span className="row">
        {editing ? (
          <>
            <Button
              size="sm"
              variant="primary"
              disabled={value.trim() === ''}
              loading={setSecret.isPending}
              onClick={save}
            >
              {setSecret.isPending ? t('chaves.salvando') : t('chaves.salvar')}
            </Button>
            {replacing ? (
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setReplacing(false);
                  setValue('');
                  setSecret.reset();
                }}
              >
                {t('chaves.cancelar')}
              </Button>
            ) : (
              <Button
                size="sm"
                variant="link"
                aria-expanded={showSteps}
                aria-controls={stepsId}
                onClick={() => {
                  setShowSteps((open) => !open);
                }}
              >
                {t('chaves.comoCriar')}
              </Button>
            )}
          </>
        ) : (
          <>
            <Button size="sm" loading={testSecret.isPending} onClick={test}>
              {testSecret.isPending ? t('chaves.testando') : t('chaves.testar')}
            </Button>
            <Button
              size="sm"
              onClick={() => {
                setReplacing(true);
                setOutcome(null);
              }}
            >
              {t('chaves.substituir')}
            </Button>
            <Button
              size="sm"
              variant="danger-ghost"
              onClick={() => {
                setConfirmRemove(true);
              }}
            >
              {t('chaves.remover')}
            </Button>
          </>
        )}
      </span>

      {editing && setSecret.isError ? (
        <div className="keyrow__feedback">
          <ErrorPanel compact error={setSecret.error} />
        </div>
      ) : null}
      {!editing && outcome ? (
        <div className="keyrow__feedback" role="status">
          {outcome.kind === 'error' ? (
            <Alert kind="danger" compact role="none">
              {appErrorMessage(toAppError(outcome.error))}
            </Alert>
          ) : outcome.result.status === 'notTestable' ? (
            <p className="t-sm t-2">{t('chaves.naoTestavel')}</p>
          ) : null}
        </div>
      ) : null}
      {editing && !replacing && showSteps ? (
        <div className="keyrow__steps stack-2 t-sm t-2" id={stepsId}>
          <ol className="stack-2">
            {t(`chaves.passos.${kind}`, { returnObjects: true }).map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ol>
          <div>
            <Button
              size="sm"
              variant="link"
              iconEnd={ExternalLink}
              onClick={() => {
                void openExternal(KEY_PAGES[kind]);
              }}
            >
              {t('chaves.abrirPagina')}
            </Button>
          </div>
        </div>
      ) : null}

      <ConfirmDialog
        open={confirmRemove}
        onOpenChange={setConfirmRemove}
        title={t('chaves.removerTitulo', { nome: name })}
        description={t('chaves.removerTexto')}
        confirmLabel={t('chaves.removerConfirmar')}
        confirmingLabel={t('chaves.removendo')}
        onConfirm={async () => {
          await removeSecret.mutateAsync(kind);
          setOutcome(null);
          showToast({ kind: 'ok', title: t('chaves.removida', { nome: name }) });
        }}
      />
    </div>
  );
}

/** "Onde guardar as chaves": cofre do Windows ou arquivo `.env` (com confirmação). */
export function SecretsBackendChoice({ backend }: { backend: BackendKind }) {
  const { t } = useTranslation('configuracoes');
  const legendId = useId();
  const [confirmEnv, setConfirmEnv] = useState(false);
  const setBackend = useSetSecretsBackend();

  const backToVault = () => {
    setBackend.mutate('keyring', {
      onSuccess: () => {
        showToast({ kind: 'ok', title: t('guardar.movidasCofre') });
      },
    });
  };

  return (
    <div className="stack-3">
      <fieldset
        className="m-0 border-0 p-0"
        aria-labelledby={legendId}
        aria-busy={setBackend.isPending}
      >
        <legend className="field__label mb-2" id={legendId}>
          {t('guardar.legenda')}
        </legend>
        <div className="choice-list choice-list--2">
          <Choice
            name={BACKEND_GROUP}
            title={t('guardar.cofre')}
            badge={<span className="tag tag--ok ml-2">{t('guardar.recomendado')}</span>}
            desc={t('guardar.cofreDesc')}
            checked={backend === 'keyring'}
            disabled={setBackend.isPending}
            onSelect={backToVault}
          />
          <Choice
            name={BACKEND_GROUP}
            title={t('guardar.env')}
            desc={t('guardar.envDesc')}
            checked={backend === 'envfile'}
            disabled={setBackend.isPending}
            onSelect={() => {
              setConfirmEnv(true);
            }}
          />
        </div>
      </fieldset>
      {setBackend.isPending ? <LoadingState inline label={t('guardar.trocando')} /> : null}
      {setBackend.isError ? <ErrorPanel compact error={setBackend.error} /> : null}
      {backend === 'envfile' ? (
        <Alert
          kind="warn"
          title={t('guardar.fixoTitulo')}
          actions={
            <Button
              size="sm"
              icon={ShieldCheck}
              loading={setBackend.isPending}
              onClick={backToVault}
            >
              {t('guardar.voltarCofre')}
            </Button>
          }
        >
          {t('guardar.fixoTexto')}
        </Alert>
      ) : null}
      <EnvConfirmDialog
        open={confirmEnv}
        onOpenChange={setConfirmEnv}
        onConfirm={async () => {
          await setBackend.mutateAsync('envfile');
          showToast({ kind: 'ok', title: t('guardar.movidasEnv') });
        }}
      />
    </div>
  );
}

/** Confirmação de guardar as chaves no `.env`, com o aviso da SPEC (T21). */
export function EnvConfirmDialog({
  open,
  onOpenChange,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => Promise<void>;
}) {
  const { t } = useTranslation('configuracoes');
  return (
    <ConfirmDialog
      open={open}
      onOpenChange={onOpenChange}
      destructive={false}
      title={t('guardar.confirmarTitulo')}
      description={
        <Trans
          t={t}
          i18nKey="guardar.confirmarTexto"
          components={{ path: <span className="path" /> }}
        />
      }
      cancelLabel={t('guardar.manter')}
      confirmLabel={t('guardar.usarEnv')}
      confirmingLabel={t('guardar.trocando')}
      onConfirm={onConfirm}
    >
      <div className="stack-3">
        <Alert kind="warn" title={t('guardar.confirmarAvisoTitulo')}>
          {t('guardar.confirmarAviso')}
        </Alert>
        <p className="t-sm t-3">{t('guardar.confirmarRodape')}</p>
      </div>
    </ConfirmDialog>
  );
}
