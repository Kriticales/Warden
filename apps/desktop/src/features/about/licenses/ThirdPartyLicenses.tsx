/**
 * "Licenças de terceiros" (A-02; SPEC T23; ADR-0004): link em "Sobre o Warden" que abre um
 * diálogo com os créditos (Forge, packwiz, motor do launcher) e a lista gerada por
 * `cargo xtask notices`, agrupada (Rust, JavaScript, packwiz, fontes), com filtro e o texto de
 * cada licença.
 *
 * A lista só é lida quando o diálogo abre, e cada grupo só monta as linhas quando é aberto: são
 * centenas de componentes.
 */
import { useQuery } from '@tanstack/react-query';
import { ChevronDown, ScrollText } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../../../components/common/EmptyState';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent, DialogTrigger } from '../../../components/ui/dialog';
import { Icon } from '../../../components/ui/icon';
import {
  countItems,
  filterNotices,
  hasItem,
  loadNotices,
  type NoticeGroup,
  type NoticeItem,
  type Notices,
} from './notices';

/** Crate do motor do launcher (spike S1): o crédito só aparece quando ela foi compilada. */
const LAUNCHER_ENGINE_CRATE = 'portablemc';

export interface ThirdPartyLicensesProps {
  /** Leitura da lista (os testes trocam). Padrão: o arquivo gerado. */
  load?: () => Promise<Notices | null>;
}

export function ThirdPartyLicenses({ load = loadNotices }: ThirdPartyLicensesProps) {
  const { t } = useTranslation('licencas');
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="link" icon={ScrollText}>
          {t('abrir')}
        </Button>
      </DialogTrigger>
      {open ? <LicensesDialog load={load} /> : null}
    </Dialog>
  );
}

function LicensesDialog({ load }: { load: () => Promise<Notices | null> }) {
  const { t } = useTranslation('licencas');
  const query = useQuery({
    queryKey: ['third-party-notices'], // Sem repassar o contexto do TanStack Query para quem lê.
    queryFn: () => load(),
    staleTime: Infinity,
  });

  let body;
  if (query.isPending) {
    body = <LoadingState label={t('carregando')} />;
  } else if (query.isError) {
    body = (
      <ErrorPanel
        compact
        error={query.error}
        onRetry={() => {
          void query.refetch();
        }}
      />
    );
  } else if (query.data === null) {
    body = (
      <EmptyState
        compact
        kind="error"
        glyph="x"
        headingLevel={3}
        title={t('naoGeradaTitulo')}
        text={t('naoGeradaTexto')}
      />
    );
  } else {
    body = <LicensesBody notices={query.data} />;
  }

  return (
    <DialogContent
      size="lg"
      title={t('titulo')}
      description={query.data ? t('descricao', { count: countItems(query.data) }) : undefined}
      footer={
        <DialogClose asChild>
          <Button>{t('fechar')}</Button>
        </DialogClose>
      }
    >
      {body}
    </DialogContent>
  );
}

function LicensesBody({ notices }: { notices: Notices }) {
  const { t } = useTranslation('licencas');
  const creditsId = useId();
  const filterId = useId();
  const [filter, setFilter] = useState('');
  const groups = filterNotices(notices.groups, filter);
  const filtering = filter.trim() !== '';

  return (
    <div className="stack-6">
      <section aria-labelledby={creditsId}>
        <h3 className="group-title" id={creditsId}>
          {t('creditosTitulo')}
        </h3>
        <ul className="stack-2 t-sm t-2">
          <li>{t('creditoForge')}</li>
          <li>{t('creditoPackwiz')}</li>
          {hasItem(notices, 'rust', LAUNCHER_ENGINE_CRATE) ? (
            <li>{t('creditoPortablemc')}</li>
          ) : null}
        </ul>
      </section>
      <div>
        <div className="field">
          <label className="field__label" htmlFor={filterId}>
            {t('filtro')}
          </label>
          <input
            id={filterId}
            className="input"
            type="search"
            autoComplete="off"
            spellCheck={false}
            value={filter}
            onChange={(event) => {
              setFilter(event.target.value);
            }}
          />
        </div>
        <div className="mt-3">
          {groups.length === 0 ? (
            <p className="t-sm t-3" role="status">
              {t('semResultado', { filtro: filter.trim() })}
            </p>
          ) : (
            groups.map((group) => (
              <LicenseGroup
                key={group.id}
                group={group}
                texts={notices.texts}
                forceOpen={filtering}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}

function LicenseGroup({
  group,
  texts,
  forceOpen,
}: {
  group: NoticeGroup;
  texts: string[];
  forceOpen: boolean;
}) {
  const { t } = useTranslation('licencas');
  const headId = useId();
  const bodyId = useId();
  const [open, setOpen] = useState(false);
  const expanded = open || forceOpen;
  return (
    <section className="listgroup" aria-labelledby={headId}>
      <div className="listgroup__head">
        <button
          type="button"
          className="listgroup__toggle"
          id={headId}
          aria-expanded={expanded}
          aria-controls={bodyId}
          onClick={() => {
            setOpen(!expanded);
          }}
        >
          <Icon icon={ChevronDown} />
          {t(`grupos.${group.id}`)}
        </button>
        <span className="t-3 t-sm">{t('grupoContagem', { count: group.items.length })}</span>
      </div>
      <ul id={bodyId} className="stack-3" hidden={!expanded}>
        {expanded
          ? group.items.map((item) => (
              <LicenseItem key={`${item.name}@${item.version}`} item={item} texts={texts} />
            ))
          : null}
      </ul>
    </section>
  );
}

function LicenseItem({ item, texts }: { item: NoticeItem; texts: string[] }) {
  const { t } = useTranslation('licencas');
  return (
    <li className="t-sm">
      <div className="row row--wrap">
        <span className="t-strong">{item.name}</span>
        {item.version ? <span className="t-mono t-3">{item.version}</span> : null}
        <span className="t-2">{item.license || t('licencaNaoIdentificada')}</span>
      </div>
      {item.texts.length > 0 ? (
        <details className="disclosure">
          <summary>{t('verLicenca')}</summary>
          <div className="stack-2">
            {item.texts.map((index) => (
              <pre key={index} className="code code--scroll">
                {texts[index]}
              </pre>
            ))}
          </div>
        </details>
      ) : (
        <p className="t-xs t-3">{t('semTexto')}</p>
      )}
    </li>
  );
}
