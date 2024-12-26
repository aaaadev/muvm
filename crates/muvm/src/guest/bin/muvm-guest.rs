use std::os::fd::AsFd;
use std::process::Command;
use std::{cmp, env};

use anyhow::{Context, Result};
use muvm::env::find_muvm_exec;
use muvm::guest::cli_options::options;
use muvm::guest::fex::setup_fex;
use muvm::guest::mount::mount_filesystems;
use muvm::guest::net::configure_network;
use muvm::guest::server::server_main;
use muvm::guest::socket::setup_socket_proxy;
use muvm::guest::user::setup_user;
use muvm::guest::x11::setup_x11_forwarding;
use muvm::guest::x11bridge::start_x11bridge;
use rustix::process::{getrlimit, setrlimit, Resource};

fn main() -> Result<()> {
    env_logger::init();

    if let Ok(val) = env::var("__X11BRIDGE_DEBUG") {
        start_x11bridge(val.parse()?);
        return Ok(());
    }

    let options = options().run();

    {
        const ESYNC_RLIMIT_NOFILE: u64 = 524288;
        // Raise RLIMIT_NOFILE. This is required for wine's esync to work.
        // See https://github.com/lutris/docs/blob/master/HowToEsync.md
        // See https://github.com/zfigura/wine/blob/esync/README.esync
        let mut rlim = getrlimit(Resource::Nofile);
        rlim.maximum = if let Some(maximum) = rlim.maximum {
            Some(cmp::max(maximum, ESYNC_RLIMIT_NOFILE))
        } else {
            Some(ESYNC_RLIMIT_NOFILE)
        };
        rlim.current = rlim.maximum;
        setrlimit(Resource::Nofile, rlim).context("Failed to raise `RLIMIT_NOFILE`")?;
    }

    if let Err(err) = mount_filesystems() {
        return Err(err).context("Failed to mount filesystems, bailing out");
    }

    // Use the correct TTY, which fixes pty issues etc. (/dev/console cannot be a controlling tty)
    let console = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .open("/dev/hvc0")?;
    rustix::stdio::dup2_stdin(console.as_fd())?;
    rustix::stdio::dup2_stdout(console.as_fd())?;
    rustix::stdio::dup2_stderr(console.as_fd())?;

    Command::new("/usr/lib/systemd/systemd-udevd").spawn()?;

    setup_fex()?;

    configure_network()?;

    let muvm_hidpipe_path = find_muvm_exec("muvm-hidpipe")?;
    Command::new(muvm_hidpipe_path)
        .arg(format!("{}", options.uid))
        .spawn()
        .context("Failed to execute `muvm-hidpipe` as child process")?;

    let run_path = match setup_user(options.username, options.uid, options.gid) {
        Ok(p) => p,
        Err(err) => return Err(err).context("Failed to set up user, bailing out"),
    };

    let pulse_path = run_path.join("pulse");
    std::fs::create_dir(&pulse_path)
        .context("Failed to create `pulse` directory in `XDG_RUNTIME_DIR`")?;
    let pulse_path = pulse_path.join("native");
    setup_socket_proxy(pulse_path, 3333)?;

    setup_x11_forwarding(run_path)?;

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server_main(options.server_port, options.command, options.command_args).await
    })
}
