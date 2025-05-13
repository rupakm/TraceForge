//! TraceForge's implementation of [`tokio::task`].

use crate::event_label::{End, TCreate, TJoin};
use crate::msg::Message;
use crate::runtime::execution::ExecutionState;
use crate::runtime::task::TaskId;
use crate::runtime::thread;
use std::cmp::Ordering;
use std::marker::PhantomData;
use std::time::Duration;

/// A unique identifier for a running thread
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// Do not derive PartialOrd or Ord --
// Reason 1: Rust's ThreadId does not derive them and we should be as interface-compatible as possible
//
// Reason 2: symmetry reduction depends on ThreadId's not being comparable.
// Symmetry reduction uses the fact that two threads that receive exactly the same history of messages act
// exactly alike if their Id's are swapped. This is true if the ThreadId is an opaque value and a thread
// can only compare it's thread id `current().id()` with others for equality/disequality.
// However, if the thread id's are comparable, a thread can decide to act differently based on whether
// it's thread id is lower than someone else.
//
// Technically, swapping thread id's will not be a group automorphism if Ord is allowed, and symmetry
// reduction heuristics need a group structure.
//
// You might want to add PartialOrd because you want to put ThreadId's as a key in a BTreeMap.
// Instead, use a HashMap. Even though HashMap iterations are randomly ordered, this should be fine
// for most applications. If you need a deterministic version, you should use a deterministic hash.
pub struct ThreadId {
    // TODO Should we add an execution id here, like Loom does?
    task_id: TaskId,
}

impl From<ThreadId> for usize {
    fn from(id: ThreadId) -> usize {
        id.task_id.into()
    }
}

impl From<u32> for ThreadId {
    fn from(id: u32) -> ThreadId {
        Self {
            task_id: (id as usize).into(),
        }
    }
}

// Needed in order to have the trait Ord
impl PartialOrd for ThreadId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some((usize::from(self.task_id)).cmp(&usize::from(other.task_id)))
    }
}

// Needed in order to use a BTreeMap whose keys are thread ids
impl Ord for ThreadId {
    fn cmp(&self, other: &Self) -> Ordering {
        (usize::from(self.task_id)).cmp(&usize::from(other.task_id))
    }
}

/// A handle to a thread.
#[derive(Debug, Clone)]
pub struct Thread {
    name: Option<String>,
    id: ThreadId,
}

impl Thread {
    /// Gets the thread's name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Gets the thread's unique identifier
    pub fn id(&self) -> ThreadId {
        self.id
    }

    /// Atomically makes the handle's token available if it is not already.
    pub fn unpark(&self) {
        ExecutionState::with(|s| {
            s.get_mut(self.id.task_id).unpark();
        });

        // Making the token available is a yield point
        thread::switch();
    }
}

/// Spawn a new thread, returning a JoinHandle for it.
///
/// The join handle can be used (via the `join` method) to block until the child thread has
/// finished.
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Message + 'static,
{
    spawn_named(f, None, None, None)
}

pub(crate) fn spawn_named<F, T>(
    f: F,
    name: Option<String>,
    stack_size: Option<usize>,
    sym_cid: Option<ThreadId>,
) -> JoinHandle<T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Message + 'static,
{
    // TODO Check if it's worth avoiding the call to `ExecutionState::config()` if we're going
    // TODO to use an existing continuation from the pool.
    let stack_size =
        stack_size.unwrap_or_else(|| ExecutionState::with(|s| s.must.borrow().config().stack_size));
    let result = std::sync::Arc::new(std::sync::Mutex::new(None));
    let task_id = {
        // let result = std::sync::Arc::clone(&result);
        let f = move || {
            let ret = f();

            // // Run thread-local destructors before publishing the result, because
            // // [`JoinHandle::join`] says join "waits for the associated thread to finish", but
            // // destructors must be run on the thread, so it can't be considered "finished" if the
            // // destructors haven't run yet.
            // // See `pop_local` for details on why this loop looks this slightly funky way.
            // while let Some(local) = ExecutionState::with(|state| state.current_mut().pop_local()) {
            //     drop(local);
            // }

            // Publish the result and unblock the waiter. We need to do this now, because once this
            // closure completes, the Execution will consider this task Finished and invoke the
            // scheduler.
            ExecutionState::with(|state| {
                let pos = state.next_pos();
                state
                    .must
                    .borrow_mut()
                    .handle_tend(End::new(pos, Box::new(ret)));
            });
            // *result.lock().unwrap() = Some(Ok(ret));
            // ExecutionState::with(|state| {
            //     if let Some(waiter) = state.current_mut().take_waiter() {
            //         state.get_mut(waiter).unblock();
            //     }
            // });
        };
        let cid = ExecutionState::spawn_thread(f, stack_size, name.clone());
        ExecutionState::with(|state| {
            let pos = state.next_pos();
            state.must.borrow_mut().handle_tcreate(TCreate::new(
                pos,
                usize::from(cid) as u32,
                name.clone(),
                sym_cid.map(|tid| usize::from(tid) as u32),
            ));
        });
        cid
    };

    thread::switch();

    let thread = Thread {
        id: ThreadId { task_id },
        name,
    };

    JoinHandle {
        task_id,
        thread,
        result,
    }
}

/// An owned permission to join on a thread (block on its termination).
#[allow(dead_code)] // linter complains that result is never read
#[derive(Debug)]
pub struct JoinHandle<T> {
    task_id: TaskId,
    thread: Thread,
    result: std::sync::Arc<std::sync::Mutex<Option<std::thread::Result<T>>>>,
}

impl<T: 'static> JoinHandle<T> {
    /// Waits for the associated thread to finish.
    pub fn join(self) -> std::thread::Result<T> {
        let ret = loop {
            thread::switch();
            let val = ExecutionState::with(|s| {
                let target_id = s.get(self.task_id).id();
                let pos = s.next_pos();
                s.must
                    .borrow_mut()
                    .handle_tjoin(TJoin::new(pos, usize::from(target_id) as u32))
            });

            if let Some(message) = val {
                break message;
            }

            ExecutionState::with(|s| s.prev_pos());
        };
        Ok(*ret.as_any().downcast().expect("wrong join return type"))
        // self.result
        //     .lock()
        //     .unwrap()
        //     .take()
        //     .expect("target should have finished")
    }

    /// Extracts a handle to the underlying thread.
    pub fn thread(&self) -> &Thread {
        &self.thread
    }
}

/// Cooperatively gives up a timeslice to the Shuttle scheduler.
///
/// Some Shuttle schedulers use this as a hint to deprioritize the current thread in order for other
/// threads to make progress (e.g., in a spin loop).
pub fn yield_now() {
    // let waker = ExecutionState::with(|state| state.current().waker());
    // waker.wake_by_ref();
    ExecutionState::request_yield();
    thread::switch();
}

/// Puts the current thread to sleep for at least the specified amount of time.
// Note that Shuttle does not model time, so this behaves just like a context switch.
pub fn sleep(_dur: Duration) {
    thread::switch();
}

/// Get a handle to the thread that invokes it
pub fn current() -> Thread {
    let (task_id, name) = ExecutionState::with(|s| {
        let me = s.current();
        (me.id(), me.name())
    });

    Thread {
        id: ThreadId { task_id },
        name,
    }
}

/// Blocks unless or until the current thread's token is made available.
pub fn park() {
    let switch = ExecutionState::with(|s| s.current_mut().park());

    // We only need to context switch if the park token was unavailable. If it was available, then
    // any execution reachable by context switching here would also be reachable by having not
    // chosen this thread at the last context switch, because the park state of a thread is only
    // observable by the thread itself.
    if switch {
        thread::switch();
    }
}

/// Blocks unless or until the current thread's token is made available or the specified duration
/// has been reached (may wake spuriously).
///
/// Note that Shuttle does not module time, so this behaves identically to `park`. It cannot
/// spuriously wake.
pub fn park_timeout(_dur: Duration) {
    park();
}

/// Thread factory, which can be used in order to configure the properties of a new thread.
#[derive(Debug, Default)]
pub struct Builder {
    name: Option<String>,
    stack_size: Option<usize>,
}

impl Builder {
    /// Generates the base configuration for spawning a thread, from which configuration methods can be chained.
    pub fn new() -> Self {
        Self {
            name: None,
            stack_size: None,
        }
    }

    /// Names the thread-to-be. Currently the name is used for identification only in panic messages.
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the size of the stack (in bytes) for the new thread.
    pub fn stack_size(mut self, stack_size: usize) -> Self {
        self.stack_size = Some(stack_size);
        self
    }

    /// Spawns a new thread by taking ownership of the Builder, and returns an `io::Result` to its `JoinHandle`.
    pub fn spawn<F, T>(self, f: F) -> std::io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Message + 'static,
    {
        Ok(spawn_named(f, self.name, self.stack_size, None))
    }
}

/// A thread local storage key which owns its contents
// Sadly, the fields of this thing need to be public because function pointers in const fns are
// unstable, so an explicit instantiation is the only way to construct this struct. User code should
// not rely on these fields.
pub struct LocalKey<T: 'static> {
    #[doc(hidden)]
    pub init: fn() -> T,
    #[doc(hidden)]
    pub _p: PhantomData<T>,
}

// Safety: `LocalKey` implements thread-local storage; each thread sees its own value of the type T.
unsafe impl<T> Send for LocalKey<T> {}
unsafe impl<T> Sync for LocalKey<T> {}

impl<T: 'static> std::fmt::Debug for LocalKey<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalKey").finish_non_exhaustive()
    }
}

impl<T: 'static> LocalKey<T> {
    /// Acquires a reference to the value in this TLS key.
    ///
    /// This will lazily initialize the value if this thread has not referenced this key yet.
    pub fn with<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        self.try_with(f).expect(
            "cannot access a Thread Local Storage value \
            during or after destruction",
        )
    }

    /// Acquires a reference to the value in this TLS key.
    ///
    /// This will lazily initialize the value if this thread has not referenced this key yet. If the
    /// key has been destroyed (which may happen if this is called in a destructor), this function
    /// will return an AccessError.
    pub fn try_with<F, R>(&'static self, f: F) -> Result<R, AccessError>
    where
        F: FnOnce(&T) -> R,
    {
        let value = self.get().unwrap_or_else(|| {
            // let value = (self.init)();

            // ExecutionState::with(move |state| {
            //     state.current_mut().init_local(self, value);
            // });

            self.get().unwrap()
        })?;

        Ok(f(value))
    }

    fn get(&'static self) -> Option<Result<&T, AccessError>> {
        // Safety: see the usage below
        // unsafe fn extend_lt<'a, 'b, T>(t: &'a T) -> &'b T {
        //     std::mem::transmute(t)
        // }

        ExecutionState::with(|_state| {
            // if let Ok(value) = state.current().local(self)? {
            // Safety: unfortunately the lifetime of a value in our thread-local storage is
            // bound to the lifetime of `ExecutionState`, which has no visible relation to the
            // lifetime of the thread we're running on. However, *we* know that the
            // `ExecutionState` outlives any thread, including the caller, and so it's safe to
            // give the caller the lifetime it's asking for here.
            // Some(Ok(unsafe { extend_lt(value) }))
            // } else {
            // Slot has already been destructed
            Some(Err(AccessError))
            // }
        })
    }
}

/// An error returned by [`LocalKey::try_with`]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub struct AccessError;

impl std::fmt::Display for AccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt("already destroyed", f)
    }
}

impl std::error::Error for AccessError {}
