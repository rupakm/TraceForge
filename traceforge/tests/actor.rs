use traceforge::msg::Message;
use traceforge::*;
/// Book example with actor abstraction
///
use std::collections::HashMap;

mod utils;

/// Adding a super trait alias that encompasses all the traits we'd like messages to satisfy
/// This will compactify the code
pub trait MessageTraits: Message + Send + 'static {}

/// Nothing needs to happen here, this is just a super trait alias
impl<E> MessageTraits for E where E: Message + Send + 'static {}

pub(crate) trait Actor<M: MessageTraits, RetType> {
    // The handle function deals with a message sent to the actor
    // It can send out  messages to other actors using `traceforge::send_msg`
    fn handle(&mut self, _msg: M) {}

    // This can contain any additional clean up, assertion checks, etc
    fn stop(&self) -> RetType;

    // This procedure hides the mechanics of terminating an actor.
    // In the common case, the message `M` will have a case `M::Terminate`
    // and the handler function for `M::Terminate` will ensure `can_shutdown` returns true
    fn can_shutdown(&self) -> bool;

    // This is the ``main loop'' that will run in a thread
    fn execute(&mut self) -> RetType {
        while !self.can_shutdown() {
            let msg = traceforge::recv_msg_block();
            self.handle(msg);
        }

        self.stop()
    }
}

/// An example protocol using the actor trait.
/// The protocol is described in the Must book.
/// Essentially, a server generates one-time use tokens. Clients can generate tokens
/// and use them. The server checks that a token used by the client exists and was not
/// used before.
///
/// TokenStatus can either be Generated (for a freshly generated token)
/// or Consumed (if it has already been consumed)
#[derive(Clone, PartialEq, Debug)]
pub enum TokenStatus {
    Generated,
    Consumed,
}

/// Every Token has a unique ID
type Token = usize;

#[derive(Clone, Debug, PartialEq)]
pub enum ServerMessage {
    Generate(ThreadId),       // provide a new token
    Consume(ThreadId, Token), // ask to consume an old token

    Terminate, // terminate the server
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClientMessage {
    Token(Token),
    Ok,
    NoSuchToken,
    ConsumedToken,
}

use traceforge::thread::{JoinHandle, ThreadId};

struct Server {
    tokens: HashMap<Token, TokenStatus>,
    next_token: usize,

    can_shutdown: bool,
}

impl Server {
    pub fn default() -> Self {
        Self {
            tokens: HashMap::new(),
            next_token: 0,

            can_shutdown: false,
        }
    }

    fn generate(&mut self) -> Token {
        let t = self.next_token;
        self.tokens.insert(t, TokenStatus::Generated);
        self.next_token += 1;
        t
    }
}

impl Actor<ServerMessage, ()> for Server {
    fn handle(&mut self, m: ServerMessage) {
        match m {
            ServerMessage::Generate(tid) => {
                let t = self.generate();
                traceforge::send_msg(tid, ClientMessage::Token(t));
            }
            ServerMessage::Consume(tid, token) => {
                let t = self.tokens.get(&token);
                if t.is_none() {
                    traceforge::send_msg(tid, ClientMessage::NoSuchToken);
                    return;
                }
                let t = t.unwrap();
                match t {
                    TokenStatus::Generated => {
                        traceforge::send_msg(tid, ClientMessage::Ok);
                        let _ = self.tokens.insert(token, TokenStatus::Consumed);
                    }
                    TokenStatus::Consumed => traceforge::send_msg(tid, ClientMessage::ConsumedToken),
                }
            }
            ServerMessage::Terminate => self.can_shutdown = true,
        }
    }

    fn can_shutdown(&self) -> bool {
        self.can_shutdown
    }
    fn stop(&self) -> () {}
}

fn client(server_id: ThreadId) {
    let myid = thread::current().id();
    traceforge::send_msg(server_id, ServerMessage::Generate(myid));
    let r: ClientMessage = recv_msg_block();
    match r {
        ClientMessage::Token(t) => {
            traceforge::send_msg(server_id, ServerMessage::Consume(myid, t));
            let ok: ClientMessage = recv_msg_block();
            assert_eq!(ok, ClientMessage::Ok);
            traceforge::send_msg(server_id, ServerMessage::Consume(myid, t));
            let nok: ClientMessage = recv_msg_block();
            assert_eq!(nok, ClientMessage::ConsumedToken);
        }
        _ => panic!("Strange response"),
    }
}

fn client2(server_id: ThreadId) {
    let myid = thread::current().id();
    traceforge::send_msg(
        server_id,
        ServerMessage::Consume(myid, Token::from(134usize)),
    );
    let r: ClientMessage = recv_msg_block();
    assert_eq!(r, ClientMessage::NoSuchToken);
}

fn client_server_scenario() {
    let server: JoinHandle<()> = thread::spawn(|| {
        let mut s = Server::default();
        s.execute();
    });
    let server_id = server.thread().id();
    let _ = thread::spawn(move || {
        client(server_id.clone());
    });
    let _ = thread::spawn(move || {
        client2(server_id.clone());
    });
    traceforge::send_msg(server_id, ServerMessage::Terminate);
    let _ = server.join();
}

#[test]
fn client_server_test() {
    //utils::init_log();
    let stats = traceforge::verify(Config::builder().build(), client_server_scenario);
    println!("Stats = {}, {}", stats.execs, stats.block);
}
