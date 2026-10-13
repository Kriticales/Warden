/**
 * Diferença por valor (SPEC T15 "Cada arquivo"): "chave: antes → depois", uma linha por chave
 * (`.kvdiff` do design system). O valor que não existia ou sumiu aparece por extenso.
 */
import { ArrowRight } from 'lucide-react';
import { Fragment } from 'react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';

export interface KeyDiffRow {
  key: string;
  before: string | null;
  after: string | null;
}

export function KeyDiff({ rows }: { rows: readonly KeyDiffRow[] }) {
  const { t } = useTranslation('captura');
  if (rows.length === 0) return null;
  return (
    <div className="kvdiff">
      {rows.map((row) => (
        <Fragment key={row.key}>
          <span>{row.key}</span>
          {row.before === null ? (
            <span className="t-3">{t('valor.naoExistia')}</span>
          ) : (
            <span className="old">{row.before}</span>
          )}
          <span>
            <Icon icon={ArrowRight} size="sm" />
            <span className="sr-only">{t('valor.para')}</span>
          </span>
          {row.after === null ? (
            <span className="t-3">{t('valor.apagada')}</span>
          ) : (
            <span className="new">{row.after}</span>
          )}
          <span />
        </Fragment>
      ))}
    </div>
  );
}
