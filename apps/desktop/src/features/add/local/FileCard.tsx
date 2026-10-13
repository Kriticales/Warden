/**
 * Um arquivo a adicionar (SPEC T08 "Arquivo"; CA-T08-04 e CA-T08-05): o que o Warden leu do
 * arquivo e o que ele oferece.
 *
 * - **Não serve para o pack** (loader ou versão do Minecraft): bloqueio com a explicação e
 *   **Adicionar mesmo assim** (o arquivo entra e aparece como problema no diagnóstico).
 * - **Achado pelo hash:** "Este arquivo é o Sodium 0.6.0 do Modrinth." com **Adicionar como
 *   referência** (recomendado; abre o diálogo de dependências, sem copiar o jar) ou **Adicionar
 *   como arquivo local** (ou **pelo link**, no link direto).
 * - **Não achado:** entra como arquivo local (copiado para a pasta do pack) ou pelo link.
 * - **Já no pack:** nada a fazer.
 */
import { CircleCheck, FileArchive, Link2, Plus, X } from 'lucide-react';
import { type ReactNode, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { formatBytes } from '../../../lib/format';
import type {
  FileReport,
  HashMatch,
  Incompatibility,
  LocalApplyItem,
  LocalFileRef,
  ProjectKind,
} from '../../../lib/ipc/bindings';
import { loaderName } from '../../packs/lib/pack-list';
import { type PackTarget, useTargetText } from '../common/text';
import { initialKind, loaderList, referenceMatch } from './model';

// Um arquivo do computador nunca entra como modpack (o Tipo = Modpacks só busca e abre).
const KINDS = ['mod', 'resourcePack', 'shader'] as const satisfies readonly ProjectKind[];

export interface FileCardProps {
  file: LocalFileRef;
  report: FileReport;
  /** Link direto de onde o arquivo veio. */
  link: string | null;
  /** Servidor do link ("files.example.com"). */
  linkHost: string;
  target: PackTarget;
  /** Gravando este arquivo. */
  adding: boolean;
  /** Já entrou no pack (caminho do item). */
  addedPath: string | null;
  /** Erro da última gravação deste arquivo, já em texto. */
  errorText: string | null;
  onReference: (match: HashMatch) => void;
  onAdd: (item: LocalApplyItem) => void;
  onRemove: () => void;
}

function IncompatibilityLine({ problem }: { problem: Incompatibility }) {
  const { t } = useTranslation('adicionarArquivo');
  if (problem.kind === 'loader') {
    return (
      <p>
        {t('bloqueio.loader', { found: loaderList(problem.found), pack: loaderName(problem.pack) })}
      </p>
    );
  }
  return <p>{t('bloqueio.minecraft', { accepts: problem.accepts, pack: problem.pack })}</p>;
}

export function FileCard({
  file,
  report,
  link,
  linkHost,
  target,
  adding,
  addedPath,
  errorText,
  onReference,
  onAdd,
  onRemove,
}: FileCardProps) {
  const { t } = useTranslation('adicionarArquivo');
  const alvo = useTargetText(target);
  const id = useId();
  const [kind, setKind] = useState<ProjectKind>(() => initialKind(report));
  const [accepted, setAccepted] = useState(false);
  const match = referenceMatch(report);
  const blocked = kind === 'mod' && report.incompatible.length > 0 && !accepted;
  const sameFile = report.inPack?.sameFile === true;
  const nameTaken = report.inPack !== null && !sameFile && link === null;
  const info = report.modInfo;

  const add = (acceptIncompatible: boolean) => {
    onAdd({
      token: file.token,
      sha256: report.sha256,
      kind,
      side: report.side,
      acceptIncompatible,
    });
  };

  const referenceLabel = (found: HashMatch) =>
    found.source === 'modrinth'
      ? t('identificado.referenciaModrinth')
      : t('identificado.referenciaCurseforge');

  let body: ReactNode;
  if (addedPath !== null) {
    body = (
      <Alert kind="ok" compact role="status" title={t('adicionadoComo', { path: addedPath })} />
    );
  } else if (sameFile) {
    body = (
      <Alert
        kind="info"
        compact
        title={t('noPack.mesmoArquivo', { name: report.inPack?.name ?? '' })}
      />
    );
  } else if (blocked) {
    body = (
      <Alert
        kind="danger"
        title={t('bloqueio.titulo')}
        actionsBelow
        actions={
          <Button
            size="sm"
            loading={adding}
            onClick={() => {
              setAccepted(true);
              // Sem achado, "Adicionar mesmo assim" já grava (CA-T08-05); com achado, a
              // escolha entre referência e arquivo local continua com o usuário.
              if (match === null && !nameTaken) add(true);
            }}
          >
            {t('bloqueio.mesmoAssim')}
          </Button>
        }
      >
        {report.incompatible.map((problem) => (
          <IncompatibilityLine key={problem.kind} problem={problem} />
        ))}
        <p>{t('bloqueio.texto')}</p>
      </Alert>
    );
  } else {
    const localLabel = link === null ? t('identificado.comoArquivo') : t('identificado.comoLink');
    body = (
      <div className="stack-2">
        {accepted && report.incompatible.length > 0 ? (
          <p className="t-xs t-3">{t('bloqueio.aceito')}</p>
        ) : null}
        {match ? (
          <Alert
            kind="ok"
            title={t(
              match.source === 'modrinth'
                ? 'identificado.tituloModrinth'
                : 'identificado.tituloCurseforge',
              { name: match.title, version: match.versionNumber },
            )}
            actionsBelow
            actions={
              <>
                <Button
                  size="sm"
                  variant="primary"
                  icon={Link2}
                  onClick={() => {
                    onReference(match);
                  }}
                >
                  {referenceLabel(match)}
                </Button>
                {nameTaken ? null : (
                  <Button
                    size="sm"
                    loading={adding}
                    onClick={() => {
                      add(accepted);
                    }}
                  >
                    {localLabel}
                  </Button>
                )}
              </>
            }
          >
            <p>{t('identificado.texto')}</p>
            {match.compatible ? null : <p>{t('bloqueio.referenciaIncompativel', { alvo })}</p>}
          </Alert>
        ) : (
          <Alert
            kind="neutral"
            title={t('identificado.naoAchado')}
            actionsBelow
            actions={
              nameTaken ? undefined : (
                <Button
                  size="sm"
                  variant="primary"
                  icon={Plus}
                  loading={adding}
                  onClick={() => {
                    add(accepted);
                  }}
                >
                  {link === null
                    ? t('identificado.adicionarArquivo', { file: file.fileName })
                    : t('identificado.adicionarLink')}
                </Button>
              )
            }
          >
            <p>
              {link === null
                ? t('identificado.naoAchadoTexto', { pasta: folderOf(kind) })
                : t('identificado.naoAchadoLinkTexto')}
            </p>
          </Alert>
        )}
        {report.lookupFailed.length > 0 ? (
          <p className="t-xs t-3">
            {t('identificado.falhou', {
              fontes: report.lookupFailed.map((source) => t(`fonte.${source}`)).join(' e '),
            })}
          </p>
        ) : null}
        {nameTaken ? (
          <Alert
            kind="warn"
            compact
            title={t('noPack.mesmoNome', { path: report.inPack?.path ?? '' })}
          />
        ) : null}
        {link !== null && report.sideNote === 'unknown' ? (
          <p className="t-xs t-3">{t('lado.desconhecido')}</p>
        ) : null}
      </div>
    );
  }

  return (
    <li className="local-file panel" aria-labelledby={`${id}-name`}>
      <div className="row row--top row--between">
        <div className="row row--top local-file__head">
          <Icon icon={link === null ? FileArchive : Link2} />
          <div className="stack-2">
            <p className="local-file__name" id={`${id}-name`}>
              {file.fileName}
            </p>
            <p className="t-xs t-3">
              {[
                formatBytes(report.size),
                link === null ? null : t('arquivo.doLink', { host: linkHost }),
                info ? [info.name ?? info.id, info.version].filter(Boolean).join(' ') : null,
                info && info.loaders.length > 0
                  ? t('arquivo.loaders', { loaders: loaderList(info.loaders) })
                  : null,
                info?.minecraft ? t('arquivo.minecraft', { range: info.minecraft }) : null,
              ]
                .filter(Boolean)
                .join(' · ')}
            </p>
          </div>
        </div>
        {addedPath === null ? (
          <Button
            size="sm"
            variant="ghost"
            icon={X}
            aria-label={t('arquivo.remover', { file: file.fileName })}
            onClick={onRemove}
          />
        ) : (
          <Icon icon={CircleCheck} />
        )}
      </div>
      {report.archive ? null : (
        <Alert kind="warn" compact title={t('arquivo.naoEZip')} className="preview__section" />
      )}
      {addedPath === null && !sameFile ? (
        <label className="field field--inline preview__section">
          <span className="field__label t-sm">{t('arquivo.tipo')}</span>
          <select
            className="select select--sm"
            value={kind}
            onChange={(event) => {
              const next = KINDS.find((value) => value === event.target.value);
              if (next) setKind(next);
            }}
          >
            {KINDS.map((value) => (
              <option key={value} value={value}>
                {t(`arquivo.tipoOpcoes.${value}`)}
              </option>
            ))}
          </select>
        </label>
      ) : null}
      <div className="preview__section">{body}</div>
      {errorText ? (
        <Alert kind="danger" compact title={errorText} className="preview__section" />
      ) : null}
    </li>
  );
}

/** A pasta do pack de cada tipo. */
export function folderOf(kind: ProjectKind): string {
  switch (kind) {
    case 'mod':
      return 'mods';
    case 'resourcePack':
      return 'resourcepacks';
    case 'shader':
      return 'shaderpacks';
    case 'modpack':
      return 'modpacks';
  }
}
