/**
 * "Entrar com o GitHub" (Configurações → Chaves e contas; UI-03, ADR-0059), no encaixe das
 * chaves (`keysSectionSlots`).
 *
 * - Sem o identificador do GitHub App: o botão desabilitado, com a explicação.
 * - Desconectado: **Entrar com o GitHub** pede o código ao GitHub (pelo Rust).
 * - Esperando: o código em letra grande, **Copiar código**, **Abrir o GitHub** (só
 *   `github.com/login/device`), "Aguardando você aprovar no GitHub…" e **Cancelar**. O
 *   progresso chega pelo evento `github-login-updated` e é anunciado (`aria-live`).
 * - Conectado: "Conectado como <login>" e **Sair do GitHub**.
 */
import { ExternalLink, LogIn, LogOut } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Loader } from '../../../components/ui/loader';
import { showToast } from '../../../components/ui/toast';
import { events, type AppError, type GithubLoginCode } from '../../../lib/ipc/bindings';
import { useTauriEvent } from '../../../lib/ipc/events';
import { log } from '../../../lib/log';
import { StatusText } from '../components/fields';
import {
  useGithubLogin,
  useGithubLoginCancel,
  useGithubLoginOpenPage,
  useGithubLoginRefresh,
  useGithubLoginStart,
  useGithubLogout,
} from './api';
import './github.css';

/** Ganchos dos testes. */
const TEST_IDS = {
  root: 'github-login',
  code: 'github-login-code',
} as const;

type Phase =
  | { kind: 'idle' }
  | { kind: 'waiting'; code: GithubLoginCode; slow: boolean }
  | { kind: 'failed'; error: AppError }
  | { kind: 'cancelled' };

export function GithubLogin() {
  const { t } = useTranslation('github');
  const status = useGithubLogin();
  useGithubLoginRefresh();

  let body;
  if (status.isPending) {
    body = <LoadingState inline label={t('carregando')} />;
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
  } else if (status.data.login !== null) {
    body = <Connected login={status.data.login} />;
  } else {
    body = <SignIn available={status.data.available} />;
  }

  return (
    <div className="stack-3 github-login" data-testid={TEST_IDS.root}>
      <h3 className="field__label">{t('titulo')}</h3>
      {body}
    </div>
  );
}

function Connected({ login }: { login: string }) {
  const { t } = useTranslation('github');
  const logout = useGithubLogout();
  return (
    <div className="stack-2">
      <div className="row github-login__account">
        <StatusText kind="ok">{t('conectado', { login })}</StatusText>
        <Button
          size="sm"
          variant="danger-ghost"
          icon={LogOut}
          loading={logout.isPending}
          onClick={() => {
            logout.mutate(undefined, {
              onSuccess: () => {
                showToast({ kind: 'ok', title: t('saiu') });
              },
            });
          }}
        >
          {logout.isPending ? t('saindo') : t('sair')}
        </Button>
      </div>
      {logout.isError ? <ErrorPanel compact error={logout.error} /> : null}
    </div>
  );
}

function SignIn({ available }: { available: boolean }) {
  const { t } = useTranslation('github');
  const hintId = useId();
  const [phase, setPhase] = useState<Phase>({ kind: 'idle' });
  const start = useGithubLoginStart();
  const cancel = useGithubLoginCancel();

  useTauriEvent(events.githubLoginUpdated, (update) => {
    // Só a espera que esta tela começou.
    if (phase.kind !== 'waiting') return;
    switch (update.state) {
      case 'waiting':
        break;
      case 'slowDown':
        setPhase({ ...phase, slow: true });
        break;
      case 'connected':
        showToast({ kind: 'ok', title: t('conectadoAviso', { login: update.login }) });
        setPhase({ kind: 'idle' });
        break;
      case 'failed':
        setPhase({ kind: 'failed', error: update.error });
        break;
      case 'cancelled':
        setPhase({ kind: 'cancelled' });
        break;
    }
  });

  const begin = () => {
    start.mutate(undefined, {
      onSuccess: (code) => {
        setPhase({ kind: 'waiting', code, slow: false });
      },
    });
  };

  if (phase.kind === 'waiting') {
    return (
      <Waiting
        code={phase.code}
        slow={phase.slow}
        cancelling={cancel.isPending}
        onCancel={() => {
          cancel.mutate(undefined, {
            onSuccess: () => {
              setPhase({ kind: 'cancelled' });
            },
          });
        }}
      />
    );
  }

  return (
    <div className="stack-2">
      <p className="t-sm t-2" id={hintId}>
        {available ? t('texto') : t('indisponivel')}
      </p>
      <div>
        <Button
          variant="primary"
          icon={LogIn}
          disabled={!available}
          aria-describedby={hintId}
          loading={start.isPending}
          onClick={begin}
        >
          {start.isPending ? t('entrando') : t('entrar')}
        </Button>
      </div>
      <div aria-live="polite" className="stack-2">
        {phase.kind === 'cancelled' ? <p className="t-sm t-2">{t('cancelado')}</p> : null}
        {phase.kind === 'failed' ? (
          <ErrorPanel compact error={phase.error} title={t('falhou')} />
        ) : null}
        {start.isError ? <ErrorPanel compact error={start.error} title={t('falhou')} /> : null}
      </div>
    </div>
  );
}

function Waiting({
  code,
  slow,
  cancelling,
  onCancel,
}: {
  code: GithubLoginCode;
  slow: boolean;
  cancelling: boolean;
  onCancel: () => void;
}) {
  const { t } = useTranslation('github');
  const labelId = useId();
  const [copied, setCopied] = useState<'idle' | 'copied' | 'failed'>('idle');
  const openPage = useGithubLoginOpenPage();

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code.userCode);
      setCopied('copied');
    } catch (cause) {
      log.warn('login do GitHub: código não copiado', cause);
      setCopied('failed');
    }
  };

  return (
    <div className="stack-3">
      <p className="t-sm t-2">
        {t('codigo.instrucao', { minutos: Math.max(1, Math.round(code.expiresInSeconds / 60)) })}
      </p>
      <div className="stack-2">
        <span className="t-xs t-3" id={labelId}>
          {t('codigo.rotulo')}
        </span>
        <output
          className="github-login__code"
          aria-labelledby={labelId}
          data-testid={TEST_IDS.code}
        >
          {code.userCode}
        </output>
      </div>
      <div className="row">
        <Button
          variant="primary"
          iconEnd={ExternalLink}
          loading={openPage.isPending}
          onClick={() => {
            openPage.mutate(undefined);
          }}
        >
          {t('codigo.abrir')}
        </Button>
        <Button
          onClick={() => {
            void copy();
          }}
        >
          {t('codigo.copiar')}
        </Button>
        <Button variant="ghost" loading={cancelling} onClick={onCancel}>
          {t('cancelar')}
        </Button>
      </div>
      <div aria-live="polite" className="stack-2">
        <p className="row t-sm t-2">
          <Loader />
          <span>{t('aguardando')}</span>
        </p>
        {slow ? <p className="t-sm t-3">{t('maisDevagar')}</p> : null}
        {copied === 'copied' ? <p className="t-sm t-2">{t('codigo.copiado')}</p> : null}
        {copied === 'failed' ? (
          <Alert kind="warn" compact role="none">
            {t('codigo.naoCopiou')}
          </Alert>
        ) : null}
      </div>
      {openPage.isError ? (
        <ErrorPanel compact error={openPage.error} title={t('naoAbriu')} />
      ) : null}
    </div>
  );
}
