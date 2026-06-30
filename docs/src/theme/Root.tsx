import React from 'react';
import {Analytics} from '@vercel/analytics/react';

// Swizzled Root — wraps the entire app. Mounts Vercel Analytics on every page.
export default function Root({children}: {children: React.ReactNode}): React.ReactElement {
  return (
    <>
      {children}
      <Analytics />
    </>
  );
}
