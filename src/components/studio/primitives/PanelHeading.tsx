import { Title } from '@mantine/core';


import type { TitleOrder } from '@mantine/core';
import type { ReactNode } from 'react';
import { layout } from '../../../theme/layout';

type PanelHeadingProps = {
  children: ReactNode;
  order?: TitleOrder;
};

/** The condensed, tracked-out plate that names a panel. */
export const PanelHeading = ({ children, order = 3 }: PanelHeadingProps) => (
  <Title order={order} style={{ letterSpacing: layout.tracking.wider }}>
    {children}
  </Title>
);
