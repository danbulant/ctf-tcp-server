use std::env::args;
use std::net::TcpListener;
use std::time::Duration;
use tokio::process::Command;
use tokio::{io, select};
use futures_util::io::AllowStdIo;
use nix::unistd::Pid;
use nix::sys::signal::{self, Signal};

fn build_command_from_args(args: &[String]) -> Command {
    let mut command = Command::new(args.get(0).unwrap());

    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    command.stdin(std::process::Stdio::piped());
    command.kill_on_drop(true);

    for arg in args.iter().skip(1) {
        command.arg(arg);
    }

    command
}

#[tokio::main]
/// This is a simple TCP server that will spawn a process for each connection
/// and pipe the connection to the process stdin and stdout.
/// The first argument is the address and port to listen on.
/// The second argument (and later arguments) is the command to run.
async fn main() -> io::Result<()> {
    let args = args().collect::<Vec<String>>();
    let listener = TcpListener::bind(args.get(1).unwrap())?;
    let listener = tokio::net::TcpListener::from_std(listener)?;

    println!("Listening on {}", args.get(1).unwrap());

    // Haxagon compatibility
    println!("SCENARIO_IS_READY");

    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        let mut command = build_command_from_args(args.get(2..).unwrap());

        println!("Got connection from {addr:?}");

        tokio::spawn(async move {
            let (mut stream_reader, mut stream_writer) = stream.into_split();

            let (child_reader, child_writer) = os_pipe::pipe().unwrap();
            let writer_clone = child_writer.try_clone().unwrap();
            command.stdout(child_writer);
            command.stderr(writer_clone);

            let child_reader = AllowStdIo::new(child_reader);
            let mut child_reader = tokio_util::compat::FuturesAsyncReadCompatExt::compat(child_reader);

            eprintln!("Starting command {:?}", command.as_std());

            let mut child = command.spawn().unwrap();

            let mut stdin = child.stdin.take().unwrap();

            let copy_stdin = io::copy(&mut stream_reader, &mut stdin);
            let copy_output = tokio::spawn(async move {
                io::copy(&mut child_reader, &mut stream_writer).await.unwrap();
            });

            drop(command);

            // wait for first to complete
            select!{biased;
                _ = child.wait() => {},
                _ = copy_output => {},
                _ = copy_stdin => {}
            }

            eprintln!("Connection from {addr:?} closed");

            if child.try_wait().unwrap().is_none() {
                signal::kill(Pid::from_raw(child.id().unwrap() as i32), Signal::SIGTERM).unwrap();

                select!{
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        child.kill().await.unwrap();
                    },
                    _ = child.wait() => {}
                }
            }

            let res = child.wait().await.unwrap();
            eprintln!("Command exited with {:?}", res.code().unwrap_or(-1));
        });
    }
}
