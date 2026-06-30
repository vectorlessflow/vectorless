import type {ReactNode} from 'react';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';

import styles from './index.module.css';

type Tok = [cls: string | null, text: string];

function Terminal(): ReactNode {
  const s = styles;
  const lines: Tok[][] = [
    [[s.cPrompt, '›'], [null, ' '], [s.cCmd, 'ls']],
    [[s.cOut, '  n1/  Executive Summary']],
    [[s.cOut, '  n2/  Financials']],
    [[s.cOut, '  n3/  Risk Factors']],
    [[s.cOut, '  n4/  Appendix']],
    [],
    [[s.cPrompt, '›'], [null, ' '], [s.cCmd, 'cd'], [null, ' n2   '], [s.cComment, '# revenue is under Financials']],
    [[s.cPrompt, '›'], [null, ' '], [s.cCmd, 'ls']],
    [[s.cOut, '  n2.1  Overview']],
    [[s.cOut, '  n2.4  Income Statement']],
    [[s.cOut, '  n2.7  Cash Flow']],
    [],
    [[s.cPrompt, '›'], [null, ' '], [s.cCmd, 'grep'], [null, ' '], [s.cStr, '"total revenue"']],
    [[s.cOut, '  n2.4  Income Statement → line 12']],
    [],
    [[s.cPrompt, '›'], [null, ' '], [s.cCmd, 'cat'], [null, ' n2.4']],
    [[s.cOut, '  Revenue (FY2025) ..... '], [s.cHit, '$4.82B']],
    [[s.cOut, '  Operating margin ..... '], [s.cHit, '23.4%']],
    [[s.cOut, '  Net income ........... '], [s.cHit, '$1.10B']],
    [],
    [[s.cPrompt, '›'], [null, ' '], [s.cCmd, 'head'], [null, ' n2.4']],
    [[s.cOut, '  Revenue grew '], [s.cHit, '19% YoY'], [s.cOut, ', led by cloud.']],
    [],
    [[s.cAnswer, '  ✓ answer'], [s.cOut, '  Total revenue was '], [s.cHit, '$4.82B'], [s.cOut, ', up 19% YoY.']],
    [[s.cAnswer, '  ✓ source'], [s.cOut, '  n2.4 · line 12']],
    [],
    [[s.cPrompt, '›'], [null, ' '], [s.cCursor, '▋']],
  ];

  return (
    <div className={styles.terminalWrap}>
      <div className={styles.terminal}>
        <div className={styles.termBar}>
          <span className={styles.dot} /><span className={styles.dot} /><span className={styles.dot} />
          <span className={styles.termTitle}>agent · navigating report.pdf</span>
        </div>
        <div className={styles.termBody}>
          {lines.map((toks, i) => (
            <div className={styles.termLine} key={i}>
              {toks.length === 0
                ? ' '
                : toks.map(([cls, text], j) => (
                    <span className={cls ?? undefined} key={j}>{text}</span>
                  ))}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout
      title={`${siteConfig.title}`}
      description="Knowing by reasoning, not vectors. AI document understanding without embeddings.">
      <header className={styles.heroBanner}>
        <div className={styles.heroContent}>

          {/* ── Hero ── */}
          <div className={styles.hero}>
            <div className={styles.heroLeft}>
              <div className={styles.badge}>
                <span className={styles.badgeDot} /> Document Understanding Engine for AI
              </div>
              <h1 className={styles.mainTitle}>
                Reason,<br /><span className={styles.grad}>don&apos;t vector.</span>
              </h1>
              <p className={styles.tagline}>
                Deep and reliable. Vectorless plays nicely with your documents.
                Ask questions in plain language; get answers by <em>reasoning</em> with Vectorless.
              </p>

              <div className={styles.install}>
                <span className={styles.installPrompt}>$</span>
                <code>pip install vectorless</code>
              </div>

              <div className={styles.heroActions}>
                <Link to="/docs/getting-started" className={styles.primaryButton}>
                  Get Started
                </Link>
                <Link
                  className={styles.secondaryButton}
                  href="https://github.com/vectorlessflow/vectorless"
                  target="_blank"
                  rel="noopener noreferrer">
                  <svg stroke="currentColor" fill="currentColor" strokeWidth="0" viewBox="0 0 496 512" height="16" width="16" xmlns="http://www.w3.org/2000/svg"><path d="M165.9 397.4c0 2-2.3 3.6-5.2 3.6-3.3.3-5.6-1.3-5.6-3.6 0-2 2.3-3.6 5.2-3.6 3-.3 5.6 1.3 5.6 3.6zm-31.1-4.5c-.7 2 1.3 4.3 4.3 4.9 2.6 1 5.6 0 6.2-2s-1.3-4.3-4.3-5.2c-2.6-.7-5.5.3-6.2 2.3zm44.2-1.7c-2.9.7-4.9 2.6-4.6 4.9.3 2 2.9 3.3 5.9 2.6 2.9-.7 4.9-2.6 4.6-4.6-.3-1.9-3-3.2-5.9-2.9zM244.8 8C106.1 8 0 113.3 0 252c0 110.9 69.8 205.8 169.5 239.2 12.8 2.3 17.3-5.6 17.3-12.1 0-6.2-.3-40.4-.3-61.4 0 0-70 15-84.7-29.8 0 0-11.4-29.1-27.8-36.6 0 0-22.9-15.7 1.6-15.4 0 0 24.9 2 38.6 25.8 21.9 38.6 58.6 27.5 72.9 20.9 2.3-16 8.8-27.1 16-33.7-55.9-6.2-112.3-14.3-112.3-110.5 0-27.5 7.6-41.3 23.6-58.9-2.6-6.5-11.1-33.3 2.6-67.9 20.9-6.5 69 27 69 27 20-5.6 41.5-8.5 62.8-8.5s42.8 2.9 62.8 8.5c0 0 48.1-33.6 69-27 13.7 34.7 5.2 61.4 2.6 67.9 16 17.7 25.8 31.5 25.8 58.9 0 96.5-58.9 104.2-114.8 110.5 9.2 7.9 17 22.9 17 46.4 0 33.7-.3 75.4-.3 83.6 0 6.5 4.6 14.4 17.3 12.1C428.2 457.8 496 362.9 496 252 496 113.3 383.5 8 244.8 8z"></path></svg>
                  Star on GitHub
                </Link>
              </div>
            </div>

            <div className={styles.heroRight}>
              <Terminal />
            </div>
          </div>

          {/* ── Demo ── */}
          <section className={styles.section}>
            <div className={styles.sectionHead}>
              <h2 className={styles.sectionTitle}>Watch it reason</h2>
              <p className={styles.sectionSub}>
                Ask in plain language. Vectorless navigates the document tree and answers
                with sources — no embeddings in sight.
              </p>
            </div>
            <div className={styles.demoFrame}>
              <div className={styles.demoBar}>
                <span className={styles.dot} /><span className={styles.dot} /><span className={styles.dot} />
                <span className={styles.demoTitle}>vectorless · ask</span>
              </div>
              <img
                className={styles.demoImg}
                src={useBaseUrl('img/demo.gif')}
                alt="Vectorless answering a question by navigating a document"
                loading="lazy"
              />
            </div>
          </section>

          {/* ── Architecture ── */}
          <section className={styles.section}>
            <div className={styles.sectionHead}>
              <h2 className={styles.sectionTitle}>Inside the engine</h2>
              <p className={styles.sectionSub}>
                From raw file to grounded answer — compile to a semantic tree, then reason over it.
                No vectors anywhere in the pipeline.
              </p>
            </div>
            <div className={styles.archFrame}>
              <img
                className={styles.archImg}
                src={useBaseUrl('img/workflow.svg')}
                alt="Vectorless architecture: file → tree index → reasoning retrieval → answer"
                loading="lazy"
              />
            </div>
          </section>

          {/* ── CTA band ── */}
          <section className={styles.cta}>
            <div className={styles.ctaInner}>
              <h2 className={styles.ctaTitle}>
                Stop embedding.<br /><span className={styles.grad}>Start reasoning.</span>
              </h2>
              <p className={styles.ctaSub}>
                Point Vectorless at your documents and ask in plain language. Every answer
                comes back with a traceable source.
              </p>
              <div className={styles.heroActions}>
                <Link to="/docs/getting-started" className={styles.primaryButton}>Get Started</Link>
                <Link className={styles.secondaryButton} to="/docs/getting-started">Read the docs</Link>
              </div>
            </div>
          </section>

        </div>
      </header>
      <main />
    </Layout>
  );
}
