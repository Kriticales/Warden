/**
 * Os resultados de "Buscar em todas as configs" (SPEC T12; protótipo `configs-busca`): no
 * lugar do editor enquanto a busca está ativa, agrupados por arquivo ("8 resultados em 5
 * arquivos"), com a chave, o valor e o trecho do comentário, e os trechos que casaram
 * destacados. Clicar abre o arquivo na linha da chave. Filtro "Só deste mod".
 *
 * A busca espera a última tecla (`TYPING_DELAY_MS`); a resposta anterior continua na tela
 * até a nova chegar, e o Rust cancela a busca que ficou para trás.
 */
import { FileText, X } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../../../components/common/EmptyState';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import type {
  ConfigOrigin,
  ConfigSearchGroup,
  ConfigSearchHit,
  MarkedText,
  PackId,
} from '../../../lib/ipc/bindings';
import { useConfigSearch, useSettledText } from './api';
import './search.css';

export interface ConfigSearchResultsProps {
  packId: PackId;
  origin: ConfigOrigin;
  /** O texto do campo (a espera da última tecla é feita aqui). */
  query: string;
  onOpen: (path: string, line: number) => void;
  onClear: () => void;
}

const NUMBER = new Intl.NumberFormat('pt-BR');

function Marked({ text }: { text: MarkedText }) {
  return (
    <>
      {text.parts.map((part, position) =>
        part.marked ? (
          // As partes não mudam de lugar: a posição é a identidade delas.
          <mark key={position}>{part.text}</mark>
        ) : (
          <span key={position}>{part.text}</span>
        ),
      )}
    </>
  );
}

function Hit({
  path,
  hit,
  onOpen,
}: {
  path: string;
  hit: ConfigSearchHit;
  onOpen: (path: string, line: number) => void;
}) {
  const { t } = useTranslation('configsBusca');
  const context = hit.comment ?? (hit.key ? null : hit.text);
  return (
    <button
      type="button"
      className="hit"
      onClick={() => {
        onOpen(path, hit.line);
      }}
    >
      <span className="hit__key">
        {hit.label ? (
          <>
            <Marked text={hit.label} />{' '}
          </>
        ) : null}
        {hit.key ? (
          <span className={hit.label ? 't-mono t-3' : 't-mono'}>
            <Marked text={hit.key} />
          </span>
        ) : null}
      </span>
      <span className="hit__val t-mono">
        {hit.value ? (
          <>
            = <Marked text={hit.value} />
          </>
        ) : (
          t('linha', { linha: NUMBER.format(hit.line) })
        )}
      </span>
      {context ? (
        <span className={hit.key ? 'hit__ctx' : 'hit__ctx t-mono'}>
          <Marked text={context} />
        </span>
      ) : null}
      {hit.value ? (
        <span className="hit__line t-xs t-3">{t('linha', { linha: NUMBER.format(hit.line) })}</span>
      ) : null}
    </button>
  );
}

/** Identidade de um resultado: a linha não basta (tabela inline e JSON numa linha só têm várias
 * chaves na mesma linha). */
function hitKey(hit: ConfigSearchHit): string {
  return `${String(hit.line)}:${hit.key?.parts.map((part) => part.text).join('') ?? ''}`;
}

function Group({
  group,
  onOpen,
}: {
  group: ConfigSearchGroup;
  onOpen: (path: string, line: number) => void;
}) {
  const { t } = useTranslation('configsBusca');
  const hidden = group.total - group.hits.length;
  return (
    <section className="hits__group" aria-label={group.path}>
      <div className="hits__file">
        <FileText className="icon icon--sm" aria-hidden="true" />
        <span className="path">{group.path}</span>
        <span className="count count--text">{NUMBER.format(group.total)}</span>
      </div>
      {group.hits.map((hit) => (
        <Hit key={hitKey(hit)} path={group.path} hit={hit} onOpen={onOpen} />
      ))}
      {hidden > 0 ? (
        <p className="t-xs t-3 cfg-search__more">{t('maisNoArquivo', { count: hidden })}</p>
      ) : null}
    </section>
  );
}

export function ConfigSearchResults({
  packId,
  origin,
  query,
  onOpen,
  onClear,
}: ConfigSearchResultsProps) {
  const { t } = useTranslation('configsBusca');
  const typed = query.trim();
  const settledText = useSettledText(typed);
  const settled = settledText ?? typed;
  const [modId, setModId] = useState<string | null>(null);
  const search = useConfigSearch(packId, origin, settled, modId, settledText !== null);
  const response = search.data;
  const results = response?.cancelled ? null : (response?.results ?? null);
  const busy = settledText !== typed || search.isPlaceholderData || response?.cancelled;
  const status = response?.status;
  const mods = results?.mods ?? [];
  const shownHits = results?.groups.reduce((sum, group) => sum + group.hits.length, 0) ?? 0;
  // A busca que gerou o que está na tela (a anterior, enquanto a nova não chega).
  const label = response?.query ?? settled;

  return (
    <section className="panel cfg-search" aria-labelledby="cfg-search-title" aria-busy={busy}>
      <div className="panel__head">
        <div>
          <h2 className="panel__title panel__title--sans" id="cfg-search-title" aria-live="polite">
            {!results
              ? t('buscando')
              : results.totalHits === 0
                ? t('nada', { busca: label })
                : t('titulo', {
                    resultados: t('resultados', { count: results.totalHits }),
                    arquivos: t('arquivos', { count: results.totalFiles }),
                    busca: label,
                  })}
          </h2>
          <p className="t-xs t-3 cfg-search__help">{t('ajuda')}</p>
        </div>
        <Button size="sm" variant="ghost" icon={X} onClick={onClear}>
          {t('limpar')}
        </Button>
      </div>

      {status && !status.ready && status.available ? (
        <p className="t-xs t-3" role="status">
          {t('lendo', {
            lidos: NUMBER.format(status.indexedFiles),
            total: NUMBER.format(status.totalFiles),
          })}
        </p>
      ) : null}

      {mods.length > 0 || modId !== null ? (
        <div className="row row--wrap cfg-search__filters">
          <select
            className="select select--sm"
            aria-label={t('filtroMod.rotulo')}
            value={modId ?? ''}
            onChange={(event) => {
              setModId(event.target.value === '' ? null : event.target.value);
            }}
          >
            <option value="">{t('filtroMod.todos')}</option>
            {modId !== null && !mods.some((mod) => mod.id === modId) ? (
              <option value={modId}>{t('filtroMod.mod', { mod: modId })}</option>
            ) : null}
            {mods.map((mod) => (
              <option key={mod.id} value={mod.id}>
                {t('filtroMod.mod', { mod: mod.id })}
              </option>
            ))}
          </select>
        </div>
      ) : null}

      {search.isError ? (
        <ErrorPanel
          title={t('erro')}
          error={search.error}
          onRetry={() => {
            void search.refetch();
          }}
        />
      ) : status && !status.available ? (
        <EmptyState compact glyph="dots" title={t('semInstancia')} headingLevel={3} />
      ) : !results ? (
        <LoadingState label={t('buscando')} />
      ) : results.groups.length === 0 ? (
        <p className="t-sm t-2">{t('nadaTexto')}</p>
      ) : (
        <>
          <div className="hits">
            {results.groups.map((group) => (
              <Group key={group.path} group={group} onOpen={onOpen} />
            ))}
          </div>
          {results.truncated ? (
            <p className="t-xs t-3 cfg-search__more">
              {t('cortado', { mostrados: NUMBER.format(shownHits) })}
            </p>
          ) : null}
        </>
      )}
    </section>
  );
}
