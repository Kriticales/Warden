/**
 * Arquivos do computador (ou o arquivo de um link direto) na página Adicionar (SPEC T08
 * "Arquivo"; P1-11): lê todos de uma vez (metadados, compatibilidade e procura pelo hash no
 * Modrinth e na CurseForge) e mostra um cartão por arquivo ([`FileCard`]).
 *
 * A referência (arquivo achado pelo hash) segue pelo diálogo de dependências da página; o
 * arquivo local e o link direto são gravados aqui (`add_local_apply`). Ao sair, os arquivos que
 * não entraram são esquecidos (e o download do link, apagado).
 */
import { ArrowLeft } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import type {
  AddResult,
  AppError,
  HashMatch,
  LocalApplyItem,
  LocalFileRef,
  PackId,
} from '../../../lib/ipc/bindings';
import { appErrorMessage } from '../../../lib/ipc/errors';
import type { CommandError } from '../../../lib/ipc/result';
import type { PackTarget } from '../common/text';
import { discardFiles, useLocalApply, useLocalInspect } from './api';
import { FileCard } from './FileCard';
import { fileErrorReason } from './model';
import './local.css';

export interface LocalFilesPanelProps {
  packId: PackId;
  files: readonly LocalFileRef[];
  /** Itens soltos que não eram `.jar`/`.zip`. */
  ignored?: number;
  /** Link direto de onde o arquivo veio. */
  link?: string | null;
  linkHost?: string;
  target: PackTarget;
  /** Arquivo achado pelo hash: abrir o diálogo de dependências com a versão achada. */
  onReference: (match: HashMatch) => void;
  onAdded: (result: AddResult) => void;
  onBack: () => void;
}

export function LocalFilesPanel({
  packId,
  files,
  ignored = 0,
  link = null,
  linkHost = '',
  target,
  onReference,
  onAdded,
  onBack,
}: LocalFilesPanelProps) {
  const { t } = useTranslation('adicionarArquivo');
  const tokens = files.map((file) => file.token);
  const inspect = useLocalInspect(packId, tokens);
  const apply = useLocalApply(packId);
  const [removed, setRemoved] = useState<ReadonlySet<string>>(new Set());
  const [added, setAdded] = useState<ReadonlyMap<string, string>>(new Map());
  const [errors, setErrors] = useState<ReadonlyMap<string, string>>(new Map());
  const [adding, setAdding] = useState<string | null>(null);

  // Ao sair, esquece todos (os que entraram já foram esquecidos pelo backend).
  const latest = useRef(tokens);
  useEffect(() => {
    latest.current = tokens;
  });
  useEffect(
    () => () => {
      discardFiles(latest.current);
    },
    [],
  );

  const errorText = (error: AppError, name: string) => {
    const reason = fileErrorReason(error);
    return reason
      ? t(`erros.${reason}`, { name: error.params.name ?? name })
      : appErrorMessage(error);
  };

  const add = (file: LocalFileRef, item: LocalApplyItem) => {
    setAdding(file.token);
    setErrors((now) => {
      const next = new Map(now);
      next.delete(file.token);
      return next;
    });
    apply.mutate([item], {
      onSuccess: (result) => {
        const path = result.added[0]?.path ?? result.skipped[0] ?? '';
        setAdded((now) => new Map(now).set(file.token, path));
        onAdded(result);
      },
      onError: (error: CommandError) => {
        setErrors((now) => new Map(now).set(file.token, errorText(error.appError, file.fileName)));
      },
      onSettled: () => {
        setAdding(null);
      },
    });
  };

  const header = (
    <div className="row row--between local-files__head">
      <Button size="sm" variant="ghost" icon={ArrowLeft} onClick={onBack}>
        {t('arquivo.voltar')}
      </Button>
      {ignored > 0 ? (
        <span className="t-xs t-3">{t('arquivo.ignorados', { count: ignored })}</span>
      ) : null}
    </div>
  );

  if (inspect.isPending) {
    return (
      <div className="stack-2" aria-busy="true">
        {header}
        <LoadingState inline label={t('arquivo.lendo', { count: files.length })} />
      </div>
    );
  }
  if (inspect.isError) {
    return (
      <div className="stack-2">
        {header}
        <ErrorPanel
          title={t('arquivo.erroLeitura')}
          error={inspect.error}
          onRetry={() => {
            void inspect.refetch();
          }}
        />
      </div>
    );
  }
  const visible = inspect.data.filter((result) => !removed.has(result.token));
  return (
    <div className="stack-2">
      {header}
      {visible.length === 0 ? (
        <Alert kind="neutral" compact title={t('arquivo.nenhum')} />
      ) : (
        <ul className="local-files" aria-label={t('arquivo.lista')}>
          {visible.map((result) => {
            const file = files.find((candidate) => candidate.token === result.token) ?? {
              token: result.token,
              fileName: result.report?.fileName ?? '',
            };
            if (!result.report) {
              return (
                <li key={result.token} className="local-file panel">
                  <p className="local-file__name">{file.fileName}</p>
                  <Alert
                    kind="danger"
                    compact
                    title={
                      result.error
                        ? errorText(result.error, file.fileName)
                        : t('arquivo.erroLeitura')
                    }
                    className="preview__section"
                  />
                </li>
              );
            }
            return (
              <FileCard
                key={result.token}
                file={file}
                report={result.report}
                link={link}
                linkHost={linkHost}
                target={target}
                adding={adding === result.token}
                addedPath={added.get(result.token) ?? null}
                errorText={errors.get(result.token) ?? null}
                onReference={onReference}
                onAdd={(item) => {
                  add(file, item);
                }}
                onRemove={() => {
                  discardFiles([result.token]);
                  setRemoved((now) => new Set(now).add(result.token));
                }}
              />
            );
          })}
        </ul>
      )}
    </div>
  );
}
