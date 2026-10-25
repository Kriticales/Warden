/**
 * Configurações → Chaves e contas (T21; esboço `design/bug-01/configuracoes.html`; ADR-0069):
 * as 5 chaves na mesma linha (serviço, para que serve, situação, final `••••xxxx`) à esquerda,
 * agrupadas em "Mods e publicação" e "Sensor Sculk, na ordem de preferência", e o detalhe da
 * escolhida à direita. As chaves ficam sempre no cofre do sistema; a escolhida vai no endereço
 * (`?chave=`), para o "Substituir a chave" do Sensor Sculk abrir direto nela.
 */
import { useNavigate } from '@tanstack/react-router';
import { CircleX, RefreshCw, ShieldCheck } from 'lucide-react';
import { Trans, useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import type { KeysOverview, SecretKind } from '../../../lib/ipc/bindings';
import { appErrorMessage } from '../../../lib/ipc/errors';
import { useAiSettings } from '../../diagnostics/ai/api';
import { AI_PROVIDERS } from '../ai/providers';
import { useKeysOverview, useRetryKeysMigration } from '../api';
import { KeyDetail } from '../keys/KeyDetail';
import { ACCOUNT_KINDS, isToken, keyOf, keyState, refusedCount } from '../keys/model';
import { KeyMask, KeyStateTag } from '../keys/parts';
import { useKeyTests, type KeyTest } from '../keys/useKeyTests';
import { SettingsFrame } from '../layout/SettingsFrame';
import { KEYS_PAGE } from '../pages';
import { useIsWindows, useVaultName } from '../layout/vault';

export function KeysPage({
  selected,
  replace,
}: {
  selected?: SecretKind | undefined;
  replace?: boolean | undefined;
}) {
  const { t } = useTranslation('configuracoes');
  const overview = useKeysOverview();
  const { tests, run, forget } = useKeyTests();

  const testAll = () => {
    for (const key of overview.data?.keys ?? []) {
      if (key.configured && tests[key.kind]?.status !== 'testing') {
        void run(key.kind);
      }
    }
  };
  const anyKey = overview.data?.keys.some((key) => key.configured) ?? false;

  let body;
  if (overview.isPending) {
    body = <LoadingState label={t('chaves.carregando')} />;
  } else if (overview.isError) {
    body = (
      <div className="spage__pad">
        <ErrorPanel
          error={overview.error}
          onRetry={() => {
            void overview.refetch();
          }}
        />
      </div>
    );
  } else {
    const current = selected ?? firstToLookAt(overview.data);
    body = (
      <>
        <MigrationNotice overview={overview.data} />
        <VaultStrip refused={refusedCount(overview.data)} />
        <div className="keys">
          <KeyList overview={overview.data} selected={current} tests={tests} />
          <KeyDetail
            // Trocar de chave começa do zero (campo vazio, sem "Substituir" aberto).
            key={`${current}-${replace ? 'substituir' : ''}`}
            kind={current}
            keyInfo={keyOf(overview.data, current)}
            test={tests[current]}
            startEditing={replace === true}
            onTest={(kind) => {
              void run(kind);
            }}
            onChanged={forget}
          />
        </div>
      </>
    );
  }

  return (
    <SettingsFrame
      page="chaves"
      tools={
        <Button size="sm" icon={RefreshCw} disabled={!anyKey} onClick={testAll}>
          {t('chaves.testarTodas')}
        </Button>
      }
    >
      {body}
    </SettingsFrame>
  );
}

/** Sem chave escolhida: a primeira recusada, senão a primeira da tela. */
function firstToLookAt(overview: KeysOverview): SecretKind {
  return overview.keys.find((key) => keyState(key, false) === 'refused')?.kind ?? ACCOUNT_KINDS[0];
}

function VaultStrip({ refused }: { refused: number }) {
  const { t } = useTranslation('configuracoes');
  const windows = useIsWindows();
  return (
    <div className="vault">
      <Icon icon={ShieldCheck} />
      <span className="vault__text">
        <Trans
          t={t}
          i18nKey={windows ? 'cofre.textoWindows' : 'cofre.textoSistema'}
          components={{ b: <b /> }}
        />
      </span>
      {refused > 0 ? (
        <span className="vault__warn">
          <Icon icon={CircleX} />
          {t('chaves.recusadasAviso', { count: refused })}
        </span>
      ) : null}
    </div>
  );
}

function KeyList({
  overview,
  selected,
  tests,
}: {
  overview: KeysOverview;
  selected: SecretKind;
  tests: Partial<Record<SecretKind, KeyTest>>;
}) {
  const { t } = useTranslation('configuracoes');
  const settings = useAiSettings();
  const order = settings.data?.order ?? AI_PROVIDERS;
  const item = (kind: SecretKind) => (
    <KeyItem
      key={kind}
      kind={kind}
      overview={overview}
      selected={selected === kind}
      testing={tests[kind]?.status === 'testing'}
    />
  );
  return (
    <nav className="keys__list" aria-label={t('chaves.listaRotulo')}>
      <h2 className="keys__group" id="grupo-contas">
        {t('chaves.grupoContas')}
      </h2>
      <ul aria-labelledby="grupo-contas">{ACCOUNT_KINDS.map(item)}</ul>
      <h2 className="keys__group" id="grupo-sculk">
        {t('chaves.grupoSculk')}
      </h2>
      <ul aria-labelledby="grupo-sculk">{order.map(item)}</ul>
    </nav>
  );
}

function KeyItem({
  kind,
  overview,
  selected,
  testing,
}: {
  kind: SecretKind;
  overview: KeysOverview;
  selected: boolean;
  testing: boolean;
}) {
  const { t } = useTranslation('configuracoes');
  const navigate = useNavigate();
  const keyInfo = keyOf(overview, kind);
  return (
    <li>
      <button
        type="button"
        className="keyitem"
        aria-current={selected}
        data-testid={`chave-${kind}`}
        onClick={() => {
          void navigate({
            to: '/configuracoes/$pagina',
            params: { pagina: KEYS_PAGE },
            search: { chave: kind },
            replace: true,
          });
        }}
      >
        <span className="keyname">
          <b>{t(`chaves.nomes.${kind}`)}</b>
          <small>{t(`chaves.tipos.${kind}`)}</small>
        </span>
        <KeyStateTag kind={kind} state={keyState(keyInfo, testing)} />
        <KeyMask kindToken={isToken(kind)} keyInfo={keyInfo} />
      </button>
    </li>
  );
}

/** O aviso da migração do `.env` antigo para o cofre (ADR-0069), nesta abertura do app. */
function MigrationNotice({ overview }: { overview: KeysOverview }) {
  const { t } = useTranslation('configuracoes');
  const vault = useVaultName();
  const retry = useRetryKeysMigration();
  const migration = overview.migration;
  if (!migration) return null;
  if (migration.kind === 'moved') {
    return (
      <div className="spage__notice">
        <Alert kind="ok" title={t('chaves.migracao.movidasTitulo', { cofre: vault })}>
          {migration.fileRemoved
            ? t('chaves.migracao.movidasTexto', { count: migration.moved.length })
            : t('chaves.migracao.sobrouTexto')}
        </Alert>
      </div>
    );
  }
  return (
    <div className="spage__notice">
      <Alert
        kind="danger"
        title={t('chaves.migracao.falhouTitulo')}
        actions={
          <Button
            size="sm"
            loading={retry.isPending}
            onClick={() => {
              retry.mutate(undefined);
            }}
          >
            {retry.isPending ? t('chaves.migracao.tentando') : t('chaves.migracao.tentarDeNovo')}
          </Button>
        }
      >
        <p>{t('chaves.migracao.falhouTexto')}</p>
        <p className="t-sm t-2 mt-1">{appErrorMessage(migration.error)}</p>
      </Alert>
      {retry.isError ? <ErrorPanel compact error={retry.error} /> : null}
    </div>
  );
}
