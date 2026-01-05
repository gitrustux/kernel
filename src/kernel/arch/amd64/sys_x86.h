/*
 * Copyright 2025 The Rustux Authors
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */

#ifndef RUSTUX_SYS_X86_H
#define RUSTUX_SYS_X86_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ============ Type Definitions ============ */

/* CPUID leaf structure */
struct x86_cpuid_leaf {
    uint32_t eax;
    uint32_t ebx;
    uint32_t ecx;
    uint32_t edx;
};

/* ============ Page Table Functions ============ */

bool sys_x86_is_vaddr_canonical(uint64_t vaddr);
bool sys_x86_mmu_check_paddr(uint64_t paddr);
uint64_t sys_x86_kernel_cr3(void);

/* ============ Per-CPU Functions ============ */

void sys_x86_init_percpu(uint32_t cpu_num);
void sys_x86_set_local_apic_id(uint32_t apic_id);
int32_t sys_x86_apic_id_to_cpu_num(uint32_t apic_id);

/* ============ Descriptor/TSS Functions ============ */

void sys_x86_initialize_percpu_tss(void);
void sys_x86_set_tss_sp(uint64_t sp);
void sys_x86_clear_tss_busy(uint16_t sel);
void sys_x86_reset_tss_io_bitmap(void);

/* ============ Extended Register Functions ============ */

void sys_x86_extended_register_init(void);
size_t sys_x86_extended_register_size(void);

/* ============ Feature Detection Functions ============ */

void sys_x86_feature_init(void);
bool sys_x86_get_cpuid_subleaf(uint32_t leaf, uint32_t subleaf,
                                  struct x86_cpuid_leaf *out);

/* ============ Bootstrap Functions ============ */

void sys_x86_bootstrap16_init(uint64_t bootstrap_base);
int32_t sys_x86_bootstrap16_acquire(uint64_t entry64,
                                     void **temp_aspace,
                                     void **bootstrap_aperture,
                                     uint64_t *instr_ptr);
void sys_x86_bootstrap16_release(void *bootstrap_aperture);

/* ============ Memory Barrier Functions ============ */

void sys_x86_mb(void);
void sys_x86_rmb(void);
void sys_x86_wmb(void);
void sys_x86_acquire(void);
void sys_x86_release(void);

/* ============ HLT/Pause Functions ============ */

void sys_x86_halt(void);
void sys_x86_pause(void);
void sys_x86_serialize(void);

/* ============ TSC Functions ============ */

void sys_x86_tsc_adjust(void);
void sys_x86_tsc_store_adjustment(void);

/* ============ MMU Init Functions ============ */

void sys_x86_mmu_early_init(void);
void sys_x86_mmu_percpu_init(void);
void sys_x86_mmu_init(void);

/* ============ TLB Flush Functions ============ */

void sys_x86_tlb_flush_global(void);
void sys_x86_tlb_flush_one(uint64_t vaddr);

/* ============ User Copy Functions ============ */

typedef ptrdiff_t ssize_t;
ssize_t sys_x86_copy_to_or_from_user(void *dst, const void *src, size_t len,
                                       uint64_t fault_return);

/* ============ APIC/MP Functions ============ */

void sys_x86_ipi_halt_handler(void) __attribute__((noreturn));
void sys_x86_secondary_entry(int32_t *aps_still_booting, void *thread);
void sys_x86_force_halt_all_but_local_and_bsp(void);
int32_t sys_x86_allocate_ap_structures(const uint32_t *apic_ids, uint8_t cpu_count);

/* ============ CPU Topology Functions ============ */

void sys_x86_cpu_topology_init(void);
int32_t sys_x86_cpu_topology_decode(uint32_t apic_id, void *topo);

/* ============ Timer Functions ============ */

uint64_t sys_x86_lookup_tsc_freq(void);
uint64_t sys_x86_lookup_core_crystal_freq(void);

/* ============ Page Table MMU Functions ============ */

uint64_t sys_x86_page_table_mmu_terminal_flags(int level, uint32_t flags);
uint64_t sys_x86_page_table_mmu_intermediate_flags(void);
bool sys_x86_page_table_mmu_supports_page_size(int level);
uint64_t sys_x86_page_table_mmu_split_flags(int level, uint64_t flags);
uint32_t sys_x86_page_table_mmu_pt_flags_to_mmu_flags(uint64_t flags, int level);

/* ============ EPT Functions ============ */

bool sys_x86_page_table_ept_allowed_flags(uint32_t flags);
bool sys_x86_page_table_ept_check_paddr(uint64_t paddr);
bool sys_x86_page_table_ept_check_vaddr(uint64_t vaddr);
bool sys_x86_page_table_ept_supports_page_size(int level);
uint64_t sys_x86_page_table_ept_intermediate_flags(void);
uint64_t sys_x86_page_table_ept_terminal_flags(int level, uint32_t flags);
uint64_t sys_x86_page_table_ept_split_flags(int level, uint64_t flags);
uint32_t sys_x86_page_table_ept_pt_flags_to_mmu_flags(uint64_t flags, int level);

/* ============ Address Space Functions ============ */

int32_t sys_x86_arch_vm_aspace_map_contiguous(void *aspace, uint64_t vaddr,
                                               uint64_t paddr, size_t count,
                                               uint32_t mmu_flags, uint64_t addrs);
int32_t sys_x86_arch_vm_aspace_map(void *aspace, uint64_t vaddr,
                                    const uint64_t *phys, size_t count,
                                    uint32_t mmu_flags, uint64_t addrs);
int32_t sys_x86_arch_vm_aspace_unmap(void *aspace, uint64_t vaddr,
                                      size_t count);
int32_t sys_x86_arch_vm_aspace_protect(void *aspace, uint64_t vaddr,
                                        size_t count, uint32_t mmu_flags);
int32_t sys_x86_arch_vm_aspace_query(void *aspace, uint64_t vaddr);
int32_t sys_x86_arch_vm_aspace_pick_spot(void *aspace, uint64_t base,
                                          uint64_t prev_region_mmu_flags,
                                          uint64_t *out_vaddr, uint64_t *out_size);
int32_t sys_x86_arch_vm_aspace_context_switch(void *from_aspace, void *to_aspace);

/* ============ PAT/Memory Type Functions ============ */

void sys_x86_mmu_mem_type_init(void);
void sys_x86_pat_sync(uint64_t targets);

/* ============ Processor Trace Functions ============ */

void sys_x86_processor_trace_init(void);

/* ============ I/O Port Functions ============ */

void sys_x86_set_tss_io_bitmap(void *bitmap);
void sys_x86_clear_tss_io_bitmap(void *bitmap);

#ifdef __cplusplus
}
#endif

#endif /* RUSTUX_SYS_X86_H */
