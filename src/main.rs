use std::env::args;
use std::net::TcpListener;
use tokio::process::Command;
use tokio::io;
use futures_util::io::AllowStdIo;


fn build_command_from_args(args: &[String]) -> Command {
    let mut command = Command::new(args.get(0).unwrap());

    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    for arg in args.iter().skip(1) {
        command.arg(arg);
    }

    command
}

#[tokio::main]
/// This is a simple TCP server that will spawn a process for each connection
/// and pipe the connection to the process stdin and stdout.
/// The first argument is the port to listen on.
/// The second argument (and later arguments) is the command to run.
async fn main() -> io::Result<()> {
    let args = args().collect::<Vec<String>>();
    let listener = TcpListener::bind(args.get(1).unwrap())?;
    let listener = tokio::net::TcpListener::from_std(listener)?;

    loop {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut command = build_command_from_args(args.get(2..).unwrap());

        println!("Got connection from {:?}", stream.peer_addr());

        tokio::spawn(async move {
            let (mut stream_reader, mut stream_writer) = stream.split();

            let (child_reader, child_writer) = os_pipe::pipe().unwrap();
            let writer_clone = child_writer.try_clone().unwrap();
            command.stdout(child_writer);
            command.stderr(writer_clone);

            let child_reader = AllowStdIo::new(child_reader);
            let mut child_reader = tokio_util::compat::FuturesAsyncReadCompatExt::compat(child_reader);

            let mut child = command.spawn().unwrap();

            let mut stdin = child.stdin.take().unwrap();

            let copy_stdin = io::copy(&mut stream_reader, &mut stdin);
            let copy_output = io::copy(&mut child_reader, &mut stream_writer);

            drop(command);

            tokio::try_join!(copy_stdin, copy_output).unwrap();

            child.wait().await.unwrap();
        });
    }
}
