/**
 * Um modpack aberto (SPEC T08 "Navegar por modpacks"; protótipo `modpack`): no painel da
 * direita, o cabeçalho (nome, autor, versão do modpack, alvo, downloads), a licença do projeto
 * (com aviso quando não é livre ou não é informada) e **Mods deste modpack (N)**, com o estado de
 * cada mod para o pack:
 *
 * - **Já no seu pack**, sem caixa;
 * - **Compatível**, com caixa;
 * - **Sem versão para o seu pack**, desabilitado, com o motivo;
 * - **Arquivo fora das lojas**, desabilitado: o Warden não copia jars de pacotes de terceiros.
 *
 * Filtro "Só os que não estão no seu pack" e seletor **Versão** ("a mesma do modpack (testada
 * junto)", padrão quando o modpack é do mesmo Minecraft e loader do pack, ou "a mais nova
 * compatível"). **Adicionar N selecionados** abre um diálogo de dependências só (T09), pelo
 * `add_plan` de vários itens. Fechar devolve o foco a quem abriu.
 */
import { Check, CircleMinus, ExternalLink, PackageX, Plus, X } from 'lucide-react';
import { memo, useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { openExternal } from '../../../lib/external';
import { toAppError } from '../../../lib/ipc/errors';
import type {
  AddChoice,
  ModpackContents,
  ModpackMod,
  ModpackModState,
  PackId,
  SearchResult,
} from '../../../lib/ipc/bindings';
import { loaderName } from '../../packs/lib/pack-list';
import { formatCount } from '../common/model';
import { ProjectIcon } from '../common/ResultRow';
import { InPackTag, type PackTarget, SourceMark, useTargetText } from '../common/text';
import { preferredRef } from '../modrinth/source';
import { useModpackContents } from './api';
import {
  choicesFor,
  countByState,
  modState,
  VERSION_MODES,
  type VersionMode,
  visibleMods,
} from './model';
import './modpacks.css';

export interface ModpackPanelProps {
  packId: PackId;
  /** O modpack, como veio da busca. */
  result: SearchResult;
  target: PackTarget;
  /** Chaves do inventário atual (o que acabou de entrar passa a "Já no seu pack"). */
  inventoryKeys: ReadonlySet<string>;
  /** "Adicionar N selecionados": os itens e os nomes, para um diálogo de dependências só. */
  onAdd: (choices: AddChoice[], titles: string[]) => void;
  onClose: () => void;
}

export function ModpackPanel(props: ModpackPanelProps) {
  const { t } = useTranslation('modpacks');
  const ref = preferredRef(props.result.sources);
  const source = ref?.source ?? 'modrinth';
  const projectId = ref?.projectId ?? '';
  const contents = useModpackContents(props.packId, source, projectId);
  const opener = useRef<Element | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);

  // Quem abriu recebe o foco de volta ao fechar; ao abrir, o foco vai para o título do painel.
  useEffect(() => {
    opener.current = document.activeElement;
    heading.current?.focus();
  }, []);

  const close = () => {
    props.onClose();
    const target = opener.current;
    if (target instanceof HTMLElement && target.isConnected) target.focus();
  };

  return (
    <aside
      className="panel panel--strong preview disc__preview modpack"
      aria-label={t('painel.rotulo', { name: props.result.title })}
    >
      <div className="row row--gap-3 row--top">
        <ProjectIcon
          url={props.result.iconUrl}
          name={props.result.title}
          className="preview__icon"
          size="xl"
        />
        <div className="grow">
          <h2 className="t-display-lg" ref={heading} tabIndex={-1}>
            {props.result.title}
          </h2>
          <Summary result={props.result} contents={contents.data} />
        </div>
        <Button size="sm" variant="ghost" icon={X} onClick={close}>
          {t('painel.fechar')}
        </Button>
      </div>

      {contents.isPending ? (
        <LoadingState label={t('painel.carregando')} />
      ) : contents.isError ? (
        <ErrorPanel
          compact
          title={t('painel.erro')}
          error={contents.error}
          {...(toAppError(contents.error).retryable
            ? {
                onRetry: () => {
                  void contents.refetch();
                },
              }
            : {})}
          actions={<PageButton url={pageOf(contents.error)} />}
        />
      ) : (
        <Contents {...props} contents={contents.data} />
      )}
    </aside>
  );
}

/** A página do modpack que um erro aponta (modpack bloqueado a apps de terceiros). */
function pageOf(error: unknown): string | null {
  const page = toAppError(error).params.page;
  return page?.startsWith('https://') ? page : null;
}

function PageButton({ url }: { url: string | null }) {
  const { t } = useTranslation('modpacks');
  if (url === null) return null;
  return (
    <Button
      size="sm"
      icon={ExternalLink}
      onClick={() => {
        void openExternal(url);
      }}
    >
      {t('painel.abrirPagina')}
    </Button>
  );
}

function Summary({
  result,
  contents,
}: {
  result: SearchResult;
  contents: ModpackContents | undefined;
}) {
  const { t } = useTranslation('modpacks');
  const modpackTarget = useModpackTarget();
  const params = {
    author: result.author,
    version: contents?.versionNumber ?? '…',
    downloads: formatCount(result.downloads),
  };
  const alvo = contents ? modpackTarget(contents) : '';
  return (
    <>
      <p className="t-xs t-3">
        {result.author ? t('painel.resumo', params) : t('painel.resumoSemAutor', params)}
        {alvo ? ` · ${alvo}` : null}
      </p>
      <div className="row row--wrap">
        <SourceMark sources={result.sources} />
      </div>
    </>
  );
}

/** "NeoForge 1.21.1": o loader e a versão do Minecraft da versão do modpack. */
function useModpackTarget(): (contents: ModpackContents) => string {
  const { t } = useTranslation('modpacks');
  return (contents) => {
    const loader = contents.loaders[0];
    const minecraft = contents.gameVersions[0];
    if (!loader || !minecraft) return minecraft ?? '';
    return t('painel.alvo', { loader: loaderName(loader), minecraft });
  };
}

function Contents({
  target,
  inventoryKeys,
  onAdd,
  contents,
}: ModpackPanelProps & { contents: ModpackContents }) {
  const { t } = useTranslation('modpacks');
  const id = useId();
  const modpackTarget = useModpackTarget();
  const [onlyNew, setOnlyNew] = useState(false);
  const [mode, setMode] = useState<VersionMode>(contents.sameTarget ? 'same' : 'newest');
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const counts = useMemo(
    () => countByState(contents.mods, inventoryKeys),
    [contents.mods, inventoryKeys],
  );
  const shown = useMemo(
    () => visibleMods(contents.mods, onlyNew, inventoryKeys),
    [contents.mods, onlyNew, inventoryKeys],
  );
  // O que entrou no pack (ou deixou de ser compatível) sai da seleção.
  const chosen = useMemo(
    () =>
      contents.mods.filter(
        (mod) => selected.has(mod.key) && modState(mod, inventoryKeys) === 'compatible',
      ),
    [contents.mods, selected, inventoryKeys],
  );

  const toggle = (mod: ModpackMod) => {
    setSelected((now) => {
      const next = new Set(now);
      if (next.has(mod.key)) next.delete(mod.key);
      else next.add(mod.key);
      return next;
    });
  };

  return (
    <>
      <License contents={contents} />
      {contents.wholeDownload ? (
        <Alert kind="info" compact className="preview__section">
          {t('inteiro')}
        </Alert>
      ) : null}

      <h2 className="field__label modpack__title">
        {t('mods.titulo', { count: contents.mods.length })}
      </h2>
      <p className="t-xs t-2 modpack__counts">
        {t('mods.contagem', {
          noPack: counts.inPack,
          compativeis: counts.compatible,
          semVersao: counts.noVersion,
          fora: counts.external,
        })}
      </p>
      <div className="row row--wrap modpack__controls">
        <label className="check">
          <input
            type="checkbox"
            checked={onlyNew}
            onChange={(event) => {
              setOnlyNew(event.target.checked);
            }}
          />
          <span>{t('mods.soNovos')}</span>
        </label>
        <select
          className="select select--sm"
          aria-label={t('versao.rotulo')}
          aria-describedby={contents.sameTarget ? undefined : `${id}-alvo`}
          value={mode}
          onChange={(event) => {
            const next = VERSION_MODES.find((value) => value === event.target.value);
            if (next) setMode(next);
          }}
        >
          {VERSION_MODES.map((value) => (
            <option key={value} value={value} disabled={value === 'same' && !contents.sameTarget}>
              {value === 'same' ? t('versao.mesma') : t('versao.nova')}
            </option>
          ))}
        </select>
      </div>
      {contents.sameTarget ? null : (
        <p className="t-xs t-3" id={`${id}-alvo`}>
          {t('versao.outroAlvo', { alvo: modpackTarget(contents) })}
        </p>
      )}

      {contents.mods.length === 0 ? (
        <p className="t-sm t-3">{t('mods.vazio')}</p>
      ) : shown.length === 0 ? (
        <p className="t-sm t-3">{t('mods.nenhumNovo')}</p>
      ) : (
        <ul className="dlist modpack__list" aria-label={t('mods.rotulo')}>
          {shown.map((mod) => (
            <ModRow
              key={mod.key}
              mod={mod}
              state={modState(mod, inventoryKeys)}
              selected={selected.has(mod.key)}
              target={target}
              onToggle={toggle}
            />
          ))}
        </ul>
      )}

      {chosen.length > 0 ? (
        <div className="selbar disc__selbar" role="region" aria-label={t('selecao.rotulo')}>
          <span className="selbar__count" aria-live="polite">
            {t('selecao.contagem', { count: chosen.length })}
          </span>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              setSelected(new Set());
            }}
          >
            {t('selecao.limpar')}
          </Button>
          <span className="grow" />
          <Button
            variant="primary"
            icon={Plus}
            onClick={() => {
              onAdd(
                choicesFor(chosen, mode),
                chosen.map((mod) => mod.title),
              );
            }}
          >
            {t('selecao.adicionar', { count: chosen.length })}
          </Button>
        </div>
      ) : null}
    </>
  );
}

function License({ contents }: { contents: ModpackContents }) {
  const { t } = useTranslation('modpacks');
  const title = contents.license.name
    ? t('licenca.nome', { license: contents.license.name })
    : t('licenca.semLicenca');
  return (
    <Alert
      kind={contents.license.free ? 'info' : 'warn'}
      compact
      title={title}
      className="preview__section"
    >
      {t('licenca.texto')}
    </Alert>
  );
}

const ModRow = memo(function ModRow({
  mod,
  state,
  selected,
  target,
  onToggle,
}: {
  mod: ModpackMod;
  state: ModpackModState;
  selected: boolean;
  target: PackTarget;
  onToggle: (mod: ModpackMod) => void;
}) {
  const { t } = useTranslation('modpacks');
  const alvo = useTargetText(target);
  const rowState =
    state === 'inPack'
      ? 'inpack'
      : state === 'noVersion'
        ? 'noversion'
        : state === 'external'
          ? 'external'
          : 'normal';
  return (
    <li className={`drow drow--compact drow--${rowState}`}>
      <span className="drow__box">
        {state === 'inPack' ? null : (
          <label className="check">
            <input
              type="checkbox"
              checked={selected && state === 'compatible'}
              disabled={state !== 'compatible'}
              aria-label={t('mods.selecionar', { name: mod.title })}
              onChange={() => {
                onToggle(mod);
              }}
            />
          </label>
        )}
      </span>
      <span className="drow__main">
        <span className="drow__line">
          <span className="modpack__name">{mod.title}</span>
        </span>
        {mod.summary ? <span className="drow__desc">{mod.summary}</span> : null}
        {mod.source ? (
          <span className="drow__meta">
            <SourceMark sources={[{ source: mod.source }]} />
          </span>
        ) : null}
      </span>
      <span className="drow__end">
        {state === 'inPack' ? (
          <InPackTag />
        ) : state === 'noVersion' ? (
          <span className="drow__why">
            <Icon icon={CircleMinus} size="sm" />
            {t('mods.semVersao', { alvo })}
          </span>
        ) : state === 'external' ? (
          <span className="drow__why">
            <Icon icon={PackageX} size="sm" />
            {t('mods.fora')}
          </span>
        ) : selected ? (
          <Icon icon={Check} size="sm" />
        ) : null}
      </span>
    </li>
  );
});
