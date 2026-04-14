import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

const config: Config = {
  title: 'Vectorless',
  tagline: 'Reasoning-native Document Intelligence Engine',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  url: 'https://vectorless.dev',
  baseUrl: '/',

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
        {to: '/docs/intro', label: 'Documentation', position: 'left'},
        // {
        //   href: 'https://github.com/vectorlessflow/vectorless/tree/main/examples',
        //   label: 'Examples',
        //   position: 'left',
        //   target: '_self',
        // },
        {to: '/blog', label: 'Blog', position: 'left'},
      ],
    },
    footer: {
      style: 'light',
      links: [
        {
          title: 'Product',
          items: [
            {
              label: 'Getting Started',
              to: '/docs/intro',
            },
            {
              label: 'Documentation',
              to: '/docs/intro',
            },
            {
              label: 'Blog',
              to: '/blog',
            },
          ],
        },
        {
          title: 'Integrations',
          items: [
            {
              label: 'Python SDK',
              href: 'https://pypi.org/project/vectorless/',
            },
            {
              label: 'Rust Crate',
              href: 'https://crates.io/crates/vectorless',
            },
            {
              label: 'API Reference',
              href: 'https://docs.rs/vectorless',
            },
          ],
        },
        {
          title: 'Community',
          items: [
            {
              label: 'GitHub',
              href: 'https://github.com/vectorlessflow/vectorless',
            },
            {
              label: 'Report a Bug',
              href: 'https://github.com/vectorlessflow/vectorless/issues',
            },
            {
              label: 'Apache 2.0 License',
              href: 'https://github.com/vectorlessflow/vectorless/blob/main/LICENSE',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Vectorless`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'toml', 'python'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
