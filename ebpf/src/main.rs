#![no_std]
#![no_main]

use aya_ebpf::{
    bindings::{TC_ACT_OK, TC_ACT_SHOT},
    macros::{classifier, map},
    maps::HashMap,
    programs::TcContext,
};
// use aya_log_ebpf::info;
use common::{RateLimitState, TrafficStats};
use network_types::{
    eth::{EthHdr, EtherType},
    ip::{IpProto, Ipv4Hdr, Ipv6Hdr},
};

// ingress: from client to server
#[map]
static UP: HashMap<u16, TrafficStats> = HashMap::with_max_entries(65536, 0);

#[map]
static UP_RATE_LIMITS: HashMap<u16, RateLimitState> = HashMap::with_max_entries(65536, 0);

// egress: from server to client
#[map]
static DOWN: HashMap<u16, TrafficStats> = HashMap::with_max_entries(65536, 0);

#[classifier]
pub fn hakkawine_egress(ctx: TcContext) -> i32 {
    match try_egress(ctx) {
        Ok(ret) => ret,
        Err(_) => TC_ACT_OK,
    }
}

fn try_egress(ctx: TcContext) -> Result<i32, i64> {
    let eth: EthHdr = ctx.load(0)?;

    let Ok(ether_type) = eth.ether_type() else {
        return Ok(TC_ACT_OK);
    };

    if !matches!(ether_type, EtherType::Ipv4 | EtherType::Ipv6) {
        return Ok(TC_ACT_OK);
    }

    let transport_offset = match ether_type {
        EtherType::Ipv4 => {
            let ipv4: Ipv4Hdr = ctx.load(EthHdr::LEN)?;
            let Ok(proto) = ipv4.proto() else {
                return Ok(TC_ACT_OK);
            };

            if !matches!(proto, IpProto::Tcp | IpProto::Udp) {
                return Ok(TC_ACT_OK);
            }

            EthHdr::LEN + Ipv4Hdr::LEN
        }
        EtherType::Ipv6 => {
            let ipv6: Ipv6Hdr = ctx.load(EthHdr::LEN)?;
            let Ok(proto) = ipv6.next_hdr() else {
                return Ok(TC_ACT_OK);
            };

            if !matches!(proto, IpProto::Tcp | IpProto::Udp) {
                return Ok(TC_ACT_OK);
            }

            EthHdr::LEN + Ipv6Hdr::LEN
        }
        _ => return Ok(TC_ACT_OK),
    };

    let src_port_bytes: [u8; 2] = ctx.load(transport_offset)?;
    let src_port = u16::from_be_bytes(src_port_bytes);

    let pkt_len = ctx.skb.len() as u64;

    let stats = DOWN.get_ptr_mut(&src_port);
    if let Some(s) = stats {
        unsafe {
            (*s).bytes += pkt_len;
            (*s).packets += 1;
        }
    } else {
        let _ = DOWN.insert(
            &src_port,
            &TrafficStats {
                bytes: pkt_len,
                packets: 1,
            },
            0,
        );
    }

    Ok(TC_ACT_OK)
}

#[classifier]
pub fn hakkawine_ingress(ctx: TcContext) -> i32 {
    match try_ingress(ctx) {
        Ok(ret) => ret,
        Err(_) => TC_ACT_OK,
    }
}

fn try_ingress(ctx: TcContext) -> Result<i32, i64> {
    let eth: EthHdr = ctx.load(0)?;

    let Ok(ether_type) = eth.ether_type() else {
        return Ok(TC_ACT_OK);
    };

    if !matches!(ether_type, EtherType::Ipv4 | EtherType::Ipv6) {
        return Ok(TC_ACT_OK);
    }

    let transport_offset = match ether_type {
        EtherType::Ipv4 => {
            let ipv4: Ipv4Hdr = ctx.load(EthHdr::LEN)?;
            let Ok(proto) = ipv4.proto() else {
                return Ok(TC_ACT_OK);
            };

            if !matches!(proto, IpProto::Tcp | IpProto::Udp) {
                return Ok(TC_ACT_OK);
            }

            EthHdr::LEN + Ipv4Hdr::LEN
        }
        EtherType::Ipv6 => {
            let ipv6: Ipv6Hdr = ctx.load(EthHdr::LEN)?;
            let Ok(proto) = ipv6.next_hdr() else {
                return Ok(TC_ACT_OK);
            };

            if !matches!(proto, IpProto::Tcp | IpProto::Udp) {
                return Ok(TC_ACT_OK);
            }

            EthHdr::LEN + Ipv6Hdr::LEN
        }
        _ => return Ok(TC_ACT_OK),
    };

    let dst_port_bytes: [u8; 2] = ctx.load(transport_offset + 2)?;
    let dst_port = u16::from_be_bytes(dst_port_bytes);

    let pkt_len = ctx.skb.len() as u64;

    if let Some(limit_ptr) = UP_RATE_LIMITS.get_ptr_mut(&dst_port) {
        unsafe {
            let now = aya_ebpf::helpers::bpf_ktime_get_ns();
            let state = &mut *limit_ptr;

            let elapsed_ns = now.saturating_sub(state.last_update);
            state.last_update = now;

            let generated_tokens =
                (elapsed_ns as u128 * state.max_bytes_per_sec as u128 / 1_000_000_000) as u64;

            state.tokens = core::cmp::min(state.max_bytes_per_sec, state.tokens + generated_tokens);

            if state.tokens >= pkt_len {
                state.tokens -= pkt_len;
            } else {
                return Ok(TC_ACT_SHOT);
            }
        }
    }

    let stats = UP.get_ptr_mut(&dst_port);
    if let Some(s) = stats {
        unsafe {
            (*s).bytes += pkt_len;
            (*s).packets += 1;
        }
    } else {
        let _ = UP.insert(
            &dst_port,
            &TrafficStats {
                bytes: pkt_len,
                packets: 1,
            },
            0,
        );
    }

    Ok(TC_ACT_OK)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(link_section = "license")]
#[unsafe(no_mangle)]
static LICENSE: [u8; 4] = *b"GPL\0";
