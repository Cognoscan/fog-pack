use std::io;
use std::io::Error;
use std::io::ErrorKind::InvalidData;
use byteorder::ReadBytesExt;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VarInt {
    n: [u8; 5]
}

impl VarInt {
    fn from_u32(i: u32) -> VarInt {
        let mut n = [0u8; 5];
        if i < 128 {
            n[0] = (1u8 << 7) | (i as u8);
        }
        else if i < (1<<14) {
            n[0] = (1u8 << 6) | ((i>>8) as u8);
            n[1] = (i & 0xFF) as u8;
        }
        else if i < (1<<21) {
            n[0] = (1u8 << 5) | ((i>>16) as u8);
            n[1] = ((i>>8) & 0xFF) as u8;
            n[2] = (i      & 0xFF) as u8;
        }
        else if i < (1<<28) {
            n[0] = (1u8 << 4) + ((i>>24) as u8);
            n[1] = ((i>>16) & 0xFF) as u8;
            n[2] = ((i>>8)  & 0xFF) as u8;
            n[3] = (i       & 0xFF) as u8;
        }
        else {
            n[0] = 1u8 << 3;
            n[1] = ((i>>24) & 0xFF) as u8;
            n[2] = ((i>>16) & 0xFF) as u8;
            n[3] = ((i>>8)  & 0xFF) as u8;
            n[4] = (i       & 0xFF) as u8;
        }
        VarInt { n }
    }

    fn to_u32(self) -> u32 {
        let zeros = self.n[0].leading_zeros();
        match zeros {
            0 => (self.n[0] & 0x7F) as u32,
            1 => (((self.n[0] & 0x3F) as u32) << 8)  | (self.n[1] as u32),
            2 => (((self.n[0] & 0x1F) as u32) << 16) | ((self.n[1] as u32) << 8) | (self.n[2] as u32),
            3 => (((self.n[0] & 0x0F) as u32) << 24) | ((self.n[1] as u32) << 16) | ((self.n[2] as u32) << 8) | (self.n[3] as u32),
            4 => ((self.n[1] as u32) << 24) | ((self.n[2] as u32) << 16) | ((self.n[3] as u32) << 8) | (self.n[4] as u32),
            _ => 0u32,
        }
    }

    fn write(&self, buf: &mut Vec<u8>) {
        let zeros = self.n[0].leading_zeros();
        buf.push(self.n[0]);
        if zeros >= 1 { buf.push(self.n[1]); }
        if zeros >= 2 { buf.push(self.n[2]); }
        if zeros >= 3 { buf.push(self.n[3]); }
        if zeros >= 4 { buf.push(self.n[4]); }
    }

    fn read(buf: &mut &[u8]) -> io::Result<VarInt> {
        let mut n = [0u8; 5];
        n[0] = buf.read_u8()?;
        let zeros = n[0].leading_zeros();
        if (zeros > 4) || (zeros == 4 && n[0] > (1u8 << 3)) {
            return Err(Error::new(InvalidData, format!("VarInt larger than a u32")));
        }
        if zeros >= 1 { n[1] = buf.read_u8()?; }
        if zeros >= 2 { n[2] = buf.read_u8()?; }
        if zeros >= 3 { n[3] = buf.read_u8()?; }
        if zeros >= 4 { n[4] = buf.read_u8()?; }
        Ok(VarInt { n })
    }
}

impl From<u8> for VarInt {
    fn from(n: u8) -> Self {
        VarInt::from_u32(n as u32)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_2() {
        for s in 0..=31 {
            let mut buf = Vec::new();
            let i = 1u32 << s;
            let i = VarInt::from_u32(i);
            i.write(&mut buf);
            let o = VarInt::read(&mut &buf[..]).unwrap();
            assert_eq!(i, o, "VarInt should match");
            assert_eq!(1u32 << s, o.to_u32(), "u32 results should match");
        }
    }

}
