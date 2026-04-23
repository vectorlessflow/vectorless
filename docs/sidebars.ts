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
