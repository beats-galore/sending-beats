export type Uuid<T> = string & {
  _brand: 'uuid';
  _type: T;
};

export type Identifier<T> = string & {
  _brand: 'identifier';
  _type: T;
};

export type Timestamp = string & {
  _brand: 'timestamp';
};

export type FilePath = string & {
  _brand: 'filepath';
};

export type FileName = string & {
  _brand: 'filename';
};
