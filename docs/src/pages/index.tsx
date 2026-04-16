import type {ReactNode} from 'react';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import Link from '@docusaurus/Link';

import styles from './index.module.css';

function HomepageHeader() {
  return (
    <header className={styles.heroBanner}>
      <div className={styles.heroInner}>
        <h1 className={styles.heroTitle}>
          <span className={styles.heroTitleEmphasis}>Reason, </span>
          <span className={styles.heroTitleLight}>don't vector</span>
        </h1>
        <p className={styles.heroTagline}>
          <span className={styles.heroTaglineLine1}>
            <span className={styles.heroTaglineHighlight}>Vectorless</span> will reason through any of your structured documents — <span className={styles.heroTaglineHighlight}>PDFs, Markdown, reports, contracts</span>,
          </span>
          <br />
          <span className={styles.heroTaglineLine2}>and retrieve only what's relevant. <span className={styles.heroTaglineHighlight}>Nothing more, nothing less.</span></span>
        </p>
        <div className={styles.heroActions}>
          <Link
            className={styles.githubStarButton}
            href="https://github.com/vectorlessflow/vectorless"
            target="_blank"
            rel="noopener noreferrer">
            <svg stroke="currentColor" fill="currentColor" strokeWidth="0" viewBox="0 0 496 512" height="22" width="22" xmlns="http://www.w3.org/2000/svg"><path d="M165.9 397.4c0 2-2.3 3.6-5.2 3.6-3.3.3-5.6-1.3-5.6-3.6 0-2 2.3-3.6 5.2-3.6 3-.3 5.6 1.3 5.6 3.6zm-31.1-4.5c-.7 2 1.3 4.3 4.3 4.9 2.6 1 5.6 0 6.2-2s-1.3-4.3-4.3-5.2c-2.6-.7-5.5.3-6.2 2.3zm44.2-1.7c-2.9.7-4.9 2.6-4.6 4.9.3 2 2.9 3.3 5.9 2.6 2.9-.7 4.9-2.6 4.6-4.6-.3-1.9-3-3.2-5.9-2.9zM244.8 8C106.1 8 0 113.3 0 252c0 110.9 69.8 205.8 169.5 239.2 12.8 2.3 17.3-5.6 17.3-12.1 0-6.2-.3-40.4-.3-61.4 0 0-70 15-84.7-29.8 0 0-11.4-29.1-27.8-36.6 0 0-22.9-15.7 1.6-15.4 0 0 24.9 2 38.6 25.8 21.9 38.6 58.6 27.5 72.9 20.9 2.3-16 8.8-27.1 16-33.7-55.9-6.2-112.3-14.3-112.3-110.5 0-27.5 7.6-41.3 23.6-58.9-2.6-6.5-11.1-33.3 2.6-67.9 20.9-6.5 69 27 69 27 20-5.6 41.5-8.5 62.8-8.5s42.8 2.9 62.8 8.5c0 0 48.1-33.6 69-27 13.7 34.7 5.2 61.4 2.6 67.9 16 17.7 25.8 31.5 25.8 58.9 0 96.5-58.9 104.2-114.8 110.5 9.2 7.9 17 22.9 17 46.4 0 33.7-.3 75.4-.3 83.6 0 6.5 4.6 14.4 17.3 12.1C428.2 457.8 496 362.9 496 252 496 113.3 383.5 8 244.8 8zM97.2 352.9c-1.3 1-1 3.3.7 5.2 1.6 1.6 3.9 2.3 5.2 1 1.3-1 1-3.3-.7-5.2-1.6-1.6-3.9-2.3-5.2-1zm-10.8-8.1c-.7 1.3.3 2.9 2.3 3.9 1.6 1 3.6.7 4.3-.7.7-1.3-.3-2.9-2.3-3.9-2-.6-3.6-.3-4.3.7zm32.4 35.6c-1.6 1.3-1 4.3 1.3 6.2 2.3 2.3 5.2 2.6 6.5 1 1.3-1.3.7-4.3-1.3-6.2-2.2-2.3-5.2-2.6-6.5-1zm-11.4-14.7c-1.6 1-1.6 3.6 0 5.9 1.6 2.3 4.3 3.3 5.6 2.3 1.6-1.3 1.6-3.9 0-6.2-1.4-2.3-4-3.3-5.6-2z"></path></svg>
            Star on GitHub
            <svg className={styles.starIcon} stroke="currentColor" fill="currentColor" strokeWidth="0" viewBox="0 0 24 24" height="22" width="22" xmlns="http://www.w3.org/2000/svg"><path d="M16.6,20.463a1.5,1.5,0,0,1-.7-.174l-3.666-1.927a.5.5,0,0,0-.464,0L8.1,20.289a1.5,1.5,0,0,1-2.177-1.581l.7-4.082a.5.5,0,0,0-.143-.442L3.516,11.293a1.5,1.5,0,0,1,.832-2.559l4.1-.6a.5.5,0,0,0,.376-.273l1.833-3.714a1.5,1.5,0,0,1,2.69,0l1.833,3.714a.5.5,0,0,0,.376.274l4.1.6a1.5,1.5,0,0,1,.832,2.559l-2.965,2.891a.5.5,0,0,0-.144.442l.7,4.082A1.5,1.5,0,0,1,16.6,20.463Zm-3.9-2.986L16.364,19.4a.5.5,0,0,0,.725-.527l-.7-4.082a1.5,1.5,0,0,1,.432-1.328l2.965-2.89a.5.5,0,0,0-.277-.853l-4.1-.6a1.5,1.5,0,0,1-1.13-.821L12.449,4.594a.516.516,0,0,0-.9,0L9.719,8.308a1.5,1.5,0,0,1-1.13.82l-4.1.6a.5.5,0,0,0-.277.853L7.18,13.468A1.5,1.5,0,0,1,7.611,14.8l-.7,4.082a.5.5,0,0,0,.726.527L11.3,17.477a1.5,1.5,0,0,1,1.4,0Z"></path></svg>
          </Link>
        </div>
      </div>
    </header>
  );
}

function SectionWhy() {
  const items = [
    {
      icon: '\u{1F9E0}',
      title: 'Reasoning-Native',
      desc: 'LLMs navigate hierarchical document trees with semantic understanding \u2014 not vector proximity.',
    },
    {
      icon: '\u{1F5C2}\u{FE0F}',
      title: 'No Vector Database',
      desc: 'Eliminate embedding pipelines, vector stores, and similarity search entirely. Trees are the index.',
    },
    {
      icon: '\u26A1',
      title: 'Rust-Powered',
      desc: 'Core engine in Rust with Python bindings. Arena-based trees, async I/O, and zero-copy traversal.',
    },
    {
      icon: '\u{1F50D}',
      title: 'Multi-Algorithm Search',
      desc: 'Beam search, MCTS, and greedy algorithms with LLM-guided Pilot at key decision points.',
    },
    {
      icon: '\u{1F4CA}',
      title: 'Explainable Results',
      desc: 'Full reasoning chain traces every navigation decision. Audit how and why content was retrieved.',
    },
    {
      icon: '\u{1F4C4}',
      title: 'PDF & Markdown',
      desc: 'Index PDFs and Markdown out of the box. Hierarchical structure extracted automatically.',
    },
  ];

  return (
    <section className={styles.section}>
      <div className={styles.sectionInner}>
        <Heading as="h2" className={styles.sectionTitle}>
          Why Vectorless?
        </Heading>
        <p className={styles.sectionSubtitle}>
          RAG without the baggage.
        </p>
        <div className={styles.grid}>
          {items.map((item, i) => (
            <div key={i} className={styles.card}>
              <span className={styles.cardIcon}>{item.icon}</span>
              <Heading as="h3" className={styles.cardTitle}>{item.title}</Heading>
              <p className={styles.cardDesc}>{item.desc}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function SectionHowItWorks() {
  const steps = [
    { num: '01', title: 'Index', desc: 'Parse documents into hierarchical semantic trees with LLM-generated summaries.' },
    { num: '02', title: 'Navigate', desc: 'Pilot uses LLM to navigate the tree at key forks \u2014 beam search explores multiple paths in parallel.' },
    { num: '03', title: 'Retrieve', desc: 'Evaluate sufficiency and backtrack if needed. Aggregate only the most relevant content within budget.' },
  ];

  return (
    <section className={`${styles.section} ${styles.sectionAlt}`}>
      <div className={styles.sectionInner}>
        <Heading as="h2" className={styles.sectionTitle}>
          How It Works
        </Heading>
        <div className={styles.steps}>
          {steps.map((step, i) => (
            <div key={i} className={styles.step}>
              <div className={styles.stepNum}>{step.num}</div>
              <div className={styles.stepBody}>
                <Heading as="h3" className={styles.stepTitle}>{step.title}</Heading>
                <p className={styles.stepDesc}>{step.desc}</p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function SectionCTA() {
  return (
    <section className={styles.section}>
      <div className={styles.sectionInner}>
        <div className={styles.ctaBox}>
          <Heading as="h2" className={styles.ctaTitle}>
            Start building in minutes
          </Heading>
          <p className={styles.ctaDesc}>
            <code>pip install vectorless</code>
          </p>
          <div className={styles.ctaActions}>
            <Link className={styles.buttonPrimary} to="/docs/intro">
              Read the Docs
            </Link>
            <Link
              className={styles.buttonSecondary}
              href="https://github.com/vectorlessflow/vectorless"
              target="_blank"
              rel="noopener noreferrer">
              View on GitHub
            </Link>
          </div>
        </div>
      </div>
    </section>
  );
}

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout
      title={`${siteConfig.title}`}
      description="Reasoning-native document intelligence engine. No vector database, no embeddings. Retrieve by reasoning.">
      <HomepageHeader />
      <main>
        <SectionWhy />
        <SectionHowItWorks />
        <SectionCTA />
      </main>
    </Layout>
  );
}
