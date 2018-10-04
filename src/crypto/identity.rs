use std::io::{self,Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;

/// Public identity that can be shared with others. An identity can be used to create locks and 
/// verify signatures
///     - Consists of a Ed25519 public key and the Curve25519 public key.
///     - When encoded, it is just the Ed25519 public key
#[derive(Debug,Clone,PartialEq,Eq,Hash)]
pub struct Identity {
    version: u8,
    id: Vec<u8>,
}

impl Identity {

    pub fn blank() -> Identity {
        Identity {
            version: 0,
            id: vec![],
        }
    }

    pub fn is_valid(&self) -> bool {
        if self.version != 0 { return false; }
        true
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_u8(self.version)?;
        wr.write_all(&self.id)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<Identity, io::Error> {
        let version = rd.read_u8()?;
        let id = Identity {version, id:vec![]};
        //rd.read_exact(&mut h.digest)?;
        Ok(id)
    }
}
