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
    image: 'img/docusaurus-social-card.jpg',
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
        {
          type: 'docSidebar',
          sidebarId: 'tutorialSidebar',
          position: 'left',
          label: 'Docs',
        },
        {to: '/blog', label: 'Blog', position: 'left'},
        {
          href: 'https://crates.io/crates/vectorless',
          label: 'Crates.io',
          position: 'right',
        },
        {
          href: 'https://pypi.org/project/vectorless/',
          label: 'PyPI',
          position: 'right',
        },
        {
          href: 'https://github.com/vectorlessflow/vectorless',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Docs',
          items: [
            {
              label: 'Getting Started',
              to: '/docs/intro',
            },
          ],
        },
        {
          title: 'Packages',
          items: [
            {
              label: 'Rust (crates.io)',
              href: 'https://crates.io/crates/vectorless',
            },
            {
              label: 'Python (PyPI)',
              href: 'https://pypi.org/project/vectorless/',
            },
          ],
        },
        {
          title: 'More',
          items: [
            {
              label: 'Blog',
              to: '/blog',
            },
            {
              label: 'GitHub',
              href: 'https://github.com/vectorlessflow/vectorless',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} vectorlessflow. Licensed under Apache-2.0.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'toml', 'python'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
