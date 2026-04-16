import type {ReactNode} from 'react';
import {useState, useMemo} from 'react';
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

/* ---- Regex-based syntax highlighter ---- */
function highlight(code: string, lang: 'python' | 'rust'): ReactNode[] {
  // Each rule has exactly ONE capture group in its regex
  const rules: {re: RegExp; cls: string}[] = lang === 'python'
    ? [
        {re: /(#.*)/g, cls: styles.hlComment},
        {re: /("(?:[^"\\]|\\.)*")/g, cls: styles.hlString},
        {re: /\b(import|from|async|def|await|return|as|with|for|in|if|else|None|True|False)\b/g, cls: styles.hlKeyword},
        {re: /\b([A-Z][A-Za-z0-9_]*)\b/g, cls: styles.hlType},
        {re: /\b([a-z_]\w*)\s*(?=\()/g, cls: styles.hlFunction},
      ]
    : [
        {re: /(\/\/.*)/g, cls: styles.hlComment},
        {re: /("(?:[^"\\]|\\.)*")/g, cls: styles.hlString},
        {re: /\b(use|let|mut|fn|async|await|return|if|else|match|struct|impl|pub|mod|crate|self|super|where|for|in|loop|while|break|continue|move|ref|type|enum|trait|const|static|unsafe|extern)\b/g, cls: styles.hlKeyword},
        {re: /\b([A-Z][A-Za-z0-9_]*)\b/g, cls: styles.hlType},
        {re: /\b(\w+!)/g, cls: styles.hlFunction},
        {re: /\b([a-z_]\w*)\s*(?=\()/g, cls: styles.hlFunction},
        {re: /(#\[.*?\])/g, cls: styles.hlAttribute},
      ];

  // Build combined regex — join the single capture-group sources directly
  const combined = rules.map(r => r.re.source).join('|');
  const re = new RegExp(combined, 'gm');

  const nodes: ReactNode[] = [];
  let lastIdx = 0;
  let m: RegExpExecArray | null;
  re.lastIndex = 0;

  while ((m = re.exec(code)) !== null) {
    if (m.index > lastIdx) {
      nodes.push(code.slice(lastIdx, m.index));
    }
    // match[1..rules.length] corresponds to each rule's capture group
    for (let i = 0; i < rules.length; i++) {
      const captured = m[i + 1];
      if (captured !== undefined) {
        nodes.push(<span key={`${m.index}-${i}`} className={rules[i].cls}>{captured}</span>);
        break;
      }
    }
    lastIdx = re.lastIndex;
  }
  if (lastIdx < code.length) {
    nodes.push(code.slice(lastIdx));
  }
  return nodes;
}

// Exact code from README
const PYTHON_CODE = `import asyncio
from vectorless import Engine, IndexContext, QueryContext

async def main():
    engine = Engine(api_key="sk-...", model="gpt-4o")

    # Index a document
    result = await engine.index(IndexContext.from_path("./report.pdf"))
    doc_id = result.doc_id

    # Query
    result = await engine.query(
        QueryContext("What is the total revenue?").with_doc_ids([doc_id])
    )
    print(result.single().content)

asyncio.run(main())`;

const RUST_CODE = `use vectorless::client::{EngineBuilder, IndexContext, QueryContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    let engine = EngineBuilder::new()
        .with_key("sk-...")
        .with_model("gpt-4o")
        .build()
        .await?;

    // Index a document
    let result = engine.index(IndexContext::from_path("./report.pdf")).await?;
    let doc_id = result.doc_id().unwrap();

    // Query
    let result = engine.query(
        QueryContext::new("What is the total revenue?")
            .with_doc_ids(vec![doc_id.to_string()])
    ).await?;
    println!("{}", result.content);

    Ok(())
}`;

function PythonCode() {
  const nodes = useMemo(() => highlight(PYTHON_CODE, 'python'), []);
  return <pre className={styles.demoPre}><code>{nodes}</code></pre>;
}

function RustCode() {
  const nodes = useMemo(() => highlight(RUST_CODE, 'rust'), []);
  return <pre className={styles.demoPre}><code>{nodes}</code></pre>;
}

function SectionGetStarted() {
  const [activeTab, setActiveTab] = useState<'python' | 'rust'>('python');
  const [copyLabel, setCopyLabel] = useState('Copy');
  const [installLabel, setInstallLabel] = useState('Copy & install');

  const installCmd = activeTab === 'python' ? 'pip install vectorless' : 'cargo add vectorless';

  const handleCopy = () => {
    const code = activeTab === 'python' ? PYTHON_CODE : RUST_CODE;
    navigator.clipboard.writeText(code);
    setCopyLabel('\u2713 Copied!');
    setTimeout(() => setCopyLabel('Copy'), 1500);
  };

  const handleInstallCopy = () => {
    navigator.clipboard.writeText(installCmd);
    setInstallLabel('\u2713 Copied!');
    setTimeout(() => setInstallLabel('Copy & install'), 1500);
  };

  return (
    <section className={styles.section}>
      <div className={styles.sectionInner}>
        <Heading as="h2" className={styles.sectionTitle}>
          Get Started
        </Heading>
        <p className={styles.sectionSubtitle}>
          Just a few lines of code to get up and running.
        </p>
        <div className={styles.demoCard}>
          {/* Tabs */}
          <div className={styles.demoTabs}>
            <button
              className={`${styles.demoTab} ${activeTab === 'python' ? styles.demoTabActive : ''}`}
              onClick={() => { setActiveTab('python'); setCopyLabel('Copy'); }}>
              Python
            </button>
            <button
              className={`${styles.demoTab} ${activeTab === 'rust' ? styles.demoTabActive : ''}`}
              onClick={() => { setActiveTab('rust'); setCopyLabel('Copy'); }}>
              Rust
            </button>
          </div>

          {/* Python panel */}
          {activeTab === 'python' && (
            <div className={styles.demoPanel}>
              <div className={styles.demoCodeHeader}>
                <div className={styles.windowDots}>
                  <span className={`${styles.windowDot} ${styles.dotRed}`} />
                  <span className={`${styles.windowDot} ${styles.dotYellow}`} />
                  <span className={`${styles.windowDot} ${styles.dotGreen}`} />
                </div>
                <button className={styles.copyBtn} onClick={handleCopy}>{copyLabel}</button>
              </div>
              <PythonCode />
              <div className={styles.terminalOutput}>
                <span className={styles.terminalPrompt}>$</span> python demo.py<br />
                <span className={styles.terminalAnswer}>&rarr; The total revenue for fiscal year 2024 was $2.3 billion, a 15% increase YoY.</span>
                <span className={styles.terminalCursor} />
              </div>
            </div>
          )}

          {/* Rust panel */}
          {activeTab === 'rust' && (
            <div className={styles.demoPanel}>
              <div className={styles.demoCodeHeader}>
                <div className={styles.windowDots}>
                  <span className={`${styles.windowDot} ${styles.dotRed}`} />
                  <span className={`${styles.windowDot} ${styles.dotYellow}`} />
                  <span className={`${styles.windowDot} ${styles.dotGreen}`} />
                </div>
                <button className={styles.copyBtn} onClick={handleCopy}>{copyLabel}</button>
              </div>
              <RustCode />
              <div className={styles.terminalOutput}>
                <span className={styles.terminalPrompt}>$</span> cargo run<br />
                <span className={styles.terminalAnswer}>&rarr; The total revenue for fiscal year 2024 was $2.3 billion, a 15% increase YoY.</span>
                <span className={styles.terminalCursor} />
              </div>
            </div>
          )}

          {/* Install bar */}
          <div className={styles.installBar}>
            <div className={styles.installCommand}>
              <span>$</span> {installCmd}
            </div>
            <button className={styles.installBtn} onClick={handleInstallCopy}>{installLabel}</button>
          </div>
        </div>
      </div>
    </section>
  );
}

function SectionHowItWorks() {
  return (
    <section className={styles.section}>
      <div className={styles.sectionInner}>
        <Heading as="h2" className={styles.sectionTitle}>
          How does Vectorless work?
        </Heading>
        <p className={styles.sectionSubtitle}>
          You declare a few lines of code. We do everything else.
        </p>
        <div className={styles.workflowWrapper}>
          <img src="/img/workflow.svg" alt="How Vectorless works" className={styles.workflowImg} />
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
        <SectionGetStarted />
        <SectionHowItWorks />
        <SectionCTA />
      </main>
    </Layout>
  );
}
