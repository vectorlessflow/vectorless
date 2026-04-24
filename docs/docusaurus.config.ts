import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

const config: Config = {
  title: 'Vectorless',
  tagline: 'Document Understanding Engine for AI',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  url: 'https://vectorless.dev',
  baseUrl: '/',

  stylesheets: [
    'https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.1/css/all.min.css',
    'https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap',
  ],

  organizationName: 'vectorlessflow',
  projectName: 'vectorless',

  onBrokenLinks: 'throw',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl:
            'https://github.com/vectorlessflow/vectorless/tree/main/docs/',
        },
        blog: {
          showReadingTime: true,
          feedOptions: {
            type: ['rss', 'atom'],
            xslt: true,
          },
          editUrl:
            'https://github.com/vectorlessflow/vectorless/tree/main/docs/',
          onInlineTags: 'warn',
          onInlineAuthors: 'warn',
          onUntruncatedBlogPosts: 'warn',
        },
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: 'img/theme-logo.png',
    colorMode: {
      defaultMode: 'dark',
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Vectorless',
      logo: {
        alt: 'Vectorless Logo',
        src: 'img/logo.png',
        href: 'https://vectorless.dev',
        target: '_self' // This makes the logo click follow the link in the same window
      },
      items: [
        {to: '/docs/getting-started', label: 'Documentation', position: 'left'},
        {href: 'https://github.com/vectorlessflow/vectorless', label: 'GitHub', position: 'left'},
      ],
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'toml', 'python'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
