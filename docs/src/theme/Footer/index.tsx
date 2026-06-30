import React from 'react';
import Link from '@docusaurus/Link';
import useBaseUrl from '@docusaurus/useBaseUrl';
import styles from './styles.module.css';

const COLUMNS = [
  {
    title: 'Products',
    links: [
      {label: 'vectorless', href: 'https://github.com/vectorlessflow/vectorless'},
      {label: 'vectorless-code', href: 'https://github.com/vectorlessflow/vectorless-code'},
      {label: 'PyPI package', href: 'https://pypi.org/project/vectorless/'},
    ],
  },
  {
    title: 'Resources',
    links: [
      {label: 'Getting Started', to: '/docs/getting-started'},
      {label: 'Documentation', to: '/docs/getting-started'},
      {label: 'Blog', to: '/blog'},
    ],
  },
  {
    title: 'Community',
    links: [
      {label: 'GitHub', href: 'https://github.com/vectorlessflow/vectorless'},
      {label: 'PyPI', href: 'https://pypi.org/project/vectorless/'},
      {label: 'Issues', href: 'https://github.com/vectorlessflow/vectorless/issues'},
    ],
  },
];

function FooterLink({link}: {link: {label: string; to?: string; href?: string}}): React.ReactElement {
  if (link.href) {
    return (
      <a className={styles.link} href={link.href} target="_blank" rel="noopener noreferrer">
        {link.label}
      </a>
    );
  }
  return (
    <Link className={styles.link} to={link.to!}>
      {link.label}
    </Link>
  );
}

export default function Footer(): React.ReactElement {
  const year = new Date().getFullYear();
  return (
    <footer className={styles.footer}>
      <div className={styles.glow} />
      <div className={styles.inner}>
        <div className={styles.top}>
          <div className={styles.brandCol}>
            <div className={styles.brand}>
              <img className={styles.logo} src={useBaseUrl('img/logo.svg')} alt="Vectorless" />
              <span className={styles.wordmark}>vector<span className={styles.wordmarkAccent}>less</span></span>
            </div>
            <p className={styles.tagline}>Knowing by reasoning, not vectors.</p>
            <div className={styles.install}>
              <span className={styles.prompt}>$</span> pip install vectorless
            </div>
          </div>

          <div className={styles.cols}>
            {COLUMNS.map((col) => (
              <div className={styles.col} key={col.title}>
                <div className={styles.colTitle}>{col.title}</div>
                {col.links.map((link) => (
                  <FooterLink link={link} key={link.label} />
                ))}
              </div>
            ))}
          </div>
        </div>

        <div className={styles.bottom}>
          <span className={styles.copy}>© {year} Vectorless · Apache License 2.0</span>
          <a
            className={styles.ghost}
            href="https://github.com/vectorlessflow/vectorless"
            target="_blank"
            rel="noopener noreferrer">
            Built in the open ↗
          </a>
        </div>
      </div>

      <div className={styles.watermark} aria-hidden="true">vectorless</div>
    </footer>
  );
}
