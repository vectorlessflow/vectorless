import type {ReactNode} from 'react';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import GitHubStats from '@site/src/components/GitHubStats';

import styles from './index.module.css';

/* ===== Hamster SVG Icon ===== */
function HamsterIcon({size = 14}: {size?: number}) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
      <ellipse cx="16" cy="19" rx="11" ry="9" fill="var(--primary, #AF788B)"/>
      <circle cx="16" cy="11" r="8" fill="var(--primary, #AF788B)"/>
      <ellipse cx="10" cy="4.5" rx="3.2" ry="3.8" fill="var(--primary, #AF788B)"/>
      <ellipse cx="10" cy="4.5" rx="2" ry="2.6" fill="var(--primary-light, #C9A0AE)"/>
      <ellipse cx="22" cy="4.5" rx="3.2" ry="3.8" fill="var(--primary, #AF788B)"/>
      <ellipse cx="22" cy="4.5" rx="2" ry="2.6" fill="var(--primary-light, #C9A0AE)"/>
      <circle cx="10" cy="13" r="2.8" fill="var(--primary-light, #C9A0AE)"/>
      <circle cx="22" cy="13" r="2.8" fill="var(--primary-light, #C9A0AE)"/>
      <circle cx="13" cy="10" r="1.6" fill="#1E293B"/>
      <circle cx="19" cy="10" r="1.6" fill="#1E293B"/>
      <circle cx="13.5" cy="9.5" r="0.5" fill="#fff"/>
      <circle cx="19.5" cy="9.5" r="0.5" fill="#fff"/>
      <ellipse cx="16" cy="13" rx="1.2" ry="0.8" fill="var(--primary-dark, #8B5E6F)"/>
      <path d="M14.5 14.2 Q16 15.5 17.5 14.2" stroke="var(--primary-deeper, #6D4A58)" strokeWidth="0.6" fill="none" strokeLinecap="round"/>
      <ellipse cx="16" cy="21" rx="6" ry="4.5" fill="var(--primary-light, #C9A0AE)"/>
      <ellipse cx="7.5" cy="22" rx="2" ry="1.2" fill="var(--primary-dark, #8B5E6F)"/>
      <ellipse cx="24.5" cy="22" rx="2" ry="1.2" fill="var(--primary-dark, #8B5E6F)"/>
    </svg>
  );
}

/* ===== Hero ===== */
function HomepageHeader() {
  return (
    <header className={styles.heroBanner}>
      <div className={styles.statsCorner}>
        <GitHubStats />
      </div>
      <div className={styles.hero}>
        {/* Left: Brand + Features */}
        <div className={styles.heroContent}>
          <h1 className={styles.mainTitle}>Vectorless</h1>
          <p className={styles.subTitle}>Document Understanding Engine for AI</p>

          <div className={styles.featureList}>
            <div className={styles.featureItem}>
              <span>Open source by design</span>
            </div>
            <div className={styles.featureItem}>
              <span>Rust-powered · Python ecosystem</span>
            </div>
            <div className={styles.featureItem}>
              <span>Rules of Three — no exceptions</span>
            </div>
          </div>

          <div className={styles.heroActions}>
            <Link
              className={styles.githubStarButton}
              href="https://github.com/vectorlessflow/vectorless"
              target="_blank"
              rel="noopener noreferrer">
              <svg stroke="currentColor" fill="currentColor" strokeWidth="0" viewBox="0 0 496 512" height="18" width="18" xmlns="http://www.w3.org/2000/svg"><path d="M165.9 397.4c0 2-2.3 3.6-5.2 3.6-3.3.3-5.6-1.3-5.6-3.6 0-2 2.3-3.6 5.2-3.6 3-.3 5.6 1.3 5.6 3.6zm-31.1-4.5c-.7 2 1.3 4.3 4.3 4.9 2.6 1 5.6 0 6.2-2s-1.3-4.3-4.3-5.2c-2.6-.7-5.5.3-6.2 2.3zm44.2-1.7c-2.9.7-4.9 2.6-4.6 4.9.3 2 2.9 3.3 5.9 2.6 2.9-.7 4.9-2.6 4.6-4.6-.3-1.9-3-3.2-5.9-2.9zM244.8 8C106.1 8 0 113.3 0 252c0 110.9 69.8 205.8 169.5 239.2 12.8 2.3 17.3-5.6 17.3-12.1 0-6.2-.3-40.4-.3-61.4 0 0-70 15-84.7-29.8 0 0-11.4-29.1-27.8-36.6 0 0-22.9-15.7 1.6-15.4 0 0 24.9 2 38.6 25.8 21.9 38.6 58.6 27.5 72.9 20.9 2.3-16 8.8-27.1 16-33.7-55.9-6.2-112.3-14.3-112.3-110.5 0-27.5 7.6-41.3 23.6-58.9-2.6-6.5-11.1-33.3 2.6-67.9 20.9-6.5 69 27 69 27 20-5.6 41.5-8.5 62.8-8.5s42.8 2.9 62.8 8.5c0 0 48.1-33.6 69-27 13.7 34.7 5.2 61.4 2.6 67.9 16 17.7 25.8 31.5 25.8 58.9 0 96.5-58.9 104.2-114.8 110.5 9.2 7.9 17 22.9 17 46.4 0 33.7-.3 75.4-.3 83.6 0 6.5 4.6 14.4 17.3 12.1C428.2 457.8 496 362.9 496 252 496 113.3 383.5 8 244.8 8zM97.2 352.9c-1.3 1-1 3.3.7 5.2 1.6 1.6 3.9 2.3 5.2 1 1.3-1 1-3.3-.7-5.2-1.6-1.6-3.9-2.3-5.2-1zm-10.8-8.1c-.7 1.3.3 2.9 2.3 3.9 1.6 1 3.6.7 4.3-.7.7-1.3-.3-2.9-2.3-3.9-2-.6-3.6-.3-4.3.7zm32.4 35.6c-1.6 1.3-1 4.3 1.3 6.2 2.3 2.3 5.2 2.6 6.5 1 1.3-1.3.7-4.3-1.3-6.2-2.2-2.3-5.2-2.6-6.5-1zm-11.4-14.7c-1.6 1-1.6 3.6 0 5.9 1.6 2.3 4.3 3.3 5.6 2.3 1.6-1.3 1.6-3.9 0-6.2-1.4-2.3-4-3.3-5.6-2z"></path></svg>
              Star on GitHub
              <svg className={styles.starIcon} stroke="currentColor" fill="currentColor" strokeWidth="0" viewBox="0 0 24 24" height="18" width="18" xmlns="http://www.w3.org/2000/svg"><path d="M16.6,20.463a1.5,1.5,0,0,1-.7-.174l-3.666-1.927a.5.5,0,0,0-.464,0L8.1,20.289a1.5,1.5,0,0,1-2.177-1.581l.7-4.082a.5.5,0,0,0-.143-.442L3.516,11.293a1.5,1.5,0,0,1,.832-2.559l4.1-.6a.5.5,0,0,0,.376-.273l1.833-3.714a1.5,1.5,0,0,1,2.69,0l1.833,3.714a.5.5,0,0,0,.376.274l4.1.6a1.5,1.5,0,0,1,.832,2.559l-2.965,2.891a.5.5,0,0,0-.144.442l.7,4.082A1.5,1.5,0,0,1,16.6,20.463Zm-3.9-2.986L16.364,19.4a.5.5,0,0,0,.725-.527l-.7-4.082a1.5,1.5,0,0,1,.432-1.328l2.965-2.89a.5.5,0,0,0-.277-.853l-4.1-.6a1.5,1.5,0,0,1-1.13-.821L12.449,4.594a.516.516,0,0,0-.9,0L9.719,8.308a1.5,1.5,0,0,1-1.13.82l-4.1.6a.5.5,0,0,0-.277.853L7.18,13.468A1.5,1.5,0,0,1,7.611,14.8l-.7,4.082a.5.5,0,0,0,.726.527L11.3,17.477a1.5,1.5,0,0,1,1.4,0Z"></path></svg>
            </Link>
          </div>
        </div>

        {/* Right: Principles Card */}
        <div className={styles.heroPrinciples}>
          <div className={styles.principlesTitle}>
            Three rules · No exceptions
          </div>

          <div className={styles.principle}>
            <div className={styles.principleHead}>
              1. Reason, don't vector
              <span className={styles.badgeRust}>core</span>
            </div>
            <div className={styles.principleDesc}>
              Every retrieval is a reasoning act, not a similarity computation. No embeddings, no approximate matches.
            </div>
          </div>

          <div className={styles.principle}>
            <div className={styles.principleHead}>
              2. Model fails, we fail
            </div>
            <div className={styles.principleDesc}>
              No heuristic fallbacks. No silent degradation. If the reasoning model cannot find an answer, we return nothing — not a guess.
            </div>
          </div>

          <div className={styles.principle}>
            <div className={styles.principleHead}>
              3. No thought, no answer
            </div>
            <div className={styles.principleDesc}>
              Only reasoned output counts as an answer. Every response must be traceable through a semantic tree path — no hallucinated filler.
            </div>
          </div>

          <div className={styles.principlesFooter}>
            <HamsterIcon size={14} />
            reason, don't vector
          </div>
        </div>
      </div>
    </header>
  );
}

/* ===== Main Page ===== */
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
