/**
 * Downloads manuais da CurseForge na etapa "Copiar o pack para o teste" (SPEC T20; protótipo
 * `teste-manual`): o aviso, a lista com o arquivo esperado e a situação de cada mod, "Abrir
 * página de download" e "Selecionar arquivo baixado…". O arquivo escolhido é conferido pelo
 * hash no backend; o errado fica marcado na linha e nada é copiado. Quando todos forem
 * encontrados, o teste continua sozinho (a lista some).
 */
import { CircleAlert, CircleCheck, ExternalLink, FilePlus } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { Loader } from '../../../components/ui/loader';
import { showToast } from '../../../components/ui/toast';
import type { ManualDownloadItem, PackId } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useOpenManualPage, useProvideManualFile } from './api';

export { useManualDownloads } from './api';

export function ManualDownloads({
  packId,
  items,
}: {
  packId: PackId;
  items: readonly ManualDownloadItem[];
}) {
  const { t } = useTranslation('downloadsManuais');
  const provide = useProvideManualFile(packId);
  const openPage = useOpenManualPage(packId);
  const [checking, setChecking] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState('');
  const onError = (error: unknown) => {
    showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
  };

  const choose = (item: ManualDownloadItem) => {
    setChecking(item.metafile);
    provide.mutate(item.metafile, {
      onSuccess: (result) => {
        if (!result) return;
        setAnnouncement(
          result.status.state === 'found'
            ? t('aviso.certo', { mod: result.name })
            : t('aviso.errado', { mod: result.name, esperado: result.fileName }),
        );
      },
      onError,
      onSettled: () => {
        setChecking(null);
      },
    });
  };

  return (
    <div className="stack manual-downloads">
      <Alert kind="warn" title={t('titulo', { count: items.length })}>
        <p>
          {t('explicacao')} {t('observando')}
        </p>
      </Alert>
      <div className="tablewrap">
        <table className="table" aria-label={t('tabela.rotulo')}>
          <thead>
            <tr>
              <th scope="col">{t('tabela.mod')}</th>
              <th scope="col">{t('tabela.arquivo')}</th>
              <th scope="col">{t('tabela.situacao')}</th>
              <th scope="col">
                <span className="sr-only">{t('tabela.acoes')}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {items.map((item) => (
              <tr key={item.metafile}>
                <td className="t-strong">{item.name}</td>
                <td className="path">{item.fileName}</td>
                <td>
                  <ItemStatus item={item} checking={checking === item.metafile} />
                </td>
                <td>
                  {item.status.state === 'found' ? null : (
                    <div className="row row--wrap">
                      {item.hasPage ? (
                        <Button
                          size="sm"
                          icon={ExternalLink}
                          aria-label={t('abrirPaginaDe', { mod: item.name })}
                          onClick={() => {
                            openPage.mutate(item.metafile, { onError });
                          }}
                        >
                          {t('abrirPagina')}
                        </Button>
                      ) : (
                        <span className="t-sm t-3">{t('semPagina', { mod: item.name })}</span>
                      )}
                      <Button
                        size="sm"
                        variant="primary"
                        icon={FilePlus}
                        loading={checking === item.metafile}
                        disabled={checking !== null && checking !== item.metafile}
                        aria-label={t('selecionarDe', { mod: item.name })}
                        onClick={() => {
                          choose(item);
                        }}
                      >
                        {checking === item.metafile ? t('conferindo') : t('selecionar')}
                      </Button>
                    </div>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="t-sm t-3">{t('continua')}</p>
      <p className="sr-only" role="status" aria-live="polite">
        {announcement}
      </p>
    </div>
  );
}

/** A situação de um mod: aguardando, encontrado ou arquivo errado (com o nome do certo). */
function ItemStatus({ item, checking }: { item: ManualDownloadItem; checking: boolean }) {
  const { t } = useTranslation('downloadsManuais');
  const status = item.status;
  if (status.state === 'found') {
    return (
      <span className="status status--ok">
        <Icon icon={CircleCheck} />
        {status.via === 'downloadsFolder' ? t('estado.pastaDownloads') : t('estado.escolhido')}
      </span>
    );
  }
  if (status.state === 'wrongFile' && !checking) {
    return (
      <div className="stack-2">
        <span className="status status--danger">
          <Icon icon={CircleAlert} />
          {t('estado.errado')}
        </span>
        <span className="t-sm t-2">
          {t('estado.erradoDetalhe', { escolhido: status.chosenFile, esperado: item.fileName })}
        </span>
      </div>
    );
  }
  return (
    <span className="status status--warn">
      <Loader />
      {checking ? t('conferindo') : t('estado.aguardando')}
    </span>
  );
}
