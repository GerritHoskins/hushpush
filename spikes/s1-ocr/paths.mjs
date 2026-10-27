import { fileURLToPath } from 'node:url';
export const root = fileURLToPath(new URL('.', import.meta.url));
export const out = fileURLToPath(new URL('out', import.meta.url));
