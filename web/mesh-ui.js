export function integerPoint(text, name = "坐标", positive = false) {
  const values = String(text).trim().split(/[\s,，]+/).map(Number);
  if (values.length !== 3 || values.some(n => !Number.isSafeInteger(n) || Math.abs(n) > 1000000 || (positive && n < 1)))
    throw Error(`${name}需要三个${positive ? "正" : "有界"}整数，以逗号分隔`);
  return values;
}
export function rgb8(hex) {
  if (!/^#[\da-f]{6}$/i.test(hex)) throw Error("颜色需要#RRGGBB");
  return [1,3,5].map(i=>parseInt(hex.slice(i,i+2),16));
}
export function setupMeshUi({$, getScene, getState, getSelected, api, edit, refresh, error}) {
  const bind = (id, callback) => { $(id).onclick = async () => {try {await callback();} catch(cause) {error(cause);} }; };
  const face = () => ({vertices:["meshA","meshB","meshC"].map(id=>integerPoint($(id).value)),color:rgb8($("meshColor").value),doubleSided:$("meshDoubleSided").checked,group:$("meshGroup").value.trim()||"polygon"});
  const selectedFace = () => {
    const faces=(getScene()?.scene.triangles||[]).filter(t=>getSelected().has(t.id));
    if(faces.length!==1)throw Error("请只选择一个三角面");return faces[0];
  };
  bind("meshAdd",()=>edit({op:"triangle",...face()}));
  bind("meshUpdate",()=>edit({op:"triangle",...face(),id:selectedFace().id}));
  bind("meshLoadSelection",()=>{
    const selected=selectedFace();["meshA","meshB","meshC"].forEach((id,i)=>{$(id).value=selected.vertices[i].join(", ");});
    $("meshColor").value="#"+selected.color.map(v=>v.toString(16).padStart(2,"0")).join("");$("meshDoubleSided").checked=!!selected.doubleSided;$("meshGroup").value=selected.group;
  });
  bind("meshDelete",()=>{
    const ids=(getScene()?.scene.triangles||[]).filter(t=>getSelected().has(t.id)).map(t=>t.id);
    if(!ids.length)throw Error("请先选择三角面");return edit({op:"remove_triangles",ids});
  });
  bind("meshWedge",async()=>{
    const state=getState();if(!state)throw Error("工程尚未加载");
    const result=await api("mesh",{action:"wedge",baseRevision:state.revision,projectId:state.project.id,min:integerPoint($("wedgeMin").value),size:integerPoint($("wedgeSize").value,"尺寸",true),direction:$("wedgeDirection").value,color:rgb8($("meshColor").value),group:$("meshGroup").value.trim()||"slope",doubleSided:$("meshDoubleSided").checked});
    await refresh(true);if(result.warning)throw Error(result.warning);
  });
  $("modelingStyle").onchange=async()=>{try{await api("project",{modelingStyle:$("modelingStyle").value,baseRevision:getState().revision});await refresh();}catch(cause){error(cause);}};
}
