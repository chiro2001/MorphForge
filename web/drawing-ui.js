import {rgb8} from '/mesh-ui.js';

export function drawingPoint(text, name='坐标') {
  const values=String(text).trim().split(/[\s,，]+/).map(Number);
  if(values.length!==3||values.some(n=>!Number.isFinite(n)||Math.abs(n)>999000))throw Error(`${name}需要三个有限数，以逗号分隔`);
  return values;
}
export function demonstrationPrimitives() {
  return [
    {kind:'axes',position:[-48,8,0],length:40,width:2},
    {kind:'points',points:[[-24,20,0],[-8,40,0],[8,28,0],[24,56,0],[40,44,0]],size:6,color:[255,191,70]},
    {kind:'polyline',points:[[-24,20,0],[-8,40,0],[8,28,0],[24,56,0],[40,44,0]],width:2,color:[88,190,255]},
    {kind:'segment',start:[-24,12,-10],end:[40,12,-10],width:2,dashed:true,dashLength:6,gapLength:4,color:[170,170,190]},
    {kind:'arrow',start:[32,68,0],end:[24,58,0],width:2,color:[239,105,140]},
    {kind:'circle',position:[0,38,-12],radius:30,width:2,segments:32,color:[135,110,230]},
  ];
}

const form=`<header class="dialog-title"><h2>快速演示 · 点与线</h2><button id="drawClose" aria-label="关闭演示图元">×</button></header>
<div class="drawing-layout"><div class="drawing-fields">
<label>图元<select id="drawKind"><option value="point">点</option><option value="points">点集</option><option value="segment">线段</option><option value="polyline">折线</option><option value="arrow">箭头</option><option value="circle">圆环</option><option value="axes">XYZ坐标轴</option><option value="wire_box">线框盒</option></select></label>
<label>显示方式<select id="drawRepresentation"><option value="native">原生点线（快速发布）</option><option value="mesh">三角面（兼容旧头像）</option></select></label>
<label data-draw-field="position">中心／起点<input id="drawPosition" value="0, 24, 0"></label>
<label data-draw-field="start">起点<input id="drawStart" value="-24, 24, 0"></label>
<label data-draw-field="end">终点<input id="drawEnd" value="24, 24, 0"></label>
<label data-draw-field="points">坐标列表（每行 X,Y,Z）<textarea id="drawPoints" rows="4">-24, 16, 0\n0, 40, 0\n24, 16, 0</textarea></label>
<div class="double"><label data-draw-field="size">点直径／格<input id="drawSize" type="number" min="0.015625" max="4096" step="0.25" value="6"></label><label data-draw-field="width">线直径／格<input id="drawWidth" type="number" min="0.015625" max="4096" step="0.25" value="2"></label></div>
<label data-draw-field="shape">点形状<select id="drawShape"><option value="sphere">圆点</option><option value="cube">方点</option></select></label>
<div class="double"><label data-draw-field="radius">圆环半径／格<input id="drawRadius" type="number" min="2" value="24"></label><label data-draw-field="plane">平面<select id="drawPlane"><option value="xy">XY</option><option value="xz">XZ</option><option value="yz">YZ</option></select></label></div>
<label data-draw-field="length">轴长／格<input id="drawLength" type="number" min="1" value="48"></label>
<label data-draw-field="extent">线框尺寸<input id="drawExtent" value="32, 24, 24"></label>
<div class="double"><label>颜色<input id="drawColor" type="color" value="#58beff"></label><label>分组<input id="drawGroup" maxlength="128" value="demo"></label></div>
<label class="check" data-draw-field="closed"><input id="drawClosed" type="checkbox">闭合折线</label>
<details data-draw-field="lineOptions"><summary>线型与细分</summary><label class="check"><input id="drawDashed" type="checkbox">虚线</label><div class="double"><label>实线段长<input id="drawDashLength" type="number" min="1" value="8"></label><label>间隔<input id="drawGapLength" type="number" min="1" value="4"></label></div><div class="double"><label>截面细分<select id="drawSides"><option>4</option><option>6</option><option selected>8</option><option>12</option><option>16</option></select></label><label data-draw-field="segments">圆环细分<input id="drawSegments" type="number" min="8" max="128" value="32"></label></div></details>
<label class="check"><input id="drawReplace" type="checkbox">替换同名分组（其余作品保留）</label>
<p class="caption">大小和粗细使用逻辑格。端点对齐整数网格，原生直径精度1/64格；图形会随距离缩小。坐标轴固定使用红X／绿Y／蓝Z。</p>
</div><div class="drawing-preview"><div id="drawingViewport"></div><p id="drawMetrics" class="caption" role="status">预览仅显示本次图元，不修改作品。</p><p id="drawError" role="alert" hidden></p></div></div>
<footer class="row drawing-actions"><button id="drawExample">预览组合演示</button><span class="grow"></span><button id="drawPreview">预览</button><button id="drawApply" class="primary" disabled>加入作品</button></footer>`;

export function setupDrawingUi({$,api,getState,refresh,renderer,error,toast}) {
  const dialog=document.createElement('dialog');dialog.id='drawingDialog';dialog.innerHTML=form;document.body.append(dialog);
  let previewRenderer=null,pending=null,example=false,busy=false,serial=0,requestId='',lastProject='',lastRevision=-1;
  const fields={point:['position','size','shape'],points:['points','size','shape'],segment:['start','end','width','lineOptions'],polyline:['points','width','closed','lineOptions'],arrow:['start','end','width','lineOptions'],circle:['position','width','radius','plane','lineOptions','segments'],axes:['position','length','width','lineOptions'],wire_box:['position','extent','width','lineOptions']};
  function controls(){const state=getState(),locked=!!state?.harness?.manualEditLocked;$('drawPreview').disabled=busy||!state;$('drawExample').disabled=busy||!state;$('drawApply').disabled=busy||!pending||locked;$('drawApply').textContent=locked?'AI修改中，暂停后加入':'加入作品';}
  function invalidate(){serial++;pending=null;example=false;requestId='';controls();if(previewRenderer)$('drawMetrics').textContent='参数已变化，请重新预览。';}
  function showFields(){const active=new Set(fields[$('drawKind').value]||[]);for(const node of dialog.querySelectorAll('[data-draw-field]'))node.hidden=!active.has(node.dataset.drawField);}
  function readPrimitive(){const kind=$('drawKind').value,p={kind,color:rgb8($('drawColor').value)};for(const field of fields[kind]||[]){if(field==='lineOptions'){Object.assign(p,{dashed:$('drawDashed').checked,dashLength:Number($('drawDashLength').value),gapLength:Number($('drawGapLength').value),sides:Number($('drawSides').value)});}else if(['position','start','end','extent'].includes(field)){p[field]=drawingPoint($('draw'+field[0].toUpperCase()+field.slice(1)).value);}else if(field==='points'){p.points=$('drawPoints').value.trim().split(/\n|;/).filter(x=>x.trim()).map((x,i)=>drawingPoint(x,`第${i+1}行`));}else if(field==='closed'){p.closed=$('drawClosed').checked;}else if(['shape','plane'].includes(field)){p[field]=$('draw'+field[0].toUpperCase()+field.slice(1)).value;}else{const n=Number($('draw'+field[0].toUpperCase()+field.slice(1)).value);if(!Number.isFinite(n)||n<=0)throw Error('大小和粗细需要正数');p[field]=n;}}return p;}
  function request(){const state=getState();if(!state)throw Error('工程尚未加载');const group=$('drawGroup').value.trim();if(!group)throw Error('请输入演示分组名称');return {action:'preview',projectId:state.project.id,baseRevision:state.revision,group,representation:$('drawRepresentation').value,mode:$('drawReplace').checked?'replace_group':'append',primitives:example?demonstrationPrimitives():[readPrimitive()]};}
  function fail(cause){$('drawError').textContent=cause.message;$('drawError').hidden=false;}
  async function preview(useExample=false){if(busy)return;invalidate();example=useExample;busy=true;controls();$('drawError').hidden=true;const token=serial;try{const args=request(),result=await api('draw-primitives',args);if(token!==serial||!dialog.open)return;if(getState()?.project.id!==args.projectId||getState()?.revision!==args.baseRevision)throw Error('工程已变化，请重新预览');if(!previewRenderer){const {VoxelRenderer}=await import('/renderer.js');if(token!==serial||!dialog.open)return;previewRenderer=new VoxelRenderer($('drawingViewport'));previewRenderer.helpers.visible=false;}
      await previewRenderer.load({boxes:[],triangles:result.triangles,drawings:result.drawings||[],revision:args.baseRevision,cellSize:.25,size:256},[],{mode:'art',scale:1});if(token!==serial||!dialog.open)return;previewRenderer.view('hero');pending=args;requestId=crypto.randomUUID();const r=result.report,t=result.target;$('drawMetrics').textContent=`${useExample?'组合演示 · ':''}${r.primitiveCount}个图元 · ${r.drawingCount||0}个原生点线 · ${r.triangleCount}面 · 坐标取整最大${Number(r.maximumCoordinateRounding).toFixed(2)}格${r.skippedCollapsedFaces?' · 已去除'+r.skippedCollapsedFaces+'个退化面':''} · ${t.fits?'当前作品满足目标范围/容量':'当前作品超出Avatar范围或容量，仍可保存草稿'}。右侧仅显示本次图元。`;
    }catch(cause){fail(cause);}finally{busy=false;controls();}}
  $('drawPreview').onclick=()=>preview(false);$('drawExample').onclick=()=>preview(true);
  $('drawApply').onclick=async()=>{if(busy||!pending)return;const args=pending,state=getState();if(state?.project.id!==args.projectId||state?.revision!==args.baseRevision){invalidate();fail(Error('工程已变化，请重新预览'));return;}busy=true;controls();$('drawError').hidden=true;try{const result=await api('draw-primitives',{...args,action:'apply',requestId});pending=null;for(let i=0;i<80;i++){await refresh(true);if(getState()?.project.id!==args.projectId)break;if(renderer.renderRevision>=result.revision){renderer.view('hero');break;}await new Promise(resolve=>setTimeout(resolve,100));}toast(`已加入${result.report.primitiveCount}个演示图元，${result.report.drawingCount||0}个原生点线、${result.report.triangleCount}面，可撤销`);if(result.publishWarning)fail(Error('草稿已保存；'+result.publishWarning));else{dialog.close();}}catch(cause){fail(cause);}finally{busy=false;controls();}};
  for(const node of dialog.querySelectorAll('input,select,textarea'))node.addEventListener('input',()=>{invalidate();if(node.id==='drawKind')showFields();});
  $('drawClose').onclick=()=>dialog.close();dialog.addEventListener('click',event=>{if(event.target!==dialog)return;const r=dialog.getBoundingClientRect();if(event.clientX<r.left||event.clientX>r.right||event.clientY<r.top||event.clientY>r.bottom)dialog.close();});
  dialog.addEventListener('close',()=>{invalidate();previewRenderer?.dispose();previewRenderer=null;$('drawMetrics').textContent='预览仅显示本次图元，不修改作品。';});
  const open=()=>{if(dialog.open){dialog.close();return;}lastProject=getState()?.project.id||'';lastRevision=getState()?.revision??-1;showFields();controls();$('drawError').hidden=true;dialog.showModal();};
  $('drawingOpen').onclick=open;document.querySelectorAll('[data-command="drawingOpen"]').forEach(b=>b.onclick=()=>{b.closest('details')?.removeAttribute('open');open();});
  return {sync(){const state=getState();if(state?.project.id!==lastProject||state?.revision!==lastRevision){lastProject=state?.project.id||'';lastRevision=state?.revision??-1;if(pending)invalidate();}controls();}};
}
