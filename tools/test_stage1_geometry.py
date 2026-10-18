import itertools
import random
import unittest

from compile_stage1 import compile_commands, demo, subtract_box


def cells(b):
    return set(itertools.product(*(range(p,p+s) for p,s in zip(b["min"],b["size"]))))


class GeometryChecks(unittest.TestCase):
    def test_subtraction_against_independent_voxel_sets(self):
        rng=random.Random(20261018)
        for _ in range(500):
            old={"min":[rng.randrange(6) for _ in range(3)],"size":[rng.randrange(1,7) for _ in range(3)],"styleId":4095,"tintId":63,"motionPhase":17}
            cut={"min":[rng.randrange(12) for _ in range(3)],"size":[rng.randrange(1,7) for _ in range(3)]}
            parts=subtract_box(old,cut);union=set()
            self.assertLessEqual(len(parts),6)
            for b in parts:
                occupied=cells(b)
                self.assertTrue(union.isdisjoint(occupied));union.update(occupied)
                self.assertEqual([b[k] for k in ("styleId","tintId","motionPhase")],[4095,63,17])
            self.assertEqual(union,cells(old)-cells(cut))

    def test_thin_wall_hole(self):
        plan=compile_commands(demo()[:3])
        self.assertEqual(len(plan["boxes"]),4)
        self.assertEqual(sum(len(cells(b)) for b in plan["boxes"]),199)
        self.assertEqual([c["op"] for c in plan["commands"]],["clear","box","remove"]+["box"]*4)

    def test_overlay_and_erase_preserve_voxel_styles(self):
        edits=[{"op":"clear"},
            {"op":"box","min":[0,0,0],"size":[5,5,5],"styleId":1},
            {"op":"box","min":[2,2,2],"size":[5,2,2],"styleId":3},
            {"op":"erase","min":[1,1,1],"size":[3,3,3]}]
        expected={}
        for e in edits[1:]:
            for p in cells(e):
                if e["op"]=="erase":expected.pop(p,None)
                else:expected[p]=e["styleId"]
        actual={}
        for b in compile_commands(edits)["boxes"]:
            for p in cells(b):
                self.assertNotIn(p,actual);actual[p]=b["styleId"]
        self.assertEqual(actual,expected)

    def test_capacity_rejected_before_transmission(self):
        with self.assertRaises(ValueError):compile_commands(demo(),3)

    def test_requires_known_empty_start_and_valid_bounds(self):
        with self.assertRaises(ValueError):compile_commands(demo()[1:])
        with self.assertRaises(ValueError):compile_commands([{"op":"clear"},{"op":"box","min":[255,0,0],"size":[2,1,1]}])
        for field in ("styleId","tintId","motionPhase"):
            with self.subTest(field=field),self.assertRaises(ValueError):
                compile_commands([{"op":"clear"},{"op":"box","min":[0,0,0],"size":[1,1,1],field:1.5}])


if __name__=="__main__":unittest.main()
