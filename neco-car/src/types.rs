use neco_cid::Cid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarV1 {
    roots: Vec<Cid>,
    blocks: Vec<CarEntry>,
}

impl CarV1 {
    pub(crate) fn new(roots: Vec<Cid>, blocks: Vec<CarEntry>) -> Self {
        Self { roots, blocks }
    }

    pub fn roots(&self) -> &[Cid] {
        &self.roots
    }

    pub fn blocks(&self) -> &[CarEntry] {
        &self.blocks
    }

    pub fn into_parts(self) -> (Vec<Cid>, Vec<CarEntry>) {
        (self.roots, self.blocks)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarEntry {
    cid: Cid,
    data: Vec<u8>,
}

impl CarEntry {
    pub(crate) fn new(cid: Cid, data: Vec<u8>) -> Self {
        Self { cid, data }
    }

    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn into_parts(self) -> (Cid, Vec<u8>) {
        (self.cid, self.data)
    }
}
