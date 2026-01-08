// Copyright 2025 Rustux Authors
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arm64::*;
use crate::debug::*;
use crate::err::*;
use crate::platform::*;
use core::str::*;

#[cfg(target_arch = "aarch64")]
use crate::lib::console::*;

#[cfg(target_arch = "aarch64")]
macro_rules! sysreg_read_command {
    ($regname:expr, $sysreg_string:expr) => {
        if $regname.eq_ignore_ascii_case($sysreg_string) {
            println!("{} = {:016x}", $sysreg_string, unsafe { __arm_rsr64($sysreg_string) });
            return 0;
        }
    };
}

#[cfg(target_arch = "aarch64")]
fn read_sysregs(regname: &str) -> u64 {
    sysreg_read_command!(regname, "actlr_el1");
    sysreg_read_command!(regname, "ccsidr_el1");
    sysreg_read_command!(regname, "clidr_el1");
    sysreg_read_command!(regname, "csselr_el1");
    sysreg_read_command!(regname, "midr_el1");
    sysreg_read_command!(regname, "mpidr_el1");
    sysreg_read_command!(regname, "sctlr_el1");
    sysreg_read_command!(regname, "spsr_el1");
    sysreg_read_command!(regname, "tcr_el1");
    sysreg_read_command!(regname, "tpidrro_el0");
    sysreg_read_command!(regname, "tpidr_el1");
    sysreg_read_command!(regname, "ttbr0_el1");
    sysreg_read_command!(regname, "ttbr1_el1");
    sysreg_read_command!(regname, "vbar_el1");

    // Generic Timer regs
    sysreg_read_command!(regname, "cntfrq_el0");
    sysreg_read_command!(regname, "cntkctl_el1");
    sysreg_read_command!(regname, "cntpct_el0");
    sysreg_read_command!(regname, "cntps_ctl_el1");
    sysreg_read_command!(regname, "cntps_cval_el1");
    sysreg_read_command!(regname, "cntps_tval_el1");
    sysreg_read_command!(regname, "cntp_ctl_el0");
    sysreg_read_command!(regname, "cntp_cval_el0");
    sysreg_read_command!(regname, "cntp_tval_el0");
    sysreg_read_command!(regname, "cntvct_el0");
    sysreg_read_command!(regname, "cntv_ctl_el0");
    sysreg_read_command!(regname, "cntv_cval_el0");
    sysreg_read_command!(regname, "cntv_tval_el0");

    println!("Could not find register {} in list (you may need to add it to kernel/kernel/sysreg.rs)", regname);
    0
}

#[cfg(target_arch = "aarch64")]
fn cmd_sysreg(argc: i32, argv: &[CmdArg], _flags: u32) -> i32 {
    if argc < 2 {
        println!("not enough arguments");
        return -1;
    }
    read_sysregs(&argv[1].str_val());
    0
}

#[cfg(target_arch = "aarch64")]
pub fn init() {
    static_command!("sysreg", "read armv8 system register", "cmd_sysreg");
}

// External function declarations
extern "C" {
    fn __arm_rsr64(reg: &str) -> u64;
    fn println(fmt: &str, ...);
}

// Structure definitions for command handling
#[cfg(target_arch = "aarch64")]
pub struct CmdArg {
    // Fields would match those from cmd_args in original code
}

#[cfg(target_arch = "aarch64")]
impl CmdArg {
    pub fn str_val(&self) -> &str {
        // Implementation would extract the string value from the cmd_args
        ""
    }
}

// Macro for static command registration
#[cfg(target_arch = "aarch64")]
macro_rules! static_command {
    ($name:expr, $desc:expr, $func:expr) => {
        // This would register the command with the console subsystem
        // The actual implementation would depend on how the console system is structured
    };
}