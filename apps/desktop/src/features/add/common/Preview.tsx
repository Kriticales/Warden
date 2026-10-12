/**
 * Pré-visualização de um resultado (SPEC T08; protótipo `preview`): cabeçalho (ícone, nome,
 * autor, fontes, downloads, atualização, licença, lado), seletores **Fonte** (só quando o
 * projeto está nas duas) e **Versão** (a mais nova compatível do canal configurado por padrão),
 * **Adicionar ao pack**, a descrição higienizada (ARCHITECTURE §18: `rehype-raw` antes do
 * `rehype-sanitize`; vídeo vira miniatura com "Abrir no navegador", nunca `iframe`) e os links.
 * Galeria, versões com changelog e dependências na própria página chegam com a P1-16.
 */
import { ExternalLink, Layers, Monitor, Plus, Server } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { SafeHtml } from '../../../components/common/SafeHtml';
import { SafeMarkdown } from '../../../components/common/SafeMarkdown';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { openExternal } from '../../../lib/external';
import type {
  AddChoice,
  PackId,
  ProjectKind,
  ProjectLinks,
  ProjectPreview,
  SearchResult,
  SideChoice,
  SourceId,
  VersionOption,
} from '../../../lib/ipc/bindings';
import { ManualDownloadNotice } from '../curseforge/ManualDownloadNotice';
import { SourceCheck } from '../curseforge/SourceCheck';
import { modrinthPageUrl, preferredRef } from '../modrinth/source';
import { useProjectDetails, useProjectVersions } from './api';
import { formatCount } from './model';
import { ProjectIcon } from './ResultRow';
import { type PackTarget, SourceMark, useAgoText, useTargetText } from './text';

const SIDE_ICONS = { both: Layers, client: Monitor, server: Server } as const;
const LINKS: readonly (keyof ProjectLinks)[] = ['issues', 'source', 'wiki', 'discord', 'page'];

export interface PreviewProps {
  packId: PackId;
  result: SearchResult;
  inPack: boolean;
  kind: ProjectKind;
  target: PackTarget;
  /** "Adicionar ao pack": o item com a fonte e a versão escolhidas. */
  onAdd: (choice: AddChoice, title: string) => void;
}

export function Preview({ packId, result, inPack, kind, target, onAdd }: PreviewProps) {
  const { t } = useTranslation('adicionar');
  const [source, setSource] = useState<SourceId>(
    () => preferredRef(result.sources)?.source ?? 'modrinth',
  );
  const [versionId, setVersionId] = useState<string | null>(null);
  const ref = result.sources.find((item) => item.source === source) ?? result.sources[0];
  const projectId = ref?.projectId ?? '';
  const details = useProjectDetails(packId, source, projectId);
  const versions = useProjectVersions(packId, source, projectId);
  const chosenVersion = versionId ?? versions.data?.defaultId ?? null;
  const agoText = useAgoText();
  const id = useId();
  const preview = details.data;
  const summaryParams = {
    author: result.author,
    downloads: formatCount(preview?.downloads ?? result.downloads),
    when: agoText(preview?.updated ?? result.updated),
  };

  return (
    <aside
      className="panel panel--strong disc__preview"
      aria-label={t('previa.rotulo', { name: result.title })}
    >
      <div className="row row--gap-3 row--top">
        <ProjectIcon
          url={preview?.iconUrl ?? result.iconUrl}
          name={result.title}
          className="preview__icon"
          size="xl"
        />
        <div className="grow">
          <h2 className="t-display-lg">{result.title}</h2>
          <p className="t-xs t-3">
            {result.author
              ? t('previa.resumo', summaryParams)
              : t('previa.resumoSemAutor', summaryParams)}
          </p>
          <div className="row row--wrap">
            <SourceMark sources={result.sources} />
            {preview?.side ? <SideTag side={preview.side} /> : null}
            {preview?.license ? (
              <span className="tag tag--plain">
                {t('previa.licenca', { license: preview.license })}
              </span>
            ) : null}
          </div>
        </div>
      </div>

      <div className="stack-2 preview__section">
        {result.sources.length > 1 ? (
          <div className="field">
            <label className="field__label" htmlFor={`${id}-src`}>
              {t('previa.fonte')}
            </label>
            <select
              id={`${id}-src`}
              className="select"
              value={source}
              aria-describedby={`${id}-src-hint`}
              onChange={(event) => {
                const next = result.sources.find((item) => item.source === event.target.value);
                if (next) {
                  setSource(next.source);
                  setVersionId(null);
                }
              }}
            >
              {result.sources.map((item) => (
                <option key={item.source} value={item.source}>
                  {item.source === preferredRef(result.sources)?.source
                    ? t('previa.fonteRecomendada', { fonte: t(`fonte.${item.source}`) })
                    : t(`fonte.${item.source}`)}
                </option>
              ))}
            </select>
            <p className="field__hint" id={`${id}-src-hint`}>
              {t('previa.fonteDica')}
            </p>
          </div>
        ) : null}
        <VersionField
          id={`${id}-ver`}
          state={versions}
          value={chosenVersion}
          defaultId={versions.data?.defaultId ?? null}
          target={target}
          onChange={setVersionId}
        />
        {result.sources.length > 1 ? (
          <SourceCheck
            packId={packId}
            sources={result.sources}
            source={source}
            chosenSha1={versions.data?.versions.find((v) => v.id === chosenVersion)?.sha1 ?? null}
          />
        ) : null}
        {source === 'curseforge' && result.manualDownload ? (
          <ManualDownloadNotice name={result.title} />
        ) : null}
        {inPack ? (
          <Alert kind="ok" compact title={t('previa.jaNoPack')} role="status" />
        ) : (
          <Button
            variant="primary"
            icon={Plus}
            block
            disabled={!chosenVersion}
            onClick={() => {
              onAdd({ source, projectId, versionId: chosenVersion }, result.title);
            }}
          >
            {t('previa.adicionar')}
          </Button>
        )}
      </div>

      <section className="preview__section" aria-labelledby={`${id}-desc`}>
        <h3 className="field__label" id={`${id}-desc`}>
          {t('previa.descricao')}
        </h3>
        {details.isPending ? (
          <LoadingState inline label={t('previa.carregando')} />
        ) : details.isError ? (
          <ErrorPanel
            compact
            title={t('previa.erro')}
            error={details.error}
            onRetry={() => {
              void details.refetch();
            }}
          />
        ) : (
          <Description preview={details.data} />
        )}
      </section>

      <section className="preview__section" aria-labelledby={`${id}-links`}>
        <h3 className="field__label" id={`${id}-links`}>
          {t('previa.links')}
        </h3>
        <div className="row row--wrap t-sm">
          {LINKS.map((name) => {
            const url =
              preview?.links[name] ??
              (name === 'page' && source === 'modrinth' && ref
                ? modrinthPageUrl(kind, ref.slug)
                : null);
            if (!url) return null;
            return (
              <Button
                key={name}
                variant="link"
                iconEnd={ExternalLink}
                onClick={() => {
                  void openExternal(url);
                }}
              >
                {name === 'page'
                  ? t('previa.link.page', { fonte: t(`fonte.${source}`) })
                  : t(`previa.link.${name}`)}
              </Button>
            );
          })}
        </div>
      </section>
    </aside>
  );
}

function SideTag({ side }: { side: SideChoice }) {
  const { t } = useTranslation('adicionar');
  return (
    <span className="tag tag--plain">
      <Icon icon={SIDE_ICONS[side]} />
      {t(`lado.${side}`)}
    </span>
  );
}

/** A descrição longa, sempre higienizada (CA-T08-12). */
export function Description({ preview }: { preview: ProjectPreview }) {
  const { t } = useTranslation('adicionar');
  if (preview.body.trim() === '') {
    return <p className="t-sm t-3">{t('previa.semDescricao')}</p>;
  }
  return preview.bodyFormat === 'html' ? (
    <SafeHtml html={preview.body} />
  ) : (
    <SafeMarkdown>{preview.body}</SafeMarkdown>
  );
}

interface VersionFieldProps {
  id: string;
  state: ReturnType<typeof useProjectVersions>;
  value: string | null;
  defaultId: string | null;
  target: PackTarget;
  onChange: (id: string) => void;
}

function VersionField({ id, state, value, defaultId, target, onChange }: VersionFieldProps) {
  const { t } = useTranslation('adicionar');
  const targetText = useTargetText(target);
  if (state.isPending) return <LoadingState inline label={t('previa.versoesCarregando')} />;
  if (state.isError) {
    return (
      <ErrorPanel
        compact
        title={t('previa.versoesErro')}
        error={state.error}
        onRetry={() => {
          void state.refetch();
        }}
      />
    );
  }
  if (state.data.versions.length === 0) {
    return (
      <Alert kind="warn" compact title={t('previa.semVersao', { alvo: targetText })}>
        <p>{t('previa.semVersaoTexto')}</p>
      </Alert>
    );
  }
  const label = (version: VersionOption) =>
    version.id === defaultId
      ? t('previa.maisNova', { number: version.number })
      : t(`previa.canal.${version.channel}`, { number: version.number });
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>
        {t('previa.versao')}
      </label>
      <select
        id={id}
        className="select"
        value={value ?? ''}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      >
        {state.data.versions.map((version) => (
          <option key={version.id} value={version.id}>
            {label(version)}
          </option>
        ))}
      </select>
    </div>
  );
}
