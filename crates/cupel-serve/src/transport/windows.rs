// ABOUTME: The daemon's transport on Windows (ADR-137): a named pipe named for the endpoint path, behind an endpoint
// ABOUTME: file at that path holding the pipe's name and this daemon's own line; instances accepted by polling.
#![allow(
    unsafe_code,
    reason = "the Win32 named-pipe functions std does not wrap; each call's safety is stated beside it"
)]

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use windows_sys::Win32::Foundation::{
    ERROR_NO_DATA, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_NOWAIT, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT, PeekNamedPipe,
    SetNamedPipeHandleState,
};

use super::pipe_name;

/// Each instance's input and output buffer size, in bytes.
const BUFFER: u32 = 64 * 1024;

/// How often a listening instance is asked for a client, and a busy pipe retried.
const POLL: Duration = Duration::from_millis(5);

/// How often a connection's peer is checked for having hung up.
const HANG_UP_POLL: Duration = Duration::from_millis(50);

/// How long a client retries while every instance of the pipe is busy.
const BUSY_WAIT: Duration = Duration::from_secs(5);

/// A daemon's listening endpoint: the pipe and its endpoint file.
pub struct Listener {
    /// The pipe's name, NUL-terminated UTF-16.
    pipe: Vec<u16>,
    /// The instance waiting for the next client, in nonblocking mode.
    waiting: OwnedHandle,
    path: PathBuf,
    /// What this listener wrote to the endpoint file.
    identity: String,
}

impl Listener {
    /// Creates the pipe `path` names, failing if another process already has it, then writes
    /// the endpoint file at `path`, where no file may be.
    pub fn bind(path: &Path) -> io::Result<Self> {
        let name = pipe_name(path);
        let pipe: Vec<u16> = name.encode_utf16().chain([0]).collect();
        let waiting = instance(&pipe, true)?;
        let started = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let identity = format!("{name}\n{} {started}\n", std::process::id());
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?
            .write_all(identity.as_bytes())?;
        Ok(Self {
            pipe,
            waiting,
            path: path.to_path_buf(),
            identity,
        })
    }

    /// The next connection, after waiting up to `wait` for one. A connected instance is put in
    /// blocking mode, and a new instance takes its place before it is handed over.
    pub fn accept(&mut self, wait: Duration) -> Option<Connection> {
        let deadline = Instant::now() + wait;
        while !connected(&self.waiting) {
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(POLL);
        }
        let next = instance(&self.pipe, false).ok()?;
        let connection = std::mem::replace(&mut self.waiting, next);
        blocking(&connection).ok()?;
        Some(Connection(File::from(connection)))
    }

    /// Whether the endpoint file still holds what this listener wrote.
    pub fn is_current(&self) -> bool {
        std::fs::read_to_string(&self.path).is_ok_and(|content| content == self.identity)
    }
}

/// A new instance of `pipe` in nonblocking mode, accepting local clients only; `first` fails
/// unless no instance of it exists.
fn instance(pipe: &[u16], first: bool) -> io::Result<OwnedHandle> {
    let open_mode = if first {
        PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE
    } else {
        PIPE_ACCESS_DUPLEX
    };
    let pipe_mode = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS;
    // SAFETY: `pipe` is NUL-terminated UTF-16 and outlives the call; a null security-attributes
    // pointer selects the default security descriptor.
    let handle = unsafe {
        CreateNamedPipeW(
            pipe.as_ptr(),
            open_mode,
            pipe_mode,
            PIPE_UNLIMITED_INSTANCES,
            BUFFER,
            BUFFER,
            0,
            std::ptr::null(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `handle` is a valid handle nothing else owns.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle as RawHandle) })
}

/// Whether a client has connected to the nonblocking `instance` (and perhaps already left,
/// which its first read then reports).
fn connected(instance: &OwnedHandle) -> bool {
    // SAFETY: `instance` is a valid pipe handle; with no OVERLAPPED the call is synchronous, and
    // a nonblocking instance answers at once.
    let listening = unsafe { ConnectNamedPipe(instance.as_raw_handle(), std::ptr::null_mut()) };
    listening == 0
        && matches!(
            os_error(&io::Error::last_os_error()),
            Some(ERROR_PIPE_CONNECTED | ERROR_NO_DATA)
        )
}

/// Puts the connected `instance` in blocking mode, as a connection reads and writes.
fn blocking(instance: &OwnedHandle) -> io::Result<()> {
    let mode = PIPE_READMODE_BYTE | PIPE_WAIT;
    // SAFETY: `instance` is a valid pipe handle and `mode` outlives the call; the null pointers
    // leave the collection settings, which apply only to remote clients, as they are.
    let set = unsafe {
        SetNamedPipeHandleState(
            instance.as_raw_handle(),
            &raw const mode,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if set == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn os_error(error: &io::Error) -> Option<u32> {
    error
        .raw_os_error()
        .and_then(|code| u32::try_from(code).ok())
}

/// One connection between a client and the daemon: an end of a pipe instance.
pub struct Connection(File);

impl Connection {
    /// Connects to the pipe the endpoint file at `path` names, retrying while every instance is
    /// busy. With no daemon behind the file, the error is "not found".
    pub fn connect(path: &Path) -> io::Result<Self> {
        let pipe = pipe_name(path);
        let deadline = Instant::now() + BUSY_WAIT;
        loop {
            match OpenOptions::new().read(true).write(true).open(&pipe) {
                Ok(file) => return Ok(Self(file)),
                Err(error)
                    if os_error(&error) == Some(ERROR_PIPE_BUSY) && Instant::now() < deadline =>
                {
                    thread::sleep(POLL);
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }

    /// Returns once the peer has closed its end, discarding whatever it still sends. It peeks
    /// rather than blocking in a read: Windows serializes the operations on a synchronous
    /// handle, so a blocked read would hold back the writes of the run on the same connection.
    pub fn wait_for_hang_up(self) {
        let mut discard = vec![0_u8; BUFFER as usize];
        loop {
            let mut available = 0_u32;
            // SAFETY: the handle is valid for `self`'s lifetime; a null buffer of size 0 asks
            // only for the count of bytes waiting, written to `available`.
            let peeked = unsafe {
                PeekNamedPipe(
                    self.0.as_raw_handle(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &raw mut available,
                    std::ptr::null_mut(),
                )
            };
            if peeked == 0 {
                return;
            }
            if available == 0 {
                thread::sleep(HANG_UP_POLL);
                continue;
            }
            let waiting =
                usize::try_from(available).map_or(discard.len(), |n| n.min(discard.len()));
            if !matches!((&self.0).read(&mut discard[..waiting]), Ok(read) if read > 0) {
                return;
            }
        }
    }
}

impl Read for &Connection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.0).read(buf)
    }
}

impl Write for &Connection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&self.0).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.0).flush()
    }
}

impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self).read(buf)
    }
}

impl Write for Connection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&*self).flush()
    }
}
