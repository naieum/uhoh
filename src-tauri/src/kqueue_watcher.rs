use tokio::sync::mpsc;

/// Process events shared across platforms.
/// On macOS, these come from kqueue. On other platforms, the coordinator
/// relies on periodic polling instead.
#[derive(Debug, Clone)]
pub enum ProcessEvent {
    Exited { pid: u32 },
}

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::os::unix::io::RawFd;

    pub enum KqueueCommand {
        Watch { pid: u32 },
    }

    /// kqueue-based process watcher for instant crash detection.
    /// Runs on a dedicated OS thread (not tokio) since kevent() is blocking.
    /// Uses EVFILT_PROC + NOTE_EXIT + EV_ONESHOT for per-PID exit monitoring.
    pub struct KqueueWatcher {
        cmd_tx: std::sync::mpsc::Sender<KqueueCommand>,
        pipe_write_fd: RawFd,
    }

    impl KqueueWatcher {
        /// Create a new kqueue watcher. Spawns a background OS thread.
        /// Returns the watcher handle and a receiver for process events.
        pub fn new() -> (Self, mpsc::UnboundedReceiver<ProcessEvent>) {
            let (event_tx, event_rx) = mpsc::unbounded_channel();
            let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();

            let mut pipe_fds = [0 as RawFd; 2];
            unsafe {
                libc::pipe(pipe_fds.as_mut_ptr());
                let flags = libc::fcntl(pipe_fds[0], libc::F_GETFL);
                libc::fcntl(pipe_fds[0], libc::F_SETFL, flags | libc::O_NONBLOCK);
            }

            let pipe_read_fd = pipe_fds[0];
            let pipe_write_fd = pipe_fds[1];

            std::thread::spawn(move || {
                run_kqueue_loop(pipe_read_fd, cmd_rx, event_tx);
            });

            let watcher = KqueueWatcher { cmd_tx, pipe_write_fd };
            (watcher, event_rx)
        }

        /// Register a PID to watch for exit. Non-blocking.
        pub fn watch_pid(&self, pid: u32) {
            let _ = self.cmd_tx.send(KqueueCommand::Watch { pid });
            unsafe {
                let byte: u8 = 1;
                libc::write(self.pipe_write_fd, &byte as *const u8 as *const libc::c_void, 1);
            }
        }
    }

    impl Drop for KqueueWatcher {
        fn drop(&mut self) {
            unsafe { libc::close(self.pipe_write_fd); }
        }
    }

    fn run_kqueue_loop(
        pipe_read_fd: RawFd,
        cmd_rx: std::sync::mpsc::Receiver<KqueueCommand>,
        event_tx: mpsc::UnboundedSender<ProcessEvent>,
    ) {
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            eprintln!("uhoh: failed to create kqueue: {}", std::io::Error::last_os_error());
            return;
        }

        let pipe_event = libc::kevent {
            ident: pipe_read_fd as libc::uintptr_t,
            filter: libc::EVFILT_READ,
            flags: libc::EV_ADD,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        unsafe {
            libc::kevent(kq, &pipe_event, 1, std::ptr::null_mut(), 0, std::ptr::null());
        }

        let mut events: [libc::kevent; 32] = unsafe { std::mem::zeroed() };

        loop {
            let n = unsafe {
                libc::kevent(
                    kq, std::ptr::null(), 0,
                    events.as_mut_ptr(), events.len() as libc::c_int,
                    std::ptr::null(),
                )
            };

            if n < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted { continue; }
                eprintln!("uhoh: kqueue error: {}", err);
                break;
            }

            for i in 0..n as usize {
                let ev = &events[i];

                if ev.filter == libc::EVFILT_READ && ev.ident == pipe_read_fd as libc::uintptr_t {
                    let mut buf = [0u8; 64];
                    unsafe {
                        libc::read(pipe_read_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len());
                    }
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        match cmd {
                            KqueueCommand::Watch { pid } => {
                                let proc_event = libc::kevent {
                                    ident: pid as libc::uintptr_t,
                                    filter: libc::EVFILT_PROC,
                                    flags: libc::EV_ADD | libc::EV_ONESHOT,
                                    fflags: libc::NOTE_EXIT,
                                    data: 0,
                                    udata: std::ptr::null_mut(),
                                };
                                let ret = unsafe {
                                    libc::kevent(kq, &proc_event, 1, std::ptr::null_mut(), 0, std::ptr::null())
                                };
                                if ret < 0 {
                                    let err = std::io::Error::last_os_error();
                                    if err.raw_os_error() == Some(libc::ESRCH) {
                                        let _ = event_tx.send(ProcessEvent::Exited { pid });
                                    }
                                }
                            }
                        }
                    }
                } else if ev.filter == libc::EVFILT_PROC {
                    let pid = ev.ident as u32;
                    if ev.fflags & (libc::NOTE_EXIT as u32) != 0 {
                        let _ = event_tx.send(ProcessEvent::Exited { pid });
                    }
                }
            }
        }

        unsafe {
            libc::close(kq);
            libc::close(pipe_read_fd);
        }
    }
}
