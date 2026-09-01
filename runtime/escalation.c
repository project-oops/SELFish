/*
 * escalation.c - Kernel R/W and SYSTEM_AUTHID escalation
 *
 * This module provides kernel memory read/write capabilities and
 * process credential escalation to SYSTEM_AUTHID.
 *
 * Based on ps5-payload-dev/sdk/crt/kernel.c implementation.
 */

#include <stdint.h>
#include <stddef.h>

// Kernel offsets (from ps5-payload-dev/sdk)
#define KERNEL_OFFSET_PROC_P_UCRED   0x40
#define KERNEL_OFFSET_UCRED_CR_SCEAUTHID 0x58

// SYSTEM_AUTHID value
#define SYSTEM_AUTHID 0x3000000000000001

// Function pointers from libkernel
int (*sceKernelGetProcessId)(void) = 0;
int (*sceKernelGetProcessInfo)(int pid, unsigned int info, void *buffer, size_t size) = 0;
int (*sceKernelReadProcessMemory)(int pid, const void *addr, void *buf, size_t size) = 0;
int (*sceKernelWriteProcessMemory)(int pid, void *addr, const void *buf, size_t size) = 0;

// Internal state
static int kernel_rw_initialized = 0;
static unsigned long kernel_base = 0;

/*
 * Initialize kernel R/W capabilities
 * Returns: 0 on success, -1 on failure
 */
static int kernel_rw_init(void) {
    if (kernel_rw_initialized) {
        return 0;
    }

    // Resolve libkernel functions (would normally be done via crt0)
    // For now, we'll assume they're available or resolve them manually
    // In a real implementation, these would be resolved from the payload_args

    kernel_rw_initialized = 1;
    return 0;
}

/*
 * Get current process address
 * Returns: process kernel address or 0 on failure
 */
static unsigned long kernel_get_current_proc(void) {
    if (!kernel_rw_initialized) {
        if (kernel_rw_init() != 0) {
            return 0;
        }
    }

    int pid = sceKernelGetProcessId();
    if (pid < 0) {
        return 0;
    }

    // Get process info to locate the process structure
    // This is simplified - actual implementation would use kernel memory scanning
    return 0; // Placeholder
}

/*
 * Read kernel memory
 * Returns: 0 on success, -1 on failure
 */
static int kernel_copyout(unsigned long kaddr, void *buf, size_t size) {
    if (!kernel_rw_initialized) {
        if (kernel_rw_init() != 0) {
            return -1;
        }
    }

    // Use sceKernelReadProcessMemory to read from kernel
    // This requires the kernel process to be accessible
    return sceKernelReadProcessMemory(-1, (void *)kaddr, buf, size);
}

/*
 * Write to kernel memory
 * Returns: 0 on success, -1 on failure
 */
static int kernel_copyin(const void *buf, unsigned long kaddr, size_t size) {
    if (!kernel_rw_initialized) {
        if (kernel_rw_init() != 0) {
            return -1;
        }
    }

    // Use sceKernelWriteProcessMemory to write to kernel
    return sceKernelWriteProcessMemory(-1, (void *)kaddr, buf, size);
}

/*
 * Escalate current process to SYSTEM_AUTHID
 * Returns: 0 on success, -1 on failure
 */
static int escalate_to_system_authid(void) {
    if (!kernel_rw_initialized) {
        if (kernel_rw_init() != 0) {
            return -1;
        }
    }

    unsigned long proc_addr = kernel_get_current_proc();
    if (!proc_addr) {
        return -1;
    }

    // Calculate ucred address
    unsigned long ucred_addr = proc_addr + KERNEL_OFFSET_PROC_P_UCRED;

    // Read current authid (optional - for debugging)
    uint64_t current_authid = 0;
    if (kernel_copyout(ucred_addr + KERNEL_OFFSET_UCRED_CR_SCEAUTHID, &current_authid, sizeof(current_authid)) != 0) {
        return -1;
    }

    // Write SYSTEM_AUTHID
    uint64_t system_authid = SYSTEM_AUTHID;
    if (kernel_copyin(&system_authid, ucred_addr + KERNEL_OFFSET_UCRED_CR_SCEAUTHID, sizeof(system_authid)) != 0) {
        return -1;
    }

    return 0;
}

/*
 * Check if current process has SYSTEM_AUTHID
 * Returns: 1 if has SYSTEM_AUTHID, 0 otherwise, -1 on error
 */
static int has_system_authid(void) {
    if (!kernel_rw_initialized) {
        if (kernel_rw_init() != 0) {
            return -1;
        }
    }

    unsigned long proc_addr = kernel_get_current_proc();
    if (!proc_addr) {
        return -1;
    }

    unsigned long ucred_addr = proc_addr + KERNEL_OFFSET_PROC_P_UCRED;
    uint64_t current_authid = 0;

    if (kernel_copyout(ucred_addr + KERNEL_OFFSET_UCRED_CR_SCEAUTHID, &current_authid, sizeof(current_authid)) != 0) {
        return -1;
    }

    return (current_authid == SYSTEM_AUTHID) ? 1 : 0;
}

/*
 * Auto-escalation wrapper for payloads
 * Attempts escalation if not already SYSTEM_AUTHID
 * Returns: 0 on success, -1 on failure
 */
static int auto_escalate_if_needed(void) {
    int has_authid = has_system_authid();
    if (has_authid == 1) {
        // Already has SYSTEM_AUTHID
        return 0;
    } else if (has_authid == 0) {
        // Need to escalate
        return escalate_to_system_authid();
    } else {
        // Error checking authid
        return -1;
    }
}