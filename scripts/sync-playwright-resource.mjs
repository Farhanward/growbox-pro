import { cp, mkdir, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const source = join(root, 'node_modules', 'playwright-core');
const target = join(root, 'src-tauri', 'resources', 'node_modules', 'playwright-core');

if (!existsSync(source)) {
  throw new Error('playwright-core is missing. Run npm install before building GrowBox Pro.');
}

await rm(target, { recursive: true, force: true });
await mkdir(dirname(target), { recursive: true });
await cp(source, target, {
  recursive: true,
  filter: (path) => {
    const normalized = path.replaceAll('\\', '/');
    return !normalized.includes('/.local-browsers/');
  },
});

console.log(`Synced playwright-core resource: ${target}`);
