use traceforge::monitor_types::*;
use traceforge::{nondet, publish, recv_msg_block, send_msg, verify, Config};
use traceforge_macros::monitor;

use traceforge::thread;
use traceforge::thread::{spawn, spawn_daemon, ThreadId};

/// This file contains an example of how to check specifications at termination.
/// Key points:
/// - Threads use 'publish' to expose their state.
/// - Monitor::on_stop is automatically invoked at the end of each execution.
///
/// This monitor observes no messages, but its on_stop checks a property.
/// In this model of a bank, our specification is to show that money is conserved--at
/// the end of the execution, no money has been created or destroyed even though
/// the actual location of the money is not part of the specification.
///
/// For example, in some executions the withdrawal will be scheduled before the deposit so the
/// money will remain in the bank, and in others, all the money will be withdrawn.
#[monitor()]
#[derive(Debug, Clone, PartialEq)]
pub struct TerminationMonitor {
    expected_sum_of_balances: i32,
}

impl Monitor for TerminationMonitor {
    fn on_stop(&mut self, execution_end: &ExecutionEnd) -> MonitorResult {
        let balances = execution_end.get_published::<State>();
        let actual_sum: i32 = balances
            .iter()
            .map(|(_thread_id, state)| state.balance)
            .sum();

        // Instead of assert we could also return MonitorResult::Err, but this is more concise.
        assert_eq!(self.expected_sum_of_balances, actual_sum);
        Ok(())
    }
}

/// The State struct exposes the internal state of each thread
#[derive(PartialEq, Clone, Debug)]
struct State {
    balance: i32,
}

#[derive(PartialEq, Clone, Debug)]
enum Transaction {
    Deposit(ThreadId, i32),
    Withdrawal(ThreadId, i32),
}

#[derive(PartialEq, Clone, Debug)]
enum TransactionResp {
    Ok,
    InsufficientBalance,
}

#[test]
fn test_termination_monitor() {
    let result = verify(Config::builder().build(), || {
        let total_balance = 100;

        start_monitor_termination_monitor(TerminationMonitor {
            expected_sum_of_balances: total_balance,
        });

        main_fn(total_balance.clone());
    });
    // one nondet chooses to withdraw all or not all the money, and either the deposit or
    // withdrawal is processed first, so there are 2 x 2 = 4 execs overall.
    assert_eq!(result.execs, 4);
    assert_eq!(result.block, 0);
}

/// The main function starts the bank thread, and withdrawal thread, then deposits all the money
fn main_fn(initial_balance: i32) {
    publish(State {
        balance: initial_balance.clone(),
    });

    let bank_handle = spawn_daemon(bank_fn);
    let bank = bank_handle.thread().id();
    let _withdraw_thread = spawn(move || withdraw_fn(bank, initial_balance.clone()));
    send_msg(
        bank,
        Transaction::Deposit(thread::current_id(), initial_balance.clone()),
    );

    // After depositing money, the main thread no longer has it.
    publish(State { balance: 0 });
}

/// The bank function processes an unbounded number of Deposit / Withdrawal transactions
fn bank_fn() {
    let mut bank_balance = 0;
    loop {
        match recv_msg_block() {
            Transaction::Deposit(sender, amount) => {
                bank_balance += amount;
                publish(State {
                    balance: bank_balance.clone(),
                });
                send_msg(sender, TransactionResp::Ok);
            }
            Transaction::Withdrawal(sender, amount) => {
                if bank_balance >= amount {
                    bank_balance -= amount;
                    publish(State {
                        balance: bank_balance.clone(),
                    });
                    send_msg(sender, TransactionResp::Ok);
                } else {
                    send_msg(sender, TransactionResp::InsufficientBalance);
                }
            }
        }
    }
}

/// The withdrawal thread will make one attempt to withdraw a specified amount of money
/// and receive one response.
fn withdraw_fn(bank: ThreadId, max_withdrawal: i32) {
    let withdrawal = if nondet() {
        max_withdrawal
    } else {
        max_withdrawal / 2
    };
    send_msg(
        bank,
        Transaction::Withdrawal(thread::current_id(), withdrawal),
    );
    let resp = recv_msg_block();
    match resp {
        TransactionResp::Ok => {
            publish(State {
                balance: withdrawal.clone(),
            });
        }
        TransactionResp::InsufficientBalance => {}
    }
}
