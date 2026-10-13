/**
 * "Ver o pack inteiro" (SPEC T06; ARCHITECTURE §18): todos os mods num desenho em canvas
 * (Cytoscape.js com o layout `dagre`), com as bibliotecas agrupadas e, sempre ao lado, a mesma
 * informação em lista de texto (acessível). O Cytoscape só é baixado quando o desenho abre
 * (`lazy`); acima de 150 mods o Warden avisa que fica difícil de ler e só desenha se pedir.
 */
import { lazy, Suspense, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import type { FocusReport, PackId } from '../../../lib/ipc/bindings';
import { useWholeGraph } from './api';
import { WHOLE_WARNING, wholeList } from './model';

const WholeGraphCanvas = lazy(() => import('./WholeGraphCanvas'));

export interface WholeGraphDialogProps {
  packId: PackId;
  open: boolean;
  onClose: () => void;
  onOpen: (path: string) => void;
}

export function WholeGraphDialog({ packId, open, onClose, onOpen }: WholeGraphDialogProps) {
  const { t } = useTranslation('grafoDesenho');
  const whole = useWholeGraph(packId, open);
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      {open ? (
        <DialogContent
          size="lg"
          title={t('inteiro.titulo')}
          description={
            whole.data
              ? t('inteiro.resumo', {
                  count: whole.data.nodes.length,
                  ligacoes: t('inteiro.ligacoes', { count: whole.data.edges.length }),
                })
              : undefined
          }
        >
          {whole.isError ? (
            <ErrorPanel
              compact
              error={whole.error}
              onRetry={() => {
                void whole.refetch();
              }}
            />
          ) : !whole.data ? (
            <LoadingState inline label={t('carregando')} />
          ) : (
            <WholeBody report={whole.data} onOpen={onOpen} />
          )}
        </DialogContent>
      ) : null}
    </Dialog>
  );
}

function WholeBody({ report, onOpen }: { report: FocusReport; onOpen: (path: string) => void }) {
  const { t } = useTranslation('grafoDesenho');
  const crowded = report.nodes.length > WHOLE_WARNING;
  const [draw, setDraw] = useState(!crowded);
  const [failed, setFailed] = useState(false);
  return (
    <div className="grafo-inteiro">
      <div className="grafo-inteiro__desenho">
        {crowded ? (
          <Alert
            kind="warn"
            compact
            title={t('inteiro.muitos', { count: report.nodes.length })}
            actions={
              draw ? null : (
                <Button
                  size="sm"
                  onClick={() => {
                    setDraw(true);
                  }}
                >
                  {t('inteiro.desenharAssim')}
                </Button>
              )
            }
          />
        ) : null}
        {failed ? (
          <Alert kind="danger" compact title={t('inteiro.falhou')} />
        ) : draw ? (
          <Suspense fallback={<LoadingState inline label={t('inteiro.desenhando')} />}>
            <WholeGraphCanvas
              report={report}
              label={t('inteiro.regiaoDesenho')}
              libraries={t('inteiro.bibliotecas')}
              drawing={t('inteiro.desenhando')}
              onOpen={onOpen}
              onFail={() => {
                setFailed(true);
              }}
            />
          </Suspense>
        ) : null}
      </div>
      <section className="grafo-inteiro__lista" aria-labelledby="grafo-inteiro-lista">
        <h3 className="panel__title panel__title--sans" id="grafo-inteiro-lista">
          {t('inteiro.lista')}
        </h3>
        <ul className="stack-2 t-sm">
          {wholeList(report).map(({ node, needs }) => (
            <li key={node.key}>
              <Button
                variant="link"
                onClick={() => {
                  if (node.item) onOpen(node.item.path);
                }}
              >
                {node.label}
              </Button>{' '}
              <span className="t-3">
                {needs.length === 0
                  ? t('inteiro.semLigacoes')
                  : t('inteiro.precisaDe', {
                      nomes: new Intl.ListFormat('pt-BR', { type: 'conjunction' }).format(needs),
                    })}
                {node.library ? ` · ${t('no.biblioteca')}` : ''}
              </span>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
