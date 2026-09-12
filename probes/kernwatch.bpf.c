/* kernwatch CO-RE raw tracepoints. SPDX-License-Identifier: MIT
 * Build: clang -target bpf -O2 -g -c kernwatch.bpf.c -o kernwatch.bpf.o
 * No generated vmlinux.h required: minimal CO-RE type declarations below.
 */
typedef unsigned long long u64;
typedef unsigned int u32;
#define SEC(name) __attribute__((section(name), used))
#define __uint(name, val) int (*name)[val]
#define __type(name, val) val *name
struct { __uint(type,27); __uint(max_entries,16777216); } events SEC(".maps");
struct { __uint(type,6); __uint(max_entries,1); __type(key,u32); __type(value,u64); } losses SEC(".maps");
struct { __uint(type,7); __uint(max_entries,8192); __type(key,u32); __uint(value_size,1016); } stacks SEC(".maps");
struct profile_key { u64 start,exec; u32 tgid,tid; int user,kernel; };
struct { __uint(type,5); __uint(max_entries,8192); __type(key,struct profile_key); __type(value,u64); } profile_counts SEC(".maps");
struct { __uint(type,6); __uint(max_entries,2); __type(key,u32); __type(value,u64); } profile_stats SEC(".maps");
struct { __uint(type,1); __uint(max_entries,8192); __type(key,u32); __type(value,u64); } profile_execs SEC(".maps");
volatile const u64 target_start_ticks=0;
volatile const u64 target_clock_hz=100;
volatile const u32 target_tgid=0;
volatile const u32 sample_tid=0;
volatile const u32 capture_stack=0;
volatile const u32 target_pid=0;
volatile const u64 target_cgroup=0;
struct kernfs_node { u64 id; } __attribute__((preserve_access_index));
struct cgroup { struct kernfs_node *kn; } __attribute__((preserve_access_index));
struct css_set { struct cgroup *dfl_cgrp; } __attribute__((preserve_access_index));
struct task_struct { int pid; u64 start_boottime; u64 self_exec_id; struct task_struct *group_leader; struct css_set *cgroups; } __attribute__((preserve_access_index));
struct gendisk { int major; int first_minor; } __attribute__((preserve_access_index));
struct request_queue { struct gendisk *disk; } __attribute__((preserve_access_index));
struct request { unsigned int __data_len; struct request_queue *q; } __attribute__((preserve_access_index));
#ifdef KERNWATCH_ARM64
struct pt_regs { unsigned long regs[31]; } __attribute__((preserve_access_index));
#define OPENAT_NR 56
#define FSTATAT_NR 79
#else
struct pt_regs { unsigned long di,si,dx,r10,r8,r9; } __attribute__((preserve_access_index));
#define OPENAT_NR 257
#define FSTATAT_NR 262
#endif
struct ctx { u64 args[0]; };
struct event { u64 ns,key,a,b; u32 type,cpu,pid,pad; u64 args[6]; char text[80]; };
static long (*update)(void*,const void*,const void*,u64)=(void*)2;
static void *(*current_task)(void)=(void*)35;
static u64 (*ktime)(void)=(void*)5;
static u64 (*pidtgid)(void)=(void*)14;
static u32 (*cpu_id)(void)=(void*)8;
static u64 (*cgroup_id)(void)=(void*)80;
static long (*stack_id)(void*,void*,u64)=(void*)27;
static void *(*lookup)(void*,const void*)=(void*)1;
static long (*read_kernel)(void*,u32,const void*)=(void*)113;
static long (*read_user_string)(void*,u32,const void*)=(void*)114;
static void *(*reserve)(void*,u64,u64)=(void*)131;
static void (*submit)(void*,u64)=(void*)132;
static __attribute__((always_inline)) u64 task_group(u64 ptr){u64 css=0,cg=0,kn=0,id=0;read_kernel(&css,8,&((struct task_struct*)ptr)->cgroups);read_kernel(&cg,8,&((struct css_set*)css)->dfl_cgrp);read_kernel(&kn,8,&((struct cgroup*)cg)->kn);read_kernel(&id,8,&((struct kernfs_node*)kn)->id);return id;}
static __attribute__((always_inline)) int task_pid(u64 ptr){int pid=0;read_kernel(&pid,sizeof(pid),&((struct task_struct*)ptr)->pid);return pid;}
static __attribute__((always_inline)) struct event *begin(u32 type,u64 key,u64 a,u64 b){if(target_pid){if((type==4||type==5)&&((u32)pidtgid()!=target_pid))return 0;if((type==1||type==3||type==11)&&key!=target_pid)return 0;if(type==2&&key!=target_pid&&a!=target_pid)return 0;}if(target_cgroup&&(type==4||type==5)&&cgroup_id()!=target_cgroup)return 0;struct event *e=reserve(&events,sizeof(*e),0);if(!e){u32 k=0;u64 *n=lookup(&losses,&k);if(n)(*n)++;return 0;}__builtin_memset(e,0,sizeof(*e));e->ns=ktime();e->type=type;e->cpu=cpu_id();e->pid=(u32)pidtgid();e->key=key;e->a=a;e->b=b;return e;}
static __attribute__((always_inline)) int emit(u32 type,u64 key,u64 a,u64 b){struct event *e=begin(type,key,a,b);if(e)submit(e,0);return 0;}
SEC("raw_tp/sched_wakeup") int kw_wake(struct ctx *c){return emit(1,task_pid(c->args[0]),0,0);}
SEC("raw_tp/sched_wakeup_new") int kw_wake_new(struct ctx *c){return emit(1,task_pid(c->args[0]),0,0);}
SEC("raw_tp/sched_switch") int kw_switch(struct ctx *c){struct event *e=begin(2,task_pid(c->args[2]),task_pid(c->args[1]),c->args[0] || c->args[3] == 0);if(!e)return 0;u64 css=0,cg=0,kn=0;read_kernel(&css,8,&((struct task_struct*)c->args[2])->cgroups);read_kernel(&cg,8,&((struct css_set*)css)->dfl_cgrp);read_kernel(&kn,8,&((struct cgroup*)cg)->kn);read_kernel(&e->args[0],8,&((struct kernfs_node*)kn)->id);read_kernel(&e->args[1],8,&((struct task_struct*)c->args[2])->start_boottime);submit(e,0);return 0;}
SEC("raw_tp/sched_process_exit") int kw_exit(struct ctx *c){return emit(3,task_pid(c->args[0]),0,0);}
SEC("raw_tp/sys_enter") int kw_sys_enter(struct ctx *c){struct event *e=begin(4,c->args[1],0,0);if(!e)return 0;e->pad=capture_stack?(u32)stack_id(c,&stacks,256):0xffffffff;struct pt_regs *r=(void*)c->args[0];
#ifdef KERNWATCH_ARM64
#pragma unroll
for(int i=0;i<6;i++)read_kernel(&e->args[i],8,&r->regs[i]);
#else
read_kernel(&e->args[0],8,&r->di);read_kernel(&e->args[1],8,&r->si);read_kernel(&e->args[2],8,&r->dx);read_kernel(&e->args[3],8,&r->r10);read_kernel(&e->args[4],8,&r->r8);read_kernel(&e->args[5],8,&r->r9);
#endif
if(c->args[1]==OPENAT_NR||c->args[1]==FSTATAT_NR)read_user_string(e->text,80,(void*)e->args[1]);submit(e,0);return 0;}
SEC("raw_tp/sys_exit") int kw_sys_exit(struct ctx *c){return emit(5,0,c->args[1],0);}
SEC("raw_tp/softirq_entry") int kw_soft_enter(struct ctx *c){return emit(6,c->args[0],0,0);}
SEC("raw_tp/softirq_exit") int kw_soft_exit(struct ctx *c){return emit(7,c->args[0],0,0);}
SEC("raw_tp/irq_handler_entry") int kw_irq_enter(struct ctx *c){return emit(6,c->args[0],1,0);}
SEC("raw_tp/irq_handler_exit") int kw_irq_exit(struct ctx *c){return emit(7,c->args[0],1,0);}
SEC("raw_tp/block_rq_issue") int kw_block_issue(struct ctx *c){u32 bytes=0;read_kernel(&bytes,4,&((struct request*)c->args[0])->__data_len);u64 q=0,disk=0;u32 major=0,minor=0;read_kernel(&q,8,&((struct request*)c->args[0])->q);read_kernel(&disk,8,&((struct request_queue*)q)->disk);read_kernel(&major,4,&((struct gendisk*)disk)->major);read_kernel(&minor,4,&((struct gendisk*)disk)->first_minor);return emit(8,c->args[0],bytes,((u64)major<<32)|minor);}
SEC("raw_tp/block_rq_complete") int kw_block_done(struct ctx *c){return emit(9,c->args[0],c->args[2],c->args[1]);}
SEC("raw_tp/block_rq_requeue") int kw_block_requeue(struct ctx *c){return emit(10,c->args[0],0,0);}
SEC("raw_tp/sched_migrate_task") int kw_migrate(struct ctx *c){return emit(11,task_pid(c->args[0]),c->args[1],task_group(c->args[0]));}
char kw_license[] SEC("license")="GPL";

/* CPU observations, independent of syscall activity. Counts are per CPU;
 * no active map deletion and no stack ID reuse during a capture. */
SEC("perf_event") int kw_profile(void *ctx) {
    u64 id=pidtgid(); u32 tgid=id>>32, tid=(u32)id;
    if (!tid || (target_tgid && tgid!=target_tgid) || (sample_tid && tid!=sample_tid)) return 0;
    struct task_struct *task=current_task(), *identity=task;
    if(target_tgid) read_kernel(&identity,8,&task->group_leader);
    if(target_start_ticks) {
        u64 start=0; read_kernel(&start,8,&identity->start_boottime);
        u64 ticks=(start/1000000000)*target_clock_hz+(start%1000000000)*target_clock_hz/1000000000;
        if(ticks!=target_start_ticks) return 0;
    }
    u32 zero=0, one=1; u64 *attempts=lookup(&profile_stats,&zero);
    if(attempts) (*attempts)++;
    struct profile_key key={.tgid=tgid,.tid=tid};
    struct task_struct *leader=task; read_kernel(&leader,8,&task->group_leader);
    read_kernel(&key.start,8,&leader->start_boottime);
    read_kernel(&key.exec,8,&task->self_exec_id);
    update(&profile_execs,&tgid,&key.exec,0);
    key.user=stack_id(ctx,&stacks,256);
    key.kernel=stack_id(ctx,&stacks,0);
    u64 *count=lookup(&profile_counts,&key);
    if(count) { (*count)++; return 0; }
    u64 initial=1;
    if(update(&profile_counts,&key,&initial,1)) {
        count=lookup(&profile_counts,&key);
        if(count) (*count)++;
        else { u64 *lost=lookup(&profile_stats,&one); if(lost) (*lost)++; }
    }
    return 0;
}

/* Invalidate symbolization of older samples even if exec is followed by no CPU
 * sample before the next userspace poll. */
SEC("raw_tp/sched_process_exec") int kw_profile_exec(struct ctx *ctx) {
    (void)ctx;
    u32 tgid=pidtgid()>>32;
    if(target_tgid && tgid!=target_tgid) return 0;
    struct task_struct *task=current_task();u64 generation=0;
    read_kernel(&generation,8,&task->self_exec_id);
    update(&profile_execs,&tgid,&generation,0);
    return 0;
}
