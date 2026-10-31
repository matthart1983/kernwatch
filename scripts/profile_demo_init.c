/* PID 1 for an isolated, networkless recording VM. No host mounts; the tour's only
 * disk is an expendable image. */
#include <dirent.h>
#include <fcntl.h>
#include <linux/module.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mount.h>
#include <sys/reboot.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <termios.h>
#include <unistd.h>

static void put(const char *path, const char *value) {
    int fd = open(path, O_WRONLY);
    if (fd < 0) return;
    if (write(fd, value, strlen(value)) < 0) perror(path);
    close(fd);
}

/* Failures only leave the tour's Modules view shorter. */
static void load_modules(void) {
    DIR *dir = opendir("/modules");
    if (!dir) return;
    struct dirent *entry;
    while ((entry = readdir(dir))) {
        char path[512];
        snprintf(path, sizeof(path), "/modules/%s", entry->d_name);
        int fd = open(path, O_RDONLY);
        if (fd < 0) continue;
        syscall(SYS_finit_module, fd, "", MODULE_INIT_COMPRESSED_FILE);
        close(fd);
    }
    closedir(dir);
}

static void service(const char *name, const char *quota, pid_t pid) {
    char path[128], file[160], value[16];
    snprintf(path, sizeof(path), "/sys/fs/cgroup/system.slice/%s", name);
    mkdir(path, 0755);
    snprintf(file, sizeof(file), "%s/cpu.max", path);
    put(file, quota);
    snprintf(file, sizeof(file), "%s/cgroup.procs", path);
    snprintf(value, sizeof(value), "%d", pid);
    put(file, value);
}

static void evidence(void) {
    FILE *out = fopen("/dev/ttyS0", "w");
    DIR *dir = opendir("/tmp");
    if (!out || !dir) return;
    struct dirent *entry;
    const char *names[] = {"profile.json", "baseline.profile.json", "comparison.json"};
    while ((entry = readdir(dir))) {
        if (strncmp(entry->d_name, "kernwatch-report-", 17)) continue;
        for (unsigned i = 0; i < sizeof(names) / sizeof(names[0]); ++i) {
            char path[512];
            snprintf(path, sizeof(path), "/tmp/%s/%s", entry->d_name, names[i]);
            FILE *in = fopen(path, "r");
            if (!in) continue;
            fprintf(out, "BEGIN %s/%s\n", entry->d_name, names[i]);
            int c;
            while ((c = fgetc(in)) != EOF) fputc(c, out);
            fprintf(out, "\nEND\n");
            fclose(in);
        }
    }
    closedir(dir);
    fclose(out);
}

int main(void) {
    mount("proc", "/proc", "proc", 0, 0);
    mount("sysfs", "/sys", "sysfs", 0, 0);
    mount("devtmpfs", "/dev", "devtmpfs", 0, 0);
    mount("tracefs", "/sys/kernel/tracing", "tracefs", 0, 0);
    mkdir("/sys/fs/cgroup", 0755);
    mount("none", "/sys/fs/cgroup", "cgroup2", 0, 0);
    sethostname("demo-vm", 7);
    setenv("TERM", "xterm-256color", 1);
    setenv("XDG_STATE_HOME", "/tmp/state", 1);
    chdir("/tmp");
    int tour = access("/tour", F_OK) == 0;
    if (tour) load_modules();
    pid_t load = fork();
    if (!load) { execl("/profile-demo", "profile-demo", (char *)0); _exit(127); }
    pid_t batch = 0;
    if (tour) {
        batch = fork();
        if (!batch) { execl("/profile-demo", "profile-demo", "batch", (char *)0); _exit(127); }
        put("/sys/fs/cgroup/cgroup.subtree_control", "+cpu +memory +io +pids");
        mkdir("/sys/fs/cgroup/system.slice", 0755);
        put("/sys/fs/cgroup/system.slice/cgroup.subtree_control", "+cpu +memory +io +pids");
        service("webapp.service", "150000 100000", load);
        service("batch.service", "max 100000", batch);
    }
    pid_t app = fork();
    if (!app) {
        setsid();
        int terminal = open("/dev/hvc0", O_RDWR);
        ioctl(terminal, TIOCSCTTY, 0);
        struct winsize size = {.ws_row = 48, .ws_col = 160};
        ioctl(terminal, TIOCSWINSZ, &size);
        for (int fd = 0; fd < 3; ++fd) dup2(terminal, fd);
        execl("/kernwatch", "kernwatch", "--view", "dense", (char *)0);
        _exit(127);
    }
    int status;
    waitpid(app, &status, 0);
    kill(load, SIGTERM);
    waitpid(load, 0, 0);
    if (batch) kill(batch, SIGTERM);
    evidence();
    sync();
    reboot(RB_POWER_OFF);
    return 0;
}
