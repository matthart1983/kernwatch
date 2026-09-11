#include <sys/mount.h>
#include <sys/wait.h>
#include <sys/reboot.h>
#include <sys/stat.h>
#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>
int main(void){mkdir("/proc",0755);mkdir("/sys",0755);mkdir("/dev",0755);mkdir("/tmp",0755);mount("proc","/proc","proc",0,0);mount("sysfs","/sys","sysfs",0,0);mount("devtmpfs","/dev","devtmpfs",0,0);mount("tracefs","/sys/kernel/tracing","tracefs",0,0);mkdir("/sys/fs/cgroup",0755);mount("none","/sys/fs/cgroup","cgroup2",0,0);setenv("KERNWATCH_DISPOSABLE_GUEST","1",1);pid_t p=fork();if(p==0){execl("/probe_smoke","probe_smoke",(char*)0);perror("exec");_exit(127);}int status;waitpid(p,&status,0);printf("KERNWATCH_GUEST_EXIT=%d\n",WEXITSTATUS(status));fflush(stdout);sync();reboot(RB_POWER_OFF);return 0;}
