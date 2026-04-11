import type {ReactNode} from 'react';
import clsx from 'clsx';
import Heading from '@theme/Heading';
import styles from './styles.module.css';

type FeatureItem = {
  title: string;
  description: ReactNode;
};

const FeatureList: FeatureItem[] = [
  {
    title: 'No Vectors',
    description: (
      <>
        No vector database, no embeddings, no similarity search. Documents are parsed
        into hierarchical semantic trees and navigated by LLM reasoning.
      </>
    ),
  },
  {
    title: 'Blazing Fast',
    description: (
      <>
        Written in Rust. Parallel indexing, incremental updates with content fingerprinting,
        and zero infrastructure dependency — just an LLM API key.
      </>
    ),
  },
  {
    title: 'Rust + Python',
    description: (
      <>
        First-class Rust library with async Python bindings via PyO3. Index, query,
        and explore cross-document relationships from either language.
      </>
    ),
  },
];

function Feature({title, description}: FeatureItem) {
  return (
    <div className={clsx('col col--4')}>
      <div className="text--center padding-horiz--md">
        <Heading as="h3">{title}</Heading>
        <p>{description}</p>
      </div>
    </div>
  );
}

export default function HomepageFeatures(): ReactNode {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {FeatureList.map((props, idx) => (
            <Feature key={idx} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}
