/// Unique identifier for sequences during the scheduling process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SeqId(u64);

impl SeqId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Simple reverse-sequential ID generator for `SeqId`s.
/// These IDs uniquely identify sequences during the scheduling process.
pub struct SeqIdGenerator {
    id: u64,
}

impl Default for SeqIdGenerator {
    fn default() -> Self {
        Self { id: u64::MAX }
    }
}

impl SeqIdGenerator {
    pub fn gen(&mut self) -> SeqId {
        self.id = self.id.wrapping_add(1);
        SeqId::new(self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seq_id_generator() {
        let mut generator = SeqIdGenerator::default();

        let id1 = generator.gen();
        let id2 = generator.gen();
        let id3 = generator.gen();

        assert_eq!(id1.id(), u64::MAX);
        assert_eq!(id2.id(), u64::MAX - 1);
        assert_eq!(id3.id(), u64::MAX - 2);
    }

    #[test]
    fn test_seq_id_wrap_around() {
        let mut generator = SeqIdGenerator { id: 0 };

        let id1 = generator.gen();
        let id2 = generator.gen();

        assert_eq!(id1.id(), 0);
        assert_eq!(id2.id(), u64::MAX);
    }
}
