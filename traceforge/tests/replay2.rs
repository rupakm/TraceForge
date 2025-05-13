use std::{
    collections::{BTreeMap, BTreeSet},
    result,
};

use traceforge::{
    send_msg,
    thread::{current_id, spawn_daemon, ThreadId},
};

type KeyName = String;
type KeyMaterial = String;

type EmState = BTreeMap<KeyName, KeyMaterial>;

#[derive(Debug, Clone, PartialEq)]
enum EncryptionModuleRequest {
    CreateKey(ThreadId, KeyName, KeyMaterial),
    DeleteKey(ThreadId, KeyName),
    GetKey(ThreadId, KeyName),
    CopyState(ThreadId),
    Fail(ThreadId),
}

#[derive(Debug, Clone, PartialEq)]
enum ClientMessage {
    CreateKeyResponse(ThreadId),
    DeleteKeyResponse(ThreadId),
    GetKeyResponse(ThreadId, Option<KeyMaterial>),
    RemoveEncryptionModule(ThreadId),
    AddEncryptionModule(ThreadId),
}

// Encryption Module (EM)
struct EncryptionModule {
    keys: EmState,
}

impl EncryptionModule {
    fn new(keys: EmState) -> Self {
        EncryptionModule { keys }
    }

    pub fn start(mut self) -> ThreadId {
        spawn_daemon(move || self.actor_loop()).thread().id()
    }

    fn actor_loop(&mut self) {
        loop {
            let request = traceforge::recv_msg_block();
            match request {
                EncryptionModuleRequest::CreateKey(sender_tid, key_name, key_material) => {
                    self.keys.insert(key_name, key_material);
                    traceforge::send_msg(sender_tid, ClientMessage::CreateKeyResponse(current_id()));
                }
                EncryptionModuleRequest::DeleteKey(sender_tid, key_name) => {
                    self.keys.remove(&key_name);
                    traceforge::send_msg(sender_tid, ClientMessage::DeleteKeyResponse(current_id()));
                }
                EncryptionModuleRequest::GetKey(sender_tid, key_name) => {
                    let key_material = self.keys.get(&key_name).cloned();
                    traceforge::send_msg(
                        sender_tid,
                        ClientMessage::GetKeyResponse(current_id(), key_material),
                    );
                }
                EncryptionModuleRequest::CopyState(sender_tid) => {
                    let state = self.keys.clone();
                    traceforge::send_msg(sender_tid, state);
                }
                EncryptionModuleRequest::Fail(sender_tid) => {
                    traceforge::send_msg(sender_tid, ());
                    return;
                }
            }
        }
    }
}

struct EmProxy {
    ems: Vec<ThreadId>,
}

impl EmProxy {
    fn new(ems: Vec<ThreadId>) -> Self {
        EmProxy { ems }
    }

    pub fn create_key(&mut self, key_name: KeyName, key_material: KeyMaterial) {
        let mut waiters = BTreeSet::new();

        // Send "create key" messages to EMs
        for em in &mut self.ems {
            let sender_tid = current_id();
            waiters.insert(em.clone());
            send_msg(
                *em,
                EncryptionModuleRequest::CreateKey(
                    sender_tid,
                    key_name.clone(),
                    key_material.clone(),
                ),
            );
        }

        self.wait(waiters);
    }

    pub fn delete_key(&mut self, key_name: KeyName) {
        let mut waiters = BTreeSet::new();

        // Send "create key" messages to EMs
        for em in &mut self.ems {
            let sender_tid = current_id();
            waiters.insert(em.clone());
            send_msg(
                *em,
                EncryptionModuleRequest::DeleteKey(sender_tid, key_name.clone()),
            );
        }

        self.wait(waiters);
    }

    fn wait(&mut self, mut waiters: BTreeSet<ThreadId>) {
        while !waiters.is_empty() {
            let msg: ClientMessage = traceforge::recv_msg_block();
            match msg {
                ClientMessage::CreateKeyResponse(em_id) => {
                    assert!(waiters.contains(&em_id));
                    waiters.remove(&em_id);
                }
                ClientMessage::DeleteKeyResponse(em_id) => {
                    assert!(waiters.contains(&em_id));
                    waiters.remove(&em_id);
                }
                ClientMessage::GetKeyResponse(_, _) => unreachable!("Unexpected GetKeyResponse"),
                ClientMessage::RemoveEncryptionModule(em_id) => {
                    // A response won't be expected from this waiter because it failed.
                    waiters.remove(&em_id);
                }
                ClientMessage::AddEncryptionModule(em_id) => {
                    self.ems.push(em_id);
                }
            }
        }
    }

    pub fn get_key(&self, key_name: &str) -> Option<KeyMaterial> {
        let em_index = traceforge::Nondet::nondet(&(0..self.ems.len()));
        let em: ThreadId = self.ems[em_index].clone();
        let sender_tid = current_id();
        send_msg(
            em,
            EncryptionModuleRequest::GetKey(sender_tid, key_name.to_string()),
        );
        let resp = traceforge::recv_tagged_msg_block(move |tid, _| tid == em);
        if let ClientMessage::GetKeyResponse(_em_tid, key_material) = resp {
            key_material
        } else {
            unreachable!("Wrong message response type");
        }
    }
}

struct HealthMonitor {
    ems: Vec<ThreadId>,
    proxies: Vec<ThreadId>,
}

impl HealthMonitor {
    fn new(ems: Vec<ThreadId>, proxies: Vec<ThreadId>) -> Self {
        HealthMonitor { ems, proxies }
    }

    pub fn run(&mut self) {
        // Choose a random EM to fail.
        let failed_em_index = traceforge::Nondet::nondet(&(0..self.ems.len()));
        let failed_em = self.ems.remove(failed_em_index);
        traceforge::send_msg(failed_em, EncryptionModuleRequest::Fail(current_id()));
        // Wait for the failed EM to acknowledge the response.
        let failed_em_clone = failed_em.clone();
        let _: () = traceforge::recv_tagged_msg_block(move |tid, _| tid == failed_em_clone);

        // Tell all the proxies that it's failed.
        for proxy in &mut self.proxies {
            send_msg(
                *proxy,
                ClientMessage::RemoveEncryptionModule(failed_em.clone()),
            );
        }

        // Create a new EM to replace the failed one.
        let fresh_em = self.clone_healthy_em();

        // Tell all the proxies about the new EM.
        for proxy in &mut self.proxies {
            traceforge::send_msg(*proxy, ClientMessage::AddEncryptionModule(fresh_em.clone()));
        }
    }

    fn clone_healthy_em(&mut self) -> ThreadId {
        let healthy_em = self.ems[0].clone();
        let sender_tid = current_id();
        send_msg(healthy_em, EncryptionModuleRequest::CopyState(sender_tid));
        let state = traceforge::recv_tagged_msg_block(move |tid, _| tid == healthy_em);

        let new_em = EncryptionModule::new(state);
        new_em.start()
    }
}

pub fn scenario() {
    println!("Starting scenario");
    let ems: Vec<ThreadId> = (0..3)
        .map(|_i| {
            let em = EncryptionModule::new(BTreeMap::new());
            em.start()
        })
        .collect();
    let mut proxy = EmProxy::new(ems.clone());
    let proxy_tid = current_id();

    let mut health_monitor = HealthMonitor::new(ems.clone(), vec![proxy_tid]);
    traceforge::thread::spawn(move || health_monitor.run());

    proxy.create_key("key1".to_string(), "value1".to_string());
    let result = proxy.get_key("key1");
    assert_eq!(result, Some("value1".to_string()));

    proxy.delete_key("key1".to_string());
    let result = proxy.get_key("key1");
    assert_eq!(result, None, "foobar is the expected error message");
}

fn validate_result(result: result::Result<(), Box<dyn std::any::Any + Send>>) {
    let panic_msg = "foobar";

    assert!(
        result.is_err(),
        "An error was expected but the result is ok"
    );
    let binding = result.unwrap_err();
    let err = binding.downcast_ref::<String>();
    assert!(err.is_some(), "The error should be a string but its not.");
    assert!(
        err.is_some_and(|s| s.contains(&panic_msg)),
        "The string should contain panic_msg but it did not"
    );
}

#[test]
pub fn test_replay_create_em() {
    // Capture a counterexample
    let filename = "/tmp/must-replay2.rs";
    let result = std::panic::catch_unwind(|| {
        let stats = traceforge::verify(
            traceforge::Config::builder()
                .with_seed(0)
                .with_verbose(0)
                .with_error_trace(&filename)
                .build(),
            || {
                scenario();
            },
        );
        println!("{:?}", stats);
    });
    validate_result(result);

    // Now do it again and make sure the panic message is the same.
    // If we get a different panic message then replay is failing.
    let result = std::panic::catch_unwind(|| {
        traceforge::replay(scenario, filename);
    });
    validate_result(result);
}
