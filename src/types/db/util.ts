import type { Timestamp, Uuid } from '../util.types';

export type AsCreationAttributes<
  T extends {
    id: Uuid<T>;
    createdAt: Timestamp;
    updatedAt: Timestamp;
    deletedAt?: Timestamp;
  },
> = Omit<Timestamp, 'id' | 'createdAt' | 'updatedAt' | 'deletedAt'>;

export type AsUpdateAttributes<
  T extends {
    id: Uuid<T>;
    createdAt: Timestamp;
    updatedAt: Timestamp;
    deletedAt?: Timestamp;
  },
> = Partial<T> & {
  createdAt?: never;
  updatedAt?: never;
  deletedAt?: never;
};
