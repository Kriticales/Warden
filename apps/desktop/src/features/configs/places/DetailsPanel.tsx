/**
 * O painel **Detalhes** à direita (esboço aprovado `design/bug-01/configs.html`, opção 1):
 * o arquivo (formato, tamanho, alterado), onde ele está (pack, teste local, este computador),
 * as diferenças com o pack com **Trazer para o pack** e **Voltar ao do pack** e as outras ações
 * (**Marcar como pessoal**). Recolhível pelo botão Detalhes do editor. Os avisos do arquivo
 * ficam no editor, junto das ações deles (Copiar para defaultconfigs/, Não substituir).
 */
import { Lock, Monitor, Package, Users } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { formatBytes } from '../../../lib/format';
import type { ConfigComparison, PlacedConfig } from '../../../lib/ipc/bindings';
import { cn } from '../../../lib/cn';
import { formatKey, languageOf } from '../model';
import type { PlaceActions } from './actions';
import type { View } from './model';

export interface DetailsPanelProps {
  file: PlacedConfig;
  view: View;
  comparison: ConfigComparison | undefined;
  actions: PlaceActions;
}

const DATE = new Intl.DateTimeFormat('pt-BR', { dateStyle: 'short', timeStyle: 'short' });

export function DetailsPanel({ file, view, comparison, actions }: DetailsPanelProps) {
  const { t } = useTranslation('configs');
  const format = formatKey(languageOf(file.path));
  const differs = file.place === 'pack' && file.inTest && file.differs;
  const perKeys = (comparison?.keys ?? []).filter((key) => key.perKey).map((key) => key.key);
  const allPerKey = comparison?.byKey === true && perKeys.length === comparison.keys.length;
  const busy = actions.busy === file.path;

  return (
    <aside className="cfgx__det" aria-labelledby="cfgx-det-title">
      <div className="cfgx__colhead">
        <h2 id="cfgx-det-title">{t('detalhes.titulo')}</h2>
      </div>
      <div className="cfgx__det-body">
        <section className="dsec">
          <h3>{t('detalhes.arquivo')}</h3>
          <dl className="kv">
            <dt>{t('detalhes.formato')}</dt>
            <dd>{t(`formato.${format}`)}</dd>
            <dt>{t('detalhes.tamanho')}</dt>
            <dd className="t-mono">{formatBytes(file.size)}</dd>
            {file.modifiedAtMs === null ? null : (
              <>
                <dt>{t('detalhes.alterado')}</dt>
                <dd>{DATE.format(new Date(file.modifiedAtMs))}</dd>
              </>
            )}
          </dl>
        </section>

        <section className="dsec">
          <h3>{t('detalhes.ondeEsta')}</h3>
          <ul className="places">
            {file.place === 'personal' ? (
              <>
                <li className="place place--off">
                  <Users className="icon" aria-hidden="true" />
                  <span className="place__t">{t('detalhes.pack')}</span>
                  <span className="place__d">{t('detalhes.packNunca')}</span>
                </li>
                <li className="place place--own">
                  <Lock className="icon" aria-hidden="true" />
                  <span className="place__t">{t('detalhes.computador')}</span>
                  <span className="place__d">{t('detalhes.computadorTexto')}</span>
                </li>
              </>
            ) : (
              <>
                <li className={cn('place', file.inPack ? 'place--pack' : 'place--off')}>
                  <Users className="icon" aria-hidden="true" />
                  <span className="place__t">{t('detalhes.pack')}</span>
                  <span className="place__d">
                    {file.inPack ? t('detalhes.packRecebe') : t('detalhes.packNao')}
                  </span>
                </li>
                <li className={cn('place', file.inTest ? 'place--test' : 'place--off')}>
                  <Monitor className="icon" aria-hidden="true" />
                  <span className="place__t">{t('detalhes.teste')}</span>
                  <span className={cn('place__d', differs && 'place__d--test')}>
                    {!file.inTest
                      ? t('detalhes.testeAusente')
                      : !file.inPack
                        ? t('detalhes.testeCriado')
                        : file.differs
                          ? t('detalhes.testeDifere', { count: file.differences })
                          : t('detalhes.testeIgual')}
                  </span>
                </li>
              </>
            )}
          </ul>
        </section>

        {differs ? (
          <section className="dsec">
            <h3>{view === 'pack' ? t('detalhes.diferencasNoTeste') : t('detalhes.diferencas')}</h3>
            {comparison?.byKey ? (
              <ul className="dlist">
                {comparison.keys.map((key) => (
                  <li key={key.key}>
                    <span className="dlist__k">{key.key}</span>
                    <span className="dlist__v">
                      <span className="dlist__a" title={t('chave.noPack')}>
                        {key.pack ?? t('chave.ausente')}
                      </span>
                      {' → '}
                      <span className="dlist__b" title={t('chave.noTeste')}>
                        {key.test ?? t('chave.ausente')}
                      </span>
                    </span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="t-sm t-2">
                {t('detalhes.linhasDiferentes', { count: file.differences })}
              </p>
            )}
            <div className="dsec__acts">
              <Button
                size="sm"
                variant="primary"
                icon={Package}
                loading={busy}
                onClick={() => {
                  actions.bring(file.path, allPerKey ? perKeys : undefined);
                }}
              >
                {t('faixa.trazerArquivo')}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                disabled={busy}
                onClick={() => {
                  actions.revert(file.path, allPerKey ? perKeys : undefined);
                }}
              >
                {t('faixa.voltar')}
              </Button>
            </div>
          </section>
        ) : null}

        <section className="dsec">
          <h3>{t('detalhes.acoes')}</h3>
          <div className="cfgx__actlist">
            {file.place === 'personal' ? (
              <Button
                size="sm"
                variant="ghost"
                icon={Lock}
                onClick={() => {
                  actions.unmarkPersonal(file);
                }}
              >
                {t('pessoal.desmarcar')}
              </Button>
            ) : (
              <Button
                size="sm"
                variant="ghost"
                icon={Lock}
                onClick={() => {
                  actions.markPersonal(file);
                }}
              >
                {t('pessoal.marcar')}
              </Button>
            )}
          </div>
        </section>
      </div>
    </aside>
  );
}
