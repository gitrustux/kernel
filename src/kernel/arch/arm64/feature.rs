// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arm64;
use crate::bits;
use crate::fbl::algorithm;
use core::sync::atomic::{AtomicU32, Ordering};

// Import dprintf macro (from crate root, not the function)
use crate::dprintf;

// saved feature bitmap
pub static mut arm64_features: u64 = 0;

static mut cache_info: [arm64::arm64_cache_info_t; arm64::SMP_MAX_CPUS as usize] = unsafe {
    const ZERO_DESC: arm64::arm64_cache_desc_t = arm64::arm64_cache_desc_t {
        ctype: 0,
        write_through: false,
        write_back: false,
        read_alloc: false,
        write_alloc: false,
        num_sets: 0,
        associativity: 0,
        line_size: 0,
    };
    const UNINIT: arm64::arm64_cache_info_t = arm64::arm64_cache_info_t {
        inner_boundary: 0,
        lou_u: 0,
        loc: 0,
        lou_is: 0,
        level_data_type: [ZERO_DESC; 7],
        level_inst_type: [ZERO_DESC; 7],
    };
    [UNINIT; arm64::SMP_MAX_CPUS as usize]
};

// cache size parameters cpus, default to a reasonable minimum
pub static mut arm64_zva_size: u32 = 32;
pub static mut arm64_icache_size: u32 = 32;
pub static mut arm64_dcache_size: u32 = 32;

fn parse_ccsid(desc: &mut arm64::arm64_cache_desc_t, ccsid: u64) {
    desc.write_through = bits::BIT(ccsid, 31) > 0;
    desc.write_back = bits::BIT(ccsid, 30) > 0;
    desc.read_alloc = bits::BIT(ccsid, 29) > 0;
    desc.write_alloc = bits::BIT(ccsid, 28) > 0;
    desc.num_sets = (bits::BITS_SHIFT(ccsid, 27, 13) + 1) as u32;
    desc.associativity = (bits::BITS_SHIFT(ccsid, 12, 3) + 1) as u32;
    desc.line_size = 1u32 << (bits::BITS(ccsid, 2, 0) as u32 + 4);
}

pub fn arm64_get_cache_info(info: &mut arm64::arm64_cache_info_t) {
    let mut temp: u64 = 0;

    let sysreg = unsafe {
        let reg: u64;
        core::arch::asm!("mrs {}, clidr_el1", out(reg) reg);
        reg
    };
    
    info.inner_boundary = bits::BITS_SHIFT(sysreg, 32, 30) as u8;
    info.lou_u = bits::BITS_SHIFT(sysreg, 29, 27) as u8;
    info.loc = bits::BITS_SHIFT(sysreg, 26, 24) as u8;
    info.lou_is = bits::BITS_SHIFT(sysreg, 23, 21) as u8;
    
    for i in 0..7 {
        let ctype = ((sysreg >> (3 * i)) & 0x07) as u8;
        if ctype == 0 {
            info.level_data_type[i].ctype = 0;
            info.level_inst_type[i].ctype = 0;
        } else if ctype == 4 {                               // Unified
            unsafe {
                // Select cache level
                core::arch::asm!("msr csselr_el1, {}", in(reg) (i << 1) as i64);
                core::arch::asm!("isb sy");
                core::arch::asm!("mrs {}, ccsidr_el1", out(reg) temp);
            }
            info.level_data_type[i].ctype = 4;
            parse_ccsid(&mut info.level_data_type[i], temp);
        } else {
            if (ctype & 0x02) != 0 {
                unsafe {
                    core::arch::asm!("msr csselr_el1, {}", in(reg) (i << 1) as i64);
                    core::arch::asm!("isb sy");
                    core::arch::asm!("mrs {}, ccsidr_el1", out(reg) temp);
                }
                info.level_data_type[i].ctype = 2;
                parse_ccsid(&mut info.level_data_type[i], temp);
            }
            if (ctype & 0x01) != 0 {
                unsafe {
                    core::arch::asm!("msr csselr_el1, {}", in(reg) ((i << 1) | 0x01) as i64);
                    core::arch::asm!("isb sy");
                    core::arch::asm!("mrs {}, ccsidr_el1", out(reg) temp);
                }
                info.level_inst_type[i].ctype = 1;
                parse_ccsid(&mut info.level_inst_type[i], temp);
            }
        }
    }
}

pub fn arm64_dump_cache_info(cpu: u32) {
    let info = unsafe { &cache_info[cpu as usize] };
    println!("==== ARM64 CACHE INFO CORE {} ====", cpu);
    println!("Inner Boundary = L{}", info.inner_boundary);
    println!("Level of Unification Uniprocessor = L{}", info.lou_u);
    println!("Level of Coherence = L{}", info.loc);
    println!("Level of Unification Inner Shareable = L{}", info.lou_is);
    
    for i in 0..7 {
        print!("L{} Details:", i + 1);
        if (info.level_data_type[i].ctype == 0) && (info.level_inst_type[i].ctype == 0) {
            println!("\tNot Implemented");
        } else {
            if info.level_data_type[i].ctype == 4 {
                println!("\tUnified Cache, sets={}, associativity={}, line size={} bytes",
                       info.level_data_type[i].num_sets,
                       info.level_data_type[i].associativity,
                       info.level_data_type[i].line_size);
            } else {
                if (info.level_data_type[i].ctype & 0x02) != 0 {
                    println!("\tData Cache, sets={}, associativity={}, line size={} bytes",
                           info.level_data_type[i].num_sets,
                           info.level_data_type[i].associativity,
                           info.level_data_type[i].line_size);
                }
                if (info.level_inst_type[i].ctype & 0x01) != 0 {
                    if (info.level_data_type[i].ctype & 0x02) != 0 {
                        print!("\t");
                    }
                    println!("\tInstruction Cache, sets={}, associativity={}, line size={} bytes",
                           info.level_inst_type[i].num_sets,
                           info.level_inst_type[i].associativity,
                           info.level_inst_type[i].line_size);
                }
            }
        }
    }
}

fn midr_to_core(midr: u32, str_buf: &mut [u8]) -> usize {
    let implementer = bits::BITS_SHIFT(midr, 31, 24);
    let variant = bits::BITS_SHIFT(midr, 23, 20);
    let _architecture = bits::BITS_SHIFT(midr, 19, 16);
    let partnum = bits::BITS_SHIFT(midr, 15, 4);
    let revision = bits::BITS_SHIFT(midr, 3, 0);

    let partnum_str = match (implementer as u8 as char, partnum) {
        ('A', 0xd03) => "ARM Cortex-a53",
        ('A', 0xd04) => "ARM Cortex-a35",
        ('A', 0xd05) => "ARM Cortex-a55",
        ('A', 0xd07) => "ARM Cortex-a57",
        ('A', 0xd08) => "ARM Cortex-a72",
        ('A', 0xd09) => "ARM Cortex-a73",
        ('A', 0xd0a) => "ARM Cortex-a75",
        ('C', 0xa1) => "Cavium CN88XX",
        ('C', 0xaf) => "Cavium CN99XX",
        _ => {
            // Unknown CPU - format directly using write_to_slice
            use core::fmt::Write;
            let str_buf_len = str_buf.len();
            let mut writer = WriteToSlice { slice: str_buf, offset: 0 };
            let _ = write!(&mut writer, "Unknown implementer {} partnum 0x{:x} r{}p{}",
                          implementer as u8 as char, partnum, variant, revision);
            // Null terminate
            if writer.offset < str_buf_len {
                writer.slice[writer.offset] = 0;
            } else if str_buf_len > 0 {
                writer.slice[str_buf_len - 1] = 0;
            }
            return writer.offset;
        }
    };

    // Format directly using write_to_slice
    use core::fmt::Write;
    let str_buf_len = str_buf.len();
    let mut writer = WriteToSlice { slice: str_buf, offset: 0 };
    let _ = write!(&mut writer, "{} r{}p{}", partnum_str, variant, revision);
    // Null terminate
    if writer.offset < str_buf_len {
        writer.slice[writer.offset] = 0;
    } else if str_buf_len > 0 {
        writer.slice[str_buf_len - 1] = 0;
    }
    writer.offset
}

fn print_cpu_info() {
    let midr = unsafe {
        let reg: u64;
        core::arch::asm!("mrs {}, midr_el1", out(reg) reg);
        reg as u32
    };
    
    let mut cpu_name = [0u8; 128];
    midr_to_core(midr, &mut cpu_name);
    
    let mpidr = unsafe {
        let reg: u64;
        core::arch::asm!("mrs {}, mpidr_el1", out(reg) reg);
        reg
    };
    
    let aff3 = ((mpidr & arm64::MPIDR_AFF3_MASK) >> arm64::MPIDR_AFF3_SHIFT) as u32;
    let aff2 = ((mpidr & arm64::MPIDR_AFF2_MASK) >> arm64::MPIDR_AFF2_SHIFT) as u32;
    let aff1 = ((mpidr & arm64::MPIDR_AFF1_MASK) >> arm64::MPIDR_AFF1_SHIFT) as u32;
    let aff0 = ((mpidr & arm64::MPIDR_AFF0_MASK) >> arm64::MPIDR_AFF0_SHIFT) as u32;
    
    dprintf!(crate::kernel::debug::LogLevel::Info, "ARM cpu {}: midr {:#x} '{}' mpidr {:#x} aff {}:{}:{}:{}\n",
            arm64::arch_curr_cpu_num(), midr, core::str::from_utf8(&cpu_name).unwrap_or("unknown"), 
            mpidr, aff3, aff2, aff1, aff0);
}

// Helper function for string formatting that mimics C's snprintf
fn snprintf(buffer: &mut [u8], format_str: &str) -> usize {
    use core::fmt::Write;

    let mut writer = WriteToSlice { slice: buffer, offset: 0 };
    let _ = write!(&mut writer, "{}", format_str);

    // Ensure null termination for C interop
    let offset = writer.offset;
    let len = writer.slice.len();
    if offset < len {
        writer.slice[offset] = 0;
    } else if !writer.slice.is_empty() {
        writer.slice[len - 1] = 0;
    }

    writer.offset
}

// Helper struct for writing to a byte slice
struct WriteToSlice<'a> {
    slice: &'a mut [u8],
    offset: usize,
}

impl<'a> core::fmt::Write for WriteToSlice<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.slice.len().saturating_sub(self.offset);
        let to_copy = bytes.len().min(remaining);
        
        if to_copy > 0 {
            self.slice[self.offset..self.offset + to_copy]
                .copy_from_slice(&bytes[..to_copy]);
            self.offset += to_copy;
        }
        
        Ok(())
    }
}

// call on every cpu to save features
pub fn arm64_feature_init() {
    // set up some global constants based on the boot cpu
    let cpu = arm64::arch_curr_cpu_num();
    
    unsafe {
        if cpu == 0 {
            // read the block size of DC ZVA
            let dczid: u64;
            core::arch::asm!("mrs {}, dczid_el0", out(reg) dczid);
            
            let mut arm64_zva_shift = 0;
            if bits::BIT(dczid, 4) == 0 {
                arm64_zva_shift = (dczid & 0xf) as u32 + 2;
            }
            
            assert!(arm64_zva_shift != 0, "DC ZVA is unavailable");
            arm64_zva_size = 1u32 << arm64_zva_shift;

            // read the dcache and icache line size
            let ctr: u64;
            core::arch::asm!("mrs {}, ctr_el0", out(reg) ctr);
            
            let arm64_dcache_shift = bits::BITS_SHIFT(ctr, 19, 16) as u32 + 2;
            arm64_dcache_size = 1u32 << arm64_dcache_shift;
            
            let arm64_icache_shift = bits::BITS(ctr, 3, 0) as u32 + 2;
            arm64_icache_size = 1u32 << arm64_icache_shift;

            // parse the ISA feature bits
            arm64_features |= arm64::RX_HAS_CPU_FEATURES;
            
            let isar0: u64;
            core::arch::asm!("mrs {}, id_aa64isar0_el1", out(reg) isar0);
            
            if bits::BITS_SHIFT(isar0, 7, 4) >= 1 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_AES;
            }
            if bits::BITS_SHIFT(isar0, 7, 4) >= 2 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_PMULL;
            }
            if bits::BITS_SHIFT(isar0, 11, 8) >= 1 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_SHA1;
            }
            if bits::BITS_SHIFT(isar0, 15, 12) >= 1 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_SHA2;
            }
            if bits::BITS_SHIFT(isar0, 19, 16) >= 1 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_CRC32;
            }
            if bits::BITS_SHIFT(isar0, 23, 20) >= 1 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_ATOMICS;
            }
            if bits::BITS_SHIFT(isar0, 31, 28) >= 1 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_RDM;
            }
            if bits::BITS_SHIFT(isar0, 35, 32) >= 1 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_SHA3;
            }
            if bits::BITS_SHIFT(isar0, 39, 36) >= 1 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_SM3;
            }
            if bits::BITS_SHIFT(isar0, 43, 40) >= 1 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_SM4;
            }
            if bits::BITS_SHIFT(isar0, 47, 44) >= 1 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_DP;
            }

            let isar1: u64;
            core::arch::asm!("mrs {}, id_aa64isar1_el1", out(reg) isar1);
            
            if bits::BITS_SHIFT(isar1, 3, 0) >= 1 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_DPB;
            }

            let pfr0: u64;
            core::arch::asm!("mrs {}, id_aa64pfr0_el1", out(reg) pfr0);
            
            if bits::BITS_SHIFT(pfr0, 19, 16) < 0b1111 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_FP;
            }
            if bits::BITS_SHIFT(pfr0, 23, 20) < 0b1111 {
                arm64_features |= arm64::RX_ARM64_FEATURE_ISA_ASIMD;
            }
        }

        // read the cache info for each cpu
        arm64_get_cache_info(&mut cache_info[cpu as usize]);

        // check to make sure implementation supports 16 bit asids
        let mmfr0: u64;
        core::arch::asm!("mrs {}, id_aa64mmfr0_el1", out(reg) mmfr0);
        
        assert!((mmfr0 & arm64::ARM64_MMFR0_ASIDBITS_MASK) == arm64::ARM64_MMFR0_ASIDBITS_16,
                "16-bit ASIDs not supported");
    }
}

#[inline]
pub fn arm64_feature_test(feature: u64) -> bool {
    unsafe { (arm64_features & feature) != 0 }
}

fn print_feature() {
    const FEATURES: &[(&str, u64)] = &[
        ("fp", arm64::RX_ARM64_FEATURE_ISA_FP),
        ("asimd", arm64::RX_ARM64_FEATURE_ISA_ASIMD),
        ("aes", arm64::RX_ARM64_FEATURE_ISA_AES),
        ("pmull", arm64::RX_ARM64_FEATURE_ISA_PMULL),
        ("sha1", arm64::RX_ARM64_FEATURE_ISA_SHA1),
        ("sha2", arm64::RX_ARM64_FEATURE_ISA_SHA2),
        ("crc32", arm64::RX_ARM64_FEATURE_ISA_CRC32),
        ("atomics", arm64::RX_ARM64_FEATURE_ISA_ATOMICS),
        ("rdm", arm64::RX_ARM64_FEATURE_ISA_RDM),
        ("sha3", arm64::RX_ARM64_FEATURE_ISA_SHA3),
        ("sm3", arm64::RX_ARM64_FEATURE_ISA_SM3),
        ("sm4", arm64::RX_ARM64_FEATURE_ISA_SM4),
        ("dp", arm64::RX_ARM64_FEATURE_ISA_DP),
        ("dpb", arm64::RX_ARM64_FEATURE_ISA_DPB),
    ];

    print!("ARM Features: ");
    let mut col = 0;
    
    for &(name, bit) in FEATURES {
        if arm64_feature_test(bit) {
            print!("{} ", name);
            col += name.len() + 1;
            
            if col >= 80 {
                println!();
                col = 0;
            }
        }
    }
    
    if col > 0 {
        println!();
    }
}

// dump the feature set
// print additional information if full is passed
pub fn arm64_feature_debug(full: bool) {
    print_cpu_info();

    if full {
        print_feature();
        
        unsafe {
            dprintf!(crate::kernel::debug::LogLevel::Info, "ARM cache line sizes: icache {} dcache {} zva {}\n",
                    arm64_icache_size, arm64_dcache_size, arm64_zva_size);
        }
        
        if crate::LK_DEBUGLEVEL > 0 {
            arm64_dump_cache_info(arm64::arch_curr_cpu_num());
        }
    }
}
/// Get ARM64 features bitmap
pub fn arm64_get_features() -> u32 {
    unsafe { arm64_features as u32 }
}
