/**
 * Partes da revisão de Importar modpack (SPEC T24): "O que acontece com cada arquivo" (viram
 * referência, ficam como arquivo local com o motivo, para revisar, arquivos copiados e só para
 * o servidor, cada linha com "Ver lista") e "O que não entra no pack" (com a caixa "Levar para o
 * pack" nos itens desconhecidos).
 */
import { CircleCheck, Server, TriangleAlert, X } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import type { ImportExcludedItem, ImportSummary } from '../../../lib/ipc/bindings';

interface Row {
  key: string;
  icon: LucideIcon;
  tone: 'ok' | 'warn' | 'info';
  count: number;
  title: string;
  text: string;
  items: { path: string; label: ReactNode }[];
}

export function SummaryPanel({ summary }: { summary: ImportSummary }) {
  const { t } = useTranslation('importar');
  const bySource = (source: 'modrinth' | 'curseforge' | 'url') =>
    summary.references.filter((item) => item.source === source);
  const reference = (items: ImportSummary['references']) =>
    items.map((item) => ({ path: item.path, label: item.name }));
  const byHash = summary.references.filter((item) => item.byHash);
  const rows: Row[] = [
    {
      key: 'modrinth',
      icon: CircleCheck,
      tone: 'ok',
      count: bySource('modrinth').length,
      title: t('resumo.modrinth', { count: bySource('modrinth').length }),
      text: t('resumo.modrinthTexto'),
      items: reference(bySource('modrinth')),
    },
    {
      key: 'curseforge',
      icon: CircleCheck,
      tone: 'ok',
      count: bySource('curseforge').length,
      title: t('resumo.curseforge', { count: bySource('curseforge').length }),
      text: t('resumo.curseforgeTexto'),
      items: reference(bySource('curseforge')),
    },
    {
      key: 'url',
      icon: CircleCheck,
      tone: 'ok',
      count: bySource('url').length,
      title: t('resumo.link', { count: bySource('url').length }),
      text: t('resumo.linkTexto'),
      items: reference(bySource('url')),
    },
    {
      key: 'hash',
      icon: CircleCheck,
      tone: 'ok',
      count: byHash.length,
      title: t('resumo.hash', { count: byHash.length }),
      text: t('resumo.hashTexto'),
      items: byHash.map((item) => ({
        path: item.path,
        label: `${item.name} · ${t(`fonte.${item.source}`)}`,
      })),
    },
    {
      key: 'locais',
      icon: TriangleAlert,
      tone: 'warn',
      count: summary.localFiles.length,
      title: t('resumo.locais', { count: summary.localFiles.length }),
      text: t('resumo.locaisTexto'),
      items: summary.localFiles.map((item) => ({
        path: item.path,
        label: `${item.name} · ${t(`motivoLocal.${item.reason}`)}`,
      })),
    },
    {
      key: 'revisar',
      icon: TriangleAlert,
      tone: 'warn',
      count: summary.review.length,
      title: t('resumo.revisar', { count: summary.review.length }),
      text: t('resumo.revisarTexto'),
      items: summary.review.map((item) => ({
        path: item.path,
        label: `${item.name} · ${t(`motivoRevisar.${item.reason}`)}`,
      })),
    },
    {
      key: 'arquivos',
      icon: CircleCheck,
      tone: 'ok',
      count: summary.files,
      title: t('resumo.arquivos', { count: summary.files }),
      text: t('resumo.arquivosTexto'),
      items: [],
    },
    {
      key: 'servidor',
      icon: Server,
      tone: 'info',
      count: summary.serverFiles,
      title: t('resumo.servidor', { count: summary.serverFiles }),
      text: t('resumo.servidorTexto'),
      items: [],
    },
  ];
  return (
    <section className="panel panel--strong" aria-labelledby="import-resumo">
      <h2 className="panel__title panel__title--sans" id="import-resumo">
        {t('resumo.titulo')}
      </h2>
      <ul className="import__rows">
        {rows
          .filter((row) => row.count > 0)
          .map((row) => (
            <li
              key={row.key}
              className={`import__row import__row--${row.tone}`}
              data-linha={row.key}
            >
              <Icon icon={row.icon} />
              <div className="import__row-body">
                <div className="t-strong">{row.title}</div>
                <div className="t-xs t-3">{row.text}</div>
                {row.items.length > 0 ? (
                  <details className="import__list">
                    <summary>{t('resumo.verLista')}</summary>
                    <ul>
                      {row.items.map((item) => (
                        <li key={item.path}>
                          <span>{item.label}</span> <span className="path">{item.path}</span>
                        </li>
                      ))}
                    </ul>
                  </details>
                ) : null}
              </div>
            </li>
          ))}
      </ul>
    </section>
  );
}

export function ExcludedPanel({
  items,
  include,
  onIncludeChange,
}: {
  items: ImportExcludedItem[];
  include: ReadonlySet<string>;
  onIncludeChange: (include: ReadonlySet<string>) => void;
}) {
  const { t } = useTranslation('importar');
  if (items.length === 0) return null;
  return (
    <section className="panel" aria-labelledby="import-lixo">
      <h2 className="panel__title panel__title--sans" id="import-lixo">
        {t('lixo.titulo')}
      </h2>
      <p className="t-sm t-2 mt-1">{t('lixo.texto')}</p>
      <ul className="checklist mt-2">
        {items.map((item) => {
          const shown = item.isDir ? `${item.path}/` : item.path;
          const detail = `${t(`lixo.motivo.${item.reason}`)} · ${t('lixo.arquivos', { count: item.files })}`;
          return (
            <li key={item.path} className={include.has(item.path) ? 'is-ok' : 'is-bad'}>
              {item.canInclude ? (
                <label className="check">
                  <input
                    type="checkbox"
                    checked={include.has(item.path)}
                    aria-label={t('lixo.levar', { path: shown })}
                    onChange={(event) => {
                      const next = new Set(include);
                      if (event.target.checked) next.add(item.path);
                      else next.delete(item.path);
                      onIncludeChange(next);
                    }}
                  />
                  <span>
                    <span className="path">{shown}</span> · {detail}
                  </span>
                </label>
              ) : (
                <>
                  <Icon icon={X} />
                  <span>
                    <span className="path">{shown}</span> · {detail}
                  </span>
                </>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
