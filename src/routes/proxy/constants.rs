use awc::http::header;

pub const IGNORED_HEADERS: &[header::HeaderName] = &[header::CONTENT_LENGTH];
pub const MIN_TTL: u64 = 60;
pub const MAX_TTL: u64 = 3600;
