import type {ReactNode} from 'react';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';

import styles from './index.module.css';

function HomepageHeader() {
  return (
    <header className={styles.heroBanner}>
      <div className={styles.hero}>
        {/* Left: Brand + Tagline + CTA */}
        <div className={styles.heroContent}>
          <h1 className={styles.mainTitle}>Vectorless</h1>
          <p className={styles.tagline}>Knowing by reasoning, not vectors.</p>
          <p className={styles.subTitle}>
            Deep and reliable. Vectorless plays nicely with your documents.
            Ask questions in plain language; get answers by reasoning with Vectorless.
          </p>

          <div className={styles.heroActions}>
            <Link
              className={styles.primaryButton}
              to="/docs/getting-started">
              Get Started
            </Link>
            <Link
              className={styles.secondaryButton}
              href="https://github.com/vectorlessflow/vectorless"
              target="_blank"
              rel="noopener noreferrer">
              <svg stroke="currentColor" fill="currentColor" strokeWidth="0" viewBox="0 0 496 512" height="16" width="16" xmlns="http://www.w3.org/2000/svg"><path d="M165.9 397.4c0 2-2.3 3.6-5.2 3.6-3.3.3-5.6-1.3-5.6-3.6 0-2 2.3-3.6 5.2-3.6 3-.3 5.6 1.3 5.6 3.6zm-31.1-4.5c-.7 2 1.3 4.3 4.3 4.9 2.6 1 5.6 0 6.2-2s-1.3-4.3-4.3-5.2c-2.6-.7-5.5.3-6.2 2.3zm44.2-1.7c-2.9.7-4.9 2.6-4.6 4.9.3 2 2.9 3.3 5.9 2.6 2.9-.7 4.9-2.6 4.6-4.6-.3-1.9-3-3.2-5.9-2.9zM244.8 8C106.1 8 0 113.3 0 252c0 110.9 69.8 205.8 169.5 239.2 12.8 2.3 17.3-5.6 17.3-12.1 0-6.2-.3-40.4-.3-61.4 0 0-70 15-84.7-29.8 0 0-11.4-29.1-27.8-36.6 0 0-22.9-15.7 1.6-15.4 0 0 24.9 2 38.6 25.8 21.9 38.6 58.6 27.5 72.9 20.9 2.3-16 8.8-27.1 16-33.7-55.9-6.2-112.3-14.3-112.3-110.5 0-27.5 7.6-41.3 23.6-58.9-2.6-6.5-11.1-33.3 2.6-67.9 20.9-6.5 69 27 69 27 20-5.6 41.5-8.5 62.8-8.5s42.8 2.9 62.8 8.5c0 0 48.1-33.6 69-27 13.7 34.7 5.2 61.4 2.6 67.9 16 17.7 25.8 31.5 25.8 58.9 0 96.5-58.9 104.2-114.8 110.5 9.2 7.9 17 22.9 17 46.4 0 33.7-.3 75.4-.3 83.6 0 6.5 4.6 14.4 17.3 12.1C428.2 457.8 496 362.9 496 252 496 113.3 383.5 8 244.8 8zM97.2 352.9c-1.3 1-1 3.3.7 5.2 1.6 1.6 3.9 2.3 5.2 1 1.3-1 1-3.3-.7-5.2-1.6-1.6-3.9-2.3-5.2-1zm-10.8-8.1c-.7 1.3.3 2.9 2.3 3.9 1.6 1 3.6.7 4.3-.7.7-1.3-.3-2.9-2.3-3.9-2-.6-3.6-.3-4.3.7zm32.4 35.6c-1.6 1.3-1 4.3 1.3 6.2 2.3 2.3 5.2 2.6 6.5 1 1.3-1.3.7-4.3-1.3-6.2-2.2-2.3-5.2-2.6-6.5-1zm-11.4-14.7c-1.6 1-1.6 3.6 0 5.9 1.6 2.3 4.3 3.3 5.6 2.3 1.6-1.3 1.6-3.9 0-6.2-1.4-2.3-4-3.3-5.6-2z"></path></svg>
              GitHub
            </Link>
          </div>
        </div>

        {/* Right: Code Preview */}
        <div className={styles.codeCard}>
          <div className={styles.codeHeader}>
            <span className={styles.codeDot} style={{background: '#FF5F57'}} />
            <span className={styles.codeDot} style={{background: '#FEBC2E'}} />
            <span className={styles.codeDot} style={{background: '#28C840'}} />
            <span className={styles.codeTitle}>quick_start.py</span>
          </div>
          <pre className={styles.codeBlock}><code dangerouslySetInnerHTML={{__html: CODE_HTML}} /></pre>
        </div>
      </div>
    </header>
  );
}

const CODE_HTML = [
  `<span class="kw">import</span> asyncio`,
  `<span class="kw">from</span> vectorless <span class="kw">import</span> Engine`,
  ``,
  `<span class="kw">async def</span> <span class="fn">main</span>():`,
  `    engine = <span class="fn">Engine</span>(`,
  `        api_key=<span class="str">"sk-..."</span>,`,
  `        model=<span class="str">"gpt-4o"</span>,`,
  `    )`,
  ``,
  `    <span class="cmt"># Compile a document</span>`,
  `    result = <span class="kw">await</span> engine.<span class="fn">compile</span>(`,
  `        path=<span class="str">"./report.pdf"</span>`,
  `    )`,
  ``,
  `    <span class="cmt"># Ask a question</span>`,
  `    response = <span class="kw">await</span> engine.<span class="fn">ask</span>(`,
  `        <span class="str">"What is the total revenue?"</span>,`,
  `        doc_ids=[result.doc_id],`,
  `    )`,
  `    <span class="fn">print</span>(response.<span class="fn">single</span>().content)`,
  ``,
  `asyncio.<span class="fn">run</span>(<span class="fn">main</span>())`,
].join('\n');

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout
      title={`${siteConfig.title}`}
      description="Document understanding engine for AI. Agents reason through your documents — navigating structure, reading passages, cross-referencing across sections.">
      <HomepageHeader />
      <main />
    </Layout>
  );
}
