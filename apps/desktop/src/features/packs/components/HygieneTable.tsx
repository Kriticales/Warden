/**
 * Verificação de higiene (T04, passo 3): "N arquivos não deveriam ir para quem joga o pack",
 * com caminho, tamanho e motivo de cada um e caixas já marcadas. Quem chama decide o que
 * "Limpar" e "Agora não" fazem.
 */
import { useTranslation } from 'react-i18next';

import type { HygieneFinding } from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { causeText } from '../lib/pack-list';

export interface HygieneTableProps {
  items: readonly HygieneFinding[];
  selected: ReadonlySet<string>;
  onSelectedChange: (selected: ReadonlySet<string>) => void;
}

export function HygieneTable({ items, selected, onSelectedChange }: HygieneTableProps) {
  const { t } = useTranslation('packs');
  const toggle = (path: string, checked: boolean) => {
    const next = new Set(selected);
    if (checked) next.add(path);
    else next.delete(path);
    onSelectedChange(next);
  };
  return (
    <section className="panel panel--strong" aria-labelledby="higiene-titulo">
      <div className="panel__head">
        <h2 className="panel__title panel__title--sans" id="higiene-titulo">
          {t('abrir.higiene.titulo', { count: items.length })}
        </h2>
      </div>
      <div className="tablewrap">
        <table className="table table--plain">
          <thead>
            <tr>
              <th scope="col" className="shrink">
                <span className="sr-only">{t('abrir.higiene.limpar')}</span>
              </th>
              <th scope="col">{t('abrir.higiene.arquivo')}</th>
              <th scope="col" className="num">
                {t('abrir.higiene.tamanho')}
              </th>
              <th scope="col">{t('abrir.higiene.motivo')}</th>
            </tr>
          </thead>
          <tbody>
            {items.map((item) => (
              <tr key={item.path}>
                <td>
                  <label className="check">
                    <input
                      type="checkbox"
                      checked={selected.has(item.path)}
                      aria-label={t('abrir.higiene.limparArquivo', { path: item.path })}
                      onChange={(event) => {
                        toggle(item.path, event.target.checked);
                      }}
                    />
                  </label>
                </td>
                <td className="path">
                  {item.path}
                  {item.inIndex ? (
                    <span className="tag tag--warn ml-2">{t('abrir.higiene.noIndice')}</span>
                  ) : null}
                </td>
                <td className="num">
                  {item.onDisk ? formatBytes(item.bytes ?? 0) : t('lista.semData')}
                </td>
                <td className="t-2">
                  {item.reasons
                    .map((cause) => {
                      const text = causeText(cause);
                      return 'count' in text
                        ? t(`motivo.${text.key}`, { count: text.count })
                        : t(`motivo.${text.key}`);
                    })
                    .join('; ')}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="t-sm t-3 mt-3">{t('abrir.higiene.seguranca')}</p>
    </section>
  );
}
