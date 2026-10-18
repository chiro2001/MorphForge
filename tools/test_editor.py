#!/usr/bin/env python3
"""Independent dense-cell oracle for cuboid edits plus paced writer convergence checks."""
import copy,json,random,struct,tempfile,time,unittest
from pathlib import Path
from editor_model import BoxDocument,geometry_key,full_key,wire_box
from editor_server import EditorEngine,placement_packet

def cells(boxes):
    out={}
    for b in boxes:
        x,y,z=b['min'];sx,sy,sz=b['size'];appearance=(b['styleId'],b['tintId'],b['motionPhase'])
        for i in range(x,x+sx):
            for j in range(y,y+sy):
                for k in range(z,z+sz):
                    assert (i,j,k) not in out,'Overlapping cuboids'
                    out[i,j,k]=appearance
    return out
class ModelTests(unittest.TestCase):
    def test_random_edits_match_dense_oracle_and_reject_atomically(self):
        rng=random.Random(1283);d=BoxDocument(8);expected={};rejected=0
        for _ in range(250):
            p=[rng.randrange(-4,3),rng.randrange(0,7),rng.randrange(-4,3)];s=[rng.randint(1,2) for _ in range(3)];op=rng.choice(['box','box','erase']);style=rng.randint(1,4)
            before=copy.deepcopy(d.__dict__);next_expected=expected.copy()
            for x in range(p[0]+128,p[0]+128+s[0]):
                for y in range(p[1],p[1]+s[1]):
                    for z in range(p[2]+128,p[2]+128+s[2]):
                        if op=='box':next_expected[x,y,z]=(style,0,0)
                        else:next_expected.pop((x,y,z),None)
            try:d.apply(dict(op=op,min=p,size=s,styleId=style))
            except ValueError:
                rejected+=1;self.assertEqual(d.__dict__,before)
            else:expected=next_expected
            self.assertEqual(cells(d.boxes),expected)
        self.assertGreater(rejected,0)
    def test_large_cube_has_one_descriptor(self):
        d=BoxDocument(256);d.apply(dict(op='box',min=[-128,0,-128],size=[256]*3,styleId=4095,tintId=63,motionPhase=63))
        self.assertEqual(len(d.boxes),1);self.assertEqual(d.boxes[0]['size'],[256]*3)
        before=d.export();d.apply(dict(op='erase',min=[-1,1,-1],size=[2]*3));self.assertEqual(len(d.boxes),6)
        d.apply(dict(op='undo'));self.assertEqual(d.export(),before)
        self.assertRaises(ValueError,wire_box,d.boxes[0],'stage2')
    def test_paint_merge_style_and_export(self):
        d=BoxDocument();d.apply(dict(op='box',min=[0,0,0],size=[3,3,3],styleId=1));d.apply(dict(op='box',min=[1,1,1],styleId=2))
        self.assertEqual(len(cells(d.boxes)),27);self.assertEqual(cells(d.boxes)[129,1,129],(2,0,0))
        d.apply(dict(op='box',min=[1,1,1],styleId=1));self.assertEqual(len(d.boxes),1)
        self.assertEqual(BoxDocument(d.size,d.export()['boxes']).export(),d.export())
    def test_invalid_import_no_mutation(self):
        d=BoxDocument();d.demo('house');before=copy.deepcopy(d.__dict__)
        document=d.export();document['boxes'].append(document['boxes'][0]);self.assertRaises(ValueError,d.apply,dict(op='import',document=document));self.assertEqual(d.__dict__,before)
class WriterTests(unittest.TestCase):
    def wait(self,predicate):
        until=time.monotonic()+5
        while time.monotonic()<until:
            if predicate():return
            time.sleep(.01)
        self.fail('OSC writer did not converge')
    def test_disabled_by_default_coalesces_and_paces(self):
        records=[]
        class FakeChannel:
            def __enter__(self):return self
            def __exit__(self,*a):pass
            def send(self,data,port):records.append((time.monotonic(),data,port))
        e=EditorEngine(channel_factory=FakeChannel)
        try:
            e.edit(dict(op='demo',name='stairs'));time.sleep(.06);self.assertEqual(records,[])
            e.configure(dict(enabled=True,port=9012,interval=.05));e.edit(dict(op='demo',name='house'));e.edit(dict(op='erase',min=[-1,0,-1],size=[2,2,2]))
            self.wait(lambda:e.state()['connection']['sentVersion']==e.document.version)
            self.assertEqual(sorted(map(full_key,e.sent)),sorted(map(full_key,e.document.boxes)))
            e.configure({'auto':False});e.edit(dict(op='clear'))
            self.wait(lambda:e.state()['connection']['sentVersion']==e.document.version)
            self.assertEqual(e.sent,[])
            e.edit(dict(op='box',min=[0,0,0]));self.assertEqual(e.sent,[])
            e.configure({'auto':True});self.wait(lambda:e.state()['connection']['sentVersion']==e.document.version)
            self.assertEqual(len(e.sent),1)
            self.assertTrue(all(b[0]-a[0]>=.048 for a,b in zip(records,records[1:])))
            e.configure({'enabled':False});n=len(records);time.sleep(.1);self.assertEqual(len(records),n)
        finally:e.close()
        self.assertFalse(e.thread.is_alive())
    def test_channel_busy_reports_error(self):
        class Busy:
            def __enter__(self):raise RuntimeError('already owned')
            def __exit__(self,*a):pass
        e=EditorEngine(channel_factory=Busy)
        try:
            e.configure({'enabled':True});self.wait(lambda:bool(e.state()['connection']['error']));self.assertFalse(e.enabled);self.assertTrue(e.thread.is_alive())
        finally:e.close()
    def test_placement_serialization(self):
        data=placement_packet({'MF_World':True,'MF_Handle':False,'MF_Scale':.25,'MF_Grab':2});self.assertIn(b',T\0\0',data);self.assertIn(b',F\0\0',data);self.assertIn(struct.pack('>f',.25),data)
        self.assertRaises(ValueError,placement_packet,{'MF_Scale':float('nan')})
    def test_persist_draft_restarts_offline(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'document.json';e=EditorEngine(path)
            try:e.edit({'op':'demo','name':'house'});e.place({'scale':.25});expected=e.document.export()
            finally:e.close()
            e=EditorEngine(path)
            try:self.assertFalse(e.enabled);self.assertEqual(e.document.export(),expected);self.assertEqual(e.placement['scale'],.25)
            finally:e.close()
if __name__=='__main__':unittest.main(verbosity=2)
