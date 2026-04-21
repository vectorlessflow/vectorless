import React, { useState, useEffect, useCallback } from 'react';
import styles from './styles.module.css';

const OWNER = 'vectorlessflow';
const REPO = 'vectorless';

function formatNumber(num: number | null): string {
  if (num === null) return '—';
  return num.toLocaleString();
}

async function fetchRepoBasics(): Promise<number> {
  const resp = await fetch(`https://api.github.com/repos/${OWNER}/${REPO}`, {
    headers: { Accept: 'application/vnd.github.v3+json' },
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  const data = await resp.json();
  return data.stargazers_count ?? 0;
}

async function fetchOpenIssues(): Promise<number> {
  const resp = await fetch(
    `https://api.github.com/search/issues?q=repo:${OWNER}/${REPO}+type:issue+state:open&per_page=1`,
    { headers: { Accept: 'application/vnd.github.v3+json' } },
  );
  if (!resp.ok) throw new Error(`Issues: ${resp.status}`);
  const data = await resp.json();
  return data.total_count ?? 0;
}

async function fetchOpenPRs(): Promise<number> {
  const resp = await fetch(
    `https://api.github.com/search/issues?q=repo:${OWNER}/${REPO}+type:pr+state:open&per_page=1`,
    { headers: { Accept: 'application/vnd.github.v3+json' } },
  );
  if (!resp.ok) throw new Error(`PR: ${resp.status}`);
  const data = await resp.json();
  return data.total_count ?? 0;
}

export default function GitHubStats(): React.ReactElement {
  const [stars, setStars] = useState<number | null>(null);
  const [issues, setIssues] = useState<number | null>(null);
  const [prs, setPrs] = useState<number | null>(null);
  const [error, setError] = useState(false);

  const load = useCallback(async () => {
    setStars(null);
    setIssues(null);
    setPrs(null);
    setError(false);
    try {
      const [s, i, p] = await Promise.all([fetchRepoBasics(), fetchOpenIssues(), fetchOpenPRs()]);
      setStars(s);
      setIssues(i);
      setPrs(p);
    } catch {
      setError(true);
    }
  }, []);

  useEffect(() => {
    load();
    const t = setInterval(load, 300000);
    return () => clearInterval(t);
  }, [load]);

  return (
    <div
      className={styles.widget}
      onClick={() => window.open('https://github.com/vectorlessflow/vectorless', '_blank', 'noopener,noreferrer')}
      role="link"
      tabIndex={0}
      onKeyDown={(e) => { if (e.key === 'Enter') window.open('https://github.com/vectorlessflow/vectorless', '_blank', 'noopener,noreferrer'); }}>
      <div className={styles.statList}>
        <div className={styles.statRow}>
          <div className={styles.statLabel}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="var(--primary)"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/></svg>
            Stars
          </div>
          <div className={styles.statNumber}>
            {error ? '—' : formatNumber(stars)}
          </div>
        </div>
        <div className={styles.statRow}>
          <div className={styles.statLabel}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="var(--primary-dark)"><circle cx="12" cy="12" r="10"/></svg>
            Issues
          </div>
          <div className={styles.statNumber}>
            {error ? '—' : formatNumber(issues)}
          </div>
        </div>
        <div className={styles.statRow}>
          <div className={styles.statLabel}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="var(--primary-light)"><path d="M6 3v18h12V3H6zm10 16H8V5h8v14z"/></svg>
            PRs
          </div>
          <div className={styles.statNumber}>
            {error ? '—' : formatNumber(prs)}
          </div>
        </div>
      </div>
    </div>
  );
}
