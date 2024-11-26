use std::num::ParseIntError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogIndex(pub u64);

impl LogIndex {
    pub fn ordered_encode(&self) -> String {
        let digits = format!("{}", self.0);

        format!("{:02}-{}", digits.len(), digits)
    }

    pub fn ordered_decode(s: &str) -> Result<Self, ParseIntError> {
        let (_len, digits) = s.split_at(3);
        let digits = digits.parse::<u64>()?;

        Ok(LogIndex(digits))
    }
}
