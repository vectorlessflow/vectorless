import type {ReactNode} from 'react';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import Link from '@docusaurus/Link';
import useBaseUrl from '@docusaurus/useBaseUrl';
import {Highlight, themes} from 'prism-react-renderer';

import styles from './index.module.css';

function HomepageHeader() {
  const {siteConfig} = useDocusaurusContext();
  return (
    <header className={styles.heroBanner}>
      <div className={styles.heroInner}>
        <div className={styles.heroContent}>
          <img
            className={styles.heroLogo}
            src={useBaseUrl('img/with-title.png')}
            alt={siteConfig.title}
          />
          <p className={styles.heroTagline}>
            No vector database. No embeddings. No similarity search.<br />
            Retrieve by reasoning, not by math.
          </p>
          <div className={styles.heroActions}>
            <Link className={styles.buttonPrimary} to="/docs/intro">
              Get Started
            </Link>
            <Link
              className={styles.buttonSecondary}
              href="https://github.com/vectorlessflow/vectorless"
              target="_blank"
              rel="noopener noreferrer">
              GitHub
            </Link>
          </div>
        </div>

        <div className={styles.codePreview}>
          <div className={styles.codeHeader}>
            <span className={styles.codeDots}>
              <span /><span /><span />
            </span>
            <span className={styles.codeLang}>Python</span>
          </div>
          <Highlight theme={themes.dracula} code={`import asyncio
from vectorless import Engine, IndexContext

async def main():
    engine = Engine(
        api_key="sk-...",
        model="gpt-4o",
    )

    # Index a document
    result = await engine.index(
        IndexContext.from_path("./report.pdf")
    )
    doc_id = result.doc_id

    # Query — LLM navigates the tree
    result = await engine.query(
        doc_id, "What is the total revenue?"
    )
    print(result.single().content)

asyncio.run(main())`} language="python">
            {({tokens, getLineProps, getTokenProps}) => (
              <pre className={styles.codeBlock}>
                <code>
                  {tokens.map((line, i) => (
                    <div key={i} {...getLineProps({line})}>
                      {line.map((token, key) => (
                        <span key={key} {...getTokenProps({token})} />
                      ))}
                    </div>
                  ))}
                </code>
              </pre>
            )}
          </Highlight>
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
