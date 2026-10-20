import { VoxelRenderer } from "/renderer.js";
import { integerPoint } from "/mesh-ui.js";
export function setupMinecraftUi({$,api,getState,getMaterials,refresh,error,toast}) {
  const dialog=$("minecraftDialog");let viewer=null,preview=null,busy=false,generation=0;
  const invalidate=()=>{generation++;preview=null;$("mcApply").disabled=true;};
  function buttons(){ $("mcPreview").disabled=busy||!$("mcSource").value;$("mcUpload").disabled=busy;$("mcApply").disabled=busy||!preview; }
  function report(value){$("mcReport").textContent=JSON.stringify(value,null,2);}
  async function sources(selected){
    const value=await api("minecraft");$("mcSource").replaceChildren();
    for(const s of value.sources){const o=document.createElement("option");o.value=s.id;o.textContent=`${s.name} · ${s.sourceBlocks} 块 · ${s.format}`;$("mcSource").append(o);}
    if(selected)$("mcSource").value=selected;
    $("mcOriginal").hidden=!$("mcSource").value;$("mcOriginal").href="/api/minecraft/source/"+$("mcSource").value;buttons();
  }
  async function perform(fn){if(busy)return;busy=true;buttons();try{await fn();}catch(cause){report({error:cause.message});error(cause);}finally{busy=false;buttons();}}
  $("minecraftOpen").onclick=()=>perform(async()=>{
    invalidate();dialog.showModal();
    $("mcPack").replaceChildren(new Option("内置原创 Minecraft 风格","builtin"));
    const packs=new Map();for(const m of getMaterials()||[])if(m.pack&&!m.pack.startsWith("builtin"))packs.set(m.pack,m.pack);
    for(const [id] of packs)$("mcPack").append(new Option(id.slice(0,16),id));
    if(!viewer)viewer=new VoxelRenderer($("mcViewport"));
    await sources();report({提示:"上传或选择建筑源，然后生成独立预览。预览不修改当前工程。"});
  });
  $("mcClose").onclick=()=>dialog.close();
  dialog.addEventListener("close",()=>{invalidate();viewer?.dispose();viewer=null;});
  $("mcUpload").onclick=()=>$("mcFile").click();
  $("mcFile").onchange=()=>perform(async()=>{
    const file=$("mcFile").files[0];if(!file)return;invalidate();
    if(file.size>64*1024*1024)throw Error("建筑文件不能超过64MiB");
    const data=new FormData();data.append("file",file);const response=await fetch("/api/minecraft/upload",{method:"POST",body:data});
    const value=await response.json();if(!response.ok||value.ok===false)throw Error(value.error||"建筑导入失败");
    await sources(value.source.id);report(value.source);$("mcFile").value="";
  });
  for(const id of ["mcSource","mcScale","mcPack","mcMode","mcOffset","mcCenter","mcSurface"]){$(id).addEventListener("change",()=>{invalidate();buttons();$("mcOriginal").hidden=!$("mcSource").value;$("mcOriginal").href="/api/minecraft/source/"+$("mcSource").value;});}
  $("mcInspect").onclick=()=>perform(async()=>report(await api("minecraft",{action:"inspect",sourceId:$("mcSource").value,limit:128})));
  $("mcPreview").onclick=()=>perform(async()=>{
    invalidate();const ticket=generation,state=getState();
    const value=await api("minecraft",{action:"preview",sourceId:$("mcSource").value,projectId:state.project.id,baseRevision:state.revision,options:{blockScale:Number($("mcScale").value),resourcePack:$("mcPack").value,mode:$("mcMode").value,offset:integerPoint($("mcOffset").value),center:$("mcCenter").checked,surfaceOnly:$("mcSurface").checked}});
    if(ticket!==generation||!dialog.open)return;
    const scene=await api("minecraft/preview/"+value.previewId);
    if(ticket!==generation||!viewer||!dialog.open)return;
    await viewer.load(scene.scene,scene.materials,{mode:"art",scale:1});
    if(ticket!==generation||!viewer)return;viewer.view();report(value.report);
    if(value.canApply)preview={previewId:value.previewId,baseRevision:value.baseRevision,projectId:value.projectId,requestId:crypto.randomUUID()};
  });
  $("mcApply").onclick=()=>perform(async()=>{
    if(!preview)return;const result=await api("minecraft",{action:"apply",...preview});invalidate();await refresh(true);report(result);toast("建筑已加入草稿，可撤销" );
  });
}
