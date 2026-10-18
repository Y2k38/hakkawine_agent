#![no_std]

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct TrafficStats {
    pub bytes: u64,
    pub packets: u64,
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct RateLimitState {
    pub max_bytes_per_sec: u64,
    pub tokens: u64,
    pub last_update: u64,
}
