(()=>{
 'use strict';
 const examples={
  gracias:{trigger:':gracias',text:'Gracias por tu tiempo.\nSeguimos en contacto.'},
  correo:{trigger:':correo',text:'hola@ejemplo.com'},
  firma:{trigger:':firma',text:'María López\nDiseño y comunicación\nmaria@ejemplo.com'},
  fecha:{trigger:':hoy',text:new Intl.DateTimeFormat('es-MX',{day:'numeric',month:'long',year:'numeric'}).format(new Date())},
 };
 let selected='gracias';let expanded=true;
 const trigger=document.getElementById('demo-trigger');
 const output=document.getElementById('demo-output');
 const expandButton=document.getElementById('demo-expand');
 function render(){const example=examples[selected];trigger.textContent=example.trigger;output.textContent=expanded?example.text:example.trigger;expandButton.textContent=expanded?'Ver el atajo':'Expandir el atajo';document.querySelectorAll('[data-example]').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.example===selected)));}
 document.querySelectorAll('[data-example]').forEach(button=>button.addEventListener('click',()=>{selected=button.dataset.example;expanded=true;render();}));
 expandButton.addEventListener('click',()=>{expanded=!expanded;render();});render();
 const config=window.KHIPU_CONFIG||{};
 function validUrl(value,hosts){if(typeof value!=='string')return null;try{const url=new URL(value);return url.protocol==='https:'&&hosts.includes(url.hostname)&&!url.username&&!url.password?url:null;}catch{return null;}}
 const github=validUrl(config.githubUrl,['github.com']);
 if(github){document.querySelectorAll('[data-github]').forEach(a=>a.href=github.href);document.querySelectorAll('[data-issues]').forEach(a=>a.href=github.href.replace(/\/$/,'')+'/issues');}
 const store=validUrl(config.storeUrl,['apps.microsoft.com','www.microsoft.com']);
 if(store){const link=document.getElementById('store-link');link.href=store.href;link.hidden=false;document.getElementById('store-pending').hidden=true;document.getElementById('install-note').textContent='Instalación desde la ficha oficial de Microsoft Store.';}
})();
