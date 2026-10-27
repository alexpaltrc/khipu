import http from 'node:http';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {readFile,stat} from 'node:fs/promises';
const root=fileURLToPath(new URL('./public/',import.meta.url));
const port=Number(process.env.PORT||4319);
const mime={'.html':'text/html; charset=utf-8','.css':'text/css; charset=utf-8','.js':'text/javascript; charset=utf-8','.svg':'image/svg+xml','.png':'image/png','.ico':'image/x-icon','.txt':'text/plain; charset=utf-8'};
http.createServer(async(req,res)=>{
 try{
  if(req.method!=='GET'&&req.method!=='HEAD'){res.writeHead(405,{Allow:'GET, HEAD'});res.end();return;}
  const pathname=decodeURIComponent(new URL(req.url,'http://127.0.0.1').pathname);
  const relative=pathname==='/'?'index.html':pathname.replace(/^\/+/,'');
  const target=path.resolve(root,relative);
  const relation=path.relative(root,target);
  if(relation.startsWith('..')||path.isAbsolute(relation)){res.writeHead(403);res.end('Forbidden');return;}
  let file=target;
  try{if((await stat(file)).isDirectory())file=path.join(file,'index.html');}catch{}
  const body=await readFile(file);
  res.writeHead(200,{'Content-Type':mime[path.extname(file)]||'application/octet-stream','Cache-Control':'no-store','X-Content-Type-Options':'nosniff'});
  res.end(req.method==='HEAD'?undefined:body);
 }catch{res.writeHead(404,{'Content-Type':'text/plain; charset=utf-8'});res.end('Página no encontrada');}
}).listen(port,'127.0.0.1',()=>console.log(`Khipu · http://127.0.0.1:${port}`));
