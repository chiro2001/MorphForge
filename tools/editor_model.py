"""Bounded, disjoint cuboid document for the manual editor; no dense voxel allocation."""
import copy
from compile_stage1 import descriptor,intersects,subtract_box

CAPACITY=64
FORMAT="MorphForgeEditor/1"


def geometry_key(box):return tuple(box["min"]+box["size"])
def full_key(box):return geometry_key(box)+(box["styleId"],box["tintId"],box["motionPhase"])
def volume(box):return box["size"][0]*box["size"][1]*box["size"][2]


def merge_boxes(boxes):
    """Merge complete adjacent faces with identical appearance, preserving exact occupied cells."""
    boxes=[descriptor(b) for b in boxes]
    while True:
        changed=False
        for axis in (0,2,1):
            groups={}
            for b in boxes:
                group=tuple((b["min"][i],b["size"][i]) for i in range(3) if i!=axis)+(b["styleId"],b["tintId"],b["motionPhase"])
                groups.setdefault(group,[]).append(b)
            output=[]
            for values in groups.values():
                values.sort(key=lambda b:b["min"][axis]);current=copy.deepcopy(values[0])
                for b in values[1:]:
                    if current["min"][axis]+current["size"][axis]==b["min"][axis]:current["size"][axis]+=b["size"][axis];changed=True
                    else:output.append(current);current=copy.deepcopy(b)
                output.append(current)
            boxes=output
        if not changed:return sorted(boxes,key=full_key)


class BoxDocument:
    def __init__(self,size=16,boxes=None):
        self.size=size;self.boxes=[];self.version=0;self.undo_stack=[];self.redo_stack=[]
        self.validate_size(size)
        if boxes:self.boxes=self.validate_boxes(boxes)
    @staticmethod
    def validate_size(size):
        if type(size) is not int or size not in (8,16,32,64,128,256):raise ValueError("网格边长须为 8、16、32、64、128 或 256")
    def validate_boxes(self,boxes,size=None):
        size=self.size if size is None else size;lo=128-size//2;hi=128+size//2
        clean=[descriptor(b) for b in boxes]
        for b in clean:
            if not (lo<=b["min"][0] and b["min"][0]+b["size"][0]<=hi and lo<=b["min"][2] and b["min"][2]+b["size"][2]<=hi and b["min"][1]+b["size"][1]<=size):raise ValueError("操作超出当前网格")
        for i,b in enumerate(clean):
            if any(intersects(b,other) for other in clean[i+1:]):raise ValueError("导入的盒体存在重叠")
        clean=merge_boxes(clean)
        if len(clean)>CAPACITY:raise ValueError("当前 Avatar 最多显示 64 个合并盒体，请简化样式或删除部分内容")
        return clean
    def commit(self,boxes,size=None,history=True):
        size=self.size if size is None else size;self.validate_size(size);boxes=self.validate_boxes(boxes,size)
        if size==self.size and [full_key(b) for b in boxes]==[full_key(b) for b in self.boxes]:return False
        if history:
            self.undo_stack.append((self.size,copy.deepcopy(self.boxes)));self.undo_stack=self.undo_stack[-32:];self.redo_stack.clear()
        self.size=size;self.boxes=boxes;self.version+=1;return True
    def apply(self,action):
        op=action.get("op")
        if op=="clear":return self.commit([])
        if op=="size":return self.commit(self.boxes,action.get("size"))
        if op in ("undo","redo"):
            source=self.undo_stack if op=="undo" else self.redo_stack;target=self.redo_stack if op=="undo" else self.undo_stack
            if not source:return False
            size,boxes=source.pop();target.append((self.size,copy.deepcopy(self.boxes)));return self.commit(boxes,size,False)
        if op=="import":
            value=action.get("document",{})
            if value.get("format")!=FORMAT:raise ValueError("不是 MorphForge 编辑器文件")
            if not isinstance(value.get("boxes"),list) or len(value["boxes"])>256:raise ValueError("导入数据过大")
            return self.commit(value["boxes"],value.get("size",16))
        if op=="demo":return self.demo(action.get("name","stairs"))
        if op not in ("box","erase"):raise ValueError("未知编辑操作")
        minimum=action.get("min");size=action.get("size",[1,1,1])
        if not isinstance(minimum,list) or len(minimum)!=3 or any(type(v) is not int for v in minimum):raise ValueError("位置须为三个整数")
        b=descriptor(dict(action,min=[minimum[0]+128,minimum[1],minimum[2]+128],size=size))
        self.validate_boxes([b])
        result=[part for old in self.boxes for part in subtract_box(old,b)]
        if op=="box":result.append(b)
        return self.commit(merge_boxes(result))
    def demo(self,name):
        if name=="stairs":
            size=max(16,self.size)
            boxes=[descriptor({"min":[120+i*2,0,128],"size":[2,i+1,2],"styleId":i%4+1}) for i in range(8)]
        elif name=="house":
            size=max(16,self.size);boxes=[]
            def put(minimum,extent,style):
                nonlocal boxes
                b=descriptor({"min":minimum,"size":extent,"styleId":style})
                boxes=[p for old in boxes for p in subtract_box(old,b)]+[b]
            put([124,0,124],[8,1,8],1)
            put([124,1,124],[8,5,1],2);put([124,1,131],[8,5,1],2)
            put([124,1,125],[1,5,6],2);put([131,1,125],[1,5,6],2)
            cut=descriptor({"min":[127,1,131],"size":[2,3,1]});boxes=[p for old in boxes for p in subtract_box(old,cut)]
            for z in (124,131):put([125,3,z],[1,2,1],3)
            for i in range(3):put([123+i,6+i,123],[10-i*2,1,10],4)
        else:raise ValueError("未知示例")
        return self.commit(merge_boxes(boxes),size)
    def export(self):return {"format":FORMAT,"size":self.size,"origin":"bottom-center","cellSize":.25,"boxes":copy.deepcopy(self.boxes)}


def wire_box(box,profile):
    b=copy.deepcopy(box)
    if profile=="stage2":b["min"][1]+=128
    elif profile!="anchor":raise ValueError("未知 Avatar 配置")
    return descriptor(b)


def next_command(sent,target):
    old={geometry_key(b):b for b in sent};new={geometry_key(b):b for b in target}
    removed=sorted(set(old)-set(new))
    if removed:return {"op":"remove",**copy.deepcopy(old[removed[0]])}
    for key in sorted(new):
        if key not in old or full_key(old[key])!=full_key(new[key]):return {"op":"box",**copy.deepcopy(new[key])}
    return None
