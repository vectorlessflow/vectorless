import React from 'react';
import NavbarLayout from '@theme/Navbar/Layout';
import {useThemeConfig} from '@docusaurus/theme-common';
import NavbarItem from '@theme/NavbarItem';
import NavbarMobileSidebarToggle from '@theme/Navbar/MobileSidebar/Toggle';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Link from '@docusaurus/Link';
import GitHubStar from '../../components/GitHubStar';
import type {Props as NavbarItemConfig} from '@theme/NavbarItem';
import styles from './styles.module.css';

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
        </div>
      </div>
    </NavbarLayout>
  );
}
