//! Bounded subprocesses for optional, read-only enrichment. Never executes a shell.
use std::{
    io::{self, Read},
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub struct Output {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}
pub fn run(program: &str, args: &[&str], timeout: Duration, limit: usize) -> io::Result<Output> {
    // At most two optional helpers at once. Waiting happens only on metadata
    // workers, never on the acquisition/UI thread; lifecycle handling is intact.
    static ACTIVE: std::sync::Mutex<usize> = std::sync::Mutex::new(0);
    static READY: std::sync::Condvar = std::sync::Condvar::new();
    struct Permit;
    impl Drop for Permit {
        fn drop(&mut self) {
            *ACTIVE.lock().unwrap() -= 1;
            READY.notify_one();
        }
    }
    let mut active = ACTIVE.lock().unwrap();
    while *active >= 2 {
        active = READY.wait(active).unwrap();
    }
    *active += 1;
    drop(active);
    let _permit = Permit;
    let _cost = crate::cpu_cost::scope("helper.parent");
    crate::cpu_cost::counter("helper_launch_attempts", 1);
    let owner = std::process::id() as i32;
    let mut command = Command::new(program);
    // SAFETY: pre-exec uses only async-signal-safe Linux syscalls; no allocation or locks.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() != owner {
                libc::_exit(125);
            }
            Ok(())
        });
    }
    let mut child = command
        .args(args)
        .env("LC_ALL", "C")
        .env("SYSTEMD_PAGER", "cat")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    for fd in [stdout.as_raw_fd(), stderr.as_raw_fd()] {
        let result = unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags < 0 {
                -1
            } else {
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK)
            }
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    }
    let started = Instant::now();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut status = None;
    let mut out_done = false;
    let mut err_done = false;
    let result = (|| {
        loop {
            for (stream, data, done) in [
                (&mut stdout as &mut dyn Read, &mut out, &mut out_done),
                (&mut stderr as &mut dyn Read, &mut err, &mut err_done),
            ] {
                let mut bytes = [0; 8192];
                loop {
                    match stream.read(&mut bytes) {
                        Ok(0) => {
                            *done = true;
                            break;
                        }
                        Ok(n) => {
                            if data.len() + n > limit {
                                return Err(io::Error::other("enrichment output limit exceeded"));
                            }
                            data.extend_from_slice(&bytes[..n]);
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(e) => return Err(e),
                    }
                }
            }
            if status.is_none() {
                status = child.try_wait()?;
            }
            if status.is_some() && out_done && err_done {
                break;
            }
            if started.elapsed() >= timeout {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("{program} exceeded {}ms", timeout.as_millis()),
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(Output {
            status: status.unwrap().code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out).into_owned(),
            stderr: String::from_utf8_lossy(&err).trim().into(),
        })
    })();
    if result.is_err() {
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
        let _ = child.kill();
    }
    let _ = child.wait();
    result
}
pub fn text(program: &str, args: &[&str]) -> io::Result<String> {
    let out = run(program, args, Duration::from_millis(900), 1024 * 1024)?;
    if out.status == 0 {
        Ok(out.stdout)
    } else {
        Err(io::Error::other(format!(
            "{program} exit {}: {}",
            out.status, out.stderr
        )))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeout_and_output_caps_reap_children() {
        assert_eq!(
            run("sleep", &["2"], Duration::from_millis(20), 1024)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(run("yes", &["bounded"], Duration::from_secs(1), 1024).is_err());
    }
    #[test]
    fn arguments_are_literal() {
        let o = text("printf", &["%s", "$(false); literal"]).unwrap();
        assert_eq!(o, "$(false); literal");
    }
}
