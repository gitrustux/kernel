Key Changes in the Rust Rewrite
    1. Naming Conventions:
        ◦ Replace zx_ prefixes with rx_.
        ◦ Replace GIC-related constants and types with Rust-friendly names.
        ◦ Use Rust's snake_case for function and variable names.
    2. Memory Safety:
        ◦ Use Rust's ownership and borrowing model to avoid unsafe memory access.
        ◦ Minimize the use of unsafe blocks, but some will be necessary for low-level hardware interactions.
    3. Error Handling:
        ◦ Replace zx_status_t with Rust’s Result type for better error handling.
    4. Data Structures:
        ◦ Replace C-style structs with Rust structs and enums.
        ◦ Use Rust's #[repr(C)] and #[repr(packed)] for compatibility with hardware registers.
    5. Hardware Interaction:
        ◦ Use Rust's volatile API for accessing hardware registers.
        ◦ Replace DEBUG_ASSERT with Rust's assert! macro.
    6. Modularity:
        ◦ Organize the code into modules that align with the provided file structure.

Key Differences in GICv3
    1. System Registers: GICv3 uses system registers (e.g., ICH_HCR_EL2, ICH_VMCR_EL2) instead of memory-mapped registers for many operations.
    2. Affinity Routing: GICv3 supports affinity routing, which requires handling SGIs and PPIs differently from SPIs.
    3. No GICV Mapping: Unlike GICv2, GICv3 does not require mapping the GICV region for virtualization.

Rust Implementation of el2.S

Since Rust does not support inline assembly directly (without the asm! macro, which is unstable), we’ll use the asm! macro to write the equivalent assembly code in Rust. This will allow us to interact with the GICv3 system registers directly.
    1. Prefix Replacement: All zx_ prefixes are replaced with rx_.
    2. Assembly in Rust: The asm! macro is used to embed assembly code directly in Rust.
    3. Safety: The unsafe keyword is used because these functions directly interact with hardware registers.
    4. Assertions: Added assertions to ensure indices are within valid ranges.
    
This file (el2_cpu_state.cpp) handles the initialization and management of EL2 (Exception Level 2) CPU state, including translation tables, stacks, and VMIDs (Virtual Machine Identifiers). Below is the Rust rewrite of this file, replacing zx_ prefixes with rx_ and adapting the code to Rust's idioms and safety guarantees.

Key Changes
    1. Prefix Replacement: All zx_ prefixes are replaced with rx_.
    2. Memory Management: Rust's Box and Vec are used for dynamic memory allocation.
    3. Concurrency: Rust's Mutex and AtomicUsize are used for thread-safe access to shared state.
    4. Error Handling: Rust's Result type is used for error handling.
    5. Safety: The unsafe keyword is used sparingly, only for low-level hardware interactions.

Guest.rs
Key Changes
    1. Prefix Replacement: All zx_ prefixes are replaced with rx_.
    2. Memory Management: Rust's Arc (Atomic Reference Counting) is used for shared ownership of resources like GuestPhysicalAddressSpace.
    3. Concurrency: Rust's Mutex is used for thread-safe access to shared state.
    4. Error Handling: Rust's Result type is used for error handling.
    5. Safety: The unsafe keyword is avoided unless absolutely necessary for low-level hardware interactions.

Vcpu.rs:

    1. Memory Management: Rust's Arc (Atomic Reference Counting) is used for shared ownership of resources like Guest and Thread.
    2. Concurrency: Rust's AtomicBool is used for thread-safe access to the running state.
    3. Error Handling: Rust's Result type is used for error handling.
    4. Safety: The unsafe keyword is used sparingly, only for low-level hardware interactions.
    
Vmexit:
    1. Prefix Replacement: All zx_ prefixes are replaced with rx_.
    2. Error Handling: Rust's Result type is used for error handling.
    3. Safety: The unsafe keyword is avoided unless absolutely necessary for low-level hardware interactions.
    4. Modularity: The code is organized into functions that match the original logic.

State Priv:
    1. Prefix Replacement: All zx_ prefixes are replaced with rx_.
    2. Memory Management: Rust's Box and Vec are used for dynamic memory allocation.
    3. Concurrency: Rust's Mutex and AtomicUsize are used for thread-safe access to shared state.
    4. Error Handling: Rust's Result type is used for error handling.
    5. Safety: The unsafe keyword is used sparingly, only for low-level hardware interactions.

    1. Structures: The structures (FpState, SystemState, GuestState, HostState, El2State) are translated into Rust structs.
    2. Functions: The functions (rx_el2_on, rx_el2_off, rx_el2_tlbi_ipa, rx_el2_tlbi_vmid, rx_el2_resume) are declared as placeholders for EL2 operations.