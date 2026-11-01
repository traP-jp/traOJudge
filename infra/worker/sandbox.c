/* Trusted launcher. The workload gets no network syscalls or elevated identity. */
#define _GNU_SOURCE
#include <errno.h>
#include <grp.h>
#include <seccomp.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <unistd.h>

static void limit(int resource, rlim_t soft, rlim_t hard) {
    struct rlimit value = {soft, hard};
    if (setrlimit(resource, &value)) { perror("setrlimit"); exit(125); }
}

int main(int argc, char **argv) {
    if (argc < 4) return 125;
    unsigned long cpu = strtoul(argv[1], NULL, 10);
    unsigned long memory = strtoul(argv[2], NULL, 10);
    limit(RLIMIT_CPU, cpu, cpu + 1);
    limit(RLIMIT_AS, memory * 1024 * 1024, memory * 1024 * 1024);
    limit(RLIMIT_FSIZE, 8 * 1024 * 1024, 8 * 1024 * 1024);
    limit(RLIMIT_NPROC, 32, 32);
    limit(RLIMIT_NOFILE, 64, 64);
    limit(RLIMIT_CORE, 0, 0);
    if (setgroups(0, NULL) || setgid(65532) || setuid(65532)) { perror("drop identity"); return 125; }
    if (prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)) { perror("no_new_privs"); return 125; }
    scmp_filter_ctx filter = seccomp_init(SCMP_ACT_ALLOW);
    if (!filter) return 125;
    const char *blocked[] = {
        "socket", "socketpair", "connect", "bind", "listen", "accept", "accept4",
        "ptrace", "process_vm_readv", "process_vm_writev", "pidfd_getfd",
        "mount", "umount2", "pivot_root", "chroot", "unshare", "setns",
        "bpf", "perf_event_open", "keyctl", "add_key", "request_key",
        "reboot", "kexec_load", "init_module", "finit_module", "delete_module",
        "setpgid", "setsid", "userfaultfd", "io_uring_setup", "open_by_handle_at"
    };
    for (size_t i = 0; i < sizeof(blocked) / sizeof(blocked[0]); ++i) {
        int number = seccomp_syscall_resolve_name(blocked[i]);
        if (number != __NR_SCMP_ERROR && seccomp_rule_add(filter, SCMP_ACT_ERRNO(EPERM), number, 0)) return 125;
    }
    if (seccomp_load(filter)) { perror("seccomp_load"); return 125; }
    seccomp_release(filter);
    execvp(argv[3], &argv[3]);
    perror("execvp");
    return 125;
}
