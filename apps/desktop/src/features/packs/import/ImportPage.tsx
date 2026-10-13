/**
 * Importar modpack (SPEC T24; protótipo `importar`): página do nível do app, com "← Meus packs".
 *
 * Ao abrir, lê o modpack escolhido (`import_analyze`, com progresso e Cancelar). Nada é gravado
 * até "Importar como pack novo": a revisão mostra o que foi reconhecido, a licença do modpack,
 * nome e pasta do pack novo (as mesmas regras do T03), o que acontece com cada arquivo e o que
 * não entra. Sair da página (Cancelar, "← Meus packs") descarta o plano guardado no Rust.
 * Depois de importar, abre o pack em Mods.
 */
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { FileArchive, FolderOpen, Import, KeyRound } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { AppPage } from '../../../app/layout/AppPage';
import { PageHead } from '../../../app/layout/PageHead';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { ProgressBar } from '../../../components/common/ProgressBar';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { ImportAnalysis, ImportSummary } from '../../../lib/ipc/bindings';
import { commandError } from '../../../lib/ipc/query';
import { createCheckQuery, useChooseFolder } from '../api';
import { checkProblem } from '../components/create/draft';
import { loaderName } from '../lib/pack-list';
import {
  cancelOperation,
  discardPlan,
  useImportAnalysis,
  useImportApply,
  useImportFile,
  useRunningAnalysis,
} from './api';
import { ExcludedPanel, SummaryPanel } from './ImportSummaryPanels';
import '../packs.css';
import './import.css';

export interface ImportPageProps {
  /** O arquivo ou a pasta escolhida. */
  origin: string;
}

/** Nome do arquivo ou da pasta, sem o caminho (a tela não repete a pasta do usuário). */
export function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter((part) => part !== '');
  return parts.at(-1) ?? path;
}

export function ImportPage({ origin }: ImportPageProps) {
  const { t } = useTranslation('importar');
  const navigate = useNavigate();
  const analysis = useImportAnalysis(origin);
  const running = useRunningAnalysis();
  const token = analysis.data?.token ?? null;

  // Sair da página (ou ler de novo) esquece o plano guardado no Rust.
  useEffect(() => {
    if (token === null) return undefined;
    return () => {
      discardPlan(token);
    };
  }, [token]);

  const leave = () => {
    if (running !== null) cancelOperation(running);
    void navigate({ to: '/packs' });
  };

  let sub: string = t('sub');
  if (analysis.data) {
    const summary = analysis.data.summary;
    sub = t('subPronto', {
      arquivo: summary.fileName,
      minecraft: summary.minecraft,
      loader: summary.loader
        ? `${loaderName(summary.loader)} ${summary.loaderVersion ?? ''}`.trim()
        : t('vanilla'),
    });
  }

  return (
    <AppPage back={{ to: '/packs', label: t('voltar') }} where={t('onde')} width="read">
      <PageHead title={t('titulo')} sub={sub} />
      {analysis.isPending ? (
        <div className="stack">
          <ProgressBar
            value={
              running?.progress?.total
                ? (100 * running.progress.current) / running.progress.total
                : null
            }
            label={t('lendo', { arquivo: baseName(origin) })}
            meta={t('lendoMeta')}
          />
          <div className="btn-row btn-row--end">
            <Button variant="ghost" onClick={leave}>
              {t('cancelar')}
            </Button>
          </div>
        </div>
      ) : analysis.isError ? (
        <AnalysisError error={analysis.error} />
      ) : (
        <Review
          analysis={analysis.data}
          onReread={() => {
            void analysis.refetch();
          }}
        />
      )}
    </AppPage>
  );
}

/** Recusa da leitura: sem chave, loader não suportado ou outro motivo, sempre sem gravar. */
function AnalysisError({ error }: { error: unknown }) {
  const { t } = useTranslation('importar');
  const navigate = useNavigate();
  const importFile = useImportFile();
  const other = (
    <Button
      size="sm"
      variant="ghost"
      icon={FileArchive}
      loading={importFile.pending}
      onClick={importFile.start}
    >
      {t('escolherOutro')}
    </Button>
  );
  const appError = commandError(error);
  if (appError?.code.domain === 'import' && appError.code.code === 'CURSEFORGE_KEY_MISSING') {
    return (
      <Alert
        kind="warn"
        title={t('semChave.titulo')}
        actions={
          <>
            <Button
              size="sm"
              icon={KeyRound}
              onClick={() => {
                void navigate({ to: '/configuracoes' });
              }}
            >
              {t('abrirConfiguracoes')}
            </Button>
            {other}
          </>
        }
      >
        <div className="alert__text">{t('semChave.texto')}</div>
      </Alert>
    );
  }
  if (appError?.code.domain === 'import' && appError.code.code === 'UNSUPPORTED_LOADER') {
    const loader = appError.params.loader;
    return (
      <Alert
        kind="danger"
        title={
          loader ? t('loader.naoSuportado', { loader: loaderName(loader) }) : t('loader.varios')
        }
        actions={other}
      >
        <div className="alert__text">{t('loader.texto')}</div>
      </Alert>
    );
  }
  return <ErrorPanel error={error} title={t('erro')} actions={other} />;
}

/** A revisão: o que foi reconhecido, nome e pasta, resumo, lixo e o botão de importar. */
function Review({ analysis, onReread }: { analysis: ImportAnalysis; onReread: () => void }) {
  const { t } = useTranslation('importar');
  const navigate = useNavigate();
  const summary = analysis.summary;
  const apply = useImportApply();
  const [name, setName] = useState(summary.name);
  const [destination, setDestination] = useState<string | null>(null);
  const [include, setInclude] = useState<ReadonlySet<string>>(new Set());
  const [preferences, setPreferences] = useState(false);
  const check = useQuery({
    ...createCheckQuery(name, destination),
    enabled: name.trim() !== '',
    placeholderData: keepPreviousData,
  });
  const nameProblem =
    checkProblem(commandError(apply.error), name) ?? checkProblem(commandError(check.error), name);
  const canImport = name.trim() !== '' && check.isSuccess && !check.isPlaceholderData;

  const run = () => {
    apply.mutate(
      {
        token: analysis.token,
        request: {
          name: name.trim(),
          destination,
          include: [...include],
          takePreferences: preferences,
        },
      },
      {
        onSuccess: (result) => {
          showToast({
            kind: 'ok',
            title: t('feito', { name: name.trim() }),
            ...(result.review > 0 ? { text: t('feitoRevisar', { count: result.review }) } : {}),
          });
          if (summary.profile !== null && !result.profileCreated) {
            showToast({ kind: 'warn', title: t('semPerfil') });
          }
          void navigate({ to: '/packs/$packId/mods', params: { packId: result.packId } });
        },
      },
    );
  };

  return (
    <div className="stack import">
      <Recognized summary={summary} />
      {summary.license !== null ? (
        <Alert
          kind={summary.license.free ? 'info' : 'warn'}
          title={t('licenca.titulo', { licenca: summary.license.name })}
        >
          <div className="alert__text">
            {summary.license.free ? t('licenca.livre') : t('licenca.reservada')}
          </div>
        </Alert>
      ) : null}
      {summary.unavailableSources.length > 0 ? (
        <Alert
          kind="warn"
          title={t('lojas.titulo', { lojas: summary.unavailableSources.join(' e ') })}
          actions={
            <Button size="sm" onClick={onReread}>
              {t('lojas.reler')}
            </Button>
          }
        >
          <div className="alert__text">{t('lojas.texto')}</div>
        </Alert>
      ) : null}
      <Destination
        name={name}
        onNameChange={setName}
        destination={destination}
        onDestinationChange={setDestination}
        shownPath={
          destination ??
          (check.isSuccess ? check.data : (commandError(check.error)?.params.path ?? ''))
        }
        problem={nameProblem}
      />
      <SummaryPanel summary={summary} />
      {summary.profile !== null ? (
        <section className="panel" aria-labelledby="import-perfil">
          <h2 className="panel__title panel__title--sans" id="import-perfil">
            {t('perfil.titulo')}
          </h2>
          <p className="t-sm t-2 mt-1">{t('perfil.texto')}</p>
          <ul className="import__facts t-sm mt-2">
            <li>
              {summary.profile.memoryMb !== null
                ? t('perfil.memoria', { mb: summary.profile.memoryMb })
                : t('perfil.memoriaAuto')}
            </li>
            {summary.profile.jvmArgs !== '' ? (
              <li className="t-mono">
                {t('perfil.argumentos', { args: summary.profile.jvmArgs })}
              </li>
            ) : null}
          </ul>
        </section>
      ) : null}
      {summary.preferences.length > 0 ? (
        <section className="panel" aria-labelledby="import-preferencias">
          <h2 className="panel__title panel__title--sans" id="import-preferencias">
            {t('preferencias.titulo')}
          </h2>
          <p className="t-sm t-2 mt-1">
            {t('preferencias.texto', { arquivos: summary.preferences.join(', ') })}
          </p>
          <label className="check mt-3">
            <input
              type="checkbox"
              checked={preferences}
              onChange={(event) => {
                setPreferences(event.target.checked);
              }}
            />
            <span className="check__text">
              {t('preferencias.levar', { arquivos: summary.preferences.join(', ') })}
            </span>
          </label>
        </section>
      ) : null}
      <ExcludedPanel items={summary.excluded} include={include} onIncludeChange={setInclude} />
      {apply.isError && nameProblem === null ? <ErrorPanel error={apply.error} /> : null}
      <div className="btn-row btn-row--end">
        <Button
          variant="ghost"
          disabled={apply.isPending}
          onClick={() => {
            void navigate({ to: '/packs' });
          }}
        >
          {t('cancelar')}
        </Button>
        <Button
          variant="primary"
          icon={Import}
          loading={apply.isPending}
          disabled={!canImport || apply.isSuccess}
          onClick={run}
        >
          {apply.isPending ? t('importando') : t('importar')}
        </Button>
      </div>
    </div>
  );
}

function Recognized({ summary }: { summary: ImportSummary }) {
  const { t } = useTranslation('importar');
  return (
    <section className="panel" aria-labelledby="import-reconhecido">
      <h2 className="panel__title panel__title--sans" id="import-reconhecido">
        {t('reconhecido.titulo')}
      </h2>
      <dl className="packcard__facts import__recognized">
        <div>
          <dt>{t('reconhecido.formato')}</dt>
          <dd>{t(`formato.${summary.kind}`)}</dd>
        </div>
        <div>
          <dt>{t('reconhecido.nome')}</dt>
          <dd>{summary.name}</dd>
        </div>
        <div>
          <dt>{t('reconhecido.versao')}</dt>
          <dd>{summary.version === '' ? t('reconhecido.semVersao') : summary.version}</dd>
        </div>
      </dl>
    </section>
  );
}

/** Nome e pasta do pack novo, conferidos pelo Rust sem escrever nada (regras do T03). */
function Destination({
  name,
  onNameChange,
  destination,
  onDestinationChange,
  shownPath,
  problem,
}: {
  name: string;
  onNameChange: (name: string) => void;
  destination: string | null;
  onDestinationChange: (destination: string | null) => void;
  shownPath: string;
  problem: ReturnType<typeof checkProblem>;
}) {
  const { t } = useTranslation('importar');
  const { t: tp } = useTranslation('packs');
  const ids = { name: useId(), path: useId() };
  const choose = useChooseFolder();
  const nameError = problem?.field === 'name' ? tp(`criar.${problem.key}`) : null;
  const pathError = problem?.field === 'path' ? tp(`criar.${problem.key}`) : null;
  return (
    <section className="panel stack-3" aria-labelledby="import-destino">
      <h2 className="panel__title panel__title--sans" id="import-destino">
        {t('destino')}
      </h2>
      <div className="field">
        <label className="field__label" htmlFor={ids.name}>
          {tp('criar.nome.rotulo')}
        </label>
        <input
          id={ids.name}
          className="input"
          type="text"
          autoComplete="off"
          required
          maxLength={128}
          value={name}
          aria-invalid={nameError !== null || undefined}
          aria-describedby={`${ids.name}-dica${nameError !== null ? ` ${ids.name}-erro` : ''}`}
          onChange={(event) => {
            onNameChange(event.target.value);
          }}
        />
        {nameError !== null ? (
          <div className="field__error" id={`${ids.name}-erro`}>
            {nameError}
          </div>
        ) : null}
        <p className="field__hint" id={`${ids.name}-dica`}>
          {tp('criar.nome.dica')}
        </p>
      </div>
      <div className="field">
        <label className="field__label" htmlFor={ids.path}>
          {tp('criar.pasta.rotulo')}
        </label>
        <div className="field__row">
          <input
            id={ids.path}
            className="input input--mono"
            type="text"
            autoComplete="off"
            spellCheck={false}
            value={shownPath}
            aria-invalid={pathError !== null || undefined}
            aria-describedby={pathError !== null ? `${ids.path}-erro` : undefined}
            onChange={(event) => {
              onDestinationChange(event.target.value === '' ? null : event.target.value);
            }}
          />
          <Button
            icon={FolderOpen}
            loading={choose.isPending}
            onClick={() => {
              choose.mutate('createDestination', {
                onSuccess: (path) => {
                  if (path !== null) onDestinationChange(path);
                },
              });
            }}
          >
            {tp('criar.pasta.escolher')}
          </Button>
        </div>
        {pathError !== null ? (
          <div className="field__error" id={`${ids.path}-erro`}>
            {pathError}
          </div>
        ) : null}
        {destination !== null ? (
          <div>
            <Button
              variant="link"
              size="sm"
              onClick={() => {
                onDestinationChange(null);
              }}
            >
              {tp('criar.pasta.padrao')}
            </Button>
          </div>
        ) : null}
      </div>
    </section>
  );
}
