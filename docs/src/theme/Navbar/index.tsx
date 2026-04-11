import React from 'react';
import NavbarLayout from '@theme/Navbar/Layout';
import {useThemeConfig} from '@docusaurus/theme-common';
import {useNavbarMobileSidebar} from '@docusaurus/theme-common/internal';
import NavbarItem from '@theme/NavbarItem';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Link from '@docusaurus/Link';
import {FaBars} from 'react-icons/fa';
import GitHubStar from '../../components/GitHubStar';
import type {Props as NavbarItemConfig} from '@theme/NavbarItem';
import styles from './styles.module.css';

export default function Navbar(): React.ReactElement {
  const mobileSidebar = useNavbarMobileSidebar();
  const {navbar: {items, logo, title}} = useThemeConfig();

  const leftItems = items.filter(item => item.position === 'left');
  const rightItems = items.filter(item => item.position === 'right');

  return (
    <NavbarLayout>
      <div className={styles.navbarContainer}>
        <button
          className={`${styles.navbarMobileMenuButton} ${mobileSidebar.shown ? styles.navbarMobileMenuButtonHidden : ''}`}
          onClick={mobileSidebar.toggle}
          aria-label="Toggle navigation bar"
          aria-expanded={mobileSidebar.shown}
        >
          <FaBars className={styles.navbarMobileMenuIcon} />
        </button>

        <div className={styles.navbarBrand}>
          <Link to={logo?.href || '/'} className={styles.navbarLogoLink}>
            <img
              className={styles.navbarLogo}
              src={useBaseUrl(logo?.src || 'img/logo.png')}
              alt={logo?.alt || title}
            />
          </Link>
        </div>

        <div className={styles.navbarItemsLeft}>
          {leftItems.map((item, i) => <NavbarItem {...(item as NavbarItemConfig)} key={i} />)}
        </div>

        <div className={styles.navbarItemsRight}>
          {rightItems.map((item, i) => <NavbarItem {...(item as NavbarItemConfig)} key={i} />)}
          <div className={styles.githubStarWrapper}>
            <GitHubStar />
          </div>
        </div>
      </div>
    </NavbarLayout>
  );
}
