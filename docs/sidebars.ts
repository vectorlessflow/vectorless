import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  tutorialSidebar: [
    {
      type: 'category',
      label: 'Get Started',
      items: [
        'getting-started',
        'installation',
      ],
    },
    'architecture',
    {
      type: 'category',
      label: 'Vectorless Compiler',
      items: [
        'compiler/overview',
        'compiler/ir-spec',
        'compiler/pipeline',
        'compiler/passes',
        'compiler/configuration',
        'compiler/incremental',
        'compiler/checkpoint',
        'compiler/custom-pass',
        'compiler/parsers',
        'compiler/standalone-usage',
      ],
    },
    {
      type: 'category',
      label: 'RFC',
      items: [
        'rfc/router',
        'rfc/roadmap',
      ],
    },
    'api-reference',
  ],
};

export default sidebars;
