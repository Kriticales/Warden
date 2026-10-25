/** A página de Configurações aberta pelo endereço `/configuracoes/<página>`. */
import type { SecretKind } from '../../../lib/ipc/bindings';
import { AboutWarden } from '../../about/components/AboutWarden';
import type { SettingsPageId } from '../pages';
import { StorageSection } from '../storage/components/StorageSection';
import { EditorPage } from './EditorPage';
import { GeneralPage } from './GeneralPage';
import { KeysPage } from './KeysPage';
import { LogsPage } from './LogsPage';
import { SculkPage } from './SculkPage';
import { TestPage } from './TestPage';

export function SettingsPageView({
  page,
  search,
}: {
  page: SettingsPageId;
  search: { chave?: SecretKind; substituir?: boolean };
}) {
  switch (page) {
    case 'geral':
      return <GeneralPage />;
    case 'chaves':
      return <KeysPage selected={search.chave} replace={search.substituir} />;
    case 'sensor-sculk':
      return <SculkPage />;
    case 'teste':
      return <TestPage />;
    case 'editor':
      return <EditorPage />;
    case 'registros':
      return <LogsPage />;
    case 'armazenamento':
      return <StorageSection />;
    case 'sobre':
      return <AboutWarden />;
  }
}
