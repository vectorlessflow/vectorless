import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  tutorialSidebar: [
    'intro',
    'getting-started',
    'architecture',
    {
      type: 'category',
      label: 'Indexing',
      items: [
        'indexing/overview',
        'indexing/configuration',
        'indexing/incremental',
      ],
    },
    {
      type: 'category',
      label: 'Retrieval',
      items: [
        'retrieval/overview',
        'retrieval/strategies',
        'retrieval/search-algorithms',
        'retrieval/cross-references',
      ],
    },
    {
      type: 'category',
      label: 'Features',
      items: [
        'features/summary-strategies',
        'features/synonym-expansion',
        'features/cross-document-graph',
        'features/pdf-support',
      ],
    },
    {
      type: 'category',
      label: 'SDK',
      items: [
        'sdk/python',
        'sdk/rust',
      ],
    },
    {
      type: 'category',
      label: 'Examples',
      items: [
        'examples/quick-query',
        'examples/multi-document',
        'examples/batch-indexing',
      ],
    },
  ],
};

export default sidebars;
