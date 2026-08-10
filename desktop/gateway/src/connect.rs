use std::io::{self, Read, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

const CONNECT_DEADLINE: Duration = Duration::from_secs(10);
const CONNECT_SESSION_DEADLINE: Duration = Duration::from_secs(30 * 60);
const CONNECT_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const CONNECT_BYTES_PER_DIRECTION: u64 = 256 * 1024 * 1024;
const MAX_CONNECT_RESOLVERS: usize = 8;
const CONNECT_LOOPBACK_ONLY_ENV: &str = "CSSWITCH_CONNECT_LOOPBACK_ONLY";

struct ResolverBudget {
    active: AtomicUsize,
    limit: usize,
}

impl ResolverBudget {
    const fn new(limit: usize) -> Self {
        Self {
            active: AtomicUsize::new(0),
            limit,
        }
    }

    fn try_acquire(&self) -> Option<ResolverPermit<'_>> {
        self.active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < self.limit).then_some(active + 1)
            })
            .ok()
            .map(|_| ResolverPermit { budget: self })
    }
}

struct ResolverPermit<'a> {
    budget: &'a ResolverBudget,
}

impl Drop for ResolverPermit<'_> {
    fn drop(&mut self) {
        self.budget.active.fetch_sub(1, Ordering::AcqRel);
    }
}

static CONNECT_RESOLVER_BUDGET: ResolverBudget = ResolverBudget::new(MAX_CONNECT_RESOLVERS);

fn write_status(mut stream: TcpStream, code: u16, reason: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {code} {reason}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    );
    let _ = stream.flush();
}

pub fn is_blocked_host(host: &str) -> bool {
    let host = host.trim_matches('.').to_ascii_lowercase();
    host == "anthropic.com"
        || host.ends_with(".anthropic.com")
        || host == "claude.ai"
        || host.ends_with(".claude.ai")
        || host == "claude.com"
        || host.ends_with(".claude.com")
}

fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_matches('.');
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false)
}

fn parse_target(target: &str) -> Result<(String, u16), ()> {
    if let Some(rest) = target.strip_prefix('[') {
        let (host, suffix) = rest.split_once(']').ok_or(())?;
        if host.is_empty() {
            return Err(());
        }
        let port = suffix
            .strip_prefix(':')
            .ok_or(())?
            .parse()
            .map_err(|_| ())?;
        return Ok((host.to_string(), port));
    }
    let (host, port) = target.rsplit_once(':').ok_or(())?;
    if host.is_empty() {
        return Err(());
    }
    Ok((host.to_string(), port.parse().map_err(|_| ())?))
}

fn connect_addrs_until<T, I, N, C>(
    addrs: I,
    deadline: Instant,
    mut now: N,
    mut connect: C,
) -> io::Result<T>
where
    I: IntoIterator<Item = SocketAddr>,
    N: FnMut() -> Instant,
    C: FnMut(&SocketAddr, Duration) -> io::Result<T>,
{
    let mut attempted = false;
    let mut last_error = None;
    for addr in addrs {
        let remaining = deadline
            .checked_duration_since(now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "CONNECT deadline elapsed"))?;
        attempted = true;
        match connect(&addr, remaining) {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
    }
    if !attempted {
        return Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "CONNECT target resolved to no addresses",
        ));
    }
    Err(last_error.expect("attempted address must record an error"))
}

fn connect_upstream(host: &str, port: u16) -> io::Result<TcpStream> {
    let deadline = Instant::now() + CONNECT_DEADLINE;
    if let Ok(address) = host.parse::<IpAddr>() {
        return connect_addrs_until(
            [SocketAddr::new(address, port)],
            deadline,
            Instant::now,
            TcpStream::connect_timeout,
        );
    }

    // The standard resolver has no cancellation API. Bound the client-visible
    // lookup by the same absolute dial deadline and cap any resolver threads
    // that remain inside libc after their caller has timed out.
    let permit = CONNECT_RESOLVER_BUDGET.try_acquire().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::WouldBlock,
            "CONNECT resolver budget is exhausted",
        )
    })?;
    let host = host.to_string();
    let (tx, rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("csswitch-connect-resolver".into())
        .spawn(move || {
            let _permit = permit;
            let result = (host.as_str(), port).to_socket_addrs().and_then(|addrs| {
                connect_addrs_until(addrs, deadline, Instant::now, TcpStream::connect_timeout)
            });
            let _ = tx.send(result);
        })
        .map_err(|error| io::Error::other(error.to_string()))?;
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "CONNECT deadline elapsed"))?;
    match rx.recv_timeout(remaining) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "CONNECT DNS resolution timed out",
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(io::Error::other("CONNECT resolver terminated unexpectedly"))
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ConnectTargetError {
    BadTarget,
    Blocked,
    DialFailed,
}

fn open_target_with<T, C>(
    target: &str,
    loopback_only: bool,
    mut connect: C,
) -> Result<T, ConnectTargetError>
where
    C: FnMut(&str, u16) -> io::Result<T>,
{
    let (host, port) = parse_target(target).map_err(|_| ConnectTargetError::BadTarget)?;
    if is_blocked_host(&host) || (loopback_only && !is_loopback_host(&host)) {
        return Err(ConnectTargetError::Blocked);
    }
    connect(&host, port).map_err(|_| ConnectTargetError::DialFailed)
}

pub fn handle_connect(target: &str, mut client: TcpStream) {
    // Managed Gateway children start from an empty environment. The acceptance
    // build explicitly sets this marker only after validating a loopback fixture
    // upstream, so any CONNECT side-channel is rejected before DNS or dialing.
    let loopback_only = std::env::var_os(CONNECT_LOOPBACK_ONLY_ENV).is_some();
    let upstream = match open_target_with(target, loopback_only, connect_upstream) {
        Ok(stream) => stream,
        Err(ConnectTargetError::BadTarget) => {
            write_status(client, 400, "Bad Request");
            return;
        }
        Err(ConnectTargetError::Blocked) => {
            write_status(client, 401, "Unauthorized");
            return;
        }
        Err(ConnectTargetError::DialFailed) => {
            write_status(client, 502, "Bad Gateway");
            return;
        }
    };
    let _ = client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n");
    let _ = client.flush();

    let mut client_r = match client.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut upstream_w = match upstream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut upstream_r = upstream;
    let mut client_w = client;
    let deadline = Instant::now() + CONNECT_SESSION_DEADLINE;
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancel_upstream = Arc::clone(&cancelled);
    let cancel_client = Arc::clone(&cancelled);

    let to_upstream = thread::spawn(move || {
        let _ = relay_direction(
            &mut client_r,
            &mut upstream_w,
            deadline,
            CONNECT_IDLE_TIMEOUT,
            CONNECT_BYTES_PER_DIRECTION,
            &cancel_upstream,
        )
        .map(|_| upstream_w.shutdown(Shutdown::Write))
        .unwrap_or_else(|_| {
            cancel_upstream.store(true, Ordering::Release);
            let _ = client_r.shutdown(Shutdown::Both);
            upstream_w.shutdown(Shutdown::Both)
        });
    });
    let to_client = thread::spawn(move || {
        let _ = relay_direction(
            &mut upstream_r,
            &mut client_w,
            deadline,
            CONNECT_IDLE_TIMEOUT,
            CONNECT_BYTES_PER_DIRECTION,
            &cancel_client,
        )
        .map(|_| client_w.shutdown(Shutdown::Write))
        .unwrap_or_else(|_| {
            cancel_client.store(true, Ordering::Release);
            let _ = upstream_r.shutdown(Shutdown::Both);
            client_w.shutdown(Shutdown::Both)
        });
    });
    let _ = to_upstream.join();
    let _ = to_client.join();
}

fn relay_direction(
    reader: &mut TcpStream,
    writer: &mut TcpStream,
    deadline: Instant,
    idle_timeout: Duration,
    byte_limit: u64,
    cancelled: &AtomicBool,
) -> io::Result<u64> {
    relay_io_with_policy(
        reader,
        writer,
        deadline,
        byte_limit,
        cancelled,
        |reader, writer, remaining| {
            let operation_timeout = idle_timeout.min(remaining);
            reader.set_read_timeout(Some(operation_timeout))?;
            writer.set_write_timeout(Some(operation_timeout))
        },
    )
}

fn relay_io_with_policy<R, W, C>(
    reader: &mut R,
    writer: &mut W,
    deadline: Instant,
    byte_limit: u64,
    cancelled: &AtomicBool,
    mut configure_operation: C,
) -> io::Result<u64>
where
    R: Read,
    W: Write,
    C: FnMut(&mut R, &mut W, Duration) -> io::Result<()>,
{
    let mut transferred = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "CONNECT peer direction stopped",
            ));
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::TimedOut, "CONNECT session deadline elapsed")
            })?;
        configure_operation(reader, writer, remaining)?;
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(transferred);
        }
        let next = transferred.saturating_add(read as u64);
        if next > byte_limit {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "CONNECT byte budget exceeded",
            ));
        }
        writer.write_all(&buffer[..read])?;
        transferred = next;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        connect_addrs_until, is_blocked_host, is_loopback_host, open_target_with, parse_target,
        relay_io_with_policy, ConnectTargetError, ResolverBudget, CONNECT_DEADLINE,
    };
    use std::io::{self, Cursor};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    #[test]
    fn blocks_anthropic_claude_hosts_only() {
        assert!(is_blocked_host("api.anthropic.com"));
        assert!(is_blocked_host("claude.ai"));
        assert!(is_blocked_host("foo.claude.com"));
        assert!(!is_blocked_host("example.com"));
    }

    #[test]
    fn parses_ipv4_hostname_and_ipv6_targets() {
        assert_eq!(
            parse_target("example.com:443"),
            Ok(("example.com".into(), 443))
        );
        assert_eq!(parse_target("127.0.0.1:80"), Ok(("127.0.0.1".into(), 80)));
        assert_eq!(parse_target("[::1]:8443"), Ok(("::1".into(), 8443)));
        assert!(parse_target("example.com").is_err());
        assert!(parse_target(":443").is_err());
        assert!(parse_target("[]:443").is_err());
        assert!(parse_target("example.com:not-a-port").is_err());
    }

    #[test]
    fn recognizes_only_literal_or_named_loopback_hosts() {
        for host in ["localhost", "LOCALHOST.", "127.0.0.1", "127.23.4.5", "::1"] {
            assert!(is_loopback_host(host), "{host} must be loopback");
        }
        for host in ["example.test", "0.0.0.0", "::", "198.18.0.54"] {
            assert!(!is_loopback_host(host), "{host} must not be loopback");
        }
    }

    #[test]
    fn blocked_and_bad_targets_never_reach_the_dialer() {
        for (target, expected) in [
            ("api.anthropic.com:443", ConnectTargetError::Blocked),
            ("example.com", ConnectTargetError::BadTarget),
        ] {
            let mut dialed = false;
            let result: Result<(), _> = open_target_with(target, false, |_host, _port| {
                dialed = true;
                Ok(())
            });
            assert_eq!(result.unwrap_err(), expected);
            assert!(!dialed, "{target} unexpectedly reached the dialer");
        }
    }

    #[test]
    fn loopback_only_rejects_before_dns_or_dial() {
        let mut dialed = false;
        let denied: Result<(), _> = open_target_with("example.test:443", true, |_host, _port| {
            dialed = true;
            Ok(())
        });
        assert_eq!(denied.unwrap_err(), ConnectTargetError::Blocked);
        assert!(!dialed);

        let allowed = open_target_with("127.0.0.1:443", true, |host, port| {
            assert_eq!(host, "127.0.0.1");
            assert_eq!(port, 443);
            Ok(())
        });
        assert_eq!(allowed, Ok(()));
    }

    #[test]
    fn all_addresses_share_one_connect_budget() {
        let start = Instant::now();
        let deadline = start + CONNECT_DEADLINE;
        let addrs = [
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 2),
        ];
        let mut times = [start, start + Duration::from_secs(6)].into_iter();
        let mut observed = Vec::new();

        let result: io::Result<()> = connect_addrs_until(
            addrs,
            deadline,
            || times.next().expect("one clock reading per address"),
            |addr, timeout| {
                observed.push((*addr, timeout));
                Err(io::Error::new(io::ErrorKind::ConnectionRefused, "test"))
            },
        );

        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::ConnectionRefused);
        assert_eq!(observed[0].1, Duration::from_secs(10));
        assert_eq!(observed[1].1, Duration::from_secs(4));
    }

    #[test]
    fn elapsed_deadline_never_attempts_a_dial() {
        let now = Instant::now();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1);
        let mut dialed = false;

        let result: io::Result<()> = connect_addrs_until(
            [addr],
            now,
            || now,
            |_addr, _| {
                dialed = true;
                Ok(())
            },
        );

        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert!(!dialed);
    }

    #[test]
    fn empty_resolution_is_an_address_error() {
        let result: io::Result<()> = connect_addrs_until(
            [],
            Instant::now() + CONNECT_DEADLINE,
            Instant::now,
            |_addr, _| Ok(()),
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::AddrNotAvailable);
    }

    #[test]
    fn relay_direction_stops_at_the_absolute_session_deadline() {
        let mut reader = Cursor::new(b"x");
        let mut writer = Vec::new();
        let cancelled = AtomicBool::new(false);
        let error = relay_io_with_policy(
            &mut reader,
            &mut writer,
            Instant::now(),
            1,
            &cancelled,
            |_, _, _| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn relay_direction_enforces_its_byte_budget() {
        let mut reader = Cursor::new(b"too large");
        let mut writer = Vec::new();
        let error = relay_io_with_policy(
            &mut reader,
            &mut writer,
            Instant::now() + Duration::from_secs(1),
            3,
            &AtomicBool::new(false),
            |_, _, _| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::FileTooLarge);
    }

    #[test]
    fn resolver_threads_have_a_fixed_nonblocking_budget() {
        let budget = ResolverBudget::new(2);
        let first = budget.try_acquire().unwrap();
        let second = budget.try_acquire().unwrap();
        assert!(budget.try_acquire().is_none());
        drop(first);
        let replacement = budget.try_acquire().unwrap();
        drop(second);
        drop(replacement);
        assert_eq!(budget.active.load(std::sync::atomic::Ordering::Acquire), 0);
    }
}
