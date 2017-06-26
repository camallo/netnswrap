#[macro_use]
extern crate clap;
#[macro_use]
extern crate error_chain;
extern crate libc;
extern crate nix;
extern crate void;

use clap::{Arg, App};
use nix::fcntl;
use std::{env, ffi, fs, io, net};

use error_chain::ChainedError;
use std::os::unix::io::AsRawFd;

error_chain!{
    foreign_links {
        Addr(net::AddrParseError);
        Cli(clap::Error);
        Ffi(ffi::NulError);
        Io(io::Error);
        Unix(nix::Error);
    }
}

fn main() {
    match run() {
        Err(e) => {
            print!("{}", e.display());
            std::process::exit(253);
        }
        Ok(_) => unreachable!(),
    }
}

fn run() -> Result<void::Void> {
    // Parse command-line arguments
    let matches = build_cli().get_matches_safe()?;
    let address = value_t!(matches, "address", net::IpAddr)?;
    let target_ns = value_t!(matches, "target-ns", String)?;
    let protocol = value_t!(matches, "protocol", String)?;
    let port = value_t!(matches, "port", u16)?;
    let cmdline = values_t!(matches, "cmd", String)?;

    // Open socket in current net-ns
    let addr = net::SocketAddr::new(address, port);
    let socket = match protocol.as_str() {
        "udp" => unimplemented!(), // TODO
        "tcp" => net::TcpListener::bind(addr)?,
        x => bail!("unknown protocol ${:?}", x),
    };

    // Unset O_CLOEXEC from socket fd
    fcntl::fcntl(
        socket.as_raw_fd(),
        fcntl::FcntlArg::F_SETFD(fcntl::FdFlag::empty()),
    )?;

    // Set environment flags, see
    // https://www.freedesktop.org/software/systemd/man/sd_listen_fds.html#
    let pid = nix::unistd::getpid();
    env::set_var("LISTEN_PID", pid.to_string());
    env::set_var("LISTEN_FDS", "1");
    env::set_var("LISTEN_FDNAMES", "netnswrap");

    // Move to target net-ns
    {
        let fp = fs::File::open(&target_ns).chain_err(|| {
            format!("unable to open {}", target_ns)
        })?;
        nix::sched::setns(fp.as_raw_fd(), nix::sched::CLONE_NEWNET)
            .chain_err(|| "setns CLONE_NEWNET failed")?;
    }

    // Exec the real binary
    let cmdpath = ffi::CString::new(cmdline[0].clone())?;
    let cmdargs: Vec<ffi::CString> = cmdline
        .iter()
        .filter_map(|s| ffi::CString::new(s.clone()).ok())
        .collect();
    let r = nix::unistd::execvp(&cmdpath, &cmdargs).chain_err(|| {
        format!("failed to exec {:?}", &cmdline)
    })?;
    Ok(r)
}

fn build_cli<'a, 'b>() -> clap::App<'a, 'b> {
    App::new("Net-ns wrapper")
        .arg(
            Arg::with_name("target-ns")
                .default_value("/target-netns")
                .short("t")
                .long("target-ns")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("address")
                .takes_value(true)
                .default_value("0.0.0.0"),
        )
        .arg(
            Arg::with_name("protocol")
                .short("p")
                .long("protocol")
                .takes_value(true)
                .default_value("tcp"),
        )
        .arg(
            Arg::with_name("port")
                .short("n")
                .long("port")
                .takes_value(true)
                .default_value("9100"),
        )
        .arg(
            Arg::with_name("cmd")
                .index(1)
                .last(true)
                .required(true)
                .multiple(true)
                .allow_hyphen_values(true),
        )
}
