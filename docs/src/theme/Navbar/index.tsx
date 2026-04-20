import React from 'react';
import NavbarLayout from '@theme/Navbar/Layout';
import {useThemeConfig, useColorMode} from '@docusaurus/theme-common';
import NavbarItem from '@theme/NavbarItem';
import NavbarMobileSidebarToggle from '@theme/Navbar/MobileSidebar/Toggle';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Link from '@docusaurus/Link';
import GitHubStar from '../../components/GitHubStar';
import type {Props as NavbarItemConfig} from '@theme/NavbarItem';
import styles from './styles.module.css';

function ColorModeToggle(): React.ReactElement {
  const {colorMode, setColorMode} = useColorMode();
  const isDark = colorMode === 'dark';
  return (
    <button
      className={styles.themeToggle}
      onClick={() => setColorMode(isDark ? 'light' : 'dark')}
      aria-label={`Switch to ${isDark ? 'light' : 'dark'} mode`}
      title={`Switch to ${isDark ? 'light' : 'dark'} mode`}>
      {isDark ? (
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="5" />
          <line x1="12" y1="1" x2="12" y2="3" />
          <line x1="12" y1="21" x2="12" y2="23" />
          <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
          <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
          <line x1="1" y1="12" x2="3" y2="12" />
          <line x1="21" y1="12" x2="23" y2="12" />
          <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
          <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
        </svg>
      ) : (
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </svg>
      )}
    </button>
  );
}

export default function Navbar(): React.ReactElement {
  const {navbar: {items, logo, title}} = useThemeConfig();
  const leftItems = items.filter(item => item.position === 'left');
  const rightItems = items.filter(item => item.position === 'right');

  return (
    <NavbarLayout>
      <div className={styles.navbarContainer}>
        <NavbarMobileSidebarToggle />
        <div className={styles.navbarBrand}>
          <Link to={logo?.href || '/'} className={styles.navbarLogoLink}>
            <img
              className={styles.navbarLogo}
              src={useBaseUrl(logo?.src || 'img/logo.png')}
              alt={logo?.alt || title}
            />
          </Link>
        <div className={styles.logo}>Vectorless</div>
        </div>
        <div className={styles.navbarCenter}>
          {leftItems.map((item, i) => <NavbarItem {...(item as NavbarItemConfig)} key={i} />)}
        </div>
        <div className={styles.navbarRight}>
          {rightItems.map((item, i) => <NavbarItem {...(item as NavbarItemConfig)} key={i} />)}
          <div className={styles.githubStarWrapper}>
            <GitHubStar />
          </div>
          <ColorModeToggle />
        </div>
      </div>
    </NavbarLayout>
  );
}
