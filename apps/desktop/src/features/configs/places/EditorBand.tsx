/**
 * A faixa abaixo da barra do editor (esboço aprovado `design/bug-01/configs.html`): diz para
 * onde o arquivo aberto vai, na cor do lugar (pack em ciano, teste local em âmbar, pessoal em
 * osso), e as ações daquele lugar: Trazer para o pack, Voltar ao do pack, Marcar como pessoal,
 * Deixar de ser pessoal… e Ver no teste local.
 */
import { Lock, Monitor, Package, Users } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import type { PlacedConfig } from '../../../lib/ipc/bindings';
import type { PlaceActions } from './actions';
import type { View } from './model';

export interface EditorBandProps {
  file: PlacedConfig;
  view: View;
  /** O teste local existe (a instância). */
  testAvailable: boolean;
  /** Chaves que dá para trazer uma a uma (as que existem nos dois lados). */
  perKeys: string[];
  actions: PlaceActions;
  onShowTest: () => void;
}

export function EditorBand({
  file,
  view,
  testAvailable,
  perKeys,
  actions,
  onShowTest,
}: EditorBandProps) {
  const { t } = useTranslation('configs');
  const busy = actions.busy === file.path;

  if (file.place === 'personal') {
    return (
      <div className="band band--own" role="note">
        <Lock className="icon" aria-hidden="true" />
        <span className="band__t">
          <b>{t('faixa.pessoal')}</b>{' '}
          {file.inPack ? t('faixa.pessoalNoPack') : t('faixa.pessoalTexto')}
        </span>
        <span className="band__acts">
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              actions.unmarkPersonal(file);
            }}
          >
            {t('pessoal.desmarcar')}
          </Button>
        </span>
      </div>
    );
  }

  const count = file.differences;
  if (view === 'pack' || !file.inTest) {
    if (view !== 'pack' && !file.inTest) {
      return (
        <div className="band band--test" role="note">
          <Monitor className="icon" aria-hidden="true" />
          <span className="band__t">
            <b>{t('faixa.naoNoTeste')}</b> {t('faixa.naoNoTesteTexto')}
          </span>
        </div>
      );
    }
    return (
      <div className="band band--pack" role="note">
        <Users className="icon" aria-hidden="true" />
        <span className="band__t">
          <b>{t('faixa.pack')}</b> {t('faixa.packTexto')}
          {file.differs ? (
            <>
              {' '}
              <b className="band__test">{t('faixa.packTesteDifere', { count })}</b>
            </>
          ) : null}
        </span>
        {file.differs && testAvailable ? (
          <span className="band__acts">
            <Button size="sm" icon={Monitor} onClick={onShowTest}>
              {t('grupos.verNoTeste')}
            </Button>
          </span>
        ) : null}
      </div>
    );
  }

  if (file.place === 'test') {
    return (
      <div className="band band--test" role="note">
        <Monitor className="icon" aria-hidden="true" />
        <span className="band__t">
          <b>{t('faixa.soTeste')}</b> {t('faixa.soTesteTexto')}
        </span>
        <span className="band__acts">
          {file.bringable ? (
            <Button
              size="sm"
              icon={Package}
              loading={busy}
              onClick={() => {
                actions.bring(file.path);
              }}
            >
              {t('faixa.trazerArquivo')}
            </Button>
          ) : null}
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
        </span>
      </div>
    );
  }

  if (!file.differs) {
    return (
      <div className="band band--test" role="note">
        <Monitor className="icon" aria-hidden="true" />
        <span className="band__t">
          <b>{t('faixa.teste')}</b> {t('faixa.testeIgual')}
        </span>
      </div>
    );
  }
  // Por chave, só as que existem nos dois lados vão uma a uma; senão, o arquivo inteiro.
  const allPerKey = file.byKey && perKeys.length === count;
  return (
    <div className="band band--test" role="note">
      <Monitor className="icon" aria-hidden="true" />
      <span className="band__t">
        <b>{t('faixa.teste')}</b>{' '}
        {file.byKey ? t('faixa.testeDifere', { count }) : t('faixa.testeDifereLinhas', { count })}
      </span>
      <span className="band__acts">
        <Button
          size="sm"
          icon={Package}
          loading={busy}
          onClick={() => {
            actions.bring(file.path, allPerKey ? perKeys : undefined);
          }}
        >
          {file.byKey ? t('faixa.trazerTudo', { count }) : t('faixa.trazerArquivo')}
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
      </span>
    </div>
  );
}
