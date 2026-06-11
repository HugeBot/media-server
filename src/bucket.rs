use std::str::FromStr;

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    Giveaways,
    StreamPreviews,
}

impl Bucket {
    pub fn as_str(&self) -> &'static str {
        match self {
            Bucket::Giveaways => "giveaways",
            Bucket::StreamPreviews => "stream-previews",
        }
    }

    pub const ALL: &'static [Bucket] = &[Bucket::Giveaways, Bucket::StreamPreviews];
}

impl FromStr for Bucket {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "giveaways" => Ok(Bucket::Giveaways),
            "stream-previews" => Ok(Bucket::StreamPreviews),
            _ => Err(AppError::InvalidBucket),
        }
    }
}
