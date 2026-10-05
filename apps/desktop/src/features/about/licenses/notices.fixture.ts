/** Avisos de terceiros de exemplo, para os testes desta pasta. */
import { NOTICES_SCHEMA, type Notices } from './notices';

export function makeNotices(): Notices {
  return {
    schema: NOTICES_SCHEMA,
    texts: ['MIT License\n\nPermission is hereby granted', 'Apache License\nVersion 2.0'],
    groups: [
      {
        id: 'rust',
        items: [
          { name: 'serde', version: '1.0.228', license: 'MIT OR Apache-2.0', texts: [0, 1] },
          { name: 'portablemc', version: '5.0.5', license: 'Apache-2.0', texts: [1] },
        ],
      },
      {
        id: 'npm',
        items: [
          {
            name: 'react',
            version: '19.3.0',
            license: 'MIT',
            url: 'https://react.dev',
            texts: [0],
          },
          { name: 'sem-texto', version: '1.0.0', license: '', texts: [] },
        ],
      },
      {
        id: 'packwiz',
        items: [{ name: 'packwiz', version: 'ef87d964f8cb', license: 'MIT', texts: [0] }],
      },
      {
        id: 'fontes',
        items: [{ name: 'Manrope', version: '', license: 'OFL-1.1', texts: [0] }],
      },
    ],
  };
}
