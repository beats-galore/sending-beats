import type { Timestamp, Uuid } from '../util.types';

export type AsCreationAttributes<
  T extends {
    id: Uuid<T>;
    createdAt: Timestamp;
    updatedAt: Timestamp;
  },
> = Omit<T, 'id' | 'createdAt' | 'updatedAt'>;

export type AsUpdateAttributes<
  T extends {
    id: Uuid<T>;
    createdAt: Timestamp;
    updatedAt: Timestamp;
  },
> = Partial<T> & {
  createdAt?: never;
  updatedAt?: never;
};
