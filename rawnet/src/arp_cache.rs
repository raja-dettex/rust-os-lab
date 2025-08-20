use core::sync::atomic::{Ordering, AtomicU64};
use core::mem::MaybeUninit;
use std::time::{SystemTime, UNIX_EPOCH};
use std::u64;

pub type IPv4 = u32;
pub type Mac = [u8; 6];

const CAPACITY : usize = 1024;
const EMPTY_KEY: u64 = 0;     // low 32 bits = 0 means empty
const MAX_PROBES: usize = 64; // how far to probe before fallback


pub const fn make_atomic_u64() -> [AtomicU64; CAPACITY] { 
    let mut arr: [MaybeUninit<AtomicU64>; CAPACITY] = unsafe {
        MaybeUninit::<[MaybeUninit<AtomicU64>; CAPACITY]>::uninit().assume_init()
    };
    let mut i = 0;
    while i < CAPACITY {
        arr[i] = MaybeUninit::new(AtomicU64::new(0));
        i += 1;
    }
    unsafe { core::mem::transmute(arr)}
}


pub struct ArpCache { 
    keys : &'static [AtomicU64; CAPACITY],
    macs : &'static [AtomicU64; CAPACITY],
    last_seen : &'static [AtomicU64; CAPACITY],
    
}


impl ArpCache { 
    pub fn new() -> Self { 
         // statics: create once
         static KEYS: [AtomicU64; CAPACITY] = make_atomic_u64();
         static MACS: [AtomicU64; CAPACITY] = make_atomic_u64();
         static LAST: [AtomicU64; CAPACITY] = make_atomic_u64();
        Self { 
            keys: &KEYS,
            macs: &MACS,
            last_seen: &LAST
        }
    }

    #[inline]
    fn pack_key(ip:IPv4, generator: u32) -> u64 { 
        ((generator as u64) << 32) | (ip as u64)
    }

    #[inline]
    fn now_miilis() -> u64 { 
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
    }

    #[inline]
    fn unpack_key(key: u64) -> IPv4 { 
        key as u32
    }

    fn hash(ip: IPv4) -> usize { 
        let mut x = ip as u64;
        x = x.wrapping_mul(0x9E3779B97F4A7C15u64);
        (((x >> 32) ^ x) as usize ) & (CAPACITY - 1)
    }
    #[inline]
    fn mac_to_u64(mac: &Mac) -> u64 {
        // pack 6 bytes into low 48 bits
        ((mac[0] as u64) << 40)
            | ((mac[1] as u64) << 32)
            | ((mac[2] as u64) << 24)
            | ((mac[3] as u64) << 16)
            | ((mac[4] as u64) << 8)
            | (mac[5] as u64)
    }

    #[inline]
    fn u64_to_mac(x: u64) -> Mac {
        [
            ((x >> 40) & 0xff) as u8,
            ((x >> 32) & 0xff) as u8,
            ((x >> 24) & 0xff) as u8,
            ((x >> 16) & 0xff) as u8,
            ((x >> 8) & 0xff) as u8,
            (x & 0xff) as u8,
        ]
    }


    pub fn lookup(&self, ip: IPv4, max_age_millis: u64) -> Option<Mac>{ 
        let h0 = Self::hash(ip);
        let now = Self::now_miilis();
        for probe in 0..MAX_PROBES { 
            let idx = (h0 + probe) & (CAPACITY - 1);
            let key = self.keys[idx].load(Ordering::Acquire);
            if key == EMPTY_KEY {
                return None;
            }
            let key_ip = Self::unpack_key(key);
            if ip == key_ip {
                let ts = self.last_seen[idx].load(Ordering::Acquire);
                if now.saturating_sub(ts) > max_age_millis { 
                    return None;
                }
                let macs = self.macs[idx].load(Ordering::Acquire);
                return Some(Self::u64_to_mac(macs));
            }
        }
        None
    }

    pub fn insert(&self, ip: IPv4, mac: Mac) { 
        let h0 = Self::hash(ip);
        let now = Self::now_miilis();
        let generator = (now & 0xffff_ffff) as u32;
        let packed = Self::pack_key(ip, generator);
        let mac_u64 = Self::mac_to_u64(&mac);
        for probe in 0..MAX_PROBES { 
            let idx = (h0 + probe) & (CAPACITY - 1);
            let key = self.keys[idx].load(Ordering::Acquire);
            if key != EMPTY_KEY { 
                if Self::unpack_key(key) == ip { 
                    self.macs[idx].store(mac_u64, Ordering::Release);
                    self.last_seen[idx].store(now,Ordering::Release);
                    let _ = self.keys[idx].compare_exchange(key, packed, Ordering::AcqRel, Ordering::Acquire);
                    return;
                }
            } else { 
                if self.keys[idx].compare_exchange(key, packed, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                    self.last_seen[idx].store(now,Ordering::Release);
                    self.macs[idx].store(mac_u64, Ordering::Release);
                    return;
                } else { 
                    //some one else claimed continue
                }
            }
        }

        // probing failed need to evict a slot of oldest timestamp
        // will do a linear scan in the probe region and reclaim the slot eventually
        let mut old_idx = h0;
        let mut old_ts = u64::MAX;
        for probe in 0..MAX_PROBES { 
            let idx = (h0 + probe) & (CAPACITY-1);
            let ts = self.last_seen[idx].load(Ordering::Acquire);
            if ts  < old_ts{ 
                old_ts = ts;
                old_idx = idx;
            }
        }

        // update the slot
        self.macs[old_idx].store(mac_u64, Ordering::Release);
        self.last_seen[old_idx].store(now, Ordering::Release);
        let _ = self.keys[old_idx].fetch_add(1u64 << 32, Ordering::AcqRel);
        let mut prev = self.keys[old_idx].load(Ordering::Acquire);
        loop { 
            let new_key = ((prev & 0xffff_ffff_0000_0000u64)) | (ip as u64);
            match self.keys[old_idx].compare_exchange(prev, new_key, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return,
                Err(reserved) => { 
                    prev = reserved;
                }
            }
        }

    }


    pub fn expire_entries(&self, max_age_millis: u64) { 
        let now = Self::now_miilis();
        for i in 0..CAPACITY {
            let key = self.keys[i].load(Ordering::Acquire);
            if key == EMPTY_KEY {
                continue;
            }
            let ts = self.last_seen[i].load(Ordering::Acquire);
            if now.saturating_sub(ts) > max_age_millis { 
                //reclaim the slot and cas update to fill it with empty key
                let _ = self.keys[i].compare_exchange(key, EMPTY_KEY, Ordering::AcqRel, Ordering::Acquire);
            }
        }
    }
}


