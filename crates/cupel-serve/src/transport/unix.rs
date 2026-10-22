// ABOUTME: The daemon's transport on Unix (ADR-085, ADR-137): a Unix-domain socket at the endpoint path, only its
// ABOUTME: owner may connect, its identity the socket file's inode, accepted by `poll` with a bounded wait.
use std::io::{self, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

/// A daemon's listening endpoint.
pub struct Listener {
    socket: UnixListener,
    path: PathBuf,
    /// The socket file's inode, which tells this listener's socket from a replacement.
    identity: Option<u64>,
}

impl Listener {
    /// Listens at `path`, where no file may be, letting only its owner connect.
    pub fn bind(path: &Path) -> io::Result<Self> {
        let socket = UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            socket,
            path: path.to_path_buf(),
            identity: inode(path),
        })
    }

    /// The next connection, after waiting up to `wait` for one.
    pub fn accept(&mut self, wait: Duration) -> Option<Connection> {
        let wait = PollTimeout::try_from(wait).unwrap_or(PollTimeout::MAX);
        let mut fds = [PollFd::new(self.socket.as_fd(), PollFlags::POLLIN)];
        if !matches!(poll(&mut fds, wait), Ok(ready) if ready > 0) {
            return None;
        }
        self.socket
            .accept()
            .ok()
            .map(|(stream, _)| Connection(stream))
    }

    /// Whether the file at the endpoint path is still this listener's socket.
    pub fn is_current(&self) -> bool {
        self.identity.is_some() && inode(&self.path) == self.identity
    }
}

fn inode(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|metadata| metadata.ino())
}

/// One connection between a client and the daemon.
pub struct Connection(UnixStream);

impl Connection {
    /// Connects to the daemon listening at `path`.
    pub fn connect(path: &Path) -> io::Result<Self> {
        UnixStream::connect(path).map(Self)
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }

    /// Returns once the peer has closed its end, discarding whatever it still sends.
    pub fn wait_for_hang_up(mut self) {
        let mut byte = [0_u8; 1];
        while matches!(self.0.read(&mut byte), Ok(1)) {}
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
