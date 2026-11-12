// Hash every packaged file, including dotfiles, independently of enumeration order.
import { createHash } from 'node:crypto';
import { lstatSync, readdirSync, realpathSync } from 'node:fs';
import { open } from 'node:fs/promises';
import { basename, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export async function inventory(directory) {
  const root = resolve(directory);
  const anchor = realpathSync.native(root);
  if (relative(anchor, root) !== '' || !lstatSync(root).isDirectory()) throw new Error('Redirected resource root');
  const paths = [];
  function collect(dir) {
    for (const name of readdirSync(dir)) {
      if (name !== basename(name) || name === '.' || name === '..' || name.includes(':')) throw new Error('Invalid resource name');
      const full = join(dir, name);
      const stat = lstatSync(full, { bigint: true });
      if (stat.isSymbolicLink() || relative(full, realpathSync.native(full)) !== '') throw new Error('Redirected resource');
      if (stat.isDirectory()) collect(full);
      else if (stat.isFile() && stat.nlink === 1n) paths.push({ path: relative(root, full).replaceAll('\\', '/'), ino: stat.ino, dev: stat.dev });
      else throw new Error('Unsupported or linked resource');
    }
  }
  collect(root);
  paths.sort((a,b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  const tree = createHash('sha256');
  let bytes = 0;
  async function fileDigest(entry) {
    const handle = await open(join(root, entry.path), 'r');
    try {
      const info = await handle.stat({ bigint: true });
      if (!info.isFile() || info.nlink !== 1n || info.ino !== entry.ino || info.dev !== entry.dev) throw new Error('Resource identity changed');
      const digest = createHash('sha256');
      if (info.size <= 1024n * 1024n) digest.update(await handle.readFile());
      else for await (const chunk of handle.createReadStream({ autoClose: false })) digest.update(chunk);
      if ((await handle.stat({ bigint: true })).mtimeNs !== info.mtimeNs) throw new Error('Resource changed while hashing');
      bytes += Number(info.size);
      return digest.digest();
    } finally { await handle.close(); }
  }
  for (let start = 0; start < paths.length; start += 12) {
    const batch = paths.slice(start, start + 12);
    const digests = await Promise.all(batch.map(fileDigest));
    batch.forEach((entry,i) => tree.update(entry.path).update('\0').update(digests[i]));
  }
  return { schema: 'openkoda.security-runtime-tree.v1', files: paths.length, bytes, sha256: tree.digest('hex') };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length !== 3) throw new Error('Supply one runtime directory');
  console.log(JSON.stringify(await inventory(process.argv[2])));
}
