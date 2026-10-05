/**
 * Primeira execução (T01): aviso legal, nome do jogador, pasta dos packs e chaves opcionais.
 * Sem menu: só o indicador de etapas. Reabre por Configurações → "Rever boas-vindas".
 *
 * Nada é gravado até **Concluir** (ou **Pular, configuro depois**), exceto a pasta, que o
 * diálogo nativo grava ao ser escolhida (ARCHITECTURE §4.1). Ao concluir, na ordem: o modo
 * das chaves (se o `.env` foi escolhido), as chaves digitadas e, por último, o nome do
 * jogador, que grava o `settings.json` e encerra a primeira execução (CA-T01-01). Uma falha no
 * meio deixa o assistente aberto, com o erro, para tentar de novo.
 */
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Check } from 'lucide-react';
import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { Trans, useTranslation } from 'react-i18next';

import { AppPage } from '../../../app/layout/AppPage';
import { focusPageTitle, PAGE_TITLE_ATTRIBUTE } from '../../../app/layout/focus';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import {
  commands,
  type BackendKind,
  type SecretKind,
  type SecretsStatus,
} from '../../../lib/ipc/bindings';
import { unwrap } from '../../../lib/ipc/result';
import {
  isValidPlayerName,
  SECRET_KINDS,
  settingsKeys,
  useSecretsStatus,
  useSettings,
  useSettingsStatus,
} from '../../settings/api';
import { Choice, TextField } from '../../settings/components/fields';
import { EnvConfirmDialog } from '../../settings/components/KeysSection';
import { PacksDirField } from '../../settings/components/PacksDirField';
import { PlayerNameField } from '../../settings/components/PlayerNameField';
import '../../settings/settings.css';

/** Nome do grupo de rádio do modo das chaves. */
const BACKEND_GROUP = 'onboarding-backend';

const STEP_KEYS = ['aviso', 'jogador', 'pasta', 'chaves'] as const;
const LAST_STEP = STEP_KEYS.length - 1;

type Keys = Record<SecretKind, string>;
const EMPTY_KEYS: Keys = { curseforge: '', gemini: '', github: '' };

export function OnboardingWizard() {
  const { t } = useTranslation('boasVindas');
  // O aviso legal tem uma redação só, a de "Sobre o Warden" (ADR-0010).
  const legal = useTranslation('sobre').t('avisoLegal');
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const settings = useSettings();
  const secrets = useSecretsStatus();
  const [step, setStep] = useState(0);
  const [name, setName] = useState<string | null>(null);
  const [nameTouched, setNameTouched] = useState(false);
  const [keys, setKeys] = useState<Keys>(EMPTY_KEYS);
  const [backend, setBackend] = useState<BackendKind | null>(null);
  const [confirmEnv, setConfirmEnv] = useState(false);
  const [finishing, setFinishing] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const firstRender = useRef(true);

  const playerName = name ?? settings.data?.playerName ?? 'Jogador';
  const savedBackend = secrets.data?.backend ?? 'keyring';
  const chosenBackend = backend ?? savedBackend;
  const nameValid = isValidPlayerName(playerName);

  // A cada etapa, o foco vai para o título novo (o leitor de tela anuncia onde está).
  useEffect(() => {
    if (firstRender.current) {
      firstRender.current = false;
      return;
    }
    focusPageTitle();
  }, [step]);

  const finish = async (withKeys: boolean) => {
    if (finishing) {
      return;
    }
    setFinishing(true);
    setError(null);
    try {
      if (withKeys) {
        let status: SecretsStatus | null = null;
        if (chosenBackend !== savedBackend) {
          status = unwrap(await commands.secretsBackendSet(chosenBackend));
        }
        for (const kind of SECRET_KINDS) {
          const value = keys[kind];
          if (value.trim() !== '') {
            status = unwrap(await commands.secretsSet(kind, value));
          }
        }
        if (status) {
          queryClient.setQueryData(settingsKeys.secrets, status);
        }
      }
      const saved = unwrap(await commands.settingsUpdate({ playerName }));
      queryClient.setQueryData(settingsKeys.settings, saved);
      await queryClient.invalidateQueries({ queryKey: settingsKeys.status });
      await navigate({ to: '/' });
    } catch (cause) {
      setError(cause);
      // Parte pode ter sido gravada antes da falha: relê o estado do disco.
      await queryClient.invalidateQueries({ queryKey: settingsKeys.secrets });
      setFinishing(false);
    }
  };

  const next = () => {
    if (step === 1 && !nameValid) {
      setNameTouched(true);
      return;
    }
    setStep((current) => Math.min(current + 1, LAST_STEP));
  };
  const back = () => {
    setError(null);
    setStep((current) => Math.max(current - 1, 0));
  };

  let body;
  if (step === 0) {
    body = (
      <>
        <Heading className="t-display-2xl">
          <Trans t={t} i18nKey="aviso.titulo" components={{ hl: <span className="t-hl" /> }} />
        </Heading>
        <p className="t-2 measure mt-3">{t('aviso.texto')}</p>
        <p className="legal mt-5" data-testid="aviso-legal">
          {legal}
        </p>
        <p className="t-sm t-2 mt-3">{t('aviso.possuir')}</p>
      </>
    );
  } else if (step === 1) {
    body = (
      <>
        <Heading className="pagehead__title">{t('jogador.titulo')}</Heading>
        <p className="pagehead__sub">{t('jogador.texto')}</p>
        <div className="wizard__field mt-5">
          <PlayerNameField
            value={playerName}
            onChange={(value) => {
              setName(value);
              setNameTouched(true);
            }}
            showError={nameTouched}
            onEnter={next}
          />
        </div>
      </>
    );
  } else if (step === 2) {
    body = (
      <>
        <Heading className="pagehead__title">{t('pasta.titulo')}</Heading>
        <p className="pagehead__sub">{t('pasta.texto')}</p>
        <div className="mt-5">
          <PacksDirField hint={t('pasta.dica')} />
        </div>
      </>
    );
  } else {
    body = (
      <>
        <Heading className="pagehead__title">{t('chaves.titulo')}</Heading>
        <p className="pagehead__sub">{t('chaves.texto')}</p>
        <div className="stack mt-5">
          {SECRET_KINDS.map((kind) => (
            <KeyInput
              key={kind}
              kind={kind}
              value={keys[kind]}
              configured={secrets.data?.[kind] ?? false}
              onChange={(value) => {
                setKeys((current) => ({ ...current, [kind]: value }));
              }}
            />
          ))}
          <BackendChoice
            value={chosenBackend}
            onVault={() => {
              setBackend('keyring');
            }}
            onEnv={() => {
              setConfirmEnv(true);
            }}
          />
        </div>
        <EnvConfirmDialog
          open={confirmEnv}
          onOpenChange={setConfirmEnv}
          onConfirm={async () => {
            setBackend('envfile');
            return Promise.resolve();
          }}
        />
      </>
    );
  }

  let foot;
  if (step === 0) {
    foot = (
      <>
        <span />
        <Button variant="primary" onClick={next}>
          {t('aviso.entendi')}
        </Button>
      </>
    );
  } else if (step < LAST_STEP) {
    foot = (
      <>
        <Button variant="ghost" onClick={back}>
          {t('voltar')}
        </Button>
        <Button variant="primary" disabled={step === 1 && !nameValid} onClick={next}>
          {t('proximo')}
        </Button>
      </>
    );
  } else {
    foot = (
      <>
        <Button variant="ghost" disabled={finishing} onClick={back}>
          {t('voltar')}
        </Button>
        <div className="row">
          <Button
            variant="ghost"
            disabled={finishing}
            onClick={() => {
              void finish(false);
            }}
          >
            {t('pular')}
          </Button>
          <Button
            variant="primary"
            loading={finishing}
            onClick={() => {
              void finish(true);
            }}
          >
            {finishing ? t('concluindo') : t('concluir')}
          </Button>
        </div>
      </>
    );
  }

  return (
    <AppPage where={t('onde')} hideLinks>
      <div className="welcome">
        <div className="wizard">
          <Steps current={step} />
          {body}
          {error ? <ErrorPanel className="mt-5" title={t('naoConcluiu')} error={error} /> : null}
          <div className="wizard__foot">{foot}</div>
        </div>
        <Aside />
      </div>
    </AppPage>
  );
}

/** O `<h1>` de cada etapa, focável por script. */
function Heading({ className, children }: { className: string; children: ReactNode }) {
  return (
    <h1 className={className} tabIndex={-1} {...{ [PAGE_TITLE_ATTRIBUTE]: '' }}>
      {children}
    </h1>
  );
}

function Steps({ current }: { current: number }) {
  const { t } = useTranslation('boasVindas');
  return (
    <ol className="steps" aria-label={t('etapas.rotulo')}>
      {STEP_KEYS.map((key, index) => {
        const done = index < current;
        const now = index === current;
        return (
          <li
            key={key}
            className={done ? 'steps__item steps__item--done' : 'steps__item'}
            aria-current={now ? 'step' : undefined}
          >
            <span className="steps__mark" aria-hidden="true">
              {done ? <Icon icon={Check} size="sm" /> : index + 1}
            </span>
            <span className="steps__label">
              {t(`etapas.${key}`)}
              <span className="sr-only">
                {done ? t('etapas.concluida') : now ? t('etapas.agora') : null}
              </span>
            </span>
          </li>
        );
      })}
    </ol>
  );
}

function KeyInput({
  kind,
  value,
  configured,
  onChange,
}: {
  kind: SecretKind;
  value: string;
  configured: boolean;
  onChange: (value: string) => void;
}) {
  const { t } = useTranslation('configuracoes');
  const { t: tw } = useTranslation('boasVindas');
  return (
    <TextField
      type="password"
      label={t(`chaves.rotulos.${kind}`)}
      placeholder={kind === 'github' ? t('chaves.campoToken') : t('chaves.campo')}
      hint={configured ? tw('chaves.jaConfigurada') : t(`chaves.usos.${kind}`)}
      value={value}
      onChange={(event) => {
        onChange(event.target.value);
      }}
    />
  );
}

function BackendChoice({
  value,
  onVault,
  onEnv,
}: {
  value: BackendKind;
  onVault: () => void;
  onEnv: () => void;
}) {
  const { t } = useTranslation('configuracoes');
  const { t: tw } = useTranslation('boasVindas');
  const legendId = useId();
  return (
    <fieldset className="m-0 border-0 p-0" aria-labelledby={legendId}>
      <legend className="field__label mb-2" id={legendId}>
        {t('guardar.legenda')}
      </legend>
      <div className="choice-list choice-list--2">
        <Choice
          name={BACKEND_GROUP}
          title={t('guardar.cofre')}
          badge={<span className="tag tag--ok ml-2">{t('guardar.recomendado')}</span>}
          desc={tw('chaves.cofreDesc')}
          checked={value === 'keyring'}
          onSelect={onVault}
        />
        <Choice
          name={BACKEND_GROUP}
          title={t('guardar.env')}
          desc={tw('chaves.envDesc')}
          checked={value === 'envfile'}
          onSelect={onEnv}
        />
      </div>
    </fieldset>
  );
}

function Aside() {
  const { t } = useTranslation('boasVindas');
  const status = useSettingsStatus();
  const titleId = useId();
  return (
    <aside className="welcome__aside" aria-labelledby={titleId}>
      <h2 className="panel__title" id={titleId}>
        {t('lateral.titulo')}
      </h2>
      <dl className="kv mt-3">
        <dt>{t('lateral.packs')}</dt>
        <dd className="path">{status.data?.packsDir}</dd>
        <dt>{t('lateral.dados')}</dt>
        <dd className="path">{t('lateral.dadosValor')}</dd>
        <dt>{t('lateral.chaves')}</dt>
        <dd>{t('lateral.chavesValor')}</dd>
      </dl>
      <p className="t-xs t-3 mt-3">{t('lateral.rodape')}</p>
    </aside>
  );
}
