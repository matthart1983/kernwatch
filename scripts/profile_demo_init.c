/* PID 1 for an isolated, networkless recording VM. No host mounts or disks. */
#include <dirent.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mount.h>
#include <sys/reboot.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <termios.h>
#include <unistd.h>

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
    sethostname("profiling-demo-vm", 17);
    setenv("TERM", "xterm-256color", 1);
    setenv("XDG_STATE_HOME", "/tmp/state", 1);
    chdir("/tmp");
    pid_t load = fork();
    if (!load) { execl("/profile-demo", "profile-demo", (char *)0); _exit(127); }
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
    evidence();
    sync();
    reboot(RB_POWER_OFF);
    return 0;
}
