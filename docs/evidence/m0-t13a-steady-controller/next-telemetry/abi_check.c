/* Measurement harness ABI evidence only, never part of the Rust city model. */
#include <stddef.h>
#include <stdio.h>
#include <sys/proc_info.h>
#include <mach/vm_statistics.h>
#include <mach/host_info.h>
int main(void) {
    printf("{\"proc_bsdshortinfo\":%zu,\"proc_taskinfo\":%zu,\"vm_statistics64\":%zu,", sizeof(struct proc_bsdshortinfo), sizeof(struct proc_taskinfo), sizeof(vm_statistics64_data_t));
    printf("\"short_pid\":%zu,\"short_ppid\":%zu,\"short_comm\":%zu,", offsetof(struct proc_bsdshortinfo,pbsi_pid), offsetof(struct proc_bsdshortinfo,pbsi_ppid), offsetof(struct proc_bsdshortinfo,pbsi_comm));
    printf("\"task_resident\":%zu,\"vm_compressor\":%zu,\"vm_compressions\":%zu,\"vm_decompressions\":%zu,\"vm_swapins\":%zu,\"vm_swapouts\":%zu,\"vm_max_mem\":%zu,\"host_vm_count\":%u}\n", offsetof(struct proc_taskinfo,pti_resident_size), offsetof(vm_statistics64_data_t,compressor_page_count), offsetof(vm_statistics64_data_t,compressions), offsetof(vm_statistics64_data_t,decompressions), offsetof(vm_statistics64_data_t,swapins), offsetof(vm_statistics64_data_t,swapouts), offsetof(vm_statistics64_data_t,max_mem_count), (unsigned)HOST_VM_INFO64_COUNT);
    return 0;
}
