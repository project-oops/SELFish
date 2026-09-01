#ifndef SELFISH_ESCALATION_H
#define SELFISH_ESCALATION_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern C {
#endif

// SYSTEM_AUTHID value
#define SYSTEM_AUTHID 0x3000000000000001

/*
 * Initialize kernel R/W capabilities
 * Returns: 0 on success, -1 on failure
 */
int kernel_rw_init(void);

/*
 * Get current process kernel address
 * Returns: process kernel address or 0 on failure
 */
unsigned long kernel_get_current_proc(void);

/*
 * Read kernel memory
 * Returns: 0 on success, -1 on failure
 */
int kernel_copyout(unsigned long kaddr, void *buf, size_t size);

/*
 * Write to kernel memory
 * Returns: 0 on success, -1 on failure
 */
int kernel_copyin(const void *buf, unsigned long kaddr, size_t size);

/*
 * Escalate current process to SYSTEM_AUTHID
 * Returns: 0 on success, -1 on failure
 */
int escalate_to_system_authid(void);

/*
 * Check if current process has SYSTEM_AUTHID
 * Returns: 1 if has SYSTEM_AUTHID, 0 otherwise, -1 on error
 */
int has_system_authid(void);

/*
 * Auto-escalation wrapper for payloads
 * Attempts escalation if not already SYSTEM_AUTHID
 * Returns: 0 on success, -1 on failure
 */
int auto_escalate_if_needed(void);

#ifdef __cplusplus
}
#endif

#endif /* SELFISH_ESCALATION_H */
