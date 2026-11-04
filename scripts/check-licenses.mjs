import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {validateLicenseInventory} from './license-inventory.mjs';
import {sourceFingerprint,sha256} from './source-fingerprint.mjs';

const result=spawnSync('cargo',['metadata','--manifest-path','src-tauri/Cargo.toml','--locked','--offline','--filter-platform','x86_64-pc-windows-msvc','--format-version','1'],{windowsHide:true,encoding:'utf8',maxBuffer:32*1024*1024,timeout:60000});
if(result.error||result.status!==0)throw Error(result.error?.message??result.stderr);
const metadata=JSON.parse(result.stdout),directory=process.argv[2]??'src-tauri/windows/third-party';
const report={...validateLicenseInventory(directory,{cargoLock:fs.readFileSync('src-tauri/Cargo.lock'),npmLock:fs.readFileSync('package-lock.json'),expectedCargo:metadata.packages}),testedAt:new Date().toISOString(),sourceDigest:sourceFingerprint().sourceDigest,metadataSha256:sha256(result.stdout),directory};
const output=process.argv[3]??'docs/evidence/native/license-inventory.json';fs.mkdirSync(path.dirname(output),{recursive:true});fs.writeFileSync(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report));
