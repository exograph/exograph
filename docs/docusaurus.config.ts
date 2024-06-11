import type { Config } from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'Exograph',
  tagline: 'Declarative Backends in Minutes',
  favicon: 'favicon.ico',

  url: 'https://exograph.dev',
  baseUrl: '/',
  trailingSlash: false,

  organizationName: 'exograph', // Usually your GitHub org/user name.
  projectName: 'exograph', // Usually your repo name.

  onBrokenLinks: 'throw',
  onBrokenMarkdownLinks: 'warn',

  staticDirectories: ['docs/static', 'static'],

  presets: [
    [
      'classic',
      {
        docs: {
          routeBasePath: '/',
          sidebarPath: './sidebars.ts',
          // editUrl: 'https://github.com/exograph/exograph-docs/edit/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: 'exograph.png',
    docs: {
      sidebar: {
        autoCollapseCategories: true,
      },
    },
    navbar: {
      title: 'Exograph',
      logo: {
        alt: 'Exograph Logo',
        src: 'logo-light.svg',
        srcDark: 'logo-dark.svg',
      },
      items: [
      ],
    },
    footer: {
      links: [
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Exograph, Inc. Built with Docusaurus.`,
    },
    prism: {
      theme: require("./src/theme/prism/light"),
      darkTheme: require("./src/theme/prism/dark"),
      additionalLanguages: ['rust', "shell-session", "toml"],
      magicComments: [
        {
          className: 'theme-code-block-highlighted-line',
          line: 'highlight-next-line',
          block: { start: 'highlight-start', end: 'highlight-end' },
        },
        {
          className: 'shell-command-line',
          line: 'shell-command-next-line',
        }
      ],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
