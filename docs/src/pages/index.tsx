import {useEffect, useReducer, useRef} from 'react';
import type {CSSProperties, ReactNode} from 'react';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import {Highlight, themes} from 'prism-react-renderer';

import styles from './index.module.css';

// ── Compile → Navigate hero animation ───────────────────────────────────────

type NodeDef = {id: string; x: number; y: number; r: number; label: string; lx: number; ly: number};

const NODES: NodeDef[] = [
  {id: 'root', x: 400, y: 46, r: 9, label: 'report.pdf', lx: 400, ly: 28},
  {id: 'n1', x: 90, y: 136, r: 7, label: 'Exec', lx: 90, ly: 160},
  {id: 'n2', x: 240, y: 136, r: 7, label: 'Financials', lx: 240, ly: 118},
  {id: 'n3', x: 400, y: 136, r: 7, label: 'Operations', lx: 400, ly: 160},
  {id: 'n4', x: 560, y: 136, r: 7, label: 'Risk', lx: 560, ly: 118},
  {id: 'n5', x: 710, y: 136, r: 7, label: 'Appendix', lx: 710, ly: 160},
  {id: 'n21', x: 150, y: 236, r: 6, label: 'Overview', lx: 150, ly: 258},
  {id: 'n22', x: 235, y: 236, r: 6, label: 'Balance', lx: 235, ly: 258},
  {id: 'n24', x: 320, y: 236, r: 6, label: 'Income', lx: 320, ly: 222},
  {id: 'n41', x: 515, y: 236, r: 6, label: 'Risk Map', lx: 515, ly: 258},
  {id: 'n42', x: 615, y: 236, r: 6, label: 'Mitigation', lx: 615, ly: 258},
  {id: 'n241', x: 285, y: 320, r: 5, label: 'Revenue', lx: 285, ly: 340},
  {id: 'n242', x: 375, y: 320, r: 5, label: 'Expenses', lx: 375, ly: 340},
];

const POS: Record<string, [number, number]> = Object.fromEntries(
  NODES.map((n) => [n.id, [n.x, n.y]]),
) as Record<string, [number, number]>;

const EDGES = [
  ['root', 'n1'], ['root', 'n2'], ['root', 'n3'], ['root', 'n4'], ['root', 'n5'],
  ['n2', 'n21'], ['n2', 'n22'], ['n2', 'n24'],
  ['n4', 'n41'], ['n4', 'n42'],
  ['n24', 'n241'], ['n24', 'n242'],
].map(([from, to]) => ({id: `${from}-${to}`, from, to}));

type Vis = {
  shown: Set<string>; lit: Set<string>; path: Set<string>; found: string;
  eShown: Set<string>; eActive: Set<string>; agent: string;
  cmd: string; phase: string; passes: Set<string>; answer: boolean;
  ripple: [number, number] | null; rippleKey: number;
};

function freshVis(): Vis {
  return {
    shown: new Set(), lit: new Set(), path: new Set(), found: '',
    eShown: new Set(), eActive: new Set(), agent: '',
    cmd: '', phase: '', passes: new Set(), answer: false, ripple: null, rippleKey: 0,
  };
}

function nodeCircleStyle(id: string, v: Vis): CSSProperties {
  if (v.found === id)
    return {fill: 'var(--primary)', stroke: '#fff', strokeWidth: 2,
            filter: 'drop-shadow(0 0 9px rgba(224,151,94,0.85))', transition: 'all 0.35s'};
  if (v.lit.has(id))
    return {fill: 'rgba(199,123,74,0.18)', stroke: 'var(--primary)', strokeWidth: 1.5, transition: 'all 0.35s'};
  if (id === 'root')
    return {fill: '#15110e', stroke: 'var(--grad-to)', strokeWidth: 1.5, transition: 'all 0.35s'};
  return {fill: '#11151b', stroke: v.path.has(id) ? 'var(--primary)' : '#8a95a5',
          strokeWidth: 1.5, transition: 'all 0.35s'};
}

function nodeTextStyle(id: string, v: Vis): CSSProperties {
  // The terminal panel is always dark, so label colors are fixed (not theme-aware).
  let fill = '#8a95a5';
  if (v.found === id) fill = '#fff';
  else if (v.lit.has(id)) fill = 'var(--primary)';
  else if (id === 'root') fill = '#ECEFF4';
  return {fill, fontSize: id === 'root' ? 11.5 : 10.5, fontWeight: 600};
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function runAnimation(visRef: {current: Vis}, alive: () => boolean, tick: () => void): Promise<void> {
  while (alive()) {
    const v = freshVis();
    visRef.current = v;
    tick();
    await sleep(300);
    if (!alive()) return;

    // ── compile ──
    v.phase = 'Compiling document → tree';
    tick();
    const steps: [string, string | null, string | null][] = [
      ['root', null, 'parse'],
      ['n1', 'root-n1', null], ['n2', 'root-n2', null], ['n3', 'root-n3', null],
      ['n4', 'root-n4', null], ['n5', 'root-n5', 'split'],
      ['n21', 'n2-n21', null], ['n22', 'n2-n22', null], ['n24', 'n2-n24', null],
      ['n41', 'n4-n41', null], ['n42', 'n4-n42', 'index'],
      ['n241', 'n24-n241', null], ['n242', 'n24-n242', 'route'],
    ];
    for (const [id, edge, pass] of steps) {
      if (!alive()) return;
      v.shown.add(id);
      if (edge) v.eShown.add(edge);
      if (pass) v.passes.add(pass);
      tick();
      await sleep(240);
    }
    v.passes.add('score');
    tick();
    await sleep(450);
    v.phase = 'Compiled · 13 nodes, 0 vectors';
    tick();
    await sleep(650);

    // ── navigate ──
    v.agent = 'root'; tick(); await sleep(500); if (!alive()) return;
    v.cmd = 'ls'; ['n1', 'n2', 'n3', 'n4', 'n5'].forEach((n) => v.lit.add(n)); tick(); await sleep(950);
    ['n1', 'n3', 'n4', 'n5'].forEach((n) => v.lit.delete(n)); tick();
    if (!alive()) return;
    v.cmd = 'cd n2'; v.eActive.add('root-n2'); v.path.add('n2'); v.agent = 'n2'; tick(); await sleep(750);
    v.cmd = 'grep "total revenue"'; ['n21', 'n22', 'n24'].forEach((n) => v.lit.add(n));
    v.ripple = POS.n2; v.rippleKey++; tick(); await sleep(1000);
    ['n21', 'n22'].forEach((n) => v.lit.delete(n)); tick();
    if (!alive()) return;
    v.cmd = 'cd n2.4'; v.eActive.add('n2-n24'); v.path.add('n24'); v.agent = 'n24'; tick(); await sleep(750);
    v.cmd = 'grep "revenue"'; ['n241', 'n242'].forEach((n) => v.lit.add(n));
    v.ripple = POS.n24; v.rippleKey++; tick(); await sleep(900);
    v.lit.delete('n242'); tick();
    if (!alive()) return;
    v.cmd = 'cat n2.4.1'; v.eActive.add('n24-n241'); v.agent = 'n241'; tick(); await sleep(650);
    v.lit.delete('n241'); v.found = 'n241'; tick(); await sleep(400);
    v.cmd = ''; v.phase = 'Answer with source · 3 levels deep'; v.answer = true; tick();
    await sleep(2700);
  }
}

function staticFinal(v: Vis): void {
  NODES.forEach((n) => v.shown.add(n.id));
  EDGES.forEach((e) => v.eShown.add(e.id));
  ['parse', 'split', 'index', 'route', 'score'].forEach((p) => v.passes.add(p));
  v.found = 'n241';
  v.agent = 'n241';
  v.answer = true;
  v.phase = 'Compiled · navigate · answer';
}

// Syntax-highlighted code with a FIXED dark theme (the card is always dark,
// in both light and dark site modes — like a real terminal).
function Code({code, language}: {code: string; language: string}): ReactNode {
  return (
    <Highlight theme={themes.vsDark} code={code.trim()} language={language}>
      {({tokens, getLineProps, getTokenProps}) => (
        <pre className={styles.codeBlock}>
          {tokens.map((line, i) => (
            <span key={i} {...getLineProps({line})} style={{display: 'block'}}>
              {line.map((token, key) => (
                <span key={key} {...getTokenProps({token})} />
              ))}
            </span>
          ))}
        </pre>
      )}
    </Highlight>
  );
}

function CompileNavigate(): ReactNode {
  const visRef = useRef<Vis>(freshVis());
  const [, force] = useReducer((x: number) => x + 1, 0);
  const idRef = useRef(0);

  useEffect(() => {
    const myId = ++idRef.current;
    const alive = () => idRef.current === myId;
    const reduce =
      typeof window !== 'undefined' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (reduce) {
      staticFinal(visRef.current);
      force();
    } else {
      void runAnimation(visRef, alive, () => { if (alive()) force(); });
    }
    return () => { idRef.current++; };
  }, []);

  const v = visRef.current;
  const [ax, ay] = POS[v.agent || 'root'];

  return (
    <div className={styles.terminalWrap}>
      <div className={styles.terminal}>
        <div className={styles.cnHead}>
          <div className={styles.cnHeadLeft}>
            <span className={styles.dot} /><span className={styles.dot} /><span className={styles.dot} />
            <span className={styles.cnPhase}>{v.phase || '…'}</span>
          </div>
          <span className={styles.cnCmd} style={{opacity: v.cmd ? 1 : 0}}>{v.cmd || ' '}</span>
        </div>
        <div className={styles.cnPasses}>
          {['parse', 'split', 'index', 'route', 'score'].map((p) => (
            <span key={p} className={`${styles.cnPass} ${v.passes.has(p) ? styles.cnPassDone : ''}`}>{p}</span>
          ))}
        </div>
        <svg className={styles.cnSvg} viewBox="0 0 800 360">
          {EDGES.map((e) => {
            const [x1, y1] = POS[e.from];
            const [x2, y2] = POS[e.to];
            const active = v.eActive.has(e.id);
            const shown = v.eShown.has(e.id);
            return (
              <line key={e.id} x1={x1} y1={y1} x2={x2} y2={y2}
                style={{
                  stroke: active ? 'var(--primary)' : '#2a3038',
                  strokeWidth: active ? 2.2 : 1.3,
                  opacity: active ? 1 : shown ? 0.7 : 0,
                  transition: 'all 0.35s',
                }} />
            );
          })}
          {v.ripple && (
            <circle key={v.rippleKey} cx={v.ripple[0]} cy={v.ripple[1]} r={6} className={styles.cnRipple} />
          )}
          {NODES.map((n) => (
            <g key={n.id} style={{opacity: v.shown.has(n.id) ? 1 : 0, transition: 'opacity 0.4s'}}>
              <circle cx={n.x} cy={n.y} r={n.r} style={nodeCircleStyle(n.id, v)} />
              <text x={n.lx} y={n.ly} textAnchor="middle" style={nodeTextStyle(n.id, v)}>{n.label}</text>
            </g>
          ))}
          <g style={{
            transform: `translate(${ax}px, ${ay}px)`,
            transition: 'transform 0.55s cubic-bezier(0.4,0,0.2,1)',
            opacity: v.agent ? 1 : 0,
          }}>
            <circle r={8} className={styles.cnRing} />
            <circle r={5} style={{fill: 'var(--primary)'}} />
          </g>
        </svg>
        <div className={styles.cnAnswer}
          style={{opacity: v.answer ? 1 : 0, transform: v.answer ? 'none' : 'translateY(8px)'}}>
          ✓ Total revenue <b>$4.82B</b>, up 19% YoY <span>· source n2.4.1 · line 12</span>
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
                Knowing by <span className={styles.grad}>reasoning</span>,<br />not vectors.
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
              <CompileNavigate />
            </div>
          </div>

          {/* ── Use it from Python ── */}
          <section className={styles.section}>
            <div className={styles.sectionHead}>
              <h2 className={styles.sectionTitle}>Use it from Python</h2>
              <p className={styles.sectionSub}>
                Compile a document, then ask in plain language — every answer carries its source.
              </p>
            </div>
            <div className={styles.codeCard}>
              <div className={styles.codeBar}>
                <span className={styles.dot} /><span className={styles.dot} /><span className={styles.dot} />
                <span className={styles.codeBarTitle}>main.py</span>
              </div>
              <Code language="python" code={`import asyncio
from vectorless import Engine

async def main():
    async with Engine(api_key="sk-...", model="gpt-4o") as engine:
        doc = await engine.compile(path="./report.pdf")

        res = await engine.ask(
            "What is the total revenue?",
            doc_ids=[doc.doc_id],
        )
        print(res.answer)            # grounded answer
        for ev in res.evidence:      # ...with sources
            print(ev.node_title, ev.source_path)

asyncio.run(main())`} />
            </div>
          </section>

          {/* ── Or from the CLI ── */}
          <section className={styles.section}>
            <div className={styles.sectionHead}>
              <h2 className={styles.sectionTitle}>Or straight from your terminal</h2>
              <p className={styles.sectionSub}>
                Same engine, same answers — index and query your documents without writing any code.
              </p>
            </div>
            <div className={styles.codeCard}>
              <div className={styles.codeBar}>
                <span className={styles.dot} /><span className={styles.dot} /><span className={styles.dot} />
                <span className={styles.codeBarTitle}>zsh — vectorless</span>
              </div>
              <Code language="bash" code={`# index a document (or a folder with -r)
$ vectorless add ./report.pdf

# ask a one-off question
$ vectorless query "What is the total revenue?"

# interactive REPL over your documents
$ vectorless ask

# inspect the compiled tree
$ vectorless tree <doc_id>
$ vectorless list`} />
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
              </div>
            </div>
          </section>

        </div>
      </header>
      <main />
    </Layout>
  );
}
