use chrono::{DateTime, TimeZone};

pub mod fs;

pub fn u32_from_be_bytes_unchecked(data: &[u8], start_idx: usize) -> u32 {
    u32::from_be_bytes(data[start_idx..(start_idx + 4)].try_into().unwrap())
}

pub fn u16_from_be_bytes_unchecked(data: &[u8], start_idx: usize) -> u16 {
    u16::from_be_bytes(data[start_idx..(start_idx + 2)].try_into().unwrap())
}

pub fn datetime_to_bytes<Z>(dt: &DateTime<Z>) -> impl Iterator<Item = u8>
where
    Z: TimeZone,
{
    (dt.timestamp() as u32)
        .to_be_bytes()
        .iter()
        .map(|b| *b)
        .chain(dt.timestamp_subsec_nanos().to_be_bytes().iter().map(|b| *b))
        .collect::<Vec<u8>>()
        .into_iter()
}
