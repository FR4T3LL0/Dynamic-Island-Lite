import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {sha256} from './source-fingerprint.mjs';

export function cargoChecksums(bytes){
  const result=new Map();
  for(const section of String(bytes).split('[[package]]').slice(1)){
    const field=name=>section.match(new RegExp('^'+name+' = "([^"]+)"','mu'))?.[1];
    const checksum=field('checksum');if(!checksum)continue;
    const id=`${field('name')}@${field('version')}`;assert(!result.has(id),`Mehrdeutiges Cargo-Paket: ${id}`);result.set(id,checksum);
  }
  return result;
}
export function validateLicenseInventory(directory,{cargoLock,npmLock,expectedCargo}){
  const root=fs.realpathSync(directory),read=name=>JSON.parse(fs.readFileSync(path.join(root,name),'utf8'));
  const inventory=read('inventory.json'),integrity=read('integrity.json'),locked=cargoChecksums(cargoLock);
  assert.equal(inventory.schemaVersion,1);assert.equal(integrity.schemaVersion,1);assert.equal(inventory.target,'x86_64-pc-windows-msvc');
  assert.equal(inventory.cargoLockSha256,sha256(cargoLock),'Cargo-Lockfile hat sich geändert');assert.equal(inventory.npmLockSha256,sha256(npmLock),'npm-Lockfile hat sich geändert');
  const records=new Map();
  for(const item of integrity.files){
    assert.equal(typeof item.path,'string');assert(!item.path.includes('\\')&&!/^(?:[A-Za-z]:|\/)/u.test(item.path)&&item.path.split('/').every(part=>part&&part!=='.'&&part!=='..'),'Ungültiger Lizenzpfad');
    assert(item.path!=='integrity.json');assert(!records.has(item.path),'Doppelter Lizenzpfad');
    const file=path.join(root,item.path);assert(!fs.lstatSync(file).isSymbolicLink(),'Lizenz-Symlink');assert(fs.realpathSync(file).startsWith(root+path.sep),'Lizenzpfad außerhalb der Sammlung');
    const bytes=fs.readFileSync(file);assert.equal(bytes.length,item.bytes);assert.equal(sha256(bytes),item.sha256,`Lizenzdatei verändert: ${item.path}`);records.set(item.path,item);
  }
  const actual=[];
  function walk(relative=''){for(const item of fs.readdirSync(path.join(root,relative),{withFileTypes:true})){assert(!item.isSymbolicLink(),'Lizenz-Symlink');const name=path.posix.join(relative,item.name);if(item.isDirectory())walk(name);else if(name!=='integrity.json')actual.push(name);}}walk();
  assert.deepEqual(actual.sort(),[...records.keys()].sort(),'Dateiliste der Lizenzsammlung stimmt nicht überein');
  const verifyRecord=record=>{assert(records.has(record.path),`Datei nicht im Lizenzindex: ${record.path}`);assert.equal(record.sha256,records.get(record.path).sha256);assert.equal(record.bytes,records.get(record.path).bytes);};
  const cargoIds=new Set();let mplArchives=0;
  for(const pkg of inventory.cargo){
    const id=`${pkg.name}@${pkg.version}`;assert(!cargoIds.has(id));cargoIds.add(id);assert.equal(pkg.crateChecksum,locked.get(id),`Cargo-Prüfsumme fehlt: ${id}`);assert(pkg.license&&pkg.licenses.length);
    assert(['release-normal-dependency-closure','release-build-dependency-closure','test-or-other-resolved-windows-dependency'].includes(pkg.scope));pkg.licenses.forEach(verifyRecord);
    if(pkg.license.includes('MPL-2.0')){assert(pkg.sourceArchive?.unmodified);verifyRecord(pkg.sourceArchive);assert.equal(pkg.sourceArchive.sha256,locked.get(id),`MPL-Quellarchiv verändert: ${id}`);mplArchives++;}
  }
  const expected=expectedCargo.filter(pkg=>pkg.source).map(pkg=>({name:pkg.name,version:pkg.version,license:pkg.license}));
  const tuple=pkg=>JSON.stringify([pkg.name,pkg.version,pkg.license]);assert.deepEqual(inventory.cargo.map(tuple).sort(),expected.map(tuple).sort(),'Windows-Cargo-Auflösung stimmt nicht mit dem Inventar überein');
  const npm=JSON.parse(String(npmLock));
  const npmTuple=pkg=>JSON.stringify([pkg.name,pkg.version,pkg.license,Boolean(pkg.dev)]);
  const npmExpected=Object.entries(npm.packages).filter(([file])=>file).map(([file,pkg])=>({...pkg,name:pkg.name??file.split('node_modules/').at(-1)}));
  const npmActual=inventory.npm.map(pkg=>({...pkg,dev:pkg.scope==='development-only-not-shipped'}));assert.deepEqual(npmActual.map(npmTuple).sort(),npmExpected.map(npmTuple).sort(),'npm-Auflösung stimmt nicht mit dem Inventar überein');
  for(const pkg of inventory.npm){assert(['development-only-not-shipped','frontend-production'].includes(pkg.scope));if(pkg.scope==='frontend-production')assert(pkg.licenses.length,`Produktionslizenz fehlt: ${pkg.name}`);pkg.licenses.forEach(verifyRecord);}
  for(const component of inventory.nativeComponents??[]){assert(component.licenses.length);component.licenses.forEach(verifyRecord);(component.metadata??[]).forEach(verifyRecord);}
  const webview=expectedCargo.find(pkg=>pkg.name==='webview2-com-sys');
  if(webview){
    const loader=inventory.nativeComponents?.find(component=>component.id==='microsoft-webview2-loader');assert(loader,'Statisch eingebundener Microsoft-WebView2-Loader fehlt im Lizenzinventar');
    assert.equal(loader.binary.cratePackage,webview.name);assert.equal(loader.binary.crateVersion,webview.version);assert.equal(loader.binary.entry,'x64/WebView2LoaderStatic.lib');
    assert(loader.licenses.some(file=>file.path.endsWith('/LICENSE.txt'))&&loader.licenses.some(file=>file.path.endsWith('/NOTICE.txt')),'Microsoft SDK LICENSE oder NOTICE fehlt');
    assert(loader.metadata?.some(file=>file.path.endsWith('/provenance.json')),'Herkunft des Microsoft-Loaders fehlt');
    const bytes=fs.readFileSync(path.join(path.dirname(webview.manifest_path),loader.binary.entry));assert.equal(bytes.length,loader.binary.bytes);assert.equal(sha256(bytes),loader.binary.sha256,'Statisch eingebundener Microsoft-Loader hat sich geändert');
  }
  assert(inventory.rustStandardLibrary.compiler.startsWith('rustc '));assert(inventory.rustStandardLibrary.files.some(file=>file.path==='rust-standard-library/COPYRIGHT-library.html'));inventory.rustStandardLibrary.files.forEach(verifyRecord);
  assert(records.has('THIRD-PARTY-NOTICES.md'));assert(records.has('UPSTREAM-PROVENANCE.json'));
  return {passed:true,files:records.size+1,bytes:[...records.values()].reduce((sum,file)=>sum+file.bytes,0),cargo:inventory.cargo.length,npm:inventory.npm.length,npmProduction:inventory.npm.filter(pkg=>pkg.scope==='frontend-production').length,mplArchives,nativeComponents:inventory.nativeComponents?.length??0,inventorySha256:sha256(fs.readFileSync(path.join(root,'inventory.json'))),integritySha256:sha256(fs.readFileSync(path.join(root,'integrity.json')))};
}
