use traceforge::channel::{Builder, Channel};
use traceforge::loc::CommunicationModel;
use traceforge::{thread, Config};

type Ip = i32;
const SERVER_IP: Ip = 42;

type ListenSocket = Ip;
type TcpSock<T> = Channel<T>;

#[derive(PartialEq, Eq, Clone, Debug, Hash)]
struct Listen(Ip);

#[derive(PartialEq, Eq, Clone, Debug, Hash)]
struct Accept(Ip);

// A simple TCP example
#[test]
fn tcp_handshake() {
    // TODO: Renew listen messages before accepting?
    let stats = traceforge::verify(Config::builder().build(), || {
        // server
        let _ = thread::spawn(|| {
            // listen_sock = bind(my_ip)
            let sock: ListenSocket = SERVER_IP;

            // listen(listen_sock)
            Builder::new()
                .with_name(Listen(sock))
                .with_comm(CommunicationModel::NoOrder)
                .build()
                .0
                .send_msg(());

            // let sock = accept(listen_sock)
            let sock: TcpSock<_> = Builder::new()
                .with_name(Accept(sock))
                .with_comm(CommunicationModel::NoOrder)
                .build()
                .1
                .recv_msg_block();

            // recv(sock)
            let msg: &str = sock.1.recv_msg_block();
            println!("Server received: {}", msg);

            // send(sock)
            sock.0.send_msg("Hello from server");
        });
        // client
        let _ = thread::spawn(|| {
            // let sock = connect(server_ip)
            let () = Builder::new()
                .with_name(Listen(SERVER_IP))
                .with_comm(CommunicationModel::NoOrder)
                .build()
                .1
                .recv_msg_block();
            let ch1 = Builder::new()
                .with_comm(CommunicationModel::LocalOrder)
                .build();
            let ch2 = Builder::new()
                .with_comm(CommunicationModel::LocalOrder)
                .build();
            let sock = (ch1.0, ch2.1);
            Builder::new()
                .with_name(Accept(SERVER_IP))
                .with_comm(CommunicationModel::NoOrder)
                .build()
                .0
                .send_msg((ch2.0, ch1.1));

            // send(sock)
            sock.0.send_msg("Hello from client");

            // recv(sock)
            let msg: &str = sock.1.recv_msg_block();
            println!("Client received: {}", msg);
        });
    });
    assert_eq!(stats.execs, 1);
}
