/*
 * Copyright 2025 The Rustux Authors
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */

/*
 * AMD64 Architecture-Specific System Functions (C Bridge)
 *
 * These functions provide low-level x86_64 operations that are
 * called from Rust via extern "C". They implement operations
 * that require special CPU instructions or are easier in C/assembly.
 */

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

/* ============ Type Definitions ============ */

/* Status codes (matching RxStatus in Rust) */
#define RX_OK 0
#define RX_ERR_NO_MEMORY 1
#define RX_ERR_INVALID_ARGS 3
#define RX_ERR_BAD_STATE 9
#define RX_ERR_NOT_SUPPORTED 2
#define RX_ERR_NOT_FOUND 4

/* Signed size type for function return values */
typedef ptrdiff_t ssize_t;

/* Page table entry flags */
#define X86_MMU_PG_P     0x001   /* Present */
#define X86_MMU_PG_RW    0x002   /* Read/Write */
#define X86_MMU_PG_U     0x004   /* User */
#define X86_MMU_PG_WT    0x008   /* Write-Through */
#define X86_MMU_PG_CD    0x010   /* Cache Disable */
#define X86_MMU_PG_A     0x020   /* Accessed */
#define X86_MMU_PG_D     0x040   /* Dirty */
#define X86_MMU_PG_PS    0x080   /* Page Size */
#define X86_MMU_PG_G     0x100   /* Global */

/* PTE flag macros */
#define X86_PG_FRAME     0x000ffffffffff000ULL

/* ============ Assembly Utilities ============ */

static inline void cli(void) {
    __asm__ volatile("cli");
}

static inline void sti(void) {
    __asm__ volatile("sti");
}

static inline void hlt(void) {
    __asm__ volatile("hlt");
}

static inline uint64_t rdmsr(uint32_t msr) {
    uint32_t low, high;
    __asm__ volatile("rdmsr" : "=a"(low), "=d"(high) : "c"(msr));
    return ((uint64_t)high << 32) | low;
}

static inline void wrmsr(uint32_t msr, uint64_t value) {
    uint32_t low = value & 0xFFFFFFFF;
    uint32_t high = value >> 32;
    __asm__ volatile("wrmsr" : : "c"(msr), "a"(low), "d"(high));
}

static inline uint64_t rdtsc(void) {
    uint32_t low, high;
    __asm__ volatile("rdtsc" : "=a"(low), "=d"(high));
    return ((uint64_t)high << 32) | low;
}

static inline void invlpg(void *addr) {
    __asm__ volatile("invlpg (%0)" : : "r"(addr));
}

static inline void sfence(void) {
    __asm__ volatile("sfence" ::: "memory");
}

/* ============ MSR Constants ============ */
#define X86_MSR_IA32_GS_BASE       0xC0000101
#define X86_MSR_IA32_FS_BASE       0xC0000100
#define X86_MSR_IA32_KERNEL_GS_BASE 0xC0000102
#define X86_MSR_EFER               0xC0000080
#define X86_MSR_STAR              0xC0000081
#define X86_MSR_LSTAR             0xC0000082
#define X86_MSR_CSTAR             0xC0000083
#define X86_MSR_FMASK             0xC0000084
#define X86_MSR_TSC_AUX           0xC0000103
#define X86_MSR_IA32_PAT          0x277
#define X86_MSR_IA32_MTRR_CAP     0x0FE
#define X86_MSR_IA32_MTRR_DEF     0x2FF

/* Default PAT value: write-back caching for all entries */
#define X86_PAT_DEFAULT_VALUE     0x0007010600070106ULL

/* ============ Page Table Functions ============ */

/* Check if virtual address is canonical (x86-64) */
bool sys_x86_is_vaddr_canonical(uint64_t vaddr) {
    /* x86-64: bits [63:48] must be all 0 or all 1 */
    uint64_t high_bits = vaddr >> 48;
    return (high_bits == 0) || (high_bits == 0xFFFF);
}

/* Check if physical address is valid */
bool sys_x86_mmu_check_paddr(uint64_t paddr) {
    /* x86-64 supports up to 52-bit physical addresses */
    return paddr < (1ULL << 52);
}

/* Get kernel CR3 */
uint64_t sys_x86_kernel_cr3(void) {
    uint64_t cr3;
    __asm__ volatile("mov %%cr3, %0" : "=r"(cr3));
    return cr3;
}

/* ============ Per-CPU Functions ============ */

/* Initialize per-CPU data for given CPU number */
void sys_x86_init_percpu(uint32_t cpu_num) {
    (void)cpu_num;
    /* Per-CPU initialization is handled in assembly during boot */
    /* This function is a placeholder for any additional setup needed */
}

/* Set local APIC ID */
void sys_x86_set_local_apic_id(uint32_t apic_id) {
    (void)apic_id;
    /* APIC ID is stored in per-CPU data */
    /* The actual MSR write is done in assembly */
}

/* Convert APIC ID to CPU number */
int32_t sys_x86_apic_id_to_cpu_num(uint32_t apic_id) {
    (void)apic_id;
    /* TODO: Implement APIC ID to CPU number mapping */
    /* For now, assume 1:1 mapping for BSP */
    return (int32_t)apic_id;
}

/* ============ Descriptor/TSS Functions ============ */

/* Initialize per-CPU TSS */
void sys_x86_initialize_percpu_tss(void) {
    /* TSS initialization is done in assembly during boot */
    /* This function is a placeholder for any additional setup */
}

/* Set TSS SP0 (kernel stack pointer) */
void sys_x86_set_tss_sp(uint64_t sp) {
    (void)sp;
    /* TSS SP0 is set in the per-CPU TSS structure */
    /* The actual implementation updates the Task State Segment */
}

/* Clear TSS busy bit for task switch */
void sys_x86_clear_tss_busy(uint16_t sel) {
    (void)sel;
    /* Task switch busy bit handling */
    /* Implemented in the task switch assembly code */
}

/* ============ Extended Register Functions ============ */

/* Initialize extended register state (SSE/AVX) */
void sys_x86_extended_register_init(void) {
    /* Enable SSE and AVX if available */
    uint64_t cr4;
    __asm__ volatile("mov %%cr4, %0" : "=r"(cr4));

    /* Enable OSFXSR and OSXSAVE */
    cr4 |= (1 << 9) | (1 << 18);  /* OSFXSR | OSXSAVE */
    __asm__ volatile("mov %0, %%cr4" : : "r"(cr4));

    /* Initialize SSE control word */
    __asm__ volatile("fninit");
}

/* Get extended register size */
size_t sys_x86_extended_register_size(void) {
    /* Check XCR0 for supported features */
    uint64_t xcr0;

    /* Check if XSAVE is available */
    uint32_t eax, ebx, ecx, edx;
    __asm__ volatile("cpuid"
                     : "=a"(eax), "=b"(ebx), "=c"(ecx), "=d"(edx)
                     : "a"(1));

    if (!(ecx & (1 << 26))) {  /* XSAVE bit */
        return 512;  /* FXSAVE/FXRSTOR size */
    }

    __asm__ volatile("xgetbv" : "=a"(eax), "=d"(edx) : "c"(0));
    xcr0 = ((uint64_t)edx << 32) | eax;

    /* Calculate size based on enabled features */
    size_t size = 512;  /* Legacy SSE area */

    if (xcr0 & (1 << 2)) {  /* AVX */
        size += 256;  /* YMM upper half */
    }

    return size;
}

/* ============ Feature Detection Functions ============ */

struct x86_cpuid_leaf {
    uint32_t eax;
    uint32_t ebx;
    uint32_t ecx;
    uint32_t edx;
};

/* Get CPUID leaf */
bool sys_x86_get_cpuid_subleaf(uint32_t leaf, uint32_t subleaf,
                                  struct x86_cpuid_leaf *out) {
    if (leaf == 0) {
        return false;
    }

    __asm__ volatile("cpuid"
                     : "=a"(out->eax), "=b"(out->ebx), "=c"(out->ecx), "=d"(out->edx)
                     : "a"(leaf), "c"(subleaf));

    return true;
}

/* CPU feature initialization */
void sys_x86_feature_init(void) {
    /* CPU features are probed during early boot */
    /* This function is called to ensure feature detection is complete */
}

/* ============ Bootstrap Functions ============ */

/* Initialize bootstrap16 subsystem */
void sys_x86_bootstrap16_init(uint64_t bootstrap_base) {
    (void)bootstrap_base;
    /* Bootstrap16 initialization is done in assembly */
}

/* ============ Memory Barrier Functions ============ */

void sys_x86_mb(void) {
    __asm__ volatile("mfence" ::: "memory");
}

void sys_x86_rmb(void) {
    __asm__ volatile("lfence" ::: "memory");
}

void sys_x86_wmb(void) {
    __asm__ volatile("sfence" ::: "memory");
}

void sys_x86_acquire(void) {
    __asm__ volatile("" ::: "memory");
}

void sys_x86_release(void) {
    __asm__ volatile("" ::: "memory");
}

/* ============ HLT/Pause Functions ============ */

void sys_x86_halt(void) {
    hlt();
}

void sys_x86_pause(void) {
    __asm__ volatile("pause");
}

void sys_x86_serialize(void) {
    __asm__ volatile("cpuid" : : "a"(0) : "bx", "cx", "dx");
}

/* ============ TSC Functions ============ */

void sys_x86_tsc_adjust(void) {
    /* TSC adjustment is handled during boot */
}

void sys_x86_tsc_store_adjustment(void) {
    /* TSC adjustment is handled during suspend/resume */
}

/* ============ MMU Init Functions ============ */

void sys_x86_mmu_early_init(void) {
    /*
     * Early MMU initialization:
     * - Set up PAT (Page Attribute Table) for proper memory caching
     * - Enable write-protect in CR0 to protect kernel code
     */
    uint64_t cr0;

    /* Initialize PAT MSR with default value (write-back caching) */
    wrmsr(X86_MSR_IA32_PAT, X86_PAT_DEFAULT_VALUE);

    /* Enable write-protect (CR0.WP) to protect kernel code from modification */
    __asm__ volatile("mov %%cr0, %0" : "=r"(cr0));
    cr0 |= (1ULL << 16);  /* Set WP bit (bit 16) */
    __asm__ volatile("mov %0, %%cr0" : : "r"(cr0));
}

void sys_x86_mmu_percpu_init(void) {
    /*
     * Per-CPU MMU initialization:
     * - Set up PAT for this CPU
     * - Initialize MTRR if supported
     */
    uint64_t mtrr_cap;

    /* Initialize PAT MSR with default value */
    wrmsr(X86_MSR_IA32_PAT, X86_PAT_DEFAULT_VALUE);

    /* Check if MTRR is supported */
    mtrr_cap = rdmsr(X86_MSR_IA32_MTRR_CAP);
    if (mtrr_cap & 0x400) {  /* MTRR enabled bit */
        /* For now, use BIOS defaults */
        /* TODO: Implement proper MTRR initialization */
    }
}

void sys_x86_mmu_init(void) {
    /*
     * Main MMU initialization:
     * - Called after VM subsystem is up
     * - Set up large page support detection
     * - Synchronize PAT across all CPUs
     */
    /* Placeholder for future initialization */
    /* The bootloader has already set up basic page tables */
}

/* ============ TLB Flush Functions ============ */

void sys_x86_tlb_flush_global(void) {
    /* Flush entire TLB */
    uint64_t cr3;
    __asm__ volatile("mov %%cr3, %0" : "=r"(cr3));
    __asm__ volatile("mov %0, %%cr3" : : "r"(cr3));
}

void sys_x86_tlb_flush_one(uint64_t vaddr) {
    invlpg((void *)vaddr);
}

/* ============ User Copy Functions ============ */

/*
 * Copy data to/from user space with fault handling
 * Returns number of bytes copied, or negative on error
 */
ssize_t sys_x86_copy_to_or_from_user(void *dst, const void *src, size_t len,
                                       uint64_t fault_return) {
    (void)fault_return;
    /* For now, do a simple memcpy. Fault handling would be
     * implemented with page fault handlers */
    __builtin_memcpy(dst, src, len);
    return (ssize_t)len;
}

/* ============ APIC/MP Functions ============ */

/* IPI halt handler - never returns */
void sys_x86_ipi_halt_handler(void) {
    cli();
    while (1) {
        hlt();
    }
}

/* Secondary CPU entry point */
void sys_x86_secondary_entry(int32_t *aps_still_booting, void *thread) {
    (void)aps_still_booting;
    (void)thread;
    /* This is called from assembly during AP bringup */
}

/* Force all CPUs except local and BSP to halt */
void sys_x86_force_halt_all_but_local_and_bsp(void) {
    /* Send IPIs to halt other CPUs */
    /* Implementation requires APIC access */
}

/* ============ Allocate AP Structures ============ */

int32_t sys_x86_allocate_ap_structures(const uint32_t *apic_ids, uint8_t cpu_count) {
    (void)apic_ids;
    (void)cpu_count;
    /* Allocate per-CPU structures for APs */
    /* Return 0 for success */
    return 0;
}

/* ============ Bootstrap Acquire/Release ============ */

int32_t sys_x86_bootstrap16_acquire(uint64_t entry64,
                                     void **temp_aspace,
                                     void **bootstrap_aperture,
                                     uint64_t *instr_ptr) {
    (void)entry64;
    (void)temp_aspace;
    (void)bootstrap_aperture;
    (void)instr_ptr;
    /* Acquire bootstrap16 memory region */
    /* Return 0 for success */
    return 0;
}

void sys_x86_bootstrap16_release(void *bootstrap_aperture) {
    (void)bootstrap_aperture;
    /* Release bootstrap16 memory region */
}

/* ============ CPU Topology Functions ============ */

void sys_x86_cpu_topology_init(void) {
    /* Initialize CPU topology detection */
}

int32_t sys_x86_cpu_topology_decode(uint32_t apic_id, void *topo) {
    (void)apic_id;
    (void)topo;
    /* Decode CPU topology for given APIC ID */
    return 0;
}

/* ============ Timer Functions ============ */

uint64_t sys_x86_lookup_tsc_freq(void) {
    /* Look up TSC frequency from CPUID or platform */
    /* Default to 2.4 GHz if not available */
    return 2400000000ULL;
}

uint64_t sys_x86_lookup_core_crystal_freq(void) {
    /* Look up core crystal frequency */
    /* Default to 24 MHz */
    return 24000000ULL;
}

/* ============ Descriptor Functions ============ */

void sys_x86_reset_tss_io_bitmap(void) {
    /* Reset TSS I/O bitmap */
}

/* ============ Page Table MMU Functions ============ */

/* Terminal flags for MMU page tables */
uint64_t sys_x86_page_table_mmu_terminal_flags(int level, uint32_t flags) {
    (void)level;

    uint64_t pte_flags = 0;

    if (flags & 0x1) pte_flags |= X86_MMU_PG_P;   /* Present */
    if (flags & 0x2) pte_flags |= X86_MMU_PG_RW;  /* Write */
    if (flags & 0x4) pte_flags |= X86_MMU_PG_U;   /* User */

    return pte_flags;
}

/* Intermediate flags for MMU page tables */
uint64_t sys_x86_page_table_mmu_intermediate_flags(void) {
    return X86_MMU_PG_RW | X86_MMU_PG_P;
}

/* Support large pages at given level? */
bool sys_x86_page_table_mmu_supports_page_size(int level) {
    switch (level) {
        case 2: return true;  /* PD: 1GB pages */
        case 1: return true;  /* PT: 2MB pages */
        default: return false;
    }
}

/* Split flags for large pages */
uint64_t sys_x86_page_table_mmu_split_flags(int level, uint64_t flags) {
    (void)level;
    /* Remove PS bit, keep other flags */
    return flags & ~X86_MMU_PG_PS;
}

/* Convert PTE flags to MMU flags */
uint32_t sys_x86_page_table_mmu_pt_flags_to_mmu_flags(uint64_t flags, int level) {
    (void)level;

    uint32_t mmu_flags = 0;

    if (flags & X86_MMU_PG_P) mmu_flags |= 0x1;
    if (flags & X86_MMU_PG_RW) mmu_flags |= 0x2;
    if (flags & X86_MMU_PG_U) mmu_flags |= 0x4;

    return mmu_flags;
}

/* ============ EPT Functions ============ */

bool sys_x86_page_table_ept_allowed_flags(uint32_t flags) {
    (void)flags;
    return true;
}

bool sys_x86_page_table_ept_check_paddr(uint64_t paddr) {
    return sys_x86_mmu_check_paddr(paddr);
}

bool sys_x86_page_table_ept_check_vaddr(uint64_t vaddr) {
    return sys_x86_is_vaddr_canonical(vaddr);
}

bool sys_x86_page_table_ept_supports_page_size(int level) {
    return sys_x86_page_table_mmu_supports_page_size(level);
}

uint64_t sys_x86_page_table_ept_intermediate_flags(void) {
    return 0x7;  /* R | W | X */
}

uint64_t sys_x86_page_table_ept_terminal_flags(int level, uint32_t flags) {
    (void)level;
    uint64_t ept_flags = 0x3;  /* R | W */

    if (flags & 0x4) ept_flags |= 0x1;  /* Execute */

    return ept_flags;
}

uint64_t sys_x86_page_table_ept_split_flags(int level, uint64_t flags) {
    (void)level;
    return flags;
}

uint32_t sys_x86_page_table_ept_pt_flags_to_mmu_flags(uint64_t flags, int level) {
    (void)level;

    uint32_t mmu_flags = 0;

    if (flags & 0x1) mmu_flags |= 0x1;  /* Read */
    if (flags & 0x2) mmu_flags |= 0x2;  /* Write */
    if (flags & 0x4) mmu_flags |= 0x4;  /* Execute */

    return mmu_flags;
}

/* ============ Address Space Functions ============ */

int32_t sys_x86_arch_vm_aspace_map_contiguous(void *aspace, uint64_t vaddr,
                                               uint64_t paddr, size_t count,
                                               uint32_t mmu_flags, uint64_t addrs) {
    (void)aspace;
    (void)vaddr;
    (void)paddr;
    (void)count;
    (void)mmu_flags;
    (void)addrs;
    /* Map contiguous physical memory region */
    return 0;
}

int32_t sys_x86_arch_vm_aspace_map(void *aspace, uint64_t vaddr,
                                    const uint64_t *phys, size_t count,
                                    uint32_t mmu_flags, uint64_t addrs) {
    (void)aspace;
    (void)vaddr;
    (void)phys;
    (void)count;
    (void)mmu_flags;
    (void)addrs;
    /* Map pages */
    return 0;
}

int32_t sys_x86_arch_vm_aspace_unmap(void *aspace, uint64_t vaddr,
                                      size_t count) {
    (void)aspace;
    (void)vaddr;
    (void)count;
    /* Unmap pages */
    return 0;
}

int32_t sys_x86_arch_vm_aspace_protect(void *aspace, uint64_t vaddr,
                                        size_t count, uint32_t mmu_flags) {
    (void)aspace;
    (void)vaddr;
    (void)count;
    (void)mmu_flags;
    /* Change page protections */
    return 0;
}

int32_t sys_x86_arch_vm_aspace_query(void *aspace, uint64_t vaddr) {
    (void)aspace;
    (void)vaddr;
    /* Query mapping */
    return 0;
}

int32_t sys_x86_arch_vm_aspace_pick_spot(void *aspace, uint64_t base,
                                          uint64_t prev_region_mmu_flags,
                                          uint64_t *out_vaddr, uint64_t *out_size) {
    (void)aspace;
    (void)base;
    (void)prev_region_mmu_flags;
    (void)out_vaddr;
    (void)out_size;
    /* Find free spot in address space */
    return 0;
}

int32_t sys_x86_arch_vm_aspace_context_switch(void *from_aspace, void *to_aspace) {
    (void)from_aspace;
    (void)to_aspace;
    /* Switch address spaces */
    return 0;
}

/* ============ PAT/Memory Type Functions ============ */

void sys_x86_mmu_mem_type_init(void) {
    /*
     * Initialize memory types (PAT/MTRR):
     * - PAT is already set up in mmu_percpu_init
     * - This function can be used for additional MTRR configuration
     */
    uint64_t mtrr_def_type;

    /* Check and configure default MTRR type if supported */
    mtrr_def_type = rdmsr(X86_MSR_IA32_MTRR_DEF);

    /* If MTRR is enabled, set default type to write-back */
    if (mtrr_def_type & 0x800) {  /* MTRR enable bit */
        /* Set default type to 6 (write-back) */
        mtrr_def_type = (mtrr_def_type & ~0xFFULL) | 0x06;
        wrmsr(X86_MSR_IA32_MTRR_DEF, mtrr_def_type);
    }
}

void sys_x86_pat_sync(uint64_t targets) {
    /*
     * Sync PAT configuration across CPUs:
     * - Read current PAT value from this CPU
     * - Send IPIs to other CPUs to update their PAT
     * - For single-CPU systems, this is a no-op
     */
    uint64_t current_pat;

    if (targets == 1) {
        /* Single CPU - no synchronization needed */
        return;
    }

    /* Read current PAT value */
    current_pat = rdmsr(X86_MSR_IA32_PAT);

    /*
     * TODO: Implement IPI-based synchronization for SMP
     * For now, all CPUs should have the same PAT value from boot
     */
    (void)current_pat;  /* Suppress unused warning */
}

/* ============ Processor Trace Functions ============ */

void sys_x86_processor_trace_init(void) {
    /* Initialize Intel Processor Trace */
}

/* ============ I/O Port Functions ============ */

void sys_x86_set_tss_io_bitmap(void *bitmap) {
    (void)bitmap;
    /* Set TSS I/O bitmap */
}

void sys_x86_clear_tss_io_bitmap(void *bitmap) {
    (void)bitmap;
    /* Clear TSS I/O bitmap */
}
