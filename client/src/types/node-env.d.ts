// This file provides TypeScript declarations for Node.js globals used in the client

declare const process: {
  env: {
    NODE_ENV: 'development' | 'production' | 'test';
    [key: string]: string | undefined;
  };
};

declare function require(id: string): any;