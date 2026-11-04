import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
export const sha256=value=>crypto.createHash('sha256').update(value).digest('hex');
export function sourceFingerprint(root=process.cwd()) {
  const files=[];
  function walk(relative) {
    for(const item of fs.readdirSync(path.join(root,relative),{withFileTypes:true})) {
      if(['target','gen','node_modules','.git'].includes(item.name))continue;
      const file=path.posix.join(relative,item.name);
      if(item.isSymbolicLink())throw Error(`Quellen-Symlink benötigt eine ausdrückliche Quellenregel: ${file}`);
      if(item.isDirectory())walk(file);else files.push(file);
    }
  }
  for(const directory of ['src','src-tauri','tests','scripts','public','.cargo','.github'])if(fs.existsSync(path.join(root,directory)))walk(directory);
  for(const item of fs.readdirSync(root,{withFileTypes:true}))if(item.isFile()&&(/\.(?:json|m?js|ts|html|toml|lock|ya?ml)$/u.test(item.name)||['.gitignore','.npmrc'].includes(item.name)))files.push(item.name);
  const sourceFiles=[...new Set(files)].sort((a,b)=>Buffer.compare(Buffer.from(a),Buffer.from(b))).map(file=>({path:file,sha256:sha256(fs.readFileSync(path.join(root,file)))}));
  return {sourceFiles,sourceDigest:sha256(JSON.stringify(sourceFiles))};
}
