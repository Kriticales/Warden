/**
 * **Comparar** (o "Comparar" da opção 3 do esboço aprovado `design/bug-01/configs.html`): o
 * teste local e o pack lado a lado. Por chave (formatos conhecidos), cada chave diferente numa
 * linha com o valor do teste, as setas → (trazer o valor do teste para o pack) e ← (voltar ao
 * do pack) e o valor do pack. Sem leitura por chave, os dois textos lado a lado com as linhas
 * diferentes marcadas.
 */
import { ArrowLeft, ArrowRight, Monitor, Package, Users } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { diffLines } from '../../../lib/diff';
import type { ConfigComparison, PlacedConfig } from '../../../lib/ipc/bindings';
import type { PlaceActions } from './actions';

export interface CompareViewProps {
  file: PlacedConfig;
  comparison: ConfigComparison;
  actions: PlaceActions;
}

/** Números (1..) das linhas de `text` que não existem em `other`. */
function differentLines(other: string, text: string): Set<number> {
  return new Set(
    diffLines(other, text, { context: null }).flatMap((line) =>
      line.kind === 'add' ? [line.newNo] : [],
    ),
  );
}

function TextColumn({ text, marked, label }: { text: string; marked: Set<number>; label: string }) {
  return (
    <pre className="cmp__code" aria-label={label}>
      {text.split('\n').map((line, index) => (
        // As linhas não mudam de lugar: o número é a identidade delas.
        <span key={index} className={marked.has(index + 1) ? 'cmp__line is-diff' : 'cmp__line'}>
          <span className="cmp__n" aria-hidden="true">
            {index + 1}
          </span>
          {line.replace(/\r$/, '')}
          {'\n'}
        </span>
      ))}
    </pre>
  );
}

export function CompareView({ file, comparison, actions }: CompareViewProps) {
  const { t } = useTranslation('configs');
  const count = comparison.byKey ? comparison.keys.length : comparison.lines;
  const perKeys = comparison.keys.filter((key) => key.perKey).map((key) => key.key);
  const allPerKey = comparison.byKey && perKeys.length === comparison.keys.length;
  const head = (
    <div className={comparison.byKey ? 'cmp__head' : 'cmp__head cmp__head--two'} aria-hidden="true">
      {comparison.byKey ? <div>{t('comparar.opcao')}</div> : null}
      <div className="cmp__side cmp__side--test">
        <span className="cmp__t">
          <Monitor className="icon icon--sm" />
          {t('comparar.teste')}
        </span>
        <span className="cmp__s">{t('comparar.testeSub')}</span>
      </div>
      {comparison.byKey ? <div /> : null}
      <div className="cmp__side cmp__side--pack">
        <span className="cmp__t">
          <Users className="icon icon--sm" />
          {t('comparar.pack')}
        </span>
        <span className="cmp__s">{t('comparar.packSub')}</span>
      </div>
    </div>
  );

  return (
    <div className="cmp" role="region" aria-label={t('comparar.area')}>
      <div className="cmp__sum">
        <span className="cmp__sum-t">
          {count === 0 ? (
            <>
              <b>{t('comparar.igual')}</b> {t('comparar.igualTexto')}
            </>
          ) : comparison.byKey ? (
            <>
              <b>{t('comparar.resumo', { count })}</b> {t('comparar.resumoDica')}
            </>
          ) : (
            <b>{t('comparar.resumoLinhas', { count })}</b>
          )}
        </span>
        {count > 0 ? (
          <>
            <Button
              size="sm"
              variant="primary"
              icon={Package}
              loading={actions.busy === file.path}
              onClick={() => {
                actions.bring(file.path, allPerKey ? perKeys : undefined);
              }}
            >
              {comparison.byKey ? t('faixa.trazerTudo', { count }) : t('faixa.trazerArquivo')}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={actions.busy === file.path}
              onClick={() => {
                actions.revert(file.path, allPerKey ? perKeys : undefined);
              }}
            >
              {t('faixa.voltar')}
            </Button>
          </>
        ) : null}
      </div>
      {head}
      {comparison.byKey ? (
        <ul className="cmp__rows">
          {comparison.keys.map((key) => {
            const busy = actions.busy === `${file.path}|${key.key}`;
            return (
              <li key={key.key} className="cmp__row">
                <span className="cmp__key">{key.key}</span>
                <span className="cmp__val cmp__val--test">{key.test ?? t('chave.ausente')}</span>
                <span className="cmp__mid">
                  {key.perKey ? (
                    <>
                      <Button
                        size="sm"
                        iconOnly
                        icon={ArrowRight}
                        loading={busy}
                        onClick={() => {
                          actions.bring(file.path, [key.key]);
                        }}
                      >
                        {t('chave.trazerChave', { chave: key.key })}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        iconOnly
                        icon={ArrowLeft}
                        disabled={busy}
                        onClick={() => {
                          actions.revert(file.path, [key.key]);
                        }}
                      >
                        {t('chave.voltarChave', { chave: key.key })}
                      </Button>
                    </>
                  ) : null}
                </span>
                <span className="cmp__val cmp__val--pack">{key.pack ?? t('chave.ausente')}</span>
              </li>
            );
          })}
        </ul>
      ) : (
        <div className="cmp__texts">
          <TextColumn
            text={comparison.testText ?? ''}
            marked={differentLines(comparison.packText ?? '', comparison.testText ?? '')}
            label={t('comparar.teste')}
          />
          <TextColumn
            text={comparison.packText ?? ''}
            marked={differentLines(comparison.testText ?? '', comparison.packText ?? '')}
            label={t('comparar.pack')}
          />
        </div>
      )}
    </div>
  );
}
