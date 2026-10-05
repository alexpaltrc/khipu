import fs from 'node:fs/promises';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(import.meta.url);
const sharp=require('sharp');
export const root=path.resolve(process.argv[2] ?? path.join(import.meta.dirname,'..')); 
const out=root;
for(const d of ['fuentes','app','bandeja','msix/Assets','store','presentacion'])await fs.mkdir(path.join(out,d),{recursive:true});
export const tokens={brand:{violet:'#A99BE7',ink:'#211B37'},themes:{dark:{surface:'#25272D',active:'#F3F4F7',paused:'#9497A1'},light:{surface:'#ECEEF2',active:'#22252C',paused:'#747781'}},presentation:{background:'#17181B'}};
const body='M7 24 C6 18 12 19 12 14 C12 7 20 6 26 9 C30 11 28 20 25 23 C25 28 18 31 13 29 C11 33 7 32 8 27 C3 25 4 20 7 24 Z';
export function symbol(color='#211B37',optical=false){
 const eye=optical?2.45:2.1,w=optical?2.5:1.8,k=optical?2.6:2.35;
 return `<g fill="${color}"><path fill-rule="evenodd" d="${body} M${22.1+eye} 14.1 a${eye} ${eye} 0 1 0 ${-2*eye} 0 a${eye} ${eye} 0 1 0 ${2*eye} 0 Z"/><path d="M26 17 L33 18 L26 21 Z"/><path d="M4 32 C12 35 25 31 35 32 M17 29 L17 33" fill="none" stroke="${color}" stroke-width="${w}" stroke-linecap="round"/><circle cx="10.85" cy="33.95" r="${k}"/><circle cx="29.35" cy="31.85" r="${k}"/></g>`;
}
export function svg(content,view='0 0 40 40',width=40,height=40){return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="${view}">${content}</svg>`;}
export function appSvg(n=256,full=false){return svg(`<rect width="40" height="40" rx="${full?0:8.8}" fill="${tokens.brand.violet}"/><g transform="translate(3.95 3.49) scale(.83)">${symbol(tokens.brand.ink,n<=24)}</g>`);}
export async function png(s,n,file){await sharp(Buffer.from(s)).resize(n,n).png().toFile(file);}
await fs.writeFile(path.join(out,'fuentes/khipu-simbolo-maestro.svg'),svg(symbol()));
await fs.writeFile(path.join(out,'fuentes/khipu-aplicacion.svg'),appSvg());
await fs.writeFile(path.join(out,'fuentes/khipu-simbolo-optico-16-24.svg'),svg(symbol('#211B37',true)));
await fs.writeFile(path.join(out,'fuentes/tokens.json'),JSON.stringify({...tokens,pausedStatus:'Propuesta de atenuación refinada, tono exacto pendiente de aprobación',uiAccent:'Sigue Windows según configuración existente. Estos tokens no modifican la interfaz.'},null,2));
const appSizes=[16,20,24,32,40,48,64,128,256];
const icoPNGs=[];
for(const n of appSizes)icoPNGs.push({n,data:await sharp(Buffer.from(appSvg(n))).resize(n,n).png().toBuffer()});
function ico(images){let offset=6+16*images.length;const header=Buffer.alloc(offset);header.writeUInt16LE(1,2);header.writeUInt16LE(images.length,4);images.forEach(({n,data},i)=>{const at=6+i*16;header[at]=n===256?0:n;header[at+1]=header[at];header.writeUInt16LE(1,at+4);header.writeUInt16LE(32,at+6);header.writeUInt32LE(data.length,at+8);header.writeUInt32LE(offset,at+12);offset+=data.length;});return Buffer.concat([header,...images.map(x=>x.data)]);}
await fs.writeFile(path.join(out,'app/Khipu.ico'),ico(icoPNGs));
await png(appSvg(),256,path.join(out,'app/Khipu-256.png'));

const manifest=[];
async function asset(file,s,w,h=w,role=''){await sharp(Buffer.from(s)).resize(w,h).png().toFile(path.join(out,file));manifest.push({file,width:w,height:h,role});}
for(const theme of ['light','dark'])for(const state of ['active','paused'])for(const n of [16,20,24,32,40,48,64])await asset(`bandeja/khipu-${theme}-${state}-${n}.png`,svg(symbol(tokens.themes[theme][state],n<=24)),n,n,`Bandeja ${theme}, ${state}, tamaño físico ${n}`);
for(const [name,baseW,baseH] of [['Square44x44Logo',44,44],['Square150x150Logo',150,150],['Wide310x150Logo',310,150],['StoreLogo',50,50]])for(const scale of [100,125,150,200,400]){
 const w=Math.floor(baseW*scale/100+.5),h=Math.floor(baseH*scale/100+.5);
 let source=appSvg(w);
 if(baseH===150){const unit=100;const x=(baseW-unit)/2,y=(baseH-unit)/2;source=svg(`<rect width="${baseW}" height="${baseH}" fill="${tokens.brand.violet}"/><g transform="translate(${x} ${y}) scale(${unit/40})">${symbol()}</g>`,`0 0 ${baseW} ${baseH}`,baseW,baseH);}
 await asset(`msix/Assets/${name}.scale-${scale}.png`,source,w,h,`Manifiesto: ${name}, escala ${scale}%`);
}
for(const n of [16,20,24,30,32,36,40,48,60,64,72,80,96,256])for(const suffix of ['','_altform-unplated','_altform-lightunplated'])await asset(`msix/Assets/Square44x44Logo.targetsize-${n}${suffix}.png`,appSvg(n),n,n,`AppList ${n}, ${suffix||'default'}; fondo propio de marca, esquinas transparentes`);
await asset('store/Khipu-app-tile-300.png',appSvg(300,true),300,300,'Ficha Microsoft Store: 1:1 App tile icon');
await fs.writeFile(path.join(out,'inventario.json'),JSON.stringify(manifest,null,2));

console.log(`Generados ${manifest.length} PNG y un ICO.`);
