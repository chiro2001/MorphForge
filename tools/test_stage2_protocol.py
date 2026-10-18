import struct
import unittest
from send_stage2 import packet


def messages(data):
    assert data[:8]==b"#bundle\0"
    offset=16;result=[]
    while offset<len(data):
        length=struct.unpack_from(">i",data,offset)[0];offset+=4
        msg=data[offset:offset+length];offset+=length
        end=msg.index(0);name=msg[:end].decode();start=(end+4)&~3
        assert msg[start:start+4]==b",i\0\0"
        result.append((name,struct.unpack_from(">i",msg,start+4)[0]))
    return result


class SingleCommitProtocol(unittest.TestCase):
    def test_every_sequence_and_opcode_uses_one_complete_bundle(self):
        for seq in range(32):
            for opcode,op in enumerate(("box","erase","remove","clear"),1):
                command={"op":op,"min":[128,64,254],"size":[20,10,2],"styleId":4095,"tintId":63,"motionPhase":63}
                result=messages(packet(command,seq))
                self.assertEqual(len(result),11)
                self.assertEqual(result[-1],("/avatar/parameters/MF2_Ctl",seq*8+opcode))
                self.assertEqual([n for n,v in result[:-1]],[f"/avatar/parameters/MF2_D{i:02d}" for i in range(10)])
                self.assertEqual([v for n,v in result[:-1]],[0]*10 if op=="clear" else [128,64,254,19,9,1]+([63]*4 if op=="box" else [0,0,63,63]))
    def test_invalid_sequence_rejected(self):
        for seq in (-1,32,1.5,True):
            with self.assertRaises(ValueError):packet({"op":"clear"},seq)


if __name__=="__main__":unittest.main()
