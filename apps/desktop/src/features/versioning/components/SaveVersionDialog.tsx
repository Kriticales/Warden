/**
 * Salvar versão (SPEC T16; protótipo `salvar` e `salvo`): o diálogo com a versão sugerida e o
 * motivo, as notas, "Marcar como versão final" e o resumo automático do changelog; depois de
 * salvar, o aviso com "Publicar versão", "Exportar arquivo" e "Fechar".
 *
 * Salvar é sempre local: nada vai ao GitHub. O número precisa ser maior que a última versão
 * salva (CA-T16-04): o backend confere enquanto a pessoa digita e de novo ao salvar.
 */
import { Link } from '@tanstack/react-router';
import { Package, Save } from 'lucide-react';
import type { TFunction } from 'i18next';
import { useId, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button, buttonVariants } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { Icon } from '../../../components/ui/icon';
import type { PackId, SavePreview, SavedVersion, SuggestReason } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { Checkbox, TextField } from '../../settings/components/fields';
import { useDebouncedValue, useSavePreview, useSaveVersion, useVersionValidation } from '../api';
import { checklistItems } from '../checklist';
import { nextVersions } from '../model';
import { publicationSlots } from '../publication';
import '../versioning.css';

/** Maior tamanho das notas (o mesmo do backend). */
export const MAX_NOTES = 20_000;

export interface SaveVersionFlowProps {
  packId: PackId;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** O diálogo Salvar versão e, depois de salvar, o aviso de próximos passos. */
export function SaveVersionFlow({ packId, open, onOpenChange }: SaveVersionFlowProps) {
  const [saved, setSaved] = useState<SavedVersion | null>(null);
  // A mutation fica aqui, que não desmonta ao salvar (veja `useSaveVersion`).
  const save = useSaveVersion(packId, {
    onSuccess: (version) => {
      onOpenChange(false);
      setSaved(version);
    },
  });
  const changeOpen = (next: boolean) => {
    if (!next) {
      save.reset();
    }
    onOpenChange(next);
  };
  return (
    <>
      <Dialog open={open} onOpenChange={changeOpen}>
        {open ? (
          <SaveVersionForm
            packId={packId}
            save={save}
            onClose={() => {
              changeOpen(false);
            }}
          />
        ) : null}
      </Dialog>
      <Dialog
        open={saved !== null}
        onOpenChange={(next) => {
          if (!next) {
            setSaved(null);
          }
        }}
      >
        {saved ? (
          <SavedNotice
            packId={packId}
            version={saved}
            onClose={() => {
              setSaved(null);
            }}
          />
        ) : null}
      </Dialog>
    </>
  );
}

type SaveMutation = ReturnType<typeof useSaveVersion>;

/** A frase do motivo da sugestão, a partir do motivo mais forte. */
export function reasonText(t: TFunction<'versoes'>, reason: SuggestReason): string {
  switch (reason.kind) {
    case 'firstVersion':
      return reason.valid
        ? t('motivo.primeira', { packVersion: reason.packVersion })
        : t('motivo.primeiraInvalida', { packVersion: reason.packVersion });
    case 'minecraftChanged':
      return t('motivo.minecraft');
    case 'loaderChanged':
      return t('motivo.loader');
    case 'removedWorldMods':
      return t('motivo.removeuMundo', { count: reason.count });
    case 'worldgenChanged':
      return t('motivo.geracao', { count: reason.count });
    case 'addedItems':
      return t('motivo.adicionou', { count: reason.count });
    case 'updatedItems':
      return t('motivo.atualizou', { count: reason.count });
    case 'configsChanged':
      return t('motivo.configs', { count: reason.count });
    case 'otherChanges':
      return t('motivo.outros');
  }
}

function SaveVersionForm({
  packId,
  save,
  onClose,
}: {
  packId: PackId;
  save: SaveMutation;
  onClose: () => void;
}) {
  const { t } = useTranslation('versoes');
  const preview = useSavePreview(packId, true);

  if (preview.isPending) {
    return (
      <DialogContent title={t('salvar.titulo')} description={t('salvar.descricao')} size="lg">
        <LoadingState inline label={t('salvar.carregando')} />
      </DialogContent>
    );
  }
  if (preview.isError) {
    return (
      <DialogContent
        title={t('salvar.titulo')}
        size="lg"
        footer={
          <DialogClose asChild>
            <Button variant="ghost">{t('salvar.cancelar')}</Button>
          </DialogClose>
        }
      >
        <ErrorPanel
          title={t('salvar.erroPrevia')}
          error={preview.error}
          onRetry={() => {
            void preview.refetch();
          }}
        />
      </DialogContent>
    );
  }
  const data = preview.data;
  if (data.suggestion === null) {
    return (
      <DialogContent
        title={t('salvar.titulo')}
        size="lg"
        footer={
          <DialogClose asChild>
            <Button variant="ghost">{t('salva.fechar')}</Button>
          </DialogClose>
        }
      >
        <p>
          {data.lastVersion
            ? t('salvar.nadaMudou', { version: data.lastVersion })
            : t('salvar.nadaMudouInicio')}
        </p>
      </DialogContent>
    );
  }
  return (
    <SaveFields
      packId={packId}
      preview={data}
      suggested={data.suggestion.version}
      reason={data.suggestion.reasons[0]}
      save={save}
      onClose={onClose}
    />
  );
}

function SaveFields({
  packId,
  preview,
  suggested,
  reason,
  save,
  onClose,
}: {
  packId: PackId;
  preview: SavePreview;
  suggested: string;
  reason: SuggestReason | undefined;
  save: SaveMutation;
  onClose: () => void;
}) {
  const { t } = useTranslation('versoes');
  const notesId = useId();
  const [version, setVersion] = useState(suggested);
  const [notes, setNotes] = useState('');
  const [markFinal, setMarkFinal] = useState(false);

  const trimmed = version.trim();
  const settled = useDebouncedValue(trimmed);
  const validation = useVersionValidation(packId, settled, true);
  const checking = trimmed !== settled || (trimmed !== '' && validation.isFetching);
  const rejected = !checking && validation.isError;
  const ready = trimmed !== '' && !checking && validation.isSuccess;
  const notesTooLong = Array.from(notes).length > MAX_NOTES;

  const base = preview.highestVersion ?? '';
  const examples = nextVersions(preview.highestVersion);
  const rules = [
    { id: 'patch', text: t('salvar.regraCorrecao'), next: examples?.patch },
    { id: 'minor', text: t('salvar.regraMenor'), next: examples?.minor },
    { id: 'major', text: t('salvar.regraMaior'), next: examples?.major },
  ];

  const submit = () => {
    if (!ready || notesTooLong || save.isPending) {
      return;
    }
    save.mutate({ version: trimmed, notes, markFinal });
  };

  return (
    <DialogContent
      title={t('salvar.titulo')}
      description={t('salvar.descricao')}
      size="lg"
      footer={
        <>
          <Button variant="ghost" disabled={save.isPending} onClick={onClose}>
            {t('salvar.cancelar')}
          </Button>
          <Button
            variant="primary"
            icon={Save}
            loading={save.isPending}
            disabled={!ready || notesTooLong}
            onClick={submit}
          >
            {save.isPending
              ? t('salvar.salvando')
              : t('salvar.salvar', { version: trimmed || suggested })}
          </Button>
        </>
      }
    >
      <form
        className="stack-3"
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <div className="grid-2">
          <TextField
            label={t('salvar.numero')}
            mono
            value={version}
            error={rejected ? appErrorMessage(toAppError(validation.error)) : null}
            hint={checking ? t('salvar.verificando') : undefined}
            onChange={(event) => {
              setVersion(event.target.value);
            }}
          />
          <div className="t-sm t-2 save__suggestion">
            {reason ? (
              <span>
                {t('salvar.sugerido', { version: suggested, motivo: reasonText(t, reason) })}
              </span>
            ) : null}
            {trimmed !== suggested ? (
              <Button
                variant="link"
                onClick={() => {
                  setVersion(suggested);
                }}
              >
                {t('salvar.usarSugerida', { version: suggested })}
              </Button>
            ) : null}
          </div>
        </div>

        <details className="disclosure">
          <summary>{t('salvar.comoEscolhido')}</summary>
          <ul className="stack-2 t-sm mt-2">
            {preview.highestVersion === null ? <li>{t('salvar.regraPrimeira')}</li> : null}
            {rules.map((rule) => (
              <li key={rule.id}>
                {rule.next ? (
                  <span className="t-mono">
                    {t('salvar.regraExemplo', { de: base, para: rule.next })}{' '}
                  </span>
                ) : null}
                {rule.text}
              </li>
            ))}
          </ul>
        </details>

        <div className="field">
          <label className="field__label" htmlFor={notesId}>
            {t('salvar.notas')} <span className="opt">{t('salvar.opcional')}</span>
          </label>
          <textarea
            id={notesId}
            className="textarea"
            rows={2}
            value={notes}
            placeholder={t('salvar.notasPlaceholder')}
            aria-invalid={notesTooLong ? true : undefined}
            aria-describedby={notesTooLong ? `${notesId}-err ${notesId}-hint` : `${notesId}-hint`}
            onChange={(event) => {
              setNotes(event.target.value);
            }}
          />
          {notesTooLong ? (
            <div className="field__error" id={`${notesId}-err`} role="alert">
              <span>{t('salvar.notasLongas', { max: MAX_NOTES })}</span>
            </div>
          ) : null}
          <div className="field__hint" id={`${notesId}-hint`}>
            {t('salvar.notasDica')}
          </div>
        </div>

        <Checkbox
          label={<b>{t('salvar.marcarFinal')}</b>}
          desc={t('salvar.marcarFinalDica')}
          checked={markFinal}
          onChange={setMarkFinal}
        />

        {preview.worldWarning ? <Alert kind="warn">{t('salvar.atencao')}</Alert> : null}

        <div>
          <div className="field__label">{t('salvar.resumo')}</div>
          <pre
            className="code code--scroll"
            // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- região rolável: o teclado precisa chegar nela
            tabIndex={0}
            aria-label={t('salvar.resumo')}
          >
            {preview.body.trim() === '' ? t('salvar.resumoVazio') : preview.body.trim()}
          </pre>
        </div>

        {checklistItems.length > 0 ? (
          <div className="panel">
            <div className="panel__title panel__title--sans">{t('salvar.antes')}</div>
            <ul className="checklist mt-2">
              {checklistItems.map(({ id, component: Item }) => (
                <Item key={id} packId={packId} />
              ))}
            </ul>
            <p className="t-xs t-3 mt-2">{t('salvar.antesDica')}</p>
          </div>
        ) : null}

        {save.isError ? <ErrorPanel compact error={save.error} /> : null}
        {/* Enter no número salva. */}
        <button type="submit" hidden />
      </form>
    </DialogContent>
  );
}

/** "Versão X salva": próximos passos (SPEC T16). */
function SavedNotice({
  packId,
  version,
  onClose,
}: {
  packId: PackId;
  version: SavedVersion;
  onClose: () => void;
}) {
  const { t } = useTranslation('versoes');
  const Publish = publicationSlots.publishButton;
  const title: ReactNode = version.isFinal
    ? t('salva.tituloFinal', { version: version.version })
    : t('salva.titulo', { version: version.version });
  return (
    <DialogContent
      title={title}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('salva.fechar')}
          </Button>
          <Link
            to="/packs/$packId/exportar"
            params={{ packId }}
            className={buttonVariants({ variant: 'secondary' })}
            onClick={onClose}
          >
            <Icon icon={Package} />
            <span>{t('salva.exportar')}</span>
          </Link>
          {version.isFinal ? <Publish packId={packId} version={version.version} size="md" /> : null}
        </>
      }
    >
      <p>{version.isFinal ? t('salva.texto') : t('salva.textoSalva')}</p>
    </DialogContent>
  );
}
