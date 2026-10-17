/**
 * "Ver todas as alterações" (SPEC T07; protótipo `mods-raio-x~todas`): a lista completa das
 * partes do jogo que o mod altera, filtrável por texto e por risco, com "← Detalhes do mod".
 *
 * Abre um painel por cima do painel de detalhes, no mesmo lugar e sem escurecer (para o usuário é
 * o mesmo painel trocando de conteúdo); assim o foco fica preso nele, Esc e "Detalhes do mod"
 * voltam e o foco retorna ao botão que abriu.
 */
import { ArrowLeft, Search } from 'lucide-react';
import { useId, useMemo, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { Sheet, SheetContent, SheetTrigger } from '../../../components/ui/sheet';
import type { ModReport, PointRow, Risk } from '../../../lib/ipc/bindings';
import { RiskBadge } from './OverlapItem';
import { annotation, fullClass, matchesText, partLabel } from './text';

/** Linhas mostradas de uma vez (o resto pelo filtro). */
export const MAX_ROWS = 200;

const SHOW = ['todas', 'compartilhadas', 'high', 'medium', 'low'] as const;
type Show = (typeof SHOW)[number];

function passes(point: PointRow, show: Show): boolean {
  if (show === 'todas') return true;
  if (show === 'compartilhadas') return point.others.length > 0;
  return point.risk === (show satisfies Risk);
}

export interface XrayAllProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** O botão que abre (o foco volta para ele ao fechar). */
  trigger: ReactNode;
  modName: string;
  report: ModReport;
  names: Partial<Record<string, string>>;
}

export function XrayAll({ open, onOpenChange, trigger, modName, report, names }: XrayAllProps) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetTrigger asChild>{trigger}</SheetTrigger>
      {open ? (
        <AllBody
          modName={modName}
          report={report}
          names={names}
          onClose={() => {
            onOpenChange(false);
          }}
        />
      ) : null}
    </Sheet>
  );
}

function AllBody({
  modName,
  report,
  names,
  onClose,
}: Pick<XrayAllProps, 'modName' | 'report' | 'names'> & { onClose: () => void }) {
  const { t } = useTranslation('raioX');
  const id = useId();
  const [text, setText] = useState('');
  const [show, setShow] = useState<Show>('todas');
  const rows = useMemo(
    () => report.points.filter((p) => passes(p, show) && matchesText(p, text)),
    [report.points, show, text],
  );
  const visible = rows.slice(0, MAX_ROWS);
  const name = (path: string) => names[path] ?? path.split('/').pop() ?? path;
  return (
    <SheetContent title={t('todas.titulo', { nome: modName })}>
      <div className="xray-all">
        <div className="back-link">
          <Button variant="ghost" size="sm" icon={ArrowLeft} onClick={onClose}>
            {t('todas.voltar')}
          </Button>
        </div>
        <p className="t-sm t-2">
          {t('todas.resumo', { count: report.changeCount })}
          {'. '}
          {t('todas.configs', { lista: report.configs.join(', ') })}{' '}
          {report.priorities.length > 0
            ? t('todas.prioridade', { lista: report.priorities.join(', ') })
            : null}
        </p>
        <div className="toolbar">
          <div className="inputwrap">
            <Icon icon={Search} />
            <input
              className="input"
              type="search"
              value={text}
              placeholder={t('todas.filtrar')}
              aria-label={t('todas.filtrar')}
              onChange={(event) => {
                setText(event.target.value);
              }}
            />
          </div>
          <label className="t-xs t-3" htmlFor={`${id}-mostrar`}>
            {t('todas.mostrar')}
          </label>
          <select
            id={`${id}-mostrar`}
            className="select"
            value={show}
            onChange={(event) => {
              setShow(SHOW.find((value) => value === event.target.value) ?? 'todas');
            }}
          >
            {SHOW.map((value) => (
              <option key={value} value={value}>
                {t(`todas.mostrarOpcoes.${value}`)}
              </option>
            ))}
          </select>
        </div>
        {visible.length === 0 ? (
          <p className="t-sm t-3">{t('todas.vazio')}</p>
        ) : (
          <div className="tablewrap">
            <table className="table">
              <thead>
                <tr>
                  <th scope="col">{t('todas.colunas.parte')}</th>
                  <th scope="col">{t('todas.colunas.tipo')}</th>
                  <th scope="col">{t('todas.colunas.outros')}</th>
                </tr>
              </thead>
              <tbody>
                {visible.map((point) => (
                  <tr key={`${point.targetClass}#${point.method}`}>
                    <td className="t-mono xray-all__part" title={fullClass(point)}>
                      {partLabel(point)}
                    </td>
                    <td className="t-mono t-2 xray-all__kind">
                      {point.kinds.map(annotation).join(', ')}
                    </td>
                    <td>
                      {point.others.length > 0 ? (
                        <span className="xray-all__others">
                          {point.risk ? <RiskBadge risk={point.risk} /> : null}{' '}
                          <span className="t-xs">{point.others.map(name).join(', ')}</span>
                        </span>
                      ) : (
                        <span className="t-3 t-xs">{t('todas.nenhum')}</span>
                      )}
                    </td>
                  </tr>
                ))}
                {rows.length > visible.length ? (
                  <tr className="more-row">
                    <td colSpan={3}>{t('todas.mais', { count: rows.length - visible.length })}</td>
                  </tr>
                ) : null}
              </tbody>
            </table>
          </div>
        )}
        <details className="disclosure">
          <summary>{t('todas.comoLer')}</summary>
          <p className="t-sm">{t('todas.comoLerTexto')}</p>
        </details>
      </div>
    </SheetContent>
  );
}
